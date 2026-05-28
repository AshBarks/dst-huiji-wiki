use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;

use rayon::prelude::*;

use dst_anim_tool::gif_export::{export_gif, export_png_sequence, ffmpeg_gif_from_sequence};
use dst_anim_tool::render::{prepare_animation_frames, render_frame_with_elements};

pub enum GifExportResult {
    Done(Vec<u8>),
    Failed(String),
}

pub enum PngExportResult {
    Done,
    Failed(String),
}

pub struct BackgroundGifExport {
    pub receiver: mpsc::Receiver<GifExportResult>,
    pub path: PathBuf,
}

pub struct BackgroundPngExport {
    pub receiver: mpsc::Receiver<PngExportResult>,
}

pub fn start_gif_export_thread(
    anim: &dst_anim_tool::anim::AnimAnimation,
    build_list: &[dst_anim_tool::render::BuildRef<'_>],
    cached_frames: HashMap<usize, Arc<image::RgbaImage>>,
    disabled_elements: HashSet<(String, String)>,
) -> mpsc::Receiver<GifExportResult> {
    let (bounds, prepared) = prepare_animation_frames(
        &anim.frames,
        build_list,
        1.0,
        (0.0, 0.0),
        &disabled_elements,
    );
    let frame_rate = anim.frame_rate;

    let (sender, receiver) = mpsc::channel::<GifExportResult>();

    std::thread::spawn(move || {
        let total_frames = prepared.len();

        let frames: Vec<image::RgbaImage> = (0..total_frames)
            .into_par_iter()
            .map(|fi| {
                if let Some(cached) = cached_frames.get(&fi) {
                    return (**cached).clone();
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

    receiver
}

pub fn start_png_export_thread(
    anim: &dst_anim_tool::anim::AnimAnimation,
    build_list: &[dst_anim_tool::render::BuildRef<'_>],
    cached_frames: HashMap<usize, Arc<image::RgbaImage>>,
    disabled_elements: HashSet<(String, String)>,
    output_dir: PathBuf,
) -> mpsc::Receiver<PngExportResult> {
    let (bounds, prepared) = prepare_animation_frames(
        &anim.frames,
        build_list,
        1.0,
        (0.0, 0.0),
        &disabled_elements,
    );
    let frame_rate = anim.frame_rate;

    let (sender, receiver) = mpsc::channel::<PngExportResult>();

    std::thread::spawn(move || {
        let total_frames = prepared.len();

        let frames: Vec<image::RgbaImage> = (0..total_frames)
            .into_par_iter()
            .map(|fi| {
                if let Some(cached) = cached_frames.get(&fi) {
                    return (**cached).clone();
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

        if let Err(e) = export_png_sequence(&frames, &output_dir) {
            let _ = sender.send(PngExportResult::Failed(format!(
                "Failed to export PNG sequence: {e}"
            )));
            return;
        }

        let gif_path = output_dir.join("animation.gif");
        match ffmpeg_gif_from_sequence(&output_dir, &gif_path, frame_rate, frames.len()) {
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

    receiver
}
