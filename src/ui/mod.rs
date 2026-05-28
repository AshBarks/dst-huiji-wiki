mod cache;
mod export;
mod panels;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cache::{BackgroundLoader, BackgroundRenderer, FrameCacheEntry};
use export::{BackgroundGifExport, BackgroundPngExport, GifExportResult, PngExportResult};

use crate::archive::{parse_dyn, parse_zip};
use crate::atlas::gather_atlas_images;
use crate::render::BoundingBox;
use crate::specs::PixelFormat;

pub(super) struct TexMeta {
    name: String,
    width: u16,
    height: u16,
    pixel_format: PixelFormat,
}

pub(super) struct AtlasEntry {
    source_name: String,
    decoded: std::collections::HashMap<String, Arc<image::RgbaImage>>,
    tex_meta: Vec<TexMeta>,
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
    pub disabled_symbols: std::collections::HashSet<String>,
}

pub struct App {
    initial_files: Option<Vec<PathBuf>>,
    anims: Vec<AnimEntry>,
    active_anim_idx: usize,
    active_bank_idx: usize,
    active_anim_inner_idx: usize,
    active_frame_idx: usize,
    builds: Vec<BuildEntry>,
    atlas_entries: Vec<Option<AtlasEntry>>,
    loaded_paths: HashSet<PathBuf>,
    playing: bool,
    play_speed: f32,
    last_frame_time: Instant,
    frame_texture: Option<egui::TextureHandle>,
    rendered_image: Option<Arc<image::RgbaImage>>,
    needs_re_render: bool,
    error_message: Option<String>,
    frame_cache: HashMap<usize, FrameCacheEntry>,
    cache_gen: u64,
    cache_anim_key: (usize, usize, usize),
    cache_dirty: bool,
    bg_renderer: Option<BackgroundRenderer>,
    bg_loader: Option<BackgroundLoader>,
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
            bg_loader: None,
            animation_bounds: None,
            gif_export: None,
            png_export: None,
        }
    }

    fn canonicalize_path(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
        let (decoded, tex_meta) = Self::decode_tex_files_with_meta(all_tex_files);
        let atlas_idx = self.atlas_entries.len();
        self.atlas_entries.push(Some(AtlasEntry {
            source_name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            decoded,
            tex_meta,
        }));

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
        let Some(atlas_entry) = self.atlas_entries.get(atlas_idx).and_then(|e| e.as_ref()) else {
            return;
        };
        let atlas_images = gather_atlas_images(&build, &atlas_entry.decoded);
        let _ = crate::atlas::split_atlas(&mut build, &atlas_images);
        self.builds[build_idx].build = Some(build);
        self.loaded_paths.insert(canonical);
        self.cache_dirty = true;
        self.needs_re_render = true;
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

        let cached_frames: HashMap<usize, Arc<image::RgbaImage>> = self
            .frame_cache
            .iter()
            .map(|(&fi, entry)| (fi, entry.image.clone()))
            .collect();

        let receiver = export::start_gif_export_thread(anim, &build_list, cached_frames);
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
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
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

        let cached_frames: HashMap<usize, Arc<image::RgbaImage>> = self
            .frame_cache
            .iter()
            .map(|(&fi, entry)| (fi, entry.image.clone()))
            .collect();

        let receiver = export::start_png_export_thread(anim, &build_list, cached_frames, dir);
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
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.png_export = None;
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(paths) = self.initial_files.take()
            && !paths.is_empty()
        {
            for path in paths {
                self.spawn_file_load(path, None);
            }
        }

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for dropped in dropped_files {
            if let Some(path) = &dropped.path {
                self.spawn_file_load(path.clone(), None);
            } else if let Some(bytes) = &dropped.bytes {
                let name = dropped.name.clone();
                let fake_path = PathBuf::from(&name);
                self.spawn_file_load(fake_path, Some(bytes.to_vec()));
            }
        }

        self.poll_loader(ctx);
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
