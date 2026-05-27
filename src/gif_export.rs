use std::io::Write;

use crate::error::Result;

pub fn export_gif(
    frames: &[image::RgbaImage],
    frame_rate: f32,
    output: &mut dyn Write,
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

        let (palette, indices) = simple_quantize(&pixels);

        let gif_frame = gif::Frame {
            width,
            height,
            buffer: std::borrow::Cow::Owned(indices),
            palette: Some(palette),
            transparent: Some(0),
            delay: frame_delay,
            ..Default::default()
        };

        encoder.write_frame(&gif_frame)?;
    }

    Ok(())
}

const TRANSPARENT_ALPHA_THRESHOLD: u8 = 128;

fn simple_quantize(rgba: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut palette = vec![0, 0, 0];
    let mut color_map: std::collections::HashMap<[u8; 3], u8> = std::collections::HashMap::new();
    color_map.insert([0, 0, 0], 0);
    let mut palette_colors: Vec<[u8; 3]> = vec![[0, 0, 0]];

    let mut indices = Vec::with_capacity(rgba.len() / 4);

    for chunk in rgba.chunks_exact(4) {
        let a = chunk[3];
        if a < TRANSPARENT_ALPHA_THRESHOLD {
            indices.push(0);
            continue;
        }

        let r = chunk[0] & 0xFC;
        let g = chunk[1] & 0xFC;
        let b = chunk[2] & 0xFC;
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
    let mut best_idx = 0u8;
    let mut best_dist = i32::MAX;
    for (i, p) in palette.iter().enumerate() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_pixels_get_index_zero() {
        let rgba: Vec<u8> = vec![
            255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 255, 128, 128, 128, 200,
        ];
        let (_, indices) = simple_quantize(&rgba);
        assert_eq!(indices[0], 0, "fully transparent pixel should be idx 0");
        assert_ne!(indices[1], 0, "opaque green pixel should not be idx 0");
        assert_ne!(indices[2], 0, "opaque blue pixel should not be idx 0");
        assert_ne!(indices[3], 0, "opaque gray pixel should not be idx 0");
    }

    #[test]
    fn semi_transparent_below_threshold_is_transparent() {
        let rgba: Vec<u8> = vec![255, 0, 0, 50, 0, 255, 0, 127];
        let (_, indices) = simple_quantize(&rgba);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 0);
    }

    #[test]
    fn semi_transparent_at_threshold_is_opaque() {
        let rgba: Vec<u8> = vec![255, 0, 0, 128, 0, 255, 0, 200];
        let (_, indices) = simple_quantize(&rgba);
        assert_ne!(indices[0], 0);
        assert_ne!(indices[1], 0);
    }

    #[test]
    fn frame_delay_min_one() {
        assert!((100.0_f32 / 200.0_f32).round().max(1.0) as u16 >= 1);
    }
}
