//! KTEX 解码兼容层：实现统一在 `dst-ktex` crate（`crates/dst-ktex`）。
//!
//! 保留 `parse` / `decode_rgba` 两个入口与旧语义：仅当文件尾存在
//! pre-multiply 标记且为真时做反预乘（对应 `DecodeOptions::demultiply`）。

pub use dst_ktex::{parse, Compression, DecodeOptions, KtexError, KtexHeader, MipmapMeta};

/// 解码 mipmap 0 为 RGBA8（等价旧 `Ktex::to_image_rgba`）。
pub fn decode_rgba(data: &[u8]) -> Result<image::RgbaImage, KtexError> {
    let demultiply = dst_ktex::trailing_pre_multiply_alpha(data).unwrap_or(false);
    Ok(dst_ktex::decode_mipmap0(data, DecodeOptions { demultiply })?.into_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn parse_ktex_real_file() {
        let zip_data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name().ends_with(".tex") {
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut buf).unwrap();
                let (_, metas, _) = parse(&buf).unwrap();
                assert!(!metas.is_empty());
                let m0 = metas[0];
                assert!(m0.width > 0 && m0.height > 0);
                let img = decode_rgba(&buf).unwrap();
                assert_eq!(img.width(), m0.width);
                assert_eq!(img.height(), m0.height);
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
