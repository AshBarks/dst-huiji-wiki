use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::archive::{BinType, parse_dyn, parse_zip};
use crate::atlas::gather_atlas_images;
use crate::ktex::parse_ktex;
use crate::render::{render_frame, render_frame_with_elements};

use super::{AnimEntry, App, AtlasEntry, BuildEntry};

pub struct FrameCacheEntry {
    pub image: Arc<image::RgbaImage>,
    pub texture: Option<egui::TextureHandle>,
}

pub struct LoadedData {
    pub canonical: PathBuf,
    pub companion_canonical: Option<PathBuf>,
    pub source_name: String,
    pub anim: Option<crate::anim::AnimFile>,
    pub build: Option<crate::build_file::BuildFile>,
    pub decoded_textures: HashMap<String, Arc<image::RgbaImage>>,
    pub is_pending_atlas: bool,
}

pub enum LoadResult {
    Success(Box<LoadedData>),
    Failed(PathBuf, String),
}

pub struct BackgroundLoader {
    pub sender: std::sync::mpsc::Sender<LoadResult>,
    pub receiver: std::sync::mpsc::Receiver<LoadResult>,
    pub pending: usize,
}

pub struct BackgroundRenderer {
    pub receiver: std::sync::mpsc::Receiver<(u64, usize, image::RgbaImage)>,
    pub stop_flag: Arc<std::sync::atomic::AtomicBool>,
    _thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl BackgroundRenderer {
    pub fn stop(&mut self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for BackgroundRenderer {
    fn drop(&mut self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl App {
    pub fn decode_tex_files(
        tex_files: &std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>,
    ) -> std::collections::HashMap<String, Arc<image::RgbaImage>> {
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

    pub fn split_atlas_for_build(&mut self, build_idx: usize) {
        let Some(atlas_idx) = self.builds[build_idx].assigned_atlas else {
            return;
        };
        let Some(atlas_entry) = self.atlas_entries.get(atlas_idx).and_then(|e| e.as_ref()) else {
            return;
        };
        let Some(build) = &mut self.builds[build_idx].build else {
            return;
        };
        let atlas_images = gather_atlas_images(build, &atlas_entry.decoded);
        let _ = crate::atlas::split_atlas(build, &atlas_images);
        self.cache_dirty = true;
        self.needs_re_render = true;
    }

    pub fn collect_build_list(&self) -> Vec<crate::render::BuildRef<'_>> {
        self.builds
            .iter()
            .rev()
            .filter(|e| e.enabled && e.build.is_some() && e.assigned_atlas.is_some())
            .filter_map(|e| {
                e.build.as_ref().map(|b| crate::render::BuildRef {
                    build: b,
                    disabled_symbols: &e.disabled_symbols,
                })
            })
            .collect()
    }

    pub fn invalidate_cache(&mut self) {
        self.cache_gen = self.cache_gen.wrapping_add(1);
        self.frame_cache.clear();
        self.animation_bounds = None;
        if let Some(bg) = self.bg_renderer.as_mut() {
            bg.stop();
        }
        self.cache_dirty = false;
        self.needs_re_render = true;
    }

    pub fn current_cache_key(&self) -> (usize, usize, usize) {
        (
            self.active_anim_idx,
            self.active_bank_idx,
            self.active_anim_inner_idx,
        )
    }

    pub fn get_current_animation(&self) -> Option<&crate::anim::AnimAnimation> {
        self.anims
            .get(self.active_anim_idx)
            .filter(|e| e.enabled)
            .and_then(|e| e.anim.banks.get(self.active_bank_idx))
            .and_then(|b| b.animations.get(self.active_anim_inner_idx))
    }

    pub fn ensure_animation_bounds(&mut self) {
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
            crate::render::compute_animation_bounds(&anim.frames, &build_list, 1.0, (0.0, 0.0));
    }

    pub fn start_background_render(&mut self) {
        let Some(anim) = self.get_current_animation() else {
            return;
        };
        let build_list = self.collect_build_list();
        if build_list.is_empty() {
            return;
        }

        let (bounds, prepared) =
            crate::render::prepare_animation_frames(&anim.frames, &build_list, 1.0, (0.0, 0.0));
        let cache_gen_val = self.cache_gen;
        let total_frames = anim.frames.len();
        if total_frames == 0 {
            return;
        }
        let start_frame = (self.active_frame_idx + 1) % total_frames;
        let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let (sender, receiver) = std::sync::mpsc::channel::<(u64, usize, image::RgbaImage)>();

        let handle = std::thread::spawn(move || {
            for offset in 0..total_frames {
                if stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let fi = (start_frame + offset) % total_frames;
                if let Some(pf) = &prepared.get(fi).and_then(|p| p.as_ref()) {
                    let render_bounds = bounds.as_ref().unwrap_or(&pf.bounds);
                    if let Some(rendered) =
                        render_frame_with_elements(&pf.elements, render_bounds, 1.0, (0.0, 0.0))
                        && sender.send((cache_gen_val, fi, rendered.image)).is_err()
                    {
                        return;
                    }
                }
            }
        });

        self.bg_renderer = Some(BackgroundRenderer {
            receiver,
            stop_flag,
            _thread_handle: Some(handle),
        });
    }

    pub fn poll_background_results(&mut self, ctx: &egui::Context) {
        let Some(bg) = self.bg_renderer.as_ref() else {
            return;
        };
        let current_gen = self.cache_gen;
        loop {
            match bg.receiver.try_recv() {
                Ok((recv_gen, fi, img)) => {
                    if recv_gen == current_gen {
                        let img_arc = Arc::new(img);
                        let size = [img_arc.width() as usize, img_arc.height() as usize];
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, img_arc.as_raw());
                        let texture = ctx.load_texture(
                            format!("cached_frame_{fi}"),
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.frame_cache.insert(
                            fi,
                            FrameCacheEntry {
                                image: img_arc,
                                texture: Some(texture),
                            },
                        );
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.bg_renderer = None;
                    break;
                }
            }
        }
    }

    pub fn render_current_frame(&mut self, ctx: &egui::Context) {
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
            let img_arc = Arc::new(rendered.image);
            let size = [img_arc.width() as usize, img_arc.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img_arc.as_raw());
            let texture = ctx.load_texture(
                format!("cached_frame_{}", self.active_frame_idx),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.frame_texture = Some(texture.clone());
            self.rendered_image = Some(img_arc.clone());
            self.frame_cache.insert(
                self.active_frame_idx,
                FrameCacheEntry {
                    image: img_arc,
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

    pub fn prev_frame(&mut self) {
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

    pub fn next_frame(&mut self) {
        if let Some(anim) = self.get_current_animation() {
            let n = anim.frames.len();
            if n > 0 {
                self.active_frame_idx = (self.active_frame_idx + 1) % n;
                self.needs_re_render = true;
            }
        }
    }

    pub fn spawn_file_load(&mut self, path: PathBuf, data: Option<Vec<u8>>) {
        let canonical = Self::canonicalize_path(&path);
        if self.loaded_paths.contains(&canonical) {
            return;
        }

        if self.bg_loader.is_none() {
            let (sender, receiver) = std::sync::mpsc::channel::<LoadResult>();
            self.bg_loader = Some(BackgroundLoader {
                sender,
                receiver,
                pending: 0,
            });
        }

        let sender = self.bg_loader.as_ref().unwrap().sender.clone();
        self.bg_loader.as_mut().unwrap().pending += 1;

        std::thread::spawn(move || {
            let data = match data {
                Some(d) => d,
                None => match std::fs::read(&path) {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = sender.send(LoadResult::Failed(
                            path,
                            format!("Failed to read file: {e}"),
                        ));
                        return;
                    }
                },
            };
            let result = Self::do_load_file(path, data, &canonical);
            let _ = sender.send(result);
        });
    }

    fn do_load_file(path: PathBuf, data: Vec<u8>, canonical: &std::path::Path) -> LoadResult {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let result = match ext.as_str() {
            "dyn" => parse_dyn(&data),
            "zip" => parse_zip(&data),
            "bin" => {
                let bin_type = crate::archive::detect_bin_type(&data);
                match bin_type {
                    BinType::Anim => crate::archive::parse_anim_bin(&data),
                    BinType::Build => crate::archive::parse_build_bin(&data),
                    BinType::Unknown => {
                        let msg = format!("Unknown .bin magic in {}", path.display());
                        return LoadResult::Failed(path, msg);
                    }
                }
            }
            _ => return LoadResult::Failed(path, format!("Unsupported file extension: .{ext}")),
        };

        let mut archive = match result {
            Ok(a) => a,
            Err(e) => {
                let msg = format!("Failed to load {}: {e}", path.display());
                return LoadResult::Failed(path, msg);
            }
        };

        let has_anim = archive.anim.is_some();
        let has_build = archive.build.is_some();
        let has_tex = archive.tex_sources.iter().any(|s| !s.tex_files.is_empty());
        let real_path = path.canonicalize().is_ok();
        let source_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut companion_canonical = None;
        let mut is_pending_atlas = false;

        match ext.as_str() {
            "zip" => {
                if !has_tex && !has_build && !has_anim {
                    return LoadResult::Success(Box::new(LoadedData {
                        canonical: canonical.to_path_buf(),
                        companion_canonical: None,
                        source_name,
                        anim: None,
                        build: None,
                        decoded_textures: HashMap::new(),
                        is_pending_atlas: false,
                    }));
                }
                if !has_tex && !has_build && has_anim {
                    return LoadResult::Success(Box::new(LoadedData {
                        canonical: canonical.to_path_buf(),
                        companion_canonical: None,
                        source_name,
                        anim: archive.anim.take(),
                        build: None,
                        decoded_textures: HashMap::new(),
                        is_pending_atlas: false,
                    }));
                }
                if !has_tex && has_build && real_path {
                    let companion = path.with_extension("dyn");
                    let comp_canonical = companion
                        .canonicalize()
                        .unwrap_or_else(|_| companion.clone());
                    if companion.exists()
                        && let Ok(companion_data) = std::fs::read(&companion)
                        && let Ok(companion_archive) = parse_dyn(&companion_data)
                    {
                        archive.merge(companion_archive);
                        companion_canonical = Some(comp_canonical);
                    }
                }
                if has_tex && !has_build && !has_anim {
                    return LoadResult::Failed(
                        path,
                        "ZIP contains only textures without build data — cannot be used alone"
                            .to_string(),
                    );
                }
            }
            "dyn" => {
                if real_path {
                    let companion = path.with_extension("zip");
                    let comp_canonical = companion
                        .canonicalize()
                        .unwrap_or_else(|_| companion.clone());
                    if companion.exists()
                        && let Ok(companion_data) = std::fs::read(&companion)
                        && let Ok(companion_archive) = parse_zip(&companion_data)
                    {
                        archive.merge(companion_archive);
                        companion_canonical = Some(comp_canonical);
                    }
                }
                if archive.build.is_none() {
                    is_pending_atlas = true;
                }
            }
            _ => {}
        }

        let decoded = if archive.tex_sources.iter().any(|s| !s.tex_files.is_empty()) {
            let all_tex = archive.tex_files();
            Self::decode_tex_files(all_tex)
        } else {
            HashMap::new()
        };

        let mut build = archive.build.take();

        if let Some(ref mut b) = build
            && !decoded.is_empty()
        {
            let atlas_images = gather_atlas_images(b, &decoded);
            let _ = crate::atlas::split_atlas(b, &atlas_images);
        }

        LoadResult::Success(Box::new(LoadedData {
            canonical: canonical.to_path_buf(),
            companion_canonical,
            source_name,
            anim: archive.anim.take(),
            build,
            decoded_textures: decoded,
            is_pending_atlas,
        }))
    }

    pub fn integrate_loaded_data(&mut self, data: LoadedData) {
        self.loaded_paths.insert(data.canonical);
        if let Some(comp) = data.companion_canonical {
            self.loaded_paths.insert(comp);
        }

        if data.anim.is_none() && data.build.is_none() && data.decoded_textures.is_empty() {
            return;
        }

        if data.is_pending_atlas {
            let atlas_idx = self.atlas_entries.len();
            self.atlas_entries.push(Some(AtlasEntry {
                source_name: data.source_name.clone(),
                decoded: data.decoded_textures,
            }));
            self.builds.push(BuildEntry {
                build: None,
                enabled: true,
                source_name: data.source_name,
                assigned_atlas: Some(atlas_idx),
                disabled_symbols: std::collections::HashSet::new(),
            });
            return;
        }

        if let Some(anim_file) = data.anim {
            self.anims.push(AnimEntry {
                anim: anim_file,
                enabled: true,
                source_name: data.source_name.clone(),
            });
            self.active_anim_idx = self.anims.len() - 1;
            self.active_bank_idx = 0;
            self.active_anim_inner_idx = 0;
            self.active_frame_idx = 0;
        }

        if !data.decoded_textures.is_empty() {
            let atlas_idx = self.atlas_entries.len();
            self.atlas_entries.push(Some(AtlasEntry {
                source_name: data.source_name.clone(),
                decoded: data.decoded_textures,
            }));

            if let Some(build) = data.build {
                self.builds.push(BuildEntry {
                    build: Some(build),
                    enabled: true,
                    source_name: data.source_name,
                    assigned_atlas: Some(atlas_idx),
                    disabled_symbols: std::collections::HashSet::new(),
                });
                self.cache_dirty = true;
                self.needs_re_render = true;
            }
        } else if let Some(build) = data.build {
            self.builds.push(BuildEntry {
                build: Some(build),
                enabled: true,
                source_name: data.source_name,
                assigned_atlas: None,
                disabled_symbols: std::collections::HashSet::new(),
            });
        }
    }

    pub fn poll_loader(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.bg_loader.as_mut() else {
            return;
        };
        let mut received: Vec<LoadResult> = Vec::new();
        loop {
            match loader.receiver.try_recv() {
                Ok(result) => {
                    loader.pending = loader.pending.saturating_sub(1);
                    received.push(result);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    loader.pending = 0;
                    break;
                }
            }
        }
        let pending = self.bg_loader.as_ref().map_or(0, |l| l.pending);
        if pending == 0 {
            self.bg_loader = None;
        }
        let any_received = !received.is_empty();
        for result in received {
            match result {
                LoadResult::Success(data) => {
                    self.integrate_loaded_data(*data);
                }
                LoadResult::Failed(_path, msg) => {
                    self.error_message = Some(msg);
                }
            }
        }
        if any_received {
            ctx.request_repaint();
        }
    }

    pub fn is_loading(&self) -> bool {
        self.bg_loader.as_ref().is_some_and(|l| l.pending > 0)
    }
}
