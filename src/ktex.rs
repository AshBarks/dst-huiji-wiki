use crate::error::{Error, Result};
use crate::reader::Reader;
use crate::specs::{MAGIC_KTEX, PRE_CAVE_SPEC, PixelFormat, Platform, TextureType, detect_spec};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KtexHeader {
    pub platform: Platform,
    pub pixel_format: PixelFormat,
    pub texture_type: TextureType,
    pub mipmap_count: u32,
    pub flags: u32,
    pub fill: u32,
}

#[derive(Debug, Clone)]
pub struct KtexMipmap {
    pub width: u16,
    pub height: u16,
    pub data_size: u32,
    pub block_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Ktex {
    pub header: KtexHeader,
    pub mipmaps: Vec<KtexMipmap>,
    pub pre_multiply_alpha: bool,
}

fn decode_spec(spec_data: u32) -> Result<KtexHeader> {
    let spec = detect_spec(spec_data);
    let platform_val =
        (spec_data >> spec.offset_platform) & if spec == PRE_CAVE_SPEC { 7 } else { 15 };
    let pixel_format_val =
        (spec_data >> spec.offset_pixel_format) & if spec == PRE_CAVE_SPEC { 7 } else { 31 };
    let texture_type_val =
        (spec_data >> spec.offset_texture_type) & if spec == PRE_CAVE_SPEC { 7 } else { 15 };
    let mipmap_count =
        (spec_data >> spec.offset_mipmap_count) & if spec == PRE_CAVE_SPEC { 15 } else { 31 };
    let flags = (spec_data >> spec.offset_flags) & if spec == PRE_CAVE_SPEC { 1 } else { 3 };
    let fill = (spec_data >> spec.offset_fill)
        & if spec == PRE_CAVE_SPEC {
            0x3FFFF
        } else {
            0xFFF
        };

    Ok(KtexHeader {
        platform: Platform::try_from(platform_val)?,
        pixel_format: PixelFormat::try_from(pixel_format_val)?,
        texture_type: TextureType::try_from(texture_type_val)?,
        mipmap_count,
        flags,
        fill,
    })
}

pub fn parse_ktex(data: &[u8]) -> Result<Ktex> {
    let mut r = Reader::new(data);
    let magic = r.read_string(4)?;
    if magic != MAGIC_KTEX {
        return Err(Error::InvalidMagic {
            expected: MAGIC_KTEX.to_string(),
            actual: magic,
        });
    }
    let spec_data = r.read_le_u32()?;
    let header = decode_spec(spec_data)?;

    let mut mipmaps = Vec::with_capacity(header.mipmap_count as usize);
    for _ in 0..header.mipmap_count {
        let width = r.read_le_u16()?;
        let height = r.read_le_u16()?;
        let _pitch = r.read_le_u16()?;
        let data_size = r.read_le_u32()?;
        mipmaps.push(KtexMipmap {
            width,
            height,
            data_size,
            block_data: Vec::new(),
        });
    }

    for mipmap in &mut mipmaps {
        let block_data = r.read_bytes(mipmap.data_size as usize)?;
        mipmap.block_data = block_data.to_vec();
    }

    let pre_multiply_alpha = if r.remaining() == 1 {
        r.read_u8()? != 0
    } else {
        false
    };

    Ok(Ktex {
        header,
        mipmaps,
        pre_multiply_alpha,
    })
}

fn rgb565_to_rgba(c: u16) -> [u8; 4] {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}

fn decode_dxt1_color_block(block: &[u8], has_alpha: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let color0 = rgb565_to_rgba(c0);
    let color1 = rgb565_to_rgba(c1);

    let mut palette = [[0u8; 4]; 4];
    palette[0] = color0;
    palette[1] = color1;
    if c0 <= c1 {
        palette[2] = [
            ((color0[0] as u16 + color1[0] as u16) >> 1) as u8,
            ((color0[1] as u16 + color1[1] as u16) >> 1) as u8,
            ((color0[2] as u16 + color1[2] as u16) >> 1) as u8,
            255,
        ];
        palette[3] = [0, 0, 0, 0];
    } else {
        palette[2] = [
            ((2u16 * color0[0] as u16 + color1[0] as u16) / 3) as u8,
            ((2u16 * color0[1] as u16 + color1[1] as u16) / 3) as u8,
            ((2u16 * color0[2] as u16 + color1[2] as u16) / 3) as u8,
            255,
        ];
        palette[3] = [
            ((color0[0] as u16 + 2u16 * color1[0] as u16) / 3) as u8,
            ((color0[1] as u16 + 2u16 * color1[1] as u16) / 3) as u8,
            ((color0[2] as u16 + 2u16 * color1[2] as u16) / 3) as u8,
            255,
        ];
    }

    let mut pixels = [[0u8; 4]; 16];
    for i in 0..4 {
        let indices = block[4 + i];
        for j in 0..4 {
            let idx = ((indices >> (j * 2)) & 3) as usize;
            let alpha = if has_alpha && idx == 3 {
                0
            } else {
                palette[idx][3]
            };
            pixels[i * 4 + j] = [palette[idx][0], palette[idx][1], palette[idx][2], alpha];
        }
    }
    pixels
}

fn decode_dxt1(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let blocks_x = w.div_ceil(4);
    let blocks_y = h.div_ceil(4);
    let mut pixels = vec![0u8; w * h * 4];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_idx = (by * blocks_x + bx) * 8;
            if block_idx + 8 > data.len() {
                break;
            }
            let block = &data[block_idx..block_idx + 8];
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let has_alpha = c0 <= c1;
            let decoded = decode_dxt1_color_block(block, has_alpha);

            let full_block = (bx + 1) * 4 <= w && (by + 1) * 4 <= h;
            if full_block {
                let base_x = bx * 4;
                let base_y = by * 4;
                for dy in 0..4usize {
                    let dst = ((base_y + dy) * w + base_x) * 4;
                    let row = dy * 4;
                    pixels[dst..dst + 4].copy_from_slice(&decoded[row]);
                    pixels[dst + 4..dst + 8].copy_from_slice(&decoded[row + 1]);
                    pixels[dst + 8..dst + 12].copy_from_slice(&decoded[row + 2]);
                    pixels[dst + 12..dst + 16].copy_from_slice(&decoded[row + 3]);
                }
            } else {
                for dy in 0..4usize {
                    for dx in 0..4usize {
                        let px = bx * 4 + dx;
                        let py = by * 4 + dy;
                        if px < w && py < h {
                            let dst = (py * w + px) * 4;
                            pixels[dst..dst + 4].copy_from_slice(&decoded[dy * 4 + dx]);
                        }
                    }
                }
            }
        }
    }
    pixels
}

fn decode_dxt_with_alpha(
    data: &[u8],
    width: u16,
    height: u16,
    extract_alpha: impl Fn(&[u8]) -> [u8; 16],
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let blocks_x = w.div_ceil(4);
    let blocks_y = h.div_ceil(4);
    let mut pixels = vec![0u8; w * h * 4];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_idx = (by * blocks_x + bx) * 16;
            if block_idx + 16 > data.len() {
                break;
            }
            let block = &data[block_idx..block_idx + 16];

            let alpha = extract_alpha(block);
            let color_block = &block[8..16];
            let decoded = decode_dxt1_color_block(color_block, false);

            let full_block = (bx + 1) * 4 <= w && (by + 1) * 4 <= h;
            if full_block {
                let base_x = bx * 4;
                let base_y = by * 4;
                for dy in 0..4usize {
                    let dst = ((base_y + dy) * w + base_x) * 4;
                    let row = dy * 4;
                    pixels[dst..dst + 4].copy_from_slice(&decoded[row]);
                    pixels[dst + 4..dst + 8].copy_from_slice(&decoded[row + 1]);
                    pixels[dst + 8..dst + 12].copy_from_slice(&decoded[row + 2]);
                    pixels[dst + 12..dst + 16].copy_from_slice(&decoded[row + 3]);
                    let a_off = dy * 4;
                    pixels[dst + 3] = alpha[a_off];
                    pixels[dst + 7] = alpha[a_off + 1];
                    pixels[dst + 11] = alpha[a_off + 2];
                    pixels[dst + 15] = alpha[a_off + 3];
                }
            } else {
                for dy in 0..4usize {
                    for dx in 0..4usize {
                        let px = bx * 4 + dx;
                        let py = by * 4 + dy;
                        if px < w && py < h {
                            let dst = (py * w + px) * 4;
                            let c = decoded[dy * 4 + dx];
                            pixels[dst] = c[0];
                            pixels[dst + 1] = c[1];
                            pixels[dst + 2] = c[2];
                            pixels[dst + 3] = alpha[dy * 4 + dx];
                        }
                    }
                }
            }
        }
    }
    pixels
}

fn decode_dxt3(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    decode_dxt_with_alpha(data, width, height, |block| {
        let mut alpha = [0u8; 16];
        for i in 0..4 {
            let a0 = block[i * 2];
            let a1 = block[i * 2 + 1];
            alpha[i * 4] = (a0 & 0x0F) * 17;
            alpha[i * 4 + 1] = ((a0 >> 4) & 0x0F) * 17;
            alpha[i * 4 + 2] = (a1 & 0x0F) * 17;
            alpha[i * 4 + 3] = ((a1 >> 4) & 0x0F) * 17;
        }
        alpha
    })
}

fn decode_dxt5(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    decode_dxt_with_alpha(data, width, height, |block| {
        let a0 = block[0];
        let a1 = block[1];
        let mut alpha_table = [0u16; 8];
        if a0 > a1 {
            alpha_table[0] = a0 as u16;
            alpha_table[1] = a1 as u16;
            alpha_table[2] = (6 * a0 as u16 + a1 as u16) / 7;
            alpha_table[3] = (5 * a0 as u16 + 2 * a1 as u16) / 7;
            alpha_table[4] = (4 * a0 as u16 + 3 * a1 as u16) / 7;
            alpha_table[5] = (3 * a0 as u16 + 4 * a1 as u16) / 7;
            alpha_table[6] = (2 * a0 as u16 + 5 * a1 as u16) / 7;
            alpha_table[7] = (a0 as u16 + 6 * a1 as u16) / 7;
        } else {
            alpha_table[0] = a0 as u16;
            alpha_table[1] = a1 as u16;
            alpha_table[2] = (4 * a0 as u16 + a1 as u16) / 5;
            alpha_table[3] = (3 * a0 as u16 + 2 * a1 as u16) / 5;
            alpha_table[4] = (2 * a0 as u16 + 3 * a1 as u16) / 5;
            alpha_table[5] = (a0 as u16 + 4 * a1 as u16) / 5;
            alpha_table[6] = 0;
            alpha_table[7] = 255;
        }

        let alpha_indices = (block[2] as u64)
            | ((block[3] as u64) << 8)
            | ((block[4] as u64) << 16)
            | ((block[5] as u64) << 24)
            | ((block[6] as u64) << 32)
            | ((block[7] as u64) << 40);

        let mut alpha = [0u8; 16];
        for (i, pa) in alpha.iter_mut().enumerate() {
            let idx = ((alpha_indices >> (i * 3)) & 7) as usize;
            *pa = alpha_table[idx] as u8;
        }
        alpha
    })
}

fn decode_rgba(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let total = w * h * 4;
    let mut pixels = vec![0u8; total];
    let copy_len = total.min(data.len());
    pixels[..copy_len].copy_from_slice(&data[..copy_len]);
    pixels
}

fn decode_rgb(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut pixels = vec![0u8; w * h * 4];
    for i in 0..(w * h) {
        let src = i * 3;
        if src + 3 > data.len() {
            break;
        }
        let dst = i * 4;
        pixels[dst] = data[src];
        pixels[dst + 1] = data[src + 1];
        pixels[dst + 2] = data[src + 2];
        pixels[dst + 3] = 255;
    }
    pixels
}

fn un_premultiply_alpha(pixels: &mut [u8], width: usize, height: usize) {
    let stride = width * 4;
    for row in 0..height {
        for col in 0..width {
            let idx = row * stride + col * 4;
            let a = pixels[idx + 3] as u32;
            if a == 0 || a == 255 {
                continue;
            }
            let r = pixels[idx] as u32;
            let g = pixels[idx + 1] as u32;
            let b = pixels[idx + 2] as u32;
            pixels[idx] = (r * 255).div_ceil(a).min(255) as u8;
            pixels[idx + 1] = (g * 255).div_ceil(a).min(255) as u8;
            pixels[idx + 2] = (b * 255).div_ceil(a).min(255) as u8;
        }
    }
}

fn flip_y(pixels: &mut [u8], width: usize, height: usize) {
    let stride = width * 4;
    let mut tmp = vec![0u8; stride];
    for row in 0..height / 2 {
        let top = row * stride;
        let bottom = (height - 1 - row) * stride;
        tmp[..stride].copy_from_slice(&pixels[top..top + stride]);
        pixels.copy_within(bottom..bottom + stride, top);
        pixels[bottom..bottom + stride].copy_from_slice(&tmp[..stride]);
    }
}

impl Ktex {
    pub fn to_image_rgba(&self) -> Result<image::RgbaImage> {
        if self.mipmaps.is_empty() {
            return Err(Error::MissingData("mipmaps".to_string()));
        }
        let mipmap = &self.mipmaps[0];
        let width = mipmap.width;
        let height = mipmap.height;

        let mut pixels = match self.header.pixel_format {
            PixelFormat::DXT1 => decode_dxt1(&mipmap.block_data, width, height),
            PixelFormat::DXT3 => decode_dxt3(&mipmap.block_data, width, height),
            PixelFormat::DXT5 => decode_dxt5(&mipmap.block_data, width, height),
            PixelFormat::RGBA => decode_rgba(&mipmap.block_data, width, height),
            PixelFormat::RGB => decode_rgb(&mipmap.block_data, width, height),
            _ => return Err(Error::UnsupportedPixelFormat(self.header.pixel_format)),
        };

        let has_alpha = self.header.pixel_format != PixelFormat::RGB;
        if has_alpha && self.pre_multiply_alpha {
            un_premultiply_alpha(&mut pixels, width as usize, height as usize);
        }

        flip_y(&mut pixels, width as usize, height as usize);

        image::RgbaImage::from_raw(width as u32, height as u32, pixels)
            .ok_or_else(|| Error::Other("decoded pixel data size mismatch".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_roundtrip() {
        let r = 0x1Fu8;
        let g = 0x3Fu8;
        let b = 0x1Fu8;
        let c: u16 = ((r as u16) << 11) | ((g as u16) << 5) | b as u16;
        let rgba = rgb565_to_rgba(c);
        assert_eq!(rgba[0], 255);
        assert_eq!(rgba[1], 255);
        assert_eq!(rgba[2], 255);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn dxt1_opaque_block() {
        let c0: u16 = 0xFFFF;
        let c1: u16 = 0x0000;
        let mut block = vec![0u8; 8];
        block[0..2].copy_from_slice(&c0.to_le_bytes());
        block[2..4].copy_from_slice(&c1.to_le_bytes());
        block[4..8].copy_from_slice(&[0xAA, 0xAA, 0xAA, 0xAA]);
        let pixels = decode_dxt1(&block, 4, 4);
        assert_eq!(pixels.len(), 64);
    }

    #[test]
    fn dxt1_1x1() {
        let c0: u16 = 0xF800;
        let c1: u16 = 0x07E0;
        let mut block = vec![0u8; 8];
        block[0..2].copy_from_slice(&c0.to_le_bytes());
        block[2..4].copy_from_slice(&c1.to_le_bytes());
        let pixels = decode_dxt1(&block, 1, 1);
        assert_eq!(pixels.len(), 4);
        let rgba = rgb565_to_rgba(c0);
        assert_eq!(pixels[0], rgba[0]);
        assert_eq!(pixels[1], rgba[1]);
        assert_eq!(pixels[2], rgba[2]);
        assert_eq!(pixels[3], rgba[3]);
    }

    #[test]
    fn dxt3_block() {
        let mut block = vec![0u8; 16];
        let c0: u16 = 0xFFFF;
        let c1: u16 = 0x0000;
        block[8..10].copy_from_slice(&c0.to_le_bytes());
        block[10..12].copy_from_slice(&c1.to_le_bytes());
        let pixels = decode_dxt3(&block, 4, 4);
        assert_eq!(pixels.len(), 64);
    }

    #[test]
    fn dxt5_block() {
        let mut block = vec![0u8; 16];
        block[0] = 255;
        block[1] = 0;
        let c0: u16 = 0xFFFF;
        let c1: u16 = 0x0000;
        block[8..10].copy_from_slice(&c0.to_le_bytes());
        block[10..12].copy_from_slice(&c1.to_le_bytes());
        let pixels = decode_dxt5(&block, 4, 4);
        assert_eq!(pixels.len(), 64);
        assert_eq!(pixels[3], 255);
    }

    #[test]
    fn rgba_decode() {
        let data: Vec<u8> = vec![255, 0, 0, 128, 0, 255, 0, 64];
        let pixels = decode_rgba(&data, 2, 1);
        assert_eq!(pixels.len(), 8);
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[3], 128);
        assert_eq!(pixels[4], 0);
        assert_eq!(pixels[5], 255);
        assert_eq!(pixels[7], 64);
    }

    #[test]
    fn rgb_decode() {
        let data: Vec<u8> = vec![255, 0, 0, 0, 255, 0];
        let pixels = decode_rgb(&data, 2, 1);
        assert_eq!(pixels.len(), 8);
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[3], 255);
        assert_eq!(pixels[4], 0);
        assert_eq!(pixels[5], 255);
        assert_eq!(pixels[7], 255);
    }

    #[test]
    fn flip_y_2x2() {
        let mut pixels: Vec<u8> = vec![1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4];
        flip_y(&mut pixels, 2, 2);
        assert_eq!(pixels[0..4], [3, 3, 3, 3]);
        assert_eq!(pixels[4..8], [4, 4, 4, 4]);
        assert_eq!(pixels[8..12], [1, 1, 1, 1]);
        assert_eq!(pixels[12..16], [2, 2, 2, 2]);
    }

    #[test]
    fn un_premultiply_identity() {
        let mut pixels: Vec<u8> = vec![255, 0, 0, 255, 0, 128, 0, 255];
        un_premultiply_alpha(&mut pixels, 2, 1);
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[4], 0);
        assert_eq!(pixels[5], 128);
    }

    #[test]
    fn un_premultiply_with_alpha() {
        let mut pixels: Vec<u8> = vec![128, 0, 0, 128];
        un_premultiply_alpha(&mut pixels, 1, 1);
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[3], 128);
    }

    #[test]
    fn parse_ktex_real_file() {
        let zip_data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name().ends_with(".tex") {
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut buf).unwrap();
                let ktex = parse_ktex(&buf).unwrap();
                assert!(!ktex.mipmaps.is_empty());
                let m0 = &ktex.mipmaps[0];
                assert!(m0.width > 0);
                assert!(m0.height > 0);
                assert!(!m0.block_data.is_empty());
                let img = ktex.to_image_rgba().unwrap();
                assert_eq!(img.width(), m0.width as u32);
                assert_eq!(img.height(), m0.height as u32);
                assert_eq!(
                    img.as_raw().len(),
                    m0.width as usize * m0.height as usize * 4
                );
                return;
            }
        }
        panic!("no .tex file found in archive");
    }
}
