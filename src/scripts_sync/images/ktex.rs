//! KTEX 容器解析与解码 —— ktech（解码方向）的内置 Rust 等价实现。
//!
//! 格式事实参考 nsimplex/ktools（GPL-2.0）的 specs.cpp/ktex.cpp 与 DST modding
//! 社区文档；实现为独立编写，块解码使用 MIT 许可的 [`texpresso`]
//! （libsquish 的纯 Rust 移植），不移植其代码。
//!
//! 容器布局：
//!
//! ```text
//! "KTEX" (4B) | u32 位域 | mipmap 元数据 × N | mipmap 数据块 × N
//! 位域（LSB-first）: platform(4b) compression(5b) texture_type(4b)
//!                    mipmap_count(5b) flags(2b) fill(12b, 恒 0xFFF)
//! mipmap 元数据（每条 10B）: width(u16) height(u16) pitch(u16) datasz(u32)
//! ```
//!
//! 兼容行为（与 ktech 一致）：
//! - 端序推断：LE 优先，fill 低位字节 ≠ 0xFF 时尝试 BE；
//! - pre-caves 旧格式（bits 14..31 全 1）：platform(3b) compression(3b)
//!   texture_type(3b) mipmap_count(4b) flags(1b) fill(18b) 重排为现代布局；
//! - 仅解码 mipmap 0（ktech 默认行为），解码后垂直翻转（UV 原点在左下），
//!   再对 a∈(0,1) 的像素做 alpha 反预乘（`rgb' = rgb / a`）。
//!
//! 反预乘的舍入与 ktech（ImageMagick Q16 管线）逐位一致的推导：
//! depth-8 读入 Q16（×257）→ `multiplyQuantum` 截断 → depth-8 写出
//! （/257 + 0.5 舍入），257a/65535 = a/255 精确成立，净效应为 8bit 域的
//! `round(255·x/a)`，即整数式 `(255·x + a/2) / a`（奇数 a 时 255x mod a ≠
//! a/2，与真四舍五入无差异，见单元测试）。

use crate::error::{Error, Result};
use image::DynamicImage;

/// 当前解码器版本；写入 manifest，变更时触发全量重处理。
pub const DECODER_VERSION: &str = "ktex-rs/1";

const MAGIC: &[u8; 4] = b"KTEX";

/// 每条 mipmap 元数据的磁盘字节数（u16×3 + u32 = 10）。
const MIPMAP_META_SIZE: usize = 10;

/// 压缩格式（specs.cpp `compression_values`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Dxt1,
    Dxt3,
    Dxt5,
    Rgba,
    Rgb,
}

impl Compression {
    fn from_bits(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Dxt1),
            1 => Some(Self::Dxt3),
            2 => Some(Self::Dxt5),
            4 => Some(Self::Rgba),
            5 => Some(Self::Rgb),
            _ => None,
        }
    }

    fn to_bits(self) -> u32 {
        match self {
            Self::Dxt1 => 0,
            Self::Dxt3 => 1,
            Self::Dxt5 => 2,
            Self::Rgba => 4,
            Self::Rgb => 5,
        }
    }
}

/// KTEX 头字段（bits 值为原始位域值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KtexHeader {
    pub platform: u32,
    pub compression: Compression,
    pub texture_type: u32,
    pub mipmap_count: u32,
    pub flags: u32,
}

/// mipmap 元数据（磁盘上 10 字节：width/height/pitch 为 u16，datasz 为 u32）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MipmapMeta {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub datasz: u32,
}

/// 解码选项。
#[derive(Debug, Clone, Copy)]
pub struct DecodeOptions {
    /// 对 a∈(0,1) 的像素做反预乘（ktech 默认行为）。
    pub demultiply: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self { demultiply: true }
    }
}

/// 解析头并返回（头, mipmap 元数据表, mipmap 数据区起始偏移）。
///
/// 端序推断与 ktech 一致（LE 优先，fill 低位字节探测），且**同时作用于
/// 头位域与 mipmap 元数据**（ktech 的 `io.read_integer` 两者共用同一 io）。
pub fn parse(bytes: &[u8]) -> Result<(KtexHeader, Vec<MipmapMeta>, usize)> {
    if bytes.len() < 8 || &bytes[..4] != MAGIC {
        return Err(Error::ParseError("非 KTEX 文件（magic 不符）".into()));
    }
    let le_word = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    // 端序推断：post-caves fill 位于 bits 20..31，其低位字节 = bits 20..27。
    let fill_low_byte = |w: u32| (w >> 20) & 0xFF;
    let (word, big_endian) = if fill_low_byte(le_word) == 0xFF {
        (le_word, false)
    } else {
        let be_word = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if fill_low_byte(be_word) == 0xFF {
            (be_word, true)
        } else {
            (le_word, false) // 无法判定时按 LE（与 ktech 一致）
        }
    };
    let read_u32 = |off: usize| {
        let b: [u8; 4] = bytes[off..off + 4].try_into().expect("4B");
        if big_endian {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        }
    };
    let read_u16 = |off: usize| {
        let b: [u8; 2] = bytes[off..off + 2].try_into().expect("2B");
        if big_endian {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        }
    };

    // pre-caves 旧格式：bits 14..31 全 1。
    let (platform, compression_bits, texture_type, mipmap_count, flags) =
        if (word >> 14) == (1 << 18) - 1 {
            (
                word & 0x7,
                (word >> 3) & 0x7,
                (word >> 6) & 0x7,
                (word >> 9) & 0xF,
                (word >> 13) & 0x1,
            )
        } else {
            (
                word & 0xF,
                (word >> 4) & 0x1F,
                (word >> 9) & 0xF,
                (word >> 13) & 0x1F,
                (word >> 18) & 0x3,
            )
        };
    let compression = Compression::from_bits(compression_bits).ok_or_else(|| {
        Error::ParseError(format!("不支持的 KTEX 压缩格式值: {compression_bits}"))
    })?;
    if mipmap_count == 0 {
        return Err(Error::ParseError("KTEX 无 mipmap（mipmap_count=0）".into()));
    }
    let header = KtexHeader {
        platform,
        compression,
        texture_type,
        mipmap_count,
        flags,
    };

    let meta_off = 8usize;
    let meta_len = mipmap_count as usize * MIPMAP_META_SIZE;
    if bytes.len() < meta_off + meta_len {
        return Err(Error::ParseError(
            "KTEX 头被截断（mipmap 元数据不足）".into(),
        ));
    }
    let mut metas = Vec::with_capacity(mipmap_count as usize);
    for i in 0..mipmap_count as usize {
        let base = meta_off + i * MIPMAP_META_SIZE;
        metas.push(MipmapMeta {
            width: u32::from(read_u16(base)),
            height: u32::from(read_u16(base + 2)),
            pitch: u32::from(read_u16(base + 4)),
            datasz: read_u32(base + 6),
        });
    }
    Ok((header, metas, meta_off + meta_len))
}

/// 解码 mipmap 0 为 RGBA8 图像（已翻转、可选反预乘）。
pub fn decode_mipmap0(bytes: &[u8], opts: DecodeOptions) -> Result<DynamicImage> {
    let (header, metas, data_off) = parse(bytes)?;
    let m = metas[0];
    if m.width == 0 || m.height == 0 {
        return Err(Error::ParseError(format!(
            "KTEX mipmap 0 尺寸非法: {}x{}",
            m.width, m.height
        )));
    }
    let end = data_off
        .checked_add(m.datasz as usize)
        .ok_or_else(|| Error::ParseError("KTEX datasz 溢出".into()))?;
    if bytes.len() < end {
        return Err(Error::ParseError(format!(
            "KTEX 数据被截断: 需 {} 字节, 实有 {}",
            end,
            bytes.len()
        )));
    }
    let data = &bytes[data_off..end];
    let (w, h) = (m.width as usize, m.height as usize);

    let rgba: Vec<u8> = match header.compression {
        Compression::Dxt1 => block_decode(texpresso::Format::Bc1, data, w, h)?,
        Compression::Dxt3 => block_decode(texpresso::Format::Bc2, data, w, h)?,
        Compression::Dxt5 => block_decode(texpresso::Format::Bc3, data, w, h)?,
        Compression::Rgba | Compression::Rgb => raw_rows(data, &m, header.compression)?,
    };

    let mut img = image::RgbaImage::from_raw(m.width, m.height, rgba)
        .ok_or_else(|| Error::ParseError("解码像素数与尺寸不符".into()))?;
    // UV 原点在左下 → 垂直翻转（ktech flip_image 默认 true）。
    image::imageops::flip_vertical_in_place(&mut img);
    if opts.demultiply {
        for px in img.pixels_mut() {
            demultiply_pixel(px);
        }
    }
    Ok(DynamicImage::ImageRgba8(img))
}

/// DXT 块解码（texpresso 输出 RGBA8；数据不足提前拦截）。
fn block_decode(format: texpresso::Format, data: &[u8], w: usize, h: usize) -> Result<Vec<u8>> {
    let expected = format.compressed_size(w, h);
    if data.len() < expected {
        return Err(Error::ParseError(format!(
            "KTEX mipmap 数据不足: 需 {expected}, 实有 {}",
            data.len()
        )));
    }
    let mut out = vec![0u8; 4 * w * h];
    format.decompress(data, w, h, &mut out);
    Ok(out)
}

/// 未压缩 RGB/RGBA：按 pitch 行距取出连续 RGBA8。
fn raw_rows(data: &[u8], m: &MipmapMeta, compression: Compression) -> Result<Vec<u8>> {
    let (w, h) = (m.width as usize, m.height as usize);
    let (bpp, channels) = match compression {
        Compression::Rgba => (4usize, 4usize),
        Compression::Rgb => (3usize, 3usize),
        _ => unreachable!("raw_rows 仅处理未压缩格式"),
    };
    let pitch = m.pitch as usize;
    if pitch < bpp * w {
        return Err(Error::ParseError(format!(
            "KTEX pitch ({pitch}) 小于行宽 ({}B)",
            bpp * w
        )));
    }
    if data.len() < pitch * (h - 1) + bpp * w {
        return Err(Error::ParseError(format!(
            "KTEX 未压缩数据不足: 需 {} 字节, 实有 {}",
            pitch * (h - 1) + bpp * w,
            data.len()
        )));
    }
    let mut out = Vec::with_capacity(4 * w * h);
    for row in 0..h {
        let line = &data[row * pitch..];
        if channels == 4 {
            out.extend_from_slice(&line[..4 * w]);
        } else {
            let (pixels, _) = line[..3 * w].as_chunks::<3>();
            for px in pixels {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
        }
    }
    Ok(out)
}

/// 8bit 域反预乘：a∈(0,1) 时 `rgb' = round(255·rgb/a)`（IM Q16 净效应，见模块文档）。
fn demultiply_pixel(px: &mut image::Rgba<u8>) {
    let a = u32::from(px.0[3]);
    if a == 0 || a == 255 {
        return;
    }
    for c in &mut px.0[..3] {
        let v = (u32::from(*c) * 255 + a / 2) / a;
        *c = v.min(255) as u8;
    }
}

/// 构造 KTEX 容器（测试与 fixture 生成用）。
pub fn build_tex(header: KtexHeader, metas: &[MipmapMeta], data_blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    let word = header.platform
        | (header.compression.to_bits() << 4)
        | (header.texture_type << 9)
        | (header.mipmap_count << 13)
        | (header.flags << 18)
        | (0xFFF << 20);
    out.extend_from_slice(&word.to_le_bytes());
    for m in metas {
        out.extend_from_slice(&(m.width as u16).to_le_bytes());
        out.extend_from_slice(&(m.height as u16).to_le_bytes());
        out.extend_from_slice(&(m.pitch as u16).to_le_bytes());
        out.extend_from_slice(&m.datasz.to_le_bytes());
    }
    for block in data_blocks {
        out.extend_from_slice(block);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView as _;
    use image::Rgba;

    fn header(compression: Compression, mipmaps: u32) -> KtexHeader {
        KtexHeader {
            platform: 12, // PC
            compression,
            texture_type: 1, // 2D
            mipmap_count: mipmaps,
            flags: 0,
        }
    }

    /// 纯色 RGBA 图（565 可精确表示的颜色 → DXT 块解码后逐位还原）。
    fn solid_image(w: u32, h: u32, color: [u8; 4]) -> image::RgbaImage {
        image::RgbaImage::from_fn(w, h, |_, _| Rgba(color))
    }

    fn dxt5_tex(img: &image::RgbaImage) -> Vec<u8> {
        let (w, h) = img.dimensions();
        let raw = img.as_raw().clone();
        let mut comp = vec![0u8; texpresso::Format::Bc3.compressed_size(w as usize, h as usize)];
        texpresso::Format::Bc3.compress(
            &raw,
            w as usize,
            h as usize,
            texpresso::Params::default(),
            &mut comp,
        );
        let pitch = 16 * (w as usize).div_ceil(4);
        build_tex(
            header(Compression::Dxt5, 1),
            &[MipmapMeta {
                width: w,
                height: h,
                pitch: pitch as u32,
                datasz: comp.len() as u32,
            }],
            &[comp],
        )
    }

    fn rgb_tex(img: &image::RgbaImage) -> Vec<u8> {
        let (w, h) = img.dimensions();
        let mut data = Vec::with_capacity(3 * w as usize * h as usize);
        for px in img.pixels() {
            data.extend_from_slice(&px.0[..3]);
        }
        build_tex(
            header(Compression::Rgb, 1),
            &[MipmapMeta {
                width: w,
                height: h,
                pitch: 3 * w,
                datasz: data.len() as u32,
            }],
            &[data],
        )
    }

    #[test]
    fn test_parse_header_fields() {
        let tex = dxt5_tex(&solid_image(4, 4, [255, 0, 0, 255]));
        let (h, metas, off) = parse(&tex).unwrap();
        assert_eq!(h.platform, 12);
        assert_eq!(h.compression, Compression::Dxt5);
        assert_eq!(h.texture_type, 1);
        assert_eq!(h.mipmap_count, 1);
        assert_eq!(h.flags, 0);
        assert_eq!(metas[0].width, 4);
        assert_eq!(metas[0].height, 4);
        assert_eq!(off, 8 + 10);
    }

    #[test]
    fn test_dxt5_roundtrip_solid_color() {
        // 纯红：565 端点可精确表示 → 解码逐位还原（翻转+反预乘不影响不透明像素）。
        let tex = dxt5_tex(&solid_image(8, 8, [255, 0, 0, 255]));
        let img = decode_mipmap0(&tex, DecodeOptions::default()).unwrap();
        assert_eq!(img.dimensions(), (8, 8));
        for px in img.to_rgba8().pixels() {
            assert_eq!(px, &Rgba([255, 0, 0, 255]));
        }
    }

    #[test]
    fn test_rgb_roundtrip_and_alpha_fill() {
        let img = solid_image(3, 2, [10, 20, 30, 255]);
        let tex = rgb_tex(&img);
        let decoded = decode_mipmap0(&tex, DecodeOptions::default()).unwrap();
        for px in decoded.to_rgba8().pixels() {
            assert_eq!(px, &Rgba([10, 20, 30, 255]));
        }
    }

    #[test]
    fn test_demultiply_math() {
        // (255, 128, 0, 128)：a=128 → rgb' = round(255*rgb/128)
        let mut img = solid_image(1, 1, [255, 128, 0, 128]);
        let px = img.pixels_mut().next().unwrap();
        demultiply_pixel(px);
        assert_eq!(px.0[..3], [255, 255, 0]); // 255→255(cap), 128*255/128=255
                                              // IM Q16 净效应 = round(255x/a)：抽验整数式
        for a in [1u8, 3, 7, 64, 127, 254] {
            for x in [0u8, 1, 17, 128, 254, 255] {
                let expect = ((u32::from(x) * 255 + u32::from(a) / 2) / u32::from(a)).min(255);
                let mut px = image::Rgba([x, 0, 0, a]);
                demultiply_pixel(&mut px);
                assert_eq!(px.0[0], expect as u8, "x={x} a={a}");
                // 真四舍五入对照（f64）
                let float = ((f64::from(x) * 255.0 / f64::from(a)) + 0.5).floor();
                assert_eq!(f64::from(px.0[0]), float.min(255.0), "x={x} a={a}");
            }
        }
        // a=0/255 不动
        let mut px = image::Rgba([10, 20, 30, 255]);
        demultiply_pixel(&mut px);
        assert_eq!(px.0, [10, 20, 30, 255]);
        let mut px = image::Rgba([10, 20, 30, 0]);
        demultiply_pixel(&mut px);
        assert_eq!(px.0, [10, 20, 30, 0]);
    }

    #[test]
    fn test_flip_vertical() {
        // 上行红、下行蓝；UV 原点在左下 → 解码后翻转，蓝应在第 0 行。
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 255, 255]));
        let tex = rgb_tex(&img);
        let decoded = decode_mipmap0(&tex, DecodeOptions::default()).unwrap();
        assert_eq!(decoded.get_pixel(0, 0), Rgba([0, 0, 255, 255]));
        assert_eq!(decoded.get_pixel(1, 1), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_two_mipmaps_uses_first() {
        // mipmap0 = 红图, mipmap1 = 蓝图；只解码 mipmap 0。
        let mut red = image::RgbaImage::new(4, 4);
        for px in red.pixels_mut() {
            *px = Rgba([255, 0, 0, 255]);
        }
        let mut blue = image::RgbaImage::new(2, 2);
        for px in blue.pixels_mut() {
            *px = Rgba([0, 0, 255, 255]);
        }
        let compress = |img: &image::RgbaImage| {
            let raw = img.as_raw().clone();
            let mut comp = vec![
                0u8;
                texpresso::Format::Bc3
                    .compressed_size(img.width() as usize, img.height() as usize)
            ];
            texpresso::Format::Bc3.compress(
                &raw,
                img.width() as usize,
                img.height() as usize,
                texpresso::Params::default(),
                &mut comp,
            );
            comp
        };
        let (c0, c1) = (compress(&red), compress(&blue));
        let tex = build_tex(
            header(Compression::Dxt5, 2),
            &[
                MipmapMeta {
                    width: 4,
                    height: 4,
                    pitch: 16,
                    datasz: c0.len() as u32,
                },
                MipmapMeta {
                    width: 2,
                    height: 2,
                    pitch: 16,
                    datasz: c1.len() as u32,
                },
            ],
            &[c0, c1],
        );
        let decoded = decode_mipmap0(&tex, DecodeOptions::default()).unwrap();
        assert_eq!(decoded.dimensions(), (4, 4));
        assert_eq!(decoded.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_big_endian_header() {
        // 同一字段用 BE 编码（含 mipmap 元数据，u16 字段也 BE）→ 还原相同字段。
        let mut tex = Vec::new();
        tex.extend_from_slice(MAGIC);
        let word = 12u32 | (2 << 4) | (1 << 9) | (1 << 13) | (0xFFF << 20);
        tex.extend_from_slice(&word.to_be_bytes());
        tex.extend_from_slice(&4u16.to_be_bytes()); // width
        tex.extend_from_slice(&4u16.to_be_bytes()); // height
        tex.extend_from_slice(&16u16.to_be_bytes()); // pitch
        tex.extend_from_slice(&16u32.to_be_bytes()); // datasz
        tex.extend_from_slice(&[0u8; 16]);
        let (h, metas, _) = parse(&tex).unwrap();
        assert_eq!(h.platform, 12);
        assert_eq!(h.compression, Compression::Dxt5);
        assert_eq!(h.mipmap_count, 1);
        assert_eq!(metas[0].width, 4);
        assert_eq!(metas[0].datasz, 16);
    }

    #[test]
    fn test_precaves_header() {
        // pre-caves: platform(3b) compression(3b) texture_type(3b) mips(4b) flags(1b) fill(18b 全1)
        let word: u32 = (2 << 3) | (1 << 6) | (1 << 9) | (((1 << 18) - 1) << 14);
        let mut tex = Vec::new();
        tex.extend_from_slice(MAGIC);
        tex.extend_from_slice(&word.to_le_bytes());
        tex.extend_from_slice(&4u16.to_le_bytes());
        tex.extend_from_slice(&4u16.to_le_bytes());
        tex.extend_from_slice(&48u16.to_le_bytes());
        tex.extend_from_slice(&48u32.to_le_bytes());
        tex.extend_from_slice(&[0u8; 48]);
        let (h, _, _) = parse(&tex).unwrap();
        assert_eq!(h.compression, Compression::Dxt5);
        assert_eq!(h.texture_type, 1);
        assert_eq!(h.mipmap_count, 1);
    }

    #[test]
    fn test_errors() {
        assert!(parse(b"NOPE").is_err());
        assert!(parse(b"KTEX").is_err()); // 无头
                                          // 未知压缩值
        let mut tex = Vec::new();
        tex.extend_from_slice(MAGIC);
        let word = 12u32 | (7 << 4) | (1 << 9) | (1 << 13) | (0xFFF << 20);
        tex.extend_from_slice(&word.to_le_bytes());
        assert!(parse(&tex).is_err());
        // mipmap_count = 0
        let mut tex = Vec::new();
        tex.extend_from_slice(MAGIC);
        let word = 12u32 | (2 << 4) | (1 << 9) | (0xFFF << 20);
        tex.extend_from_slice(&word.to_le_bytes());
        assert!(parse(&tex).is_err());
        // 数据截断
        let tex = dxt5_tex(&solid_image(8, 8, [255, 0, 0, 255]));
        assert!(decode_mipmap0(&tex[..tex.len() - 8], DecodeOptions::default()).is_err());
    }

    #[test]
    fn test_pitch_stride_rgb() {
        // pitch > 3*w：行间有填充字节，应按行距读取。
        let mut data = Vec::new();
        let (w, h) = (2u32, 2u32);
        for y in 0..h {
            for _ in 0..w {
                data.extend_from_slice(&[10 + y as u8, 20, 30]); // 每像素 RGB
            }
            data.extend_from_slice(&[0xFF, 0xEE]); // 2B 行填充 → 行距 8B
        }
        let tex = build_tex(
            header(Compression::Rgb, 1),
            &[MipmapMeta {
                width: w,
                height: h,
                pitch: 8,
                datasz: data.len() as u32,
            }],
            &[data],
        );
        let decoded = decode_mipmap0(&tex, DecodeOptions::default()).unwrap();
        // 翻转后第 0 行是原图最后一行
        assert_eq!(decoded.get_pixel(0, 0), Rgba([11, 20, 30, 255]));
        assert_eq!(decoded.get_pixel(1, 1), Rgba([10, 20, 30, 255]));
    }

    // -- conformance（对真实 corpus 与 ktech 产物逐像素对比） ----------------

    /// 需要本机 DST + 既有 ktech 产物。手动运行：
    /// `cargo test conformance -- --ignored --nocapture`
    /// 前提：`current/kteched/` 仍为 ktech 产物（迁移到内置解码器后的首次
    /// images-sync 会移除该目录，此后本测试自动跳过）。
    #[test]
    #[ignore = "需要本机 DST 与 ktech 产物（KTOOLS__OUT_DIR/current/kteched）"]
    fn conformance_against_ktech_output() {
        use std::path::PathBuf;
        let Some(dst_root) = crate::platform::config::dst_root_opt() else {
            eprintln!("DST__ROOT 未设置，跳过");
            return;
        };
        let out_dir = crate::platform::config::ktools_out_dir();
        let out = out_dir;
        let kteched = out.join("current/kteched");
        let unzipped = out.join("current/unzipped");
        if !kteched.is_dir() {
            eprintln!("kteched 目录不存在（已迁移内置解码器？），跳过");
            return;
        }
        let dst = PathBuf::from(&dst_root);
        let scan = crate::scripts_sync::images::scan::scan(
            &dst.join("data/databundles/images.zip"),
            &dst.join("data/images"),
            &unzipped,
        )
        .expect("scan 失败");

        let mut compared = 0u64;
        let mut skipped = 0u64;
        let mut max_delta = [0u32; 4];
        let mut over_one = 0u64;
        for tex in &scan.all_tex {
            let reference = kteched.join(format!("{}.png", tex.base));
            let Ok(expected) = image::open(&reference) else {
                skipped += 1;
                continue;
            };
            let bytes = std::fs::read(&tex.path).expect("读取 tex 失败");
            let got = match decode_mipmap0(&bytes, DecodeOptions::default()) {
                Ok(img) => img.to_rgba8(),
                Err(e) => panic!("解码 {} 失败: {e}", tex.base),
            };
            let expected = expected.to_rgba8();
            assert_eq!(
                got.dimensions(),
                expected.dimensions(),
                "{} 尺寸不一致",
                tex.base
            );
            compared += 1;
            for (g, e) in got.pixels().zip(expected.pixels()) {
                for (c, (gv, ev)) in g.0.iter().zip(e.0.iter()).enumerate() {
                    let d = (i32::from(*gv) - i32::from(*ev)).unsigned_abs();
                    max_delta[c] = max_delta[c].max(d);
                    if d > 1 {
                        over_one += 1;
                    }
                }
            }
        }
        println!("conformance: 对比 {compared} / 跳过 {skipped} / maxΔ = {max_delta:?} / Δ>1 像素 {over_one}");
        assert_eq!(over_one, 0, "存在 Δ>1 的像素（maxΔ={max_delta:?}）");
    }
}
