use std::io::Write;
use std::path::Path;

use crate::error::Result;

pub fn export_gif(
    frames: &[image::RgbaImage],
    frame_rate: f32,
    output: &mut dyn Write,
) -> Result<()> {
    export_gif_with_bg(frames, frame_rate, output, [255, 255, 255])
}

pub fn export_gif_with_bg(
    frames: &[image::RgbaImage],
    frame_rate: f32,
    output: &mut dyn Write,
    bg: [u8; 3],
) -> Result<()> {
    if frames.is_empty() {
        return Err(crate::error::Error::UnknownFormat(
            "no frames to export".to_string(),
        ));
    }

    let max_width = frames.iter().map(|f| f.width()).max().unwrap_or(1);
    let max_height = frames.iter().map(|f| f.height()).max().unwrap_or(1);
    let width = max_width as u16;
    let height = max_height as u16;
    let frame_delay = (100.0 / frame_rate).round().max(1.0) as u16;

    let mut encoder = gif::Encoder::new(output, width, height, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;

    for frame in frames {
        let mut pixels: Vec<u8> =
            Vec::with_capacity((max_width as usize) * (max_height as usize) * 4);
        let fw = frame.width() as usize;
        let fh = frame.height() as usize;
        let raw: &[u8] = frame.as_raw();

        for y in 0..max_height as usize {
            if y < fh {
                let row_start = y * fw * 4;
                let row_end = row_start + fw * 4;
                pixels.extend_from_slice(&raw[row_start..row_end]);
                let remaining = (max_width as usize - fw) * 4;
                pixels.extend(std::iter::repeat_n(0, remaining));
            } else {
                pixels.extend(std::iter::repeat_n(0, max_width as usize * 4));
            }
        }

        let (palette, indices) = simple_quantize(&pixels, bg);

        let gif_frame = gif::Frame {
            width,
            height,
            buffer: std::borrow::Cow::Owned(indices),
            palette: Some(palette),
            transparent: Some(0),
            delay: frame_delay,
            dispose: gif::DisposalMethod::Background,
            ..Default::default()
        };

        encoder.write_frame(&gif_frame)?;
    }

    Ok(())
}

const TRANSPARENT_ALPHA_THRESHOLD: u8 = 128;

fn simple_quantize(rgba: &[u8], bg: [u8; 3]) -> (Vec<u8>, Vec<u8>) {
    let sentinel = [0, 0, 2];
    let mut palette = vec![sentinel[0], sentinel[1], sentinel[2]];
    let mut color_map: std::collections::HashMap<[u8; 3], u8> = std::collections::HashMap::new();
    color_map.insert(sentinel, 0);
    let mut palette_colors: Vec<[u8; 3]> = vec![sentinel];

    let mut indices = Vec::with_capacity(rgba.len() / 4);

    for chunk in rgba.chunks_exact(4) {
        let a = chunk[3];
        if a < TRANSPARENT_ALPHA_THRESHOLD {
            indices.push(0);
            continue;
        }

        let (r, g, b) = if a < 255 {
            let ia = 255 - a as u32;
            let sa = a as u32;
            (
                ((chunk[0] as u32 * sa + bg[0] as u32 * ia + 127) / 255) as u8,
                ((chunk[1] as u32 * sa + bg[1] as u32 * ia + 127) / 255) as u8,
                ((chunk[2] as u32 * sa + bg[2] as u32 * ia + 127) / 255) as u8,
            )
        } else {
            (chunk[0], chunk[1], chunk[2])
        };

        let r = r & 0xFC;
        let g = g & 0xFC;
        let b = b & 0xFC;
        let key = [r, g, b];

        let idx = match color_map.get(&key) {
            Some(&i) => i,
            None => {
                if color_map.len() < 256 {
                    let i = color_map.len() as u8;
                    palette.push(r);
                    palette.push(g);
                    palette.push(b);
                    palette_colors.push(key);
                    color_map.insert(key, i);
                    i
                } else {
                    nearest_palette_index(&palette_colors, &key)
                }
            }
        };
        indices.push(idx);
    }

    while palette.len() < 256 * 3 {
        palette.push(0);
    }

    (palette, indices)
}

fn nearest_palette_index(palette: &[[u8; 3]], color: &[u8; 3]) -> u8 {
    let mut best_idx = 1u8;
    let mut best_dist = i32::MAX;
    for (i, p) in palette.iter().enumerate().skip(1) {
        let dr = color[0] as i32 - p[0] as i32;
        let dg = color[1] as i32 - p[1] as i32;
        let db = color[2] as i32 - p[2] as i32;
        let dist = dr * dr + dg * dg + db * db;
        if dist < best_dist {
            best_dist = dist;
            best_idx = i as u8;
        }
    }
    best_idx
}

pub fn export_png_sequence(frames: &[image::RgbaImage], output_dir: &Path) -> Result<()> {
    if frames.is_empty() {
        return Err(crate::error::Error::UnknownFormat(
            "no frames to export".to_string(),
        ));
    }
    std::fs::create_dir_all(output_dir)?;
    let digits = format!("{}", frames.len()).len().max(4);
    for (i, frame) in frames.iter().enumerate() {
        let filename = format!("frame_{:0>width$}.png", i, width = digits);
        let path = output_dir.join(filename);
        frame
            .save(&path)
            .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;
    }
    Ok(())
}

pub fn ffmpeg_gif_from_sequence(
    input_dir: &Path,
    output_path: &Path,
    frame_rate: f32,
    frame_count: usize,
) -> Result<()> {
    let has_ffmpeg = which::which("ffmpeg").is_ok();
    if !has_ffmpeg {
        return Err(crate::error::Error::UnknownFormat(
            "ffmpeg not found in PATH".to_string(),
        ));
    }
    let digits = format!("{}", frame_count).len().max(4);
    let pattern = format!("frame_%0{}d.png", digits);
    let input_pattern = input_dir.join(&pattern);
    let palette_path = input_dir.join("palette.png");
    let fps = format!("{:.2}", frame_rate);

    let palette_status = std::process::Command::new("ffmpeg")
        .args([
            "-framerate",
            &fps,
            "-i",
            input_pattern.to_str().unwrap_or(""),
            "-vf",
            "palettegen=stats_mode=diff",
            "-y",
            palette_path.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(crate::error::Error::Io)?;

    if !palette_status.success() {
        let _ = std::fs::remove_file(&palette_path);
        return Err(crate::error::Error::UnknownFormat(
            "ffmpeg palettegen failed".to_string(),
        ));
    }

    let gif_status = std::process::Command::new("ffmpeg")
        .args([
            "-framerate",
            &fps,
            "-i",
            input_pattern.to_str().unwrap_or(""),
            "-i",
            palette_path.to_str().unwrap_or(""),
            "-lavfi",
            "paletteuse=dither=sierra2_4a",
            "-y",
            output_path.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(crate::error::Error::Io)?;

    let _ = std::fs::remove_file(&palette_path);

    if !gif_status.success() {
        return Err(crate::error::Error::UnknownFormat(
            "ffmpeg gif encoding failed".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_pixels_get_index_zero() {
        let rgba: Vec<u8> = vec![
            255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 255, 128, 128, 128, 200,
        ];
        let (_, indices) = simple_quantize(&rgba, [255, 255, 255]);
        assert_eq!(indices[0], 0, "fully transparent pixel should be idx 0");
        assert_ne!(indices[1], 0, "opaque green pixel should not be idx 0");
        assert_ne!(indices[2], 0, "opaque blue pixel should not be idx 0");
        assert_ne!(indices[3], 0, "opaque gray pixel should not be idx 0");
    }

    #[test]
    fn semi_transparent_below_threshold_is_transparent() {
        let rgba: Vec<u8> = vec![255, 0, 0, 50, 0, 255, 0, 127];
        let (_, indices) = simple_quantize(&rgba, [255, 255, 255]);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 0);
    }

    #[test]
    fn semi_transparent_at_threshold_is_opaque() {
        let rgba: Vec<u8> = vec![255, 0, 0, 128, 0, 255, 0, 200];
        let (_, indices) = simple_quantize(&rgba, [255, 255, 255]);
        assert_ne!(indices[0], 0);
        assert_ne!(indices[1], 0);
    }

    #[test]
    fn frame_delay_min_one() {
        assert!((100.0_f32 / 200.0_f32).round().max(1.0) as u16 >= 1);
    }
}
