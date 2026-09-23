use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;

use rayon::prelude::*;

use dst_anim_tool::gif_export::{GifWriter, ffmpeg_gif_from_sequence};
use dst_anim_tool::render::{
    SymbolOverrideMap, prepare_animation_frames_with_overrides, render_frame_with_elements,
    snap_frame_bounds,
};

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

const EXPORT_CHUNK: usize = 24;
const EXPORT_CHUNK_BYTES: u64 = 128 * 1024 * 1024;

fn export_chunk_size(canvas_w: u32, canvas_h: u32) -> usize {
    let frame_bytes = (canvas_w as u64) * (canvas_h as u64) * 4;
    if frame_bytes == 0 {
        return EXPORT_CHUNK;
    }
    ((EXPORT_CHUNK_BYTES / frame_bytes).max(1) as usize).min(EXPORT_CHUNK)
}

fn union_size(bounds: Option<&dst_anim_tool::render::BoundingBox>) -> (u32, u32) {
    bounds
        .map(|b| {
            (
                (b.right - b.left).ceil().max(1.0) as u32,
                (b.bottom - b.top).ceil().max(1.0) as u32,
            )
        })
        .unwrap_or((1, 1))
}

fn render_chunk(
    start: usize,
    end: usize,
    prepared: &[Option<dst_anim_tool::render::PreparedFrame>],
    bounds: Option<&dst_anim_tool::render::BoundingBox>,
    cached_frames: &HashMap<usize, (Arc<image::RgbaImage>, (i64, i64))>,
) -> Vec<(image::RgbaImage, i64, i64)> {
    (start..end)
        .into_par_iter()
        .map(|fi| {
            if let Some((cached, off)) = cached_frames.get(&fi) {
                return ((**cached).clone(), off.0, off.1);
            }
            if let Some(pf) = prepared.get(fi).and_then(|p| p.as_ref()) {
                if let Some(union) = bounds {
                    let (snapped, off_x, off_y) = snap_frame_bounds(&pf.bounds, union);
                    return render_frame_with_elements(&pf.elements, &snapped, 1.0, (0.0, 0.0))
                        .map(|r| (r.image, off_x, off_y))
                        .unwrap_or_else(|| (image::RgbaImage::new(1, 1), 0, 0));
                }
                return render_frame_with_elements(&pf.elements, &pf.bounds, 1.0, (0.0, 0.0))
                    .map(|r| (r.image, 0i64, 0i64))
                    .unwrap_or_else(|| (image::RgbaImage::new(1, 1), 0, 0));
            }
            (image::RgbaImage::new(1, 1), 0, 0)
        })
        .collect()
}

fn pad_to_union(
    frame: &image::RgbaImage,
    off_x: i64,
    off_y: i64,
    uw: u32,
    uh: u32,
) -> image::RgbaImage {
    let mut canvas = image::RgbaImage::new(uw, uh);
    let dst_x = off_x.max(0) as usize;
    let dst_y = off_y.max(0) as usize;
    let src_x0 = (-off_x).max(0) as usize;
    let src_y0 = (-off_y).max(0) as usize;
    let fw = frame.width() as usize;
    let fh = frame.height() as usize;
    let src_buf = frame.as_raw();
    let dst_buf = canvas.as_mut();
    let cw = uw as usize;
    for y in src_y0..fh {
        let dy = dst_y + (y - src_y0);
        if dy >= uh as usize {
            break;
        }
        let src_row = y * fw * 4;
        let dst_row = dy * cw * 4;
        for x in src_x0..fw {
            let dx = dst_x + (x - src_x0);
            if dx >= uw as usize {
                break;
            }
            let src_off = src_row + x * 4;
            let dst_off = dst_row + dx * 4;
            if src_buf[src_off + 3] != 0 {
                dst_buf[dst_off..dst_off + 4].copy_from_slice(&src_buf[src_off..src_off + 4]);
            }
        }
    }
    canvas
}

pub fn start_gif_export_thread(
    anim: &dst_anim_tool::anim::AnimAnimation,
    build_list: &[dst_anim_tool::render::BuildRef<'_>],
    cached_frames: HashMap<usize, (Arc<image::RgbaImage>, (i64, i64))>,
    disabled_elements: HashSet<(String, String)>,
    disabled_symbols: HashSet<String>,
    symbol_overrides: SymbolOverrideMap,
) -> mpsc::Receiver<GifExportResult> {
    let (bounds, prepared) = prepare_animation_frames_with_overrides(
        &anim.frames,
        build_list,
        1.0,
        (0.0, 0.0),
        &disabled_elements,
        &disabled_symbols,
        &symbol_overrides,
    );
    let frame_rate = anim.frame_rate;

    let (sender, receiver) = mpsc::channel::<GifExportResult>();

    std::thread::spawn(move || {
        let total_frames = prepared.len();
        let (canvas_w, canvas_h) = union_size(bounds.as_ref());

        let mut buf = Vec::new();
        let mut writer = match GifWriter::new(
            &mut buf,
            canvas_w as u16,
            canvas_h as u16,
            frame_rate,
            [255, 255, 255],
        ) {
            Ok(w) => w,
            Err(e) => {
                let _ = sender.send(GifExportResult::Failed(format!(
                    "Failed to initialize GIF encoder: {e}"
                )));
                return;
            }
        };

        let mut has_real_frames = false;
        let chunk_size = export_chunk_size(canvas_w, canvas_h);
        for start in (0..total_frames).step_by(chunk_size) {
            let end = (start + chunk_size).min(total_frames);
            let chunk = render_chunk(start, end, &prepared, bounds.as_ref(), &cached_frames);
            for (frame, off_x, off_y) in &chunk {
                if frame.width() > 1 || frame.height() > 1 {
                    has_real_frames = true;
                }
                if let Err(e) = writer.write_frame(frame, *off_x, *off_y) {
                    let _ = sender.send(GifExportResult::Failed(format!(
                        "Failed to encode GIF: {e}"
                    )));
                    return;
                }
            }
        }

        drop(writer);
        if !has_real_frames {
            let _ = sender.send(GifExportResult::Failed("No frames to export".to_string()));
            return;
        }
        let _ = sender.send(GifExportResult::Done(buf));
    });

    receiver
}

pub fn start_png_export_thread(
    anim: &dst_anim_tool::anim::AnimAnimation,
    build_list: &[dst_anim_tool::render::BuildRef<'_>],
    cached_frames: HashMap<usize, (Arc<image::RgbaImage>, (i64, i64))>,
    disabled_elements: HashSet<(String, String)>,
    disabled_symbols: HashSet<String>,
    symbol_overrides: SymbolOverrideMap,
    output_dir: PathBuf,
) -> mpsc::Receiver<PngExportResult> {
    let (bounds, prepared) = prepare_animation_frames_with_overrides(
        &anim.frames,
        build_list,
        1.0,
        (0.0, 0.0),
        &disabled_elements,
        &disabled_symbols,
        &symbol_overrides,
    );
    let frame_rate = anim.frame_rate;

    let (sender, receiver) = mpsc::channel::<PngExportResult>();

    std::thread::spawn(move || {
        let total_frames = prepared.len();
        if let Err(e) = std::fs::create_dir_all(&output_dir) {
            let _ = sender.send(PngExportResult::Failed(format!(
                "Failed to create output directory: {e}"
            )));
            return;
        }
        let digits = format!("{}", total_frames).len().max(4);

        let (canvas_w, canvas_h) = union_size(bounds.as_ref());
        let mut has_real_frames = false;
        let chunk_size = export_chunk_size(canvas_w, canvas_h);
        for start in (0..total_frames).step_by(chunk_size) {
            let end = (start + chunk_size).min(total_frames);
            let chunk = render_chunk(start, end, &prepared, bounds.as_ref(), &cached_frames);
            for (frame, _off_x, _off_y) in &chunk {
                if frame.width() > 1 || frame.height() > 1 {
                    has_real_frames = true;
                }
            }
            let save_result: Result<(), dst_anim_tool::error::Error> = chunk
                .par_iter()
                .enumerate()
                .map(|(i, (frame, off_x, off_y))| {
                    let path = output_dir.join(format!(
                        "frame_{:0>width$}.png",
                        start + i,
                        width = digits
                    ));
                    pad_to_union(frame, *off_x, *off_y, canvas_w, canvas_h)
                        .save(&path)
                        .map_err(|e| {
                            dst_anim_tool::error::Error::Io(std::io::Error::other(e.to_string()))
                        })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|_| ());
            if let Err(e) = save_result {
                let _ = sender.send(PngExportResult::Failed(format!(
                    "Failed to export PNG sequence: {e}"
                )));
                return;
            }
        }

        if !has_real_frames {
            let _ = sender.send(PngExportResult::Failed("No frames to export".to_string()));
            return;
        }

        let gif_path = output_dir.join("animation.gif");
        match ffmpeg_gif_from_sequence(&output_dir, &gif_path, frame_rate, total_frames) {
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
