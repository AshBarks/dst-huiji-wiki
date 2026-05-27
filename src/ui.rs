use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use crate::archive::{BinType, ParsedArchive, parse_dyn, parse_zip};
use crate::atlas::{gather_atlas_images, split_atlas};
use crate::build_file::BuildFile;
use crate::gif_export::export_gif;
use crate::ktex::parse_ktex;
use crate::render::{BoundingBox, compute_animation_bounds, render_frame};

struct AnimEntry {
    anim: crate::anim::AnimFile,
    enabled: bool,
    source_name: String,
}

pub struct BuildEntry {
    pub build: BuildFile,
    pub enabled: bool,
    pub source_name: String,
    pub assigned_atlas: Option<usize>,
}

struct AtlasEntry {
    enabled: bool,
    source_name: String,
    decoded: HashMap<String, Arc<image::RgbaImage>>,
}

struct PendingBuild {
    build: BuildFile,
    source_name: String,
}

struct AtlasDisplayInfo {
    atlas_idx: usize,
    header: String,
    decoded_images: Vec<(String, u32, u32)>,
    build_options: Vec<(usize, String)>,
    default_open: bool,
}

struct FrameCacheEntry {
    image: image::RgbaImage,
    texture: Option<egui::TextureHandle>,
}

struct BackgroundRenderer {
    receiver: mpsc::Receiver<(u64, usize, image::RgbaImage)>,
    stop_flag: Arc<AtomicBool>,
    _thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl BackgroundRenderer {
    fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl Drop for BackgroundRenderer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

pub struct App {
    initial_files: Option<Vec<PathBuf>>,
    anims: Vec<AnimEntry>,
    active_anim_idx: usize,
    active_bank_idx: usize,
    active_anim_inner_idx: usize,
    active_frame_idx: usize,
    builds: Vec<BuildEntry>,
    selected_build_idx: Option<usize>,
    atlas_entries: Vec<AtlasEntry>,
    pending_builds: Vec<PendingBuild>,
    playing: bool,
    play_speed: f32,
    last_frame_time: Instant,
    frame_texture: Option<egui::TextureHandle>,
    rendered_image: Option<image::RgbaImage>,
    needs_re_render: bool,
    error_message: Option<String>,
    frame_cache: HashMap<usize, FrameCacheEntry>,
    cache_gen: u64,
    cache_anim_key: (usize, usize, usize),
    cache_dirty: bool,
    bg_renderer: Option<BackgroundRenderer>,
    animation_bounds: Option<BoundingBox>,
}

impl App {
    pub fn new(initial_files: Option<Vec<PathBuf>>) -> Self {
        Self {
            initial_files,
            anims: Vec::new(),
            active_anim_idx: 0,
            active_bank_idx: 0,
            active_anim_inner_idx: 0,
            active_frame_idx: 0,
            builds: Vec::new(),
            selected_build_idx: None,
            atlas_entries: Vec::new(),
            pending_builds: Vec::new(),
            playing: false,
            play_speed: 1.0,
            last_frame_time: Instant::now(),
            frame_texture: None,
            rendered_image: None,
            needs_re_render: false,
            error_message: None,
            frame_cache: HashMap::new(),
            cache_gen: 0,
            cache_anim_key: (0, 0, 0),
            cache_dirty: false,
            bg_renderer: None,
            animation_bounds: None,
        }
    }

    fn decode_tex_files(
        tex_files: &HashMap<String, Vec<u8>>,
    ) -> HashMap<String, Arc<image::RgbaImage>> {
        let mut decoded = HashMap::new();
        for (name, data) in tex_files {
            if let Ok(ktex) = parse_ktex(data)
                && let Ok(img) = ktex.to_image_rgba()
            {
                decoded.insert(name.clone(), Arc::new(img));
            }
        }
        decoded
    }

    fn split_atlas_for_build(&mut self, build_idx: usize) {
        let Some(atlas_idx) = self.builds[build_idx].assigned_atlas else {
            return;
        };
        let Some(atlas_entry) = self.atlas_entries.get(atlas_idx) else {
            return;
        };
        let atlas_images = gather_atlas_images(&self.builds[build_idx].build, &atlas_entry.decoded);
        let _ = split_atlas(&mut self.builds[build_idx].build, &atlas_images);
        self.cache_dirty = true;
        self.needs_re_render = true;
    }

    fn try_auto_match_atlas(&mut self, build: &mut BuildFile, build_source: &str) -> Option<usize> {
        let lower_name = build.name.to_lowercase();
        let lower_source = build_source.to_lowercase();

        for (i, atlas_entry) in self.atlas_entries.iter().enumerate() {
            let atlas_lower = atlas_entry.source_name.to_lowercase();
            if atlas_lower == lower_name || atlas_lower == lower_source {
                let atlas_images = gather_atlas_images(build, &atlas_entry.decoded);
                let _ = split_atlas(build, &atlas_images);
                return Some(i);
            }
        }
        None
    }

    fn try_auto_match_build(&mut self, atlas_idx: usize) {
        let atlas_entry = &self.atlas_entries[atlas_idx];
        let lower = atlas_entry.source_name.to_lowercase();

        for entry in self.builds.iter_mut() {
            if entry.assigned_atlas.is_some() {
                continue;
            }
            let build_lower = entry.build.name.to_lowercase();
            let source_lower = entry.source_name.to_lowercase();
            if build_lower == lower || source_lower == lower {
                let atlas_images = gather_atlas_images(&entry.build, &atlas_entry.decoded);
                let _ = split_atlas(&mut entry.build, &atlas_images);
                entry.assigned_atlas = Some(atlas_idx);
                self.cache_dirty = true;
                self.needs_re_render = true;
                return;
            }
        }

        for (i, pb) in self.pending_builds.iter().enumerate() {
            let pb_lower = pb.build.name.to_lowercase();
            let pb_src_lower = pb.source_name.to_lowercase();
            if pb_lower == lower || pb_src_lower == lower {
                let mut build = std::mem::replace(
                    &mut self.pending_builds[i].build,
                    BuildFile {
                        version: 0,
                        name: String::new(),
                        symbols: Vec::new(),
                        atlases: Vec::new(),
                        symbol_index: HashMap::new(),
                    },
                );
                let atlas_images = gather_atlas_images(&build, &atlas_entry.decoded);
                let _ = split_atlas(&mut build, &atlas_images);
                self.pending_builds.remove(i);
                self.builds.push(BuildEntry {
                    build,
                    enabled: true,
                    source_name: atlas_entry.source_name.clone(),
                    assigned_atlas: Some(atlas_idx),
                });
                self.selected_build_idx = Some(self.builds.len() - 1);
                self.cache_dirty = true;
                self.needs_re_render = true;
                return;
            }
        }
    }

    fn load_file(&mut self, name: &str, data: &[u8]) {
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        let result = match ext.as_str() {
            "dyn" => parse_dyn(data),
            "zip" => parse_zip(data),
            "bin" => {
                let bin_type = crate::archive::detect_bin_type(data);
                match bin_type {
                    BinType::Anim => crate::archive::parse_anim_bin(data),
                    BinType::Build => crate::archive::parse_build_bin(data),
                    BinType::Unknown => {
                        self.error_message = Some(format!("Unknown .bin magic in {name}"));
                        return;
                    }
                }
            }
            _ => {
                self.error_message = Some(format!("Unsupported file extension: .{ext}"));
                return;
            }
        };

        let archive = match result {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to load {name}: {e}"));
                return;
            }
        };

        self.integrate_archive(name, archive);
    }

    fn integrate_archive(&mut self, name: &str, archive: ParsedArchive) {
        let ParsedArchive {
            anim,
            build,
            tex_files,
            raw_files: _,
        } = archive;

        let has_build = build.is_some();
        let has_tex = !tex_files.is_empty();

        if let Some(anim_file) = anim {
            self.anims.push(AnimEntry {
                anim: anim_file,
                enabled: true,
                source_name: name.to_string(),
            });
            self.active_anim_idx = self.anims.len() - 1;
            self.active_bank_idx = 0;
            self.active_anim_inner_idx = 0;
            self.active_frame_idx = 0;
        }

        if has_tex {
            let decoded = Self::decode_tex_files(&tex_files);
            let atlas_idx = self.atlas_entries.len();
            self.atlas_entries.push(AtlasEntry {
                enabled: true,
                source_name: name.to_string(),
                decoded,
            });

            if has_build {
                if let Some(mut build) = build {
                    let atlas_entry = &self.atlas_entries[atlas_idx];
                    let atlas_images = gather_atlas_images(&build, &atlas_entry.decoded);
                    let _ = split_atlas(&mut build, &atlas_images);
                    self.builds.push(BuildEntry {
                        build,
                        enabled: true,
                        source_name: name.to_string(),
                        assigned_atlas: Some(atlas_idx),
                    });
                    self.selected_build_idx = Some(self.builds.len() - 1);
                    self.cache_dirty = true;
                    self.needs_re_render = true;
                }
            } else {
                self.try_auto_match_build(atlas_idx);
            }
        } else if has_build && let Some(mut build) = build {
            let assigned = self.try_auto_match_atlas(&mut build, name);
            if assigned.is_some() {
                self.builds.push(BuildEntry {
                    build,
                    enabled: true,
                    source_name: name.to_string(),
                    assigned_atlas: assigned,
                });
                self.selected_build_idx = Some(self.builds.len() - 1);
                self.cache_dirty = true;
                self.needs_re_render = true;
            } else {
                self.pending_builds.push(PendingBuild {
                    build,
                    source_name: name.to_string(),
                });
            }
        }
    }

    fn get_current_animation(&self) -> Option<&crate::anim::AnimAnimation> {
        self.anims
            .get(self.active_anim_idx)
            .filter(|e| e.enabled)
            .and_then(|e| e.anim.banks.get(self.active_bank_idx))
            .and_then(|b| b.animations.get(self.active_anim_inner_idx))
    }

    fn invalidate_cache(&mut self) {
        self.cache_gen = self.cache_gen.wrapping_add(1);
        self.frame_cache.clear();
        self.animation_bounds = None;
        if let Some(bg) = self.bg_renderer.as_mut() {
            bg.stop();
        }
        self.cache_dirty = false;
        self.needs_re_render = true;
    }

    fn current_cache_key(&self) -> (usize, usize, usize) {
        (
            self.active_anim_idx,
            self.active_bank_idx,
            self.active_anim_inner_idx,
        )
    }

    fn collect_build_list(&self) -> Vec<&BuildFile> {
        self.builds
            .iter()
            .rev()
            .filter(|e| e.enabled && e.assigned_atlas.is_some())
            .map(|e| &e.build)
            .collect()
    }

    fn ensure_animation_bounds(&mut self) {
        if self.animation_bounds.is_some() {
            return;
        }
        let Some(anim) = self.get_current_animation() else {
            return;
        };
        let build_list = self.collect_build_list();
        if build_list.is_empty() {
            return;
        }
        self.animation_bounds =
            compute_animation_bounds(&anim.frames, &build_list, 1.0, (0.0, 0.0));
    }

    fn start_background_render(&mut self) {
        let Some(anim) = self.get_current_animation() else {
            return;
        };
        let build_list = self.collect_build_list();
        if build_list.is_empty() {
            return;
        }

        let anim_data = anim.clone();
        let build_data: Vec<BuildFile> = build_list.iter().map(|b| (*b).clone()).collect();
        let cache_gen_val = self.cache_gen;
        let total_frames = anim_data.frames.len();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let bounds = self.animation_bounds.clone();
        let (sender, receiver) = mpsc::channel::<(u64, usize, image::RgbaImage)>();

        let handle = std::thread::spawn(move || {
            let bl: Vec<&BuildFile> = build_data.iter().collect();
            for fi in 0..total_frames {
                if stop_flag_clone.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(rendered) =
                    render_frame(&anim_data.frames[fi], &bl, 1.0, (0.0, 0.0), bounds.as_ref())
                    && sender.send((cache_gen_val, fi, rendered.image)).is_err()
                {
                    return;
                }
            }
        });

        self.bg_renderer = Some(BackgroundRenderer {
            receiver,
            stop_flag,
            _thread_handle: Some(handle),
        });
    }

    fn poll_background_results(&mut self, ctx: &egui::Context) {
        let Some(bg) = self.bg_renderer.as_ref() else {
            return;
        };
        let current_gen = self.cache_gen;
        loop {
            match bg.receiver.try_recv() {
                Ok((recv_gen, fi, img)) => {
                    if recv_gen == current_gen {
                        let size = [img.width() as usize, img.height() as usize];
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                        let texture = ctx.load_texture(
                            format!("cached_frame_{fi}"),
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.frame_cache.insert(
                            fi,
                            FrameCacheEntry {
                                image: img,
                                texture: Some(texture),
                            },
                        );
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.bg_renderer = None;
                    break;
                }
            }
        }
    }

    fn render_current_frame(&mut self, ctx: &egui::Context) {
        if self.cache_dirty {
            self.invalidate_cache();
        }

        let anim_key = self.current_cache_key();
        if anim_key != self.cache_anim_key {
            self.cache_anim_key = anim_key;
            self.invalidate_cache();
        }

        self.ensure_animation_bounds();

        let Some(anim_frame) = self
            .get_current_animation()
            .and_then(|a| a.frames.get(self.active_frame_idx))
        else {
            self.frame_texture = None;
            self.rendered_image = None;
            self.needs_re_render = false;
            return;
        };

        if let Some(entry) = self.frame_cache.get(&self.active_frame_idx) {
            if entry.texture.is_some() {
                self.frame_texture = entry.texture.clone();
                self.rendered_image = Some(entry.image.clone());
                self.needs_re_render = false;
                return;
            } else {
                let size = [entry.image.width() as usize, entry.image.height() as usize];
                let color_image =
                    egui::ColorImage::from_rgba_unmultiplied(size, entry.image.as_raw());
                let texture = ctx.load_texture(
                    format!("cached_frame_{}", self.active_frame_idx),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.frame_texture = Some(texture.clone());
                self.rendered_image = Some(entry.image.clone());
                self.frame_cache
                    .get_mut(&self.active_frame_idx)
                    .unwrap()
                    .texture = Some(texture);
                self.needs_re_render = false;
                return;
            }
        }

        let build_list = self.collect_build_list();

        if build_list.is_empty() {
            self.frame_texture = None;
            self.rendered_image = None;
            self.needs_re_render = false;
            return;
        }

        if let Some(rendered) = render_frame(
            anim_frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            self.animation_bounds.as_ref(),
        ) {
            let size = [
                rendered.image.width() as usize,
                rendered.image.height() as usize,
            ];
            let color_image =
                egui::ColorImage::from_rgba_unmultiplied(size, rendered.image.as_raw());
            let texture = ctx.load_texture(
                format!("cached_frame_{}", self.active_frame_idx),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.frame_texture = Some(texture.clone());
            self.rendered_image = Some(rendered.image.clone());
            self.frame_cache.insert(
                self.active_frame_idx,
                FrameCacheEntry {
                    image: rendered.image,
                    texture: Some(texture),
                },
            );
        } else {
            self.frame_texture = None;
            self.rendered_image = None;
        }
        self.needs_re_render = false;

        if self.bg_renderer.is_none()
            && !self.frame_cache.contains_key(&(self.active_frame_idx + 1))
        {
            self.start_background_render();
        }
    }

    fn prev_frame(&mut self) {
        if let Some(anim) = self.get_current_animation() {
            let n = anim.frames.len();
            if n > 0 {
                self.active_frame_idx = if self.active_frame_idx == 0 {
                    n - 1
                } else {
                    self.active_frame_idx - 1
                };
                self.needs_re_render = true;
            }
        }
    }

    fn next_frame(&mut self) {
        if let Some(anim) = self.get_current_animation() {
            let n = anim.frames.len();
            if n > 0 {
                self.active_frame_idx = (self.active_frame_idx + 1) % n;
                self.needs_re_render = true;
            }
        }
    }

    fn export_gif(&mut self) {
        let Some(anim) = self.get_current_animation() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("GIF", &["gif"])
            .set_file_name("animation.gif")
            .save_file()
        else {
            return;
        };

        let build_list = self.collect_build_list();

        if build_list.is_empty() {
            self.error_message = Some("No builds loaded".to_string());
            return;
        }

        let frame_rate = anim.frame_rate;
        let total_frames = anim.frames.len();
        let bounds = self.animation_bounds.clone();
        let frames: Vec<image::RgbaImage> = (0..total_frames)
            .map(|fi| {
                if let Some(entry) = self.frame_cache.get(&fi) {
                    return entry.image.clone();
                }
                anim.frames
                    .get(fi)
                    .and_then(|f| render_frame(f, &build_list, 1.0, (0.0, 0.0), bounds.as_ref()))
                    .map(|r| r.image)
                    .unwrap_or_else(|| image::RgbaImage::new(1, 1))
            })
            .collect();

        let has_real_frames = frames.iter().any(|f| f.width() > 1 || f.height() > 1);
        if !has_real_frames {
            self.error_message = Some("No frames to export".to_string());
            return;
        }

        let mut buf = Vec::new();
        match export_gif(&frames, frame_rate, &mut buf) {
            Ok(()) => {
                if let Err(e) = std::fs::write(&path, &buf) {
                    self.error_message = Some(format!("Failed to save GIF: {e}"));
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to encode GIF: {e}"));
            }
        }
    }

    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open File").clicked()
                    && let Some(paths) = rfd::FileDialog::new()
                        .add_filter("DST Files", &["zip", "dyn", "bin"])
                        .pick_files()
                {
                    for path in paths {
                        if let Ok(data) = std::fs::read(&path) {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown");
                            self.load_file(name, &data);
                        }
                    }
                }

                ui.add_enabled_ui(self.rendered_image.is_some(), |ui| {
                    if ui.button("Export PNG").clicked()
                        && let Some(img) = &self.rendered_image
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .set_file_name("frame.png")
                            .save_file()
                        && let Err(e) = img.save(&path)
                    {
                        self.error_message = Some(format!("Failed to save PNG: {e}"));
                    }

                    let has_animation = self.get_current_animation().is_some();
                    ui.add_enabled_ui(has_animation, |ui| {
                        if ui.button("Export GIF").clicked() {
                            self.export_gif();
                        }
                    });
                });
            });
        });
    }

    fn show_left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .default_width(260.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_anim_section(ui);
                    ui.separator();
                    self.show_build_section(ui);
                    ui.separator();
                    self.show_atlas_section(ui);
                });
            });
    }

    fn show_anim_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Animations");

        if self.anims.is_empty() {
            ui.label("No animations loaded");
            return;
        }

        let mut remove_idx: Option<usize> = None;
        let anim_summaries: Vec<(bool, String)> = self
            .anims
            .iter()
            .map(|e| (e.enabled, e.source_name.clone()))
            .collect();

        for (idx, (_enabled, source_name)) in anim_summaries.iter().enumerate() {
            let is_active = idx == self.active_anim_idx;
            let header = egui::RichText::new(source_name).color(if is_active {
                egui::Color32::from_rgb(100, 180, 255)
            } else {
                egui::Color32::PLACEHOLDER
            });

            egui::CollapsingHeader::new(header)
                .id_salt(format!("anim_{idx}"))
                .default_open(is_active)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut en = self.anims[idx].enabled;
                        if ui.checkbox(&mut en, "Enabled").changed() {
                            self.anims[idx].enabled = en;
                            self.needs_re_render = true;
                        }
                        if ui.small_button("X").clicked() {
                            remove_idx = Some(idx);
                        }
                    });

                    let bank_data: Vec<(usize, String, Vec<(usize, String)>)> = self.anims[idx]
                        .anim
                        .banks
                        .iter()
                        .enumerate()
                        .map(|(bi, b)| {
                            (
                                bi,
                                b.name.clone(),
                                b.animations
                                    .iter()
                                    .enumerate()
                                    .map(|(ai, a)| (ai, a.name.clone()))
                                    .collect(),
                            )
                        })
                        .collect();

                    for (bank_idx, bank_name, animations) in bank_data {
                        egui::CollapsingHeader::new(&bank_name)
                            .id_salt(format!("anim_{idx}_bank_{bank_idx}"))
                            .default_open(false)
                            .show(ui, |ui| {
                                for (anim_inner_idx, anim_name) in &animations {
                                    let is_selected = is_active
                                        && bank_idx == self.active_bank_idx
                                        && *anim_inner_idx == self.active_anim_inner_idx;
                                    if ui.selectable_label(is_selected, anim_name).clicked() {
                                        self.active_anim_idx = idx;
                                        self.active_bank_idx = bank_idx;
                                        self.active_anim_inner_idx = *anim_inner_idx;
                                        self.active_frame_idx = 0;
                                        self.playing = false;
                                        self.cache_dirty = true;
                                        self.needs_re_render = true;
                                    }
                                }
                            });
                    }
                });
        }

        if let Some(idx) = remove_idx {
            self.anims.remove(idx);
            if self.anims.is_empty() {
                self.active_anim_idx = 0;
                self.active_bank_idx = 0;
                self.active_anim_inner_idx = 0;
                self.active_frame_idx = 0;
                self.cache_dirty = true;
                self.needs_re_render = true;
            } else if idx <= self.active_anim_idx {
                self.active_anim_idx = self.active_anim_idx.saturating_sub(1);
                self.active_bank_idx = 0;
                self.active_anim_inner_idx = 0;
                self.active_frame_idx = 0;
                self.cache_dirty = true;
                self.needs_re_render = true;
            }
        }
    }

    fn show_build_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Builds");

        if self.builds.is_empty() {
            ui.label("No builds loaded");
            return;
        }

        let build_summaries: Vec<(bool, String, String, usize, usize, Option<usize>)> = self
            .builds
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let is_selected = self.selected_build_idx == Some(idx);
                let frame_count: usize = entry.build.symbols.iter().map(|s| s.frames.len()).sum();
                let atlas_info = match entry.assigned_atlas {
                    Some(ai) => self
                        .atlas_entries
                        .get(ai)
                        .map(|a| a.source_name.clone())
                        .unwrap_or_else(|| "?".to_string()),
                    None => String::new(),
                };
                (
                    is_selected,
                    entry.source_name.clone(),
                    atlas_info,
                    entry.build.symbols.len(),
                    frame_count,
                    entry.assigned_atlas,
                )
            })
            .collect();

        let builds_len = self.builds.len();
        let mut new_enabled: Vec<(usize, bool)> = Vec::new();
        let mut swap_actions: Vec<(usize, usize)> = Vec::new();
        let mut remove_indices: Vec<usize> = Vec::new();
        let mut select_idx: Option<usize> = None;
        let mut need_re_render = false;

        for (idx, (is_selected, source_name, atlas_info, sym_count, _frame_count, _assigned)) in
            build_summaries.iter().enumerate()
        {
            let header = if atlas_info.is_empty() {
                format!("{} (no atlas)", source_name)
            } else {
                source_name.clone()
            };
            egui::CollapsingHeader::new(egui::RichText::new(header).color(if *is_selected {
                egui::Color32::from_rgb(100, 180, 255)
            } else {
                egui::Color32::PLACEHOLDER
            }))
            .id_salt(format!("build_{idx}"))
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut enabled = self.builds[idx].enabled;
                    if ui.checkbox(&mut enabled, "Enabled").changed() {
                        new_enabled.push((idx, enabled));
                    }
                    if idx > 0 && ui.small_button("Up").clicked() {
                        swap_actions.push((idx, idx - 1));
                    }
                    if idx + 1 < builds_len && ui.small_button("Down").clicked() {
                        swap_actions.push((idx, idx + 1));
                    }
                    if ui.small_button("X").clicked() {
                        remove_indices.push(idx);
                    }
                });

                if atlas_info.is_empty() {
                    ui.label(
                        egui::RichText::new("No atlas assigned")
                            .small()
                            .color(egui::Color32::YELLOW),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!("Atlas: {}", atlas_info))
                            .small()
                            .italics(),
                    );
                }

                ui.label(
                    egui::RichText::new(format!("Build: {}", self.builds[idx].build.name))
                        .small()
                        .italics(),
                );

                egui::CollapsingHeader::new(
                    egui::RichText::new(format!("Symbols ({})", sym_count)).small(),
                )
                .id_salt(format!("build_{idx}_symbols"))
                .default_open(false)
                .show(ui, |ui| {
                    for symbol in &self.builds[idx].build.symbols {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}  ({} frames)",
                                symbol.name,
                                symbol.frames.len(),
                            ))
                            .small(),
                        );
                    }
                });

                if ui.small_button("Select").clicked() {
                    select_idx = Some(idx);
                }
            });
        }

        for (idx, enabled) in new_enabled {
            self.builds[idx].enabled = enabled;
            need_re_render = true;
        }
        for (a, b) in swap_actions {
            self.builds.swap(a, b);
            need_re_render = true;
            if let Some(sel) = self.selected_build_idx {
                if sel == a {
                    self.selected_build_idx = Some(b);
                } else if sel == b {
                    self.selected_build_idx = Some(a);
                }
            }
        }
        remove_indices.sort();
        remove_indices.reverse();
        for idx in remove_indices {
            self.builds.remove(idx);
            need_re_render = true;
            if let Some(sel) = self.selected_build_idx {
                if sel == idx {
                    self.selected_build_idx = None;
                } else if sel > idx {
                    self.selected_build_idx = Some(sel - 1);
                }
            }
        }
        if let Some(idx) = select_idx {
            self.selected_build_idx = Some(idx);
        }

        if need_re_render {
            self.cache_dirty = true;
            self.needs_re_render = true;
        }

        if self.builds.is_empty() {
            ui.label("No builds loaded");
        }
    }

    fn show_atlas_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Atlas Pool");

        if self.atlas_entries.is_empty() && self.pending_builds.is_empty() {
            ui.label("No atlas files loaded");
            return;
        }

        let builds_len = self.builds.len();

        let atlas_info: Vec<AtlasDisplayInfo> = self
            .atlas_entries
            .iter()
            .enumerate()
            .map(|(atlas_idx, atlas_entry)| {
                let assigned_builds: Vec<(usize, String)> = self
                    .builds
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.assigned_atlas == Some(atlas_idx))
                    .map(|(i, b)| (i, b.build.name.clone()))
                    .collect();

                let status = if assigned_builds.is_empty() {
                    "unassigned".to_string()
                } else {
                    assigned_builds
                        .iter()
                        .map(|(_, n)| n.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let tex_count = atlas_entry.decoded.len();
                let decoded_images: Vec<(String, u32, u32)> = atlas_entry
                    .decoded
                    .iter()
                    .map(|(name, img)| (name.clone(), img.width(), img.height()))
                    .collect();

                let build_options: Vec<(usize, String)> =
                    self.builds
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| b.assigned_atlas.is_none())
                        .map(|(i, b)| (i, format!("{} ({})", b.source_name, b.build.name)))
                        .chain(self.pending_builds.iter().enumerate().map(|(i, pb)| {
                            (builds_len + i, format!("{} (pending)", pb.source_name))
                        }))
                        .collect();

                let header = format!("{} ({}) → {}", atlas_entry.source_name, tex_count, status);

                let default_open = assigned_builds.is_empty();

                AtlasDisplayInfo {
                    atlas_idx,
                    header,
                    decoded_images,
                    build_options,
                    default_open,
                }
            })
            .collect();

        let mut remove_atlas_idx: Option<usize> = None;
        let mut assign_action: Option<(usize, usize)> = None;

        for info in &atlas_info {
            egui::CollapsingHeader::new(&info.header)
                .id_salt(format!("atlas_{atlas_idx}", atlas_idx = info.atlas_idx))
                .default_open(info.default_open)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("X").clicked() {
                            remove_atlas_idx = Some(info.atlas_idx);
                        }
                    });

                    if !info.build_options.is_empty() {
                        let mut selected: usize = 0;
                        egui::ComboBox::from_id_salt(format!("atlas_assign_{}", info.atlas_idx))
                            .selected_text("Assign to Build...")
                            .show_ui(ui, |ui| {
                                for (i, label) in &info.build_options {
                                    ui.selectable_value(&mut selected, *i, label);
                                }
                            });

                        if ui.small_button("Assign").clicked() {
                            assign_action = Some((info.atlas_idx, info.build_options[selected].0));
                        }
                    }

                    for (name, w, h) in &info.decoded_images {
                        ui.label(
                            egui::RichText::new(format!("{} ({}×{})", name, w, h))
                                .small()
                                .color(egui::Color32::LIGHT_BLUE),
                        );
                    }
                });
        }

        if let Some((atlas_idx, target)) = assign_action {
            if target < builds_len {
                self.builds[target].assigned_atlas = Some(atlas_idx);
                self.split_atlas_for_build(target);
            } else {
                let pb_idx = target - builds_len;
                if pb_idx < self.pending_builds.len() {
                    let mut build = std::mem::replace(
                        &mut self.pending_builds[pb_idx].build,
                        BuildFile {
                            version: 0,
                            name: String::new(),
                            symbols: Vec::new(),
                            atlases: Vec::new(),
                            symbol_index: HashMap::new(),
                        },
                    );
                    let atlas_entry = &self.atlas_entries[atlas_idx];
                    let atlas_images = gather_atlas_images(&build, &atlas_entry.decoded);
                    let _ = split_atlas(&mut build, &atlas_images);
                    self.pending_builds.remove(pb_idx);
                    self.builds.push(BuildEntry {
                        build,
                        enabled: true,
                        source_name: self.atlas_entries[atlas_idx].source_name.clone(),
                        assigned_atlas: Some(atlas_idx),
                    });
                    self.selected_build_idx = Some(self.builds.len() - 1);
                    self.cache_dirty = true;
                    self.needs_re_render = true;
                }
            }
        }

        if let Some(idx) = remove_atlas_idx {
            for entry in self.builds.iter_mut() {
                if entry.assigned_atlas == Some(idx) {
                    entry.assigned_atlas = None;
                } else if let Some(ai) = entry.assigned_atlas
                    && ai > idx
                {
                    entry.assigned_atlas = Some(ai - 1);
                }
            }
            self.atlas_entries.remove(idx);
            self.cache_dirty = true;
            self.needs_re_render = true;
        }

        if !self.pending_builds.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Pending Builds (need atlas):")
                    .small()
                    .color(egui::Color32::YELLOW),
            );
            let mut remove_pb: Option<usize> = None;
            for (i, pb) in self.pending_builds.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} — {}", pb.source_name, pb.build.name))
                            .small()
                            .color(egui::Color32::LIGHT_BLUE),
                    );
                    if ui.small_button("X").clicked() {
                        remove_pb = Some(i);
                    }
                });
            }
            if let Some(i) = remove_pb {
                self.pending_builds.remove(i);
            }
        }
    }

    fn show_right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("right_panel")
            .default_width(300.0)
            .min_width(240.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_anim_info(ui);
                    ui.separator();
                    self.show_build_info(ui);
                });
            });
    }

    fn show_anim_info(&mut self, ui: &mut egui::Ui) {
        ui.heading("Animation Info");
        let Some(anim_entry) = self.anims.get(self.active_anim_idx) else {
            ui.label("No animation loaded");
            return;
        };
        if !anim_entry.enabled {
            ui.label("Animation disabled");
            return;
        }

        let anim = &anim_entry.anim;
        let Some(bank) = anim.banks.get(self.active_bank_idx) else {
            ui.label("No bank selected");
            return;
        };
        let Some(animation) = bank.animations.get(self.active_anim_inner_idx) else {
            ui.label("No animation selected");
            return;
        };

        ui.label(egui::RichText::new("Source").strong());
        ui.label(format!("  {}", anim_entry.source_name));
        ui.label(egui::RichText::new("Bank").strong());
        ui.label(format!("  {}", bank.name));
        ui.label(egui::RichText::new("Animation").strong());
        ui.label(format!("  {}", animation.name));
        ui.label(format!("  Frame rate: {}", animation.frame_rate));
        ui.label(format!("  Total frames: {}", animation.frames.len()));

        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Current Frame {}", self.active_frame_idx)).strong());

        if let Some(frame) = animation.frames.get(self.active_frame_idx) {
            ui.label(format!(
                "  Bounding box: ({:.1}, {:.1}) {:.1} x {:.1}",
                frame.x, frame.y, frame.width, frame.height,
            ));
            ui.label(format!("  Elements: {}", frame.elements.len()));
            ui.label(format!("  Events: {}", frame.events.len()));

            if !frame.events.is_empty() {
                ui.label(egui::RichText::new("  Events:").italics());
                for event in &frame.events {
                    ui.label(egui::RichText::new(format!("    {}", event)).small());
                }
            }

            ui.add_space(2.0);
            egui::CollapsingHeader::new(format!("Elements ({})", frame.elements.len()))
                .id_salt("anim_info_elements")
                .default_open(false)
                .show(ui, |ui| {
                    for (i, elem) in frame.elements.iter().enumerate() {
                        egui::CollapsingHeader::new(format!(
                            "#{} z={:.1} {}",
                            i, elem.z_index, elem.symbol
                        ))
                        .id_salt(format!("anim_info_elem_{i}"))
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!("  symbol: {}", elem.symbol)).small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("  frameNum: {}", elem.frame_num))
                                    .small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("  layer: {}", elem.layer_name))
                                    .small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("  zIndex: {:.2}", elem.z_index))
                                    .small(),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "  matrix: [{:.3}, {:.3}, {:.3}, {:.3}, {:.1}, {:.1}]",
                                    elem.a, elem.b, elem.c, elem.d, elem.tx, elem.ty
                                ))
                                .small(),
                            );
                        });
                    }
                });
        }
    }

    fn show_build_info(&mut self, ui: &mut egui::Ui) {
        ui.heading("Build Info");
        let Some(idx) = self.selected_build_idx else {
            ui.label("No build selected");
            return;
        };
        let Some(entry) = self.builds.get(idx) else {
            ui.label("Build not found");
            return;
        };

        let build = &entry.build;

        ui.label(egui::RichText::new("Source").strong());
        ui.label(format!("  {}", entry.source_name));
        ui.label(egui::RichText::new("Build").strong());
        ui.label(format!("  Name: {}", build.name));
        ui.label(format!("  Version: {}", build.version));
        ui.label(format!("  Symbols: {}", build.symbols.len()));

        ui.label(egui::RichText::new("Atlas").strong());
        if let Some(ai) = entry.assigned_atlas {
            if let Some(ae) = self.atlas_entries.get(ai) {
                let atlas_name = build
                    .atlases
                    .first()
                    .map(|a| a.name.as_str())
                    .unwrap_or("?");
                ui.label(format!("  {} (from {})", atlas_name, ae.source_name));
            } else {
                ui.label("  (invalid ref)");
            }
        } else {
            ui.label("  (none)");
        }

        ui.add_space(2.0);
        egui::CollapsingHeader::new(format!("Symbols ({})", build.symbols.len()))
            .id_salt("build_info_symbols")
            .default_open(false)
            .show(ui, |ui| {
                for symbol in &build.symbols {
                    let frame_count = symbol.frames.len();
                    let has_image = symbol.frames.iter().any(|f| f.image.is_some());
                    let img_mark = if has_image { " [img]" } else { "" };
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  ({} frames){}",
                            symbol.name, frame_count, img_mark
                        ))
                        .small(),
                    );
                }
            });
    }

    fn show_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.frame_texture {
                let avail = ui.available_size();
                let img_size = texture.size_vec2();
                let scale_x = avail.x / img_size.x;
                let scale_y = avail.y / img_size.y;
                let scale = scale_x.min(scale_y).min(1.0);
                let display_size = img_size * scale;
                ui.centered_and_justified(|ui| {
                    ui.image((texture.id(), display_size));
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "No animation loaded.\nOpen .zip, .dyn or .bin files, or drag and drop.",
                        )
                        .text_style(egui::TextStyle::Heading)
                        .color(egui::Color32::GRAY),
                    );
                });
            }
        });
    }

    fn show_bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Prev").clicked() {
                    self.prev_frame();
                    self.playing = false;
                }

                let play_label = if self.playing { "Pause" } else { "Play" };
                if ui.button(play_label).clicked() {
                    self.playing = !self.playing;
                    self.last_frame_time = Instant::now();
                    if self.playing && self.bg_renderer.is_none() {
                        self.start_background_render();
                    }
                }

                if ui.button("Next").clicked() {
                    self.next_frame();
                    self.playing = false;
                }

                ui.separator();

                if let Some(anim) = self.get_current_animation() {
                    let total = anim.frames.len();
                    ui.label(format!("Frame {} / {}", self.active_frame_idx + 1, total));
                    ui.label(format!("  {} fps", anim.frame_rate));
                } else {
                    ui.label("No animation selected");
                }

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.play_speed, 0.1..=5.0)
                        .text("Speed")
                        .step_by(0.1),
                );
            });
        });
    }

    fn show_error_overlay(&mut self, ctx: &egui::Context) {
        if self.error_message.is_none() {
            return;
        }
        egui::Area::new(egui::Id::new("error_overlay"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 30.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, self.error_message.as_ref().unwrap());
                        if ui.button("Close").clicked() {
                            self.error_message = None;
                        }
                    });
                });
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(paths) = self.initial_files.take()
            && !paths.is_empty()
        {
            for path in paths {
                if let Ok(data) = std::fs::read(&path) {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    self.load_file(name, &data);
                }
            }
        }

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for dropped in dropped_files {
            let name = dropped.name.clone();
            if let Some(bytes) = &dropped.bytes {
                self.load_file(&name, bytes);
            } else if let Some(path) = &dropped.path
                && let Ok(data) = std::fs::read(path)
            {
                let display_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(&name);
                self.load_file(display_name, &data);
            }
        }

        self.poll_background_results(ctx);

        if self.playing {
            if let Some(anim) = self.get_current_animation() {
                let frame_duration =
                    Duration::from_secs_f32(1.0 / (anim.frame_rate * self.play_speed));
                if self.last_frame_time.elapsed() >= frame_duration {
                    self.next_frame();
                    self.last_frame_time = Instant::now();
                }
            }
            ctx.request_repaint();
        }

        if self.needs_re_render {
            self.render_current_frame(ctx);
        }

        self.show_top_bar(ctx);
        self.show_left_panel(ctx);
        self.show_right_panel(ctx);
        self.show_central_panel(ctx);
        self.show_bottom_bar(ctx);
        self.show_error_overlay(ctx);
    }
}
