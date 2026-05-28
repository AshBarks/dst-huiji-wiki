use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::archive::{BinType, ParsedArchive, parse_dyn, parse_zip};
use crate::atlas::{gather_atlas_images, split_atlas};
use crate::gif_export::{export_gif, export_png_sequence, ffmpeg_gif_from_sequence};
use crate::ktex::parse_ktex;
use crate::render::{
    BoundingBox, compute_animation_bounds, prepare_animation_frames, render_frame,
    render_frame_with_elements,
};

enum GifExportResult {
    Done(Vec<u8>),
    Failed(String),
}

enum PngExportResult {
    Done,
    Failed(String),
}

struct AnimEntry {
    anim: crate::anim::AnimFile,
    enabled: bool,
    source_name: String,
}

pub struct BuildEntry {
    pub build: Option<crate::build_file::BuildFile>,
    pub enabled: bool,
    pub source_name: String,
    pub assigned_atlas: Option<usize>,
}

struct AtlasEntry {
    source_name: String,
    decoded: std::collections::HashMap<String, Arc<image::RgbaImage>>,
}

struct FrameCacheEntry {
    image: image::RgbaImage,
    texture: Option<egui::TextureHandle>,
}

struct BackgroundGifExport {
    receiver: mpsc::Receiver<GifExportResult>,
    path: PathBuf,
}

struct BackgroundPngExport {
    receiver: mpsc::Receiver<PngExportResult>,
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
    loaded_paths: HashSet<PathBuf>,
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
    gif_export: Option<BackgroundGifExport>,
    png_export: Option<BackgroundPngExport>,
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
            loaded_paths: HashSet::new(),
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
            gif_export: None,
            png_export: None,
        }
    }

    fn decode_tex_files(
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

    fn split_atlas_for_build(&mut self, build_idx: usize) {
        let Some(atlas_idx) = self.builds[build_idx].assigned_atlas else {
            return;
        };
        let Some(atlas_entry) = self.atlas_entries.get(atlas_idx) else {
            return;
        };
        let Some(build) = &mut self.builds[build_idx].build else {
            return;
        };
        let atlas_images = gather_atlas_images(build, &atlas_entry.decoded);
        let _ = split_atlas(build, &atlas_images);
        self.cache_dirty = true;
        self.needs_re_render = true;
    }

    fn canonicalize_path(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn is_real_path(path: &Path) -> bool {
        path.canonicalize().is_ok()
    }

    fn load_file(&mut self, path: &Path, data: &[u8]) {
        let canonical = Self::canonicalize_path(path);
        if self.loaded_paths.contains(&canonical) {
            return;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let result = match ext.as_str() {
            "dyn" => parse_dyn(data),
            "zip" => parse_zip(data),
            "bin" => {
                let bin_type = crate::archive::detect_bin_type(data);
                match bin_type {
                    BinType::Anim => crate::archive::parse_anim_bin(data),
                    BinType::Build => crate::archive::parse_build_bin(data),
                    BinType::Unknown => {
                        self.error_message =
                            Some(format!("Unknown .bin magic in {}", path.display()));
                        return;
                    }
                }
            }
            _ => {
                self.error_message = Some(format!("Unsupported file extension: .{ext}"));
                return;
            }
        };

        let mut archive = match result {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to load {}: {e}", path.display()));
                return;
            }
        };

        let has_anim = archive.anim.is_some();
        let has_build = archive.build.is_some();
        let has_tex = archive.tex_sources.iter().any(|s| !s.tex_files.is_empty());
        let real_path = Self::is_real_path(path);

        match ext.as_str() {
            "zip" => {
                if !has_tex && !has_build && !has_anim {
                    return;
                }
                if !has_tex && !has_build && has_anim {
                    self.loaded_paths.insert(canonical);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    self.integrate_archive(name, archive);
                    return;
                }
                if !has_tex && has_build {
                    if real_path {
                        let companion = path.with_extension("dyn");
                        let companion_canonical = Self::canonicalize_path(&companion);
                        if companion.exists()
                            && !self.loaded_paths.contains(&companion_canonical)
                            && let Ok(companion_data) = std::fs::read(&companion)
                            && let Ok(companion_archive) = parse_dyn(&companion_data)
                        {
                            archive.merge(companion_archive);
                            self.loaded_paths.insert(companion_canonical);
                        }
                    }
                    self.loaded_paths.insert(canonical);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    self.integrate_archive(name, archive);
                    return;
                }
                if has_tex && !has_build && !has_anim {
                    self.error_message = Some(
                        "ZIP contains only textures without build data — cannot be used alone"
                            .to_string(),
                    );
                    return;
                }
                self.loaded_paths.insert(canonical);
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                self.integrate_archive(name, archive);
            }
            "dyn" => {
                if real_path {
                    let companion = path.with_extension("zip");
                    let companion_canonical = Self::canonicalize_path(&companion);
                    if companion.exists()
                        && !self.loaded_paths.contains(&companion_canonical)
                        && let Ok(companion_data) = std::fs::read(&companion)
                        && let Ok(companion_archive) = parse_zip(&companion_data)
                    {
                        archive.merge(companion_archive);
                        self.loaded_paths.insert(companion_canonical);
                    }
                }
                self.loaded_paths.insert(canonical);
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                if archive.build.is_some() {
                    self.integrate_archive(name, archive);
                } else {
                    self.integrate_as_pending_atlas(name, archive);
                }
            }
            _ => {
                self.loaded_paths.insert(canonical);
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                self.integrate_archive(name, archive);
            }
        }
    }

    fn integrate_archive(&mut self, name: &str, mut archive: ParsedArchive) {
        let has_tex = archive.tex_sources.iter().any(|s| !s.tex_files.is_empty());

        if let Some(anim_file) = archive.anim.take() {
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
            let all_tex_files = archive.tex_files();
            let decoded = Self::decode_tex_files(&all_tex_files);
            let atlas_idx = self.atlas_entries.len();
            self.atlas_entries.push(AtlasEntry {
                source_name: name.to_string(),
                decoded,
            });

            if let Some(mut build) = archive.build.take() {
                let atlas_entry = &self.atlas_entries[atlas_idx];
                let atlas_images = gather_atlas_images(&build, &atlas_entry.decoded);
                let _ = split_atlas(&mut build, &atlas_images);
                self.builds.push(BuildEntry {
                    build: Some(build),
                    enabled: true,
                    source_name: name.to_string(),
                    assigned_atlas: Some(atlas_idx),
                });
                self.selected_build_idx = Some(self.builds.len() - 1);
                self.cache_dirty = true;
                self.needs_re_render = true;
            }
        } else if let Some(build) = archive.build.take() {
            self.builds.push(BuildEntry {
                build: Some(build),
                enabled: true,
                source_name: name.to_string(),
                assigned_atlas: None,
            });
            self.selected_build_idx = Some(self.builds.len() - 1);
        }
    }

    fn integrate_as_pending_atlas(&mut self, name: &str, archive: ParsedArchive) {
        let all_tex_files = archive.tex_files();
        let decoded = Self::decode_tex_files(&all_tex_files);
        let atlas_idx = self.atlas_entries.len();
        self.atlas_entries.push(AtlasEntry {
            source_name: name.to_string(),
            decoded,
        });

        self.builds.push(BuildEntry {
            build: None,
            enabled: true,
            source_name: name.to_string(),
            assigned_atlas: Some(atlas_idx),
        });
    }

    fn associate_dyn_to_build(&mut self, build_idx: usize, path: &Path) {
        let canonical = Self::canonicalize_path(path);
        if self.loaded_paths.contains(&canonical) {
            self.error_message = Some("File already loaded".to_string());
            return;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                self.error_message = Some(format!("Failed to read {}: {e}", path.display()));
                return;
            }
        };

        let archive = match parse_dyn(&data) {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to parse {}: {e}", path.display()));
                return;
            }
        };

        if archive.tex_sources.iter().all(|s| s.tex_files.is_empty()) {
            self.error_message = Some("No textures in .dyn file".to_string());
            return;
        }

        let all_tex_files = archive.tex_files();
        let decoded = Self::decode_tex_files(&all_tex_files);
        let atlas_idx = self.atlas_entries.len();
        self.atlas_entries.push(AtlasEntry {
            source_name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            decoded,
        });

        self.loaded_paths.insert(canonical);
        self.builds[build_idx].assigned_atlas = Some(atlas_idx);
        self.split_atlas_for_build(build_idx);
    }

    fn associate_zip_to_pending_atlas(&mut self, build_idx: usize, path: &Path) {
        let canonical = Self::canonicalize_path(path);
        if self.loaded_paths.contains(&canonical) {
            self.error_message = Some("File already loaded".to_string());
            return;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                self.error_message = Some(format!("Failed to read {}: {e}", path.display()));
                return;
            }
        };

        let archive = match parse_zip(&data) {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to parse {}: {e}", path.display()));
                return;
            }
        };

        if archive.build.is_none() {
            self.error_message = Some("No build.bin in .zip file".to_string());
            return;
        }

        if let Some(anim_file) = archive.anim {
            self.anims.push(AnimEntry {
                anim: anim_file,
                enabled: true,
                source_name: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            });
            self.active_anim_idx = self.anims.len() - 1;
            self.active_bank_idx = 0;
            self.active_anim_inner_idx = 0;
            self.active_frame_idx = 0;
        }

        let mut build = archive.build.unwrap();
        let Some(atlas_idx) = self.builds[build_idx].assigned_atlas else {
            return;
        };
        let Some(atlas_entry) = self.atlas_entries.get(atlas_idx) else {
            return;
        };
        let atlas_images = gather_atlas_images(&build, &atlas_entry.decoded);
        let _ = split_atlas(&mut build, &atlas_images);
        self.builds[build_idx].build = Some(build);
        self.loaded_paths.insert(canonical);
        self.cache_dirty = true;
        self.needs_re_render = true;
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

    fn collect_build_list(&self) -> Vec<&crate::build_file::BuildFile> {
        self.builds
            .iter()
            .rev()
            .filter(|e| e.enabled && e.build.is_some() && e.assigned_atlas.is_some())
            .filter_map(|e| e.build.as_ref())
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

        let (bounds, prepared) =
            prepare_animation_frames(&anim.frames, &build_list, 1.0, (0.0, 0.0));
        let cache_gen_val = self.cache_gen;
        let total_frames = anim.frames.len();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let (sender, receiver) = mpsc::channel::<(u64, usize, image::RgbaImage)>();

        let handle = std::thread::spawn(move || {
            for fi in 0..total_frames {
                if stop_flag_clone.load(Ordering::Relaxed) {
                    return;
                }
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

    fn start_gif_export(&mut self) {
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

        let (bounds, prepared) =
            prepare_animation_frames(&anim.frames, &build_list, 1.0, (0.0, 0.0));
        let frame_rate = anim.frame_rate;

        let cached_frames: HashMap<usize, image::RgbaImage> = self
            .frame_cache
            .iter()
            .map(|(&fi, entry)| (fi, entry.image.clone()))
            .collect();

        let (sender, receiver) = mpsc::channel::<GifExportResult>();

        std::thread::spawn(move || {
            let total_frames = prepared.len();

            let frames: Vec<image::RgbaImage> = (0..total_frames)
                .into_par_iter()
                .map(|fi| {
                    if let Some(cached) = cached_frames.get(&fi) {
                        return cached.clone();
                    }
                    if let Some(pf) = prepared.get(fi).and_then(|p| p.as_ref()) {
                        let render_bounds = bounds.as_ref().unwrap_or(&pf.bounds);
                        render_frame_with_elements(&pf.elements, render_bounds, 1.0, (0.0, 0.0))
                            .map(|r| r.image)
                            .unwrap_or_else(|| image::RgbaImage::new(1, 1))
                    } else {
                        image::RgbaImage::new(1, 1)
                    }
                })
                .collect();

            let has_real_frames = frames.iter().any(|f| f.width() > 1 || f.height() > 1);
            if !has_real_frames {
                let _ = sender.send(GifExportResult::Failed("No frames to export".to_string()));
                return;
            }

            let mut buf = Vec::new();
            match export_gif(&frames, frame_rate, &mut buf) {
                Ok(()) => {
                    let _ = sender.send(GifExportResult::Done(buf));
                }
                Err(e) => {
                    let _ = sender.send(GifExportResult::Failed(format!(
                        "Failed to encode GIF: {e}"
                    )));
                }
            }
        });

        self.gif_export = Some(BackgroundGifExport { receiver, path });
    }

    fn poll_gif_export(&mut self) {
        let Some(bg) = self.gif_export.as_ref() else {
            return;
        };
        match bg.receiver.try_recv() {
            Ok(GifExportResult::Done(buf)) => {
                let path = self.gif_export.take().unwrap().path;
                if let Err(e) = std::fs::write(&path, &buf) {
                    self.error_message = Some(format!("Failed to save GIF: {e}"));
                }
            }
            Ok(GifExportResult::Failed(msg)) => {
                self.gif_export = None;
                self.error_message = Some(msg);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.gif_export = None;
            }
        }
    }

    fn start_png_export(&mut self) {
        let Some(anim) = self.get_current_animation() else {
            return;
        };
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Select output directory for PNG sequence")
            .pick_folder()
        else {
            return;
        };

        let build_list = self.collect_build_list();
        if build_list.is_empty() {
            self.error_message = Some("No builds loaded".to_string());
            return;
        }

        let (bounds, prepared) =
            prepare_animation_frames(&anim.frames, &build_list, 1.0, (0.0, 0.0));
        let frame_rate = anim.frame_rate;
        let cached_frames: HashMap<usize, image::RgbaImage> = self
            .frame_cache
            .iter()
            .map(|(&fi, entry)| (fi, entry.image.clone()))
            .collect();

        let (sender, receiver) = mpsc::channel::<PngExportResult>();
        let dir_clone = dir.clone();

        std::thread::spawn(move || {
            let total_frames = prepared.len();

            let frames: Vec<image::RgbaImage> = (0..total_frames)
                .into_par_iter()
                .map(|fi| {
                    if let Some(cached) = cached_frames.get(&fi) {
                        return cached.clone();
                    }
                    if let Some(pf) = prepared.get(fi).and_then(|p| p.as_ref()) {
                        let render_bounds = bounds.as_ref().unwrap_or(&pf.bounds);
                        render_frame_with_elements(&pf.elements, render_bounds, 1.0, (0.0, 0.0))
                            .map(|r| r.image)
                            .unwrap_or_else(|| image::RgbaImage::new(1, 1))
                    } else {
                        image::RgbaImage::new(1, 1)
                    }
                })
                .collect();

            let has_real_frames = frames.iter().any(|f| f.width() > 1 || f.height() > 1);
            if !has_real_frames {
                let _ = sender.send(PngExportResult::Failed("No frames to export".to_string()));
                return;
            }

            if let Err(e) = export_png_sequence(&frames, &dir_clone) {
                let _ = sender.send(PngExportResult::Failed(format!(
                    "Failed to export PNG sequence: {e}"
                )));
                return;
            }

            let gif_path = dir_clone.join("animation.gif");
            match ffmpeg_gif_from_sequence(&dir_clone, &gif_path, frame_rate, frames.len()) {
                Ok(()) => {
                    let _ = sender.send(PngExportResult::Done);
                }
                Err(_) => {
                    let _ = sender.send(PngExportResult::Failed(
                        "PNG sequence exported. ffmpeg not found or failed — GIF not generated."
                            .to_string(),
                    ));
                }
            }
        });

        self.png_export = Some(BackgroundPngExport { receiver });
    }

    fn poll_png_export(&mut self) {
        let Some(bg) = self.png_export.as_ref() else {
            return;
        };
        match bg.receiver.try_recv() {
            Ok(PngExportResult::Done) => {
                self.png_export = None;
            }
            Ok(PngExportResult::Failed(msg)) => {
                self.png_export = None;
                self.error_message = Some(msg);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.png_export = None;
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
                            self.load_file(&path, &data);
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
                        let exporting = self.gif_export.is_some() || self.png_export.is_some();
                        let label = if exporting {
                            "Exporting..."
                        } else {
                            "Export GIF"
                        };
                        if ui
                            .add_enabled(!exporting, egui::Button::new(label))
                            .on_hover_text(if exporting {
                                "Export in progress..."
                            } else {
                                "Export animation as GIF (builtin quantizer)"
                            })
                            .clicked()
                        {
                            self.start_gif_export();
                        }

                        let png_exporting = self.gif_export.is_some() || self.png_export.is_some();
                        let png_label = if png_exporting {
                            "Exporting..."
                        } else {
                            "Export PNG+GIF"
                        };
                        if ui
                            .add_enabled(!png_exporting, egui::Button::new(png_label))
                            .on_hover_text(if png_exporting {
                                "Export in progress..."
                            } else {
                                "Export PNG sequence, then convert to GIF via ffmpeg (better quality)"
                            })
                            .clicked()
                        {
                            self.start_png_export();
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

        let build_summaries: Vec<(bool, String, Option<usize>, bool)> = self
            .builds
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let is_selected = self.selected_build_idx == Some(idx);
                let has_build = entry.build.is_some();
                (
                    is_selected,
                    entry.source_name.clone(),
                    entry.assigned_atlas,
                    has_build,
                )
            })
            .collect();

        let builds_len = self.builds.len();
        let mut new_enabled: Vec<(usize, bool)> = Vec::new();
        let mut swap_actions: Vec<(usize, usize)> = Vec::new();
        let mut remove_indices: Vec<usize> = Vec::new();
        let mut select_idx: Option<usize> = None;
        let mut browse_dyn: Option<usize> = None;
        let mut browse_zip: Option<usize> = None;
        let mut need_re_render = false;

        for (idx, (is_selected, source_name, assigned, has_build)) in
            build_summaries.iter().enumerate()
        {
            let atlas_name = assigned
                .and_then(|ai| self.atlas_entries.get(ai))
                .map(|a| a.source_name.clone())
                .unwrap_or_default();

            let header = if *has_build && assigned.is_some() {
                if atlas_name.is_empty() {
                    source_name.clone()
                } else {
                    format!("{} [{}]", source_name, atlas_name)
                }
            } else if *has_build {
                format!("{} (pending atlas)", source_name)
            } else {
                format!("{} (pending build)", source_name)
            };

            let header_color = if !has_build || assigned.is_none() {
                egui::Color32::YELLOW
            } else if *is_selected {
                egui::Color32::from_rgb(100, 180, 255)
            } else {
                egui::Color32::PLACEHOLDER
            };

            egui::CollapsingHeader::new(egui::RichText::new(header).color(header_color))
                .id_salt(format!("build_{idx}"))
                .default_open(!*has_build || assigned.is_none())
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if *has_build {
                            let mut enabled = self.builds[idx].enabled;
                            if ui.checkbox(&mut enabled, "Enabled").changed() {
                                new_enabled.push((idx, enabled));
                            }
                        }
                        if *has_build && idx > 0 && ui.small_button("Up").clicked() {
                            swap_actions.push((idx, idx - 1));
                        }
                        if *has_build && idx + 1 < builds_len && ui.small_button("Down").clicked() {
                            swap_actions.push((idx, idx + 1));
                        }
                        if ui.small_button("X").clicked() {
                            remove_indices.push(idx);
                        }
                    });

                    if *has_build && assigned.is_none() {
                        ui.label(
                            egui::RichText::new("No atlas — needs a .dyn atlas file")
                                .small()
                                .color(egui::Color32::YELLOW),
                        );
                        if ui.small_button("Browse .dyn...").clicked() {
                            browse_dyn = Some(idx);
                        }
                    }

                    if !has_build && assigned.is_some() {
                        ui.label(
                            egui::RichText::new("No build — needs a .zip build file")
                                .small()
                                .color(egui::Color32::YELLOW),
                        );
                        if ui.small_button("Browse .zip...").clicked() {
                            browse_zip = Some(idx);
                        }
                        if let Some(atlas_idx) = assigned
                            && let Some(atlas_entry) = self.atlas_entries.get(*atlas_idx)
                        {
                            for (name, img) in &atlas_entry.decoded {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ({}x{})",
                                        name,
                                        img.width(),
                                        img.height()
                                    ))
                                    .small()
                                    .color(egui::Color32::LIGHT_BLUE),
                                );
                            }
                        }
                    }

                    if *has_build {
                        if let Some(atlas_idx) = assigned
                            && let Some(ae) = self.atlas_entries.get(*atlas_idx)
                        {
                            ui.label(
                                egui::RichText::new(format!("Atlas: {}", ae.source_name))
                                    .small()
                                    .italics(),
                            );
                        }

                        if let Some(build) = &self.builds[idx].build {
                            ui.label(
                                egui::RichText::new(format!("Build: {}", build.name))
                                    .small()
                                    .italics(),
                            );

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("Symbols ({})", build.symbols.len()))
                                    .small(),
                            )
                            .id_salt(format!("build_{idx}_symbols"))
                            .default_open(false)
                            .show(ui, |ui| {
                                for symbol in &build.symbols {
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
                        }

                        if ui.small_button("Select").clicked() {
                            select_idx = Some(idx);
                        }
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
            if let Some(atlas_idx) = self.builds[idx].assigned_atlas {
                let other_refs = self
                    .builds
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != idx)
                    .filter(|(_, b)| b.assigned_atlas == Some(atlas_idx))
                    .count();
                if other_refs == 0 {
                    self.atlas_entries.remove(atlas_idx);
                    for entry in self.builds.iter_mut() {
                        if let Some(ai) = entry.assigned_atlas {
                            if ai > atlas_idx {
                                entry.assigned_atlas = Some(ai - 1);
                            } else if ai == atlas_idx {
                                entry.assigned_atlas = None;
                            }
                        }
                    }
                }
            }
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

        if let Some(build_idx) = browse_dyn
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("DST Atlas", &["dyn"])
                .pick_file()
        {
            self.associate_dyn_to_build(build_idx, &path);
        }
        if let Some(build_idx) = browse_zip
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("DST Build", &["zip"])
                .pick_file()
        {
            self.associate_zip_to_pending_atlas(build_idx, &path);
        }

        if need_re_render {
            self.cache_dirty = true;
            self.needs_re_render = true;
        }

        if self.builds.is_empty() {
            ui.label("No builds loaded");
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

        ui.label(egui::RichText::new("Source").strong());
        ui.label(format!("  {}", entry.source_name));

        if let Some(build) = &entry.build {
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
        } else {
            ui.label(egui::RichText::new("Atlas").strong());
            if let Some(ai) = entry.assigned_atlas
                && let Some(ae) = self.atlas_entries.get(ai)
            {
                for (name, img) in &ae.decoded {
                    ui.label(format!("  {} ({}x{})", name, img.width(), img.height()));
                }
            }
            ui.label(
                egui::RichText::new("Pending — needs a .zip build file")
                    .color(egui::Color32::YELLOW),
            );
        }
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
                    self.load_file(&path, &data);
                }
            }
        }

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for dropped in dropped_files {
            if let Some(path) = &dropped.path {
                if let Ok(data) = std::fs::read(path) {
                    self.load_file(path, &data);
                }
            } else if let Some(bytes) = &dropped.bytes {
                let name = dropped.name.clone();
                let fake_path = PathBuf::from(&name);
                self.load_file(&fake_path, bytes);
            }
        }

        self.poll_background_results(ctx);
        self.poll_gif_export();
        self.poll_png_export();

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
