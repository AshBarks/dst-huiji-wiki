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
        return Err(crate::error::Error::MissingData(
            "frames to export".to_string(),
        ));
    }

    let max_width = frames.iter().map(|f| f.width()).max().unwrap_or(1);
    let max_height = frames.iter().map(|f| f.height()).max().unwrap_or(1);
    let mut writer = GifWriter::new(output, max_width as u16, max_height as u16, frame_rate, bg)?;
    for frame in frames {
        writer.write_frame(frame, 0, 0)?;
    }
    Ok(())
}

pub struct GifWriter<W: Write> {
    encoder: gif::Encoder<W>,
    quantize_ctx: QuantizeContext,
    width: u16,
    height: u16,
    frame_delay: u16,
    bg: [u8; 3],
    pixels: Vec<u8>,
}

impl<W: Write> GifWriter<W> {
    pub fn new(output: W, width: u16, height: u16, frame_rate: f32, bg: [u8; 3]) -> Result<Self> {
        let frame_delay = (100.0 / frame_rate).round().max(1.0) as u16;
        let mut encoder = gif::Encoder::new(output, width, height, &[])?;
        encoder.set_repeat(gif::Repeat::Infinite)?;
        Ok(Self {
            encoder,
            quantize_ctx: QuantizeContext::new(),
            width,
            height,
            frame_delay,
            bg,
            pixels: Vec::new(),
        })
    }

    pub fn write_frame(&mut self, frame: &image::RgbaImage, off_x: i64, off_y: i64) -> Result<()> {
        let max_width = self.width as usize;
        let max_height = self.height as usize;
        self.pixels.clear();
        let fw = frame.width() as usize;
        let fh = frame.height() as usize;
        let raw: &[u8] = frame.as_raw();
        let off_x = off_x.max(0) as usize;
        let off_y = off_y.max(0) as usize;

        for y in 0..max_height {
            if y >= off_y && y < off_y + fh {
                let fy = y - off_y;
                let x_off = off_x.min(max_width);
                let copy_w = fw.min(max_width - x_off);
                if x_off > 0 {
                    self.pixels.extend(std::iter::repeat_n(0, x_off * 4));
                }
                let row_start = fy * fw * 4;
                self.pixels
                    .extend_from_slice(&raw[row_start..row_start + copy_w * 4]);
                let remaining = max_width - x_off - copy_w;
                self.pixels.extend(std::iter::repeat_n(0, remaining * 4));
            } else {
                self.pixels.extend(std::iter::repeat_n(0, max_width * 4));
            }
        }

        let (palette, indices) = self.quantize_ctx.quantize(&self.pixels, self.bg);

        let gif_frame = gif::Frame {
            width: self.width,
            height: self.height,
            buffer: std::borrow::Cow::Owned(indices),
            palette: Some(palette),
            transparent: Some(0),
            delay: self.frame_delay,
            dispose: gif::DisposalMethod::Background,
            ..Default::default()
        };

        self.encoder.write_frame(&gif_frame)?;
        Ok(())
    }
}

const TRANSPARENT_ALPHA_THRESHOLD: u8 = 128;
const COLOR_TABLE_SIZE: usize = 64 * 64 * 64;

struct QuantizeContext {
    color_table: Box<[u16; COLOR_TABLE_SIZE]>,
}

impl QuantizeContext {
    fn new() -> Self {
        Self {
            color_table: Box::new([0xFFFFu16; COLOR_TABLE_SIZE]),
        }
    }

    fn quantize(&mut self, rgba: &[u8], bg: [u8; 3]) -> (Vec<u8>, Vec<u8>) {
        let sentinel = [0, 0, 2];
        let mut palette = vec![sentinel[0], sentinel[1], sentinel[2]];
        let mut palette_colors: Vec<[u8; 3]> = vec![sentinel];

        self.color_table.fill(0xFFFF);
        let sq = (sentinel[0] >> 2) as usize;
        let sg = (sentinel[1] >> 2) as usize;
        let sb = (sentinel[2] >> 2) as usize;
        self.color_table[sq * 64 * 64 + sg * 64 + sb] = 0;
        let mut palette_len = 1usize;

        let mut indices = Vec::with_capacity(rgba.len() / 4);

        for chunk in rgba.as_chunks::<4>().0 {
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

            let rq = (r >> 2) as usize;
            let gq = (g >> 2) as usize;
            let bq = (b >> 2) as usize;
            let table_idx = rq * 64 * 64 + gq * 64 + bq;

            let idx = match self.color_table[table_idx] {
                0xFFFF => {
                    let key = [r & 0xFC, g & 0xFC, b & 0xFC];
                    if palette_len < 256 {
                        let i = palette_len as u8;
                        palette.push(key[0]);
                        palette.push(key[1]);
                        palette.push(key[2]);
                        palette_colors.push(key);
                        self.color_table[table_idx] = i as u16;
                        palette_len += 1;
                        i
                    } else {
                        nearest_palette_index(&palette_colors, &key)
                    }
                }
                i => i as u8,
            };
            indices.push(idx);
        }

        while palette.len() < 256 * 3 {
            palette.push(0);
        }

        (palette, indices)
    }
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
        return Err(crate::error::Error::MissingData(
            "frames to export".to_string(),
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
        return Err(crate::error::Error::Other(
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
        return Err(crate::error::Error::Other(
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
        return Err(crate::error::Error::Other(
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
        let mut ctx = QuantizeContext::new();
        let (_, indices) = ctx.quantize(&rgba, [255, 255, 255]);
        assert_eq!(indices[0], 0, "fully transparent pixel should be idx 0");
        assert_ne!(indices[1], 0, "opaque green pixel should not be idx 0");
        assert_ne!(indices[2], 0, "opaque blue pixel should not be idx 0");
        assert_ne!(indices[3], 0, "opaque gray pixel should not be idx 0");
    }

    #[test]
    fn semi_transparent_below_threshold_is_transparent() {
        let rgba: Vec<u8> = vec![255, 0, 0, 50, 0, 255, 0, 127];
        let mut ctx = QuantizeContext::new();
        let (_, indices) = ctx.quantize(&rgba, [255, 255, 255]);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 0);
    }

    #[test]
    fn semi_transparent_at_threshold_is_opaque() {
        let rgba: Vec<u8> = vec![255, 0, 0, 128, 0, 255, 0, 200];
        let mut ctx = QuantizeContext::new();
        let (_, indices) = ctx.quantize(&rgba, [255, 255, 255]);
        assert_ne!(indices[0], 0);
        assert_ne!(indices[1], 0);
    }

    #[test]
    fn frame_delay_min_one() {
        assert!((100.0_f32 / 200.0_f32).round().max(1.0) as u16 >= 1);
    }
}
