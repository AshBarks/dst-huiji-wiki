use std::collections::HashMap;
use std::sync::Arc;

use crate::build_file::{BuildFile, BuildFrame, BuildVert, RowSpan, SpriteSpans};
use crate::error::Result;
use crate::ktex::parse_ktex;

fn calc_uv_bounds(verts: &[BuildVert]) -> (f32, f32, f32, f32) {
    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for chunk in verts.as_chunks::<6>().0 {
        min_u = min_u.min(chunk[0].u);
        max_u = max_u.max(chunk[1].u);
        max_v = max_v.max(chunk[0].v);
        min_v = min_v.min(chunk[2].v);
    }
    (min_u, max_u, min_v, max_v)
}

fn calc_xy_bounds(verts: &[BuildVert]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for chunk in verts.as_chunks::<6>().0 {
        min_x = min_x.min(chunk[0].x);
        max_x = max_x.max(chunk[1].x);
        min_y = min_y.min(chunk[0].y);
        max_y = max_y.max(chunk[2].y);
    }
    (min_x, max_x, min_y, max_y)
}

pub(crate) fn paste(
    canvas: &mut image::RgbaImage,
    sprite: &image::RgbaImage,
    dest_x: i64,
    dest_y: i64,
) {
    let cw = canvas.width() as usize;
    let ch = canvas.height() as i64;
    let sw = sprite.width() as usize;
    let sh = sprite.height() as usize;
    let canvas_w = canvas.width() as i64;
    let canvas_buf = canvas.as_mut();
    let sprite_buf = sprite.as_raw();

    let y_start = 0i64.max(-dest_y) as usize;
    let y_end = sh.min((ch - dest_y).max(0) as usize);
    let x_start = 0i64.max(-dest_x) as usize;
    let x_end = sw.min((canvas_w - dest_x).max(0) as usize);

    for sy in y_start..y_end {
        let dy = (dest_y + sy as i64) as usize;
        let src_row = sy * sw * 4;
        let dst_row = dy * cw * 4;
        for sx in x_start..x_end {
            let dx = (dest_x + sx as i64) as usize;
            let src_off = src_row + sx * 4;
            let dst_off = dst_row + dx * 4;
            if sprite_buf[src_off + 3] > 0 {
                canvas_buf[dst_off..dst_off + 4].copy_from_slice(&sprite_buf[src_off..src_off + 4]);
            }
        }
    }
}

fn compute_spans(img: &image::RgbaImage) -> SpriteSpans {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let raw = img.as_raw();
    let mut rows = Vec::with_capacity(h);
    let mut fully_opaque = true;
    for y in 0..h {
        let row = &raw[y * w * 4..(y + 1) * w * 4];
        let mut start = None;
        let mut end = 0usize;
        let mut opaque = true;
        for (x, px) in row.as_chunks::<4>().0.iter().enumerate() {
            if px[3] != 0 {
                if start.is_none() {
                    start = Some(x);
                }
                end = x + 1;
                if px[3] != 255 {
                    opaque = false;
                }
            }
        }
        match start {
            Some(s) => {
                fully_opaque = fully_opaque && opaque && s == 0 && end == w;
                rows.push(Some(RowSpan {
                    start: s,
                    end,
                    opaque,
                }));
            }
            None => rows.push(None),
        }
    }
    SpriteSpans {
        rows: rows.into_boxed_slice(),
        fully_opaque,
    }
}

pub fn rebuild_frame_canvas(frame: &BuildFrame) -> Option<image::RgbaImage> {
    let sprite = frame.image_ref()?;
    let mut canvas =
        image::RgbaImage::new(frame.canvas_w.round() as u32, frame.canvas_h.round() as u32);
    paste(&mut canvas, sprite, frame.dest_x, frame.dest_y);
    Some(canvas)
}

pub fn split_atlas(build: &mut BuildFile, atlas_images: &[Arc<image::RgbaImage>]) -> Result<()> {
    type CropKey = (u32, u32, u32, u32, u32, u32, u32);
    type CropEntry = (Arc<image::RgbaImage>, Arc<SpriteSpans>);
    let crop_cache: std::sync::Mutex<HashMap<CropKey, CropEntry>> =
        std::sync::Mutex::new(HashMap::new());
    build.symbols.iter_mut().for_each(|symbol| {
        for frame in &mut symbol.frames {
            let verts = &frame.verts;
            if verts.is_empty() || verts.len() < 6 {
                continue;
            }

            let (min_u, max_u, min_v, max_v) = calc_uv_bounds(verts);
            let (min_x, max_x, min_y, max_y) = calc_xy_bounds(verts);

            let atlas_idx = verts[0].w as usize;
            if atlas_idx >= atlas_images.len() {
                continue;
            }
            let atlas_img: &image::RgbaImage = atlas_images[atlas_idx].as_ref();

            let src_x = (min_u * atlas_img.width() as f32).round() as u32;
            let src_y = ((1.0 - max_v) * atlas_img.height() as f32).round() as u32;
            let src_w = ((max_u - min_u) * atlas_img.width() as f32).round() as u32;
            let src_h = ((max_v - min_v) * atlas_img.height() as f32).round() as u32;

            let src_x = src_x.min(atlas_img.width().saturating_sub(1));
            let src_y = src_y.min(atlas_img.height().saturating_sub(1));
            let src_w = src_w.min(atlas_img.width() - src_x);
            let src_h = src_h.min(atlas_img.height() - src_y);

            if src_w == 0 || src_h == 0 {
                continue;
            }

            let expected_w = (max_x - min_x).round().max(1.0) as u32;
            let expected_h = (max_y - min_y).round().max(1.0) as u32;

            let key = (
                atlas_idx as u32,
                src_x,
                src_y,
                src_w,
                src_h,
                expected_w,
                expected_h,
            );
            // 先把锁作用域收敛到本语句内（edition 2021 下 if-let 临时值活到整个 if/else 结束，
            // 直接在 else 里二次 lock 会死锁）。
            let cached = crop_cache.lock().unwrap().get(&key).cloned();
            let entry = if let Some(cached) = cached {
                cached
            } else {
                let mut sprite =
                    image::imageops::crop_imm(atlas_img, src_x, src_y, src_w, src_h).to_image();
                if sprite.width() != expected_w || sprite.height() != expected_h {
                    sprite = image::imageops::resize(
                        &sprite,
                        expected_w,
                        expected_h,
                        image::imageops::FilterType::Triangle,
                    );
                }
                let spans = compute_spans(&sprite);
                let pair = (Arc::new(sprite), Arc::new(spans));
                let mut cache = crop_cache.lock().unwrap();
                cache.entry(key).or_insert_with(|| pair.clone()).clone()
            };

            let pivot_x = frame.x - (frame.width / 2.0).floor();
            let pivot_y = frame.y - (frame.height / 2.0).floor();
            let dest_x = (min_x - pivot_x).round() as i64;
            let dest_y = (min_y - pivot_y).round() as i64;

            let canvas_w = frame.width.round().max(1.0);
            let canvas_h = frame.height.round().max(1.0);

            frame.image = Some(entry.0.clone());
            frame.spans = Some(entry.1.clone());
            frame.dest_x = dest_x;
            frame.dest_y = dest_y;
            frame.canvas_w = canvas_w;
            frame.canvas_h = canvas_h;
        }
    });
    Ok(())
}

pub fn decode_atlas_images_from_tex(
    atlases: &[crate::build_file::BuildAtlasRef],
    tex_files: &std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>,
) -> Vec<Arc<image::RgbaImage>> {
    decode_atlas_images_inner(atlases, tex_files)
}

pub fn gather_atlas_images(
    build: &BuildFile,
    atlas_decoded: &std::collections::HashMap<String, Arc<image::RgbaImage>>,
) -> Vec<Arc<image::RgbaImage>> {
    build
        .atlases
        .iter()
        .map(|atlas| {
            atlas_decoded
                .get(&atlas.name)
                .cloned()
                .unwrap_or_else(|| Arc::new(image::RgbaImage::new(1, 1)))
        })
        .collect()
}

fn decode_atlas_images_inner(
    atlases: &[crate::build_file::BuildAtlasRef],
    tex_files: &std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>,
) -> Vec<Arc<image::RgbaImage>> {
    let mut images = Vec::new();
    for atlas in atlases {
        let tex_data = tex_files.get(&atlas.name);
        if let Some(tex_data) = tex_data {
            if let Ok(ktex) = parse_ktex(tex_data) {
                if let Ok(img) = ktex.to_image_rgba() {
                    images.push(Arc::new(img));
                    continue;
                }
            }
        }
        images.push(Arc::new(image::RgbaImage::new(1, 1)));
    }
    images
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_file::{BuildAtlasRef, BuildFrame, BuildSymbol};
    use std::collections::HashMap;

    fn make_vert(x: f32, y: f32, z: f32, u: f32, v: f32, w: u32) -> BuildVert {
        BuildVert { x, y, z, u, v, w }
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn split_atlas_abigail_flower() {
        use crate::archive::parse_zip;

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();

        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );

        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let build = archive.build.as_ref().unwrap();
        let frames_with_images: usize = build
            .symbols
            .iter()
            .map(|s| s.frames.iter().filter(|f| f.image.is_some()).count())
            .sum();
        assert!(frames_with_images > 0);
    }

    #[test]
    fn calc_uv_bounds_basic() {
        let verts = vec![
            make_vert(0.0, 0.0, 0.0, 0.1, 0.9, 0),
            make_vert(1.0, 0.0, 0.0, 0.5, 0.9, 0),
            make_vert(0.0, 1.0, 0.0, 0.1, 0.5, 0),
            make_vert(0.0, 1.0, 0.0, 0.1, 0.5, 0),
            make_vert(1.0, 0.0, 0.0, 0.5, 0.9, 0),
            make_vert(1.0, 1.0, 0.0, 0.5, 0.5, 0),
        ];
        let (min_u, max_u, min_v, max_v) = calc_uv_bounds(&verts);
        assert!((min_u - 0.1).abs() < 1e-6, "min_u={min_u}");
        assert!((max_u - 0.5).abs() < 1e-6, "max_u={max_u}");
        assert!((min_v - 0.5).abs() < 1e-6, "min_v={min_v}");
        assert!((max_v - 0.9).abs() < 1e-6, "max_v={max_v}");
    }

    #[test]
    fn calc_uv_bounds_empty_verts() {
        let verts: Vec<BuildVert> = Vec::new();
        let (min_u, max_u, _min_v, _max_v) = calc_uv_bounds(&verts);
        assert!(min_u.is_infinite(), "empty verts should yield INF min_u");
        assert!(
            max_u.is_infinite() && max_u.is_sign_negative(),
            "empty verts should yield -INF max_u"
        );
    }

    #[test]
    fn calc_uv_bounds_partial_chunk_ignored() {
        let mut verts = vec![
            make_vert(0.0, 0.0, 0.0, 0.1, 0.9, 0),
            make_vert(1.0, 0.0, 0.0, 0.5, 0.9, 0),
            make_vert(0.0, 1.0, 0.0, 0.1, 0.5, 0),
            make_vert(0.0, 1.0, 0.0, 0.1, 0.5, 0),
            make_vert(1.0, 0.0, 0.0, 0.5, 0.9, 0),
            make_vert(1.0, 1.0, 0.0, 0.5, 0.5, 0),
        ];
        verts.push(make_vert(0.0, 0.0, 0.0, 0.9, 0.1, 0));
        let (min_u, max_u, _min_v, _max_v) = calc_uv_bounds(&verts);
        assert!((min_u - 0.1).abs() < 1e-6, "7th vert should be ignored");
        assert!((max_u - 0.5).abs() < 1e-6);
    }

    #[test]
    fn calc_xy_bounds_basic() {
        let verts = vec![
            make_vert(10.0, 20.0, 0.0, 0.0, 1.0, 0),
            make_vert(50.0, 20.0, 0.0, 1.0, 1.0, 0),
            make_vert(10.0, 60.0, 0.0, 0.0, 0.0, 0),
            make_vert(10.0, 60.0, 0.0, 0.0, 0.0, 0),
            make_vert(50.0, 20.0, 0.0, 1.0, 1.0, 0),
            make_vert(50.0, 60.0, 0.0, 1.0, 0.0, 0),
        ];
        let (min_x, max_x, min_y, max_y) = calc_xy_bounds(&verts);
        assert!((min_x - 10.0).abs() < 1e-6, "min_x={min_x}");
        assert!((max_x - 50.0).abs() < 1e-6, "max_x={max_x}");
        assert!((min_y - 20.0).abs() < 1e-6, "min_y={min_y}");
        assert!((max_y - 60.0).abs() < 1e-6, "max_y={max_y}");
    }

    #[test]
    fn paste_within_canvas() {
        let mut canvas = image::RgbaImage::new(4, 4);
        let sprite = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        paste(&mut canvas, &sprite, 1, 1);
        assert_eq!(canvas.get_pixel(1, 1), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(canvas.get_pixel(2, 2), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(canvas.get_pixel(0, 0), &image::Rgba([0, 0, 0, 0]));
        assert_eq!(canvas.get_pixel(3, 3), &image::Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn paste_clips_at_edge() {
        let mut canvas = image::RgbaImage::new(4, 4);
        let sprite = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 255, 0, 255]));
        paste(&mut canvas, &sprite, -1, -1);
        assert_eq!(canvas.get_pixel(0, 0), &image::Rgba([0, 255, 0, 255]));
        assert_eq!(canvas.get_pixel(2, 2), &image::Rgba([0, 255, 0, 255]));
        assert_eq!(
            canvas.get_pixel(3, 3),
            &image::Rgba([0, 0, 0, 0]),
            "pixel at (3,3) should remain untouched since sprite extends to canvas edge at offset -1"
        );
    }

    #[test]
    fn paste_transparent_pixels_ignored() {
        let mut canvas = image::RgbaImage::from_pixel(4, 4, image::Rgba([100, 100, 100, 255]));
        let mut sprite = image::RgbaImage::new(2, 2);
        *sprite.get_pixel_mut(0, 0) = image::Rgba([255, 0, 0, 0]);
        *sprite.get_pixel_mut(1, 0) = image::Rgba([0, 255, 0, 255]);
        paste(&mut canvas, &sprite, 0, 0);
        assert_eq!(
            canvas.get_pixel(0, 0),
            &image::Rgba([100, 100, 100, 255]),
            "transparent sprite pixel should not overwrite canvas"
        );
        assert_eq!(
            canvas.get_pixel(1, 0),
            &image::Rgba([0, 255, 0, 255]),
            "opaque sprite pixel should overwrite canvas"
        );
    }

    #[test]
    fn split_atlas_skip_empty_verts() {
        let mut build = BuildFile {
            version: 6,
            name: "test".into(),
            symbols: vec![BuildSymbol {
                name: "empty_sym".into(),
                frames: vec![BuildFrame {
                    frame_num: 0,
                    duration: 1,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts: Vec::new(),
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                }],
                frame_index: HashMap::new(),
            }],
            atlases: Vec::new(),
            symbol_index: HashMap::new(),
        };
        let atlas_images: Vec<Arc<image::RgbaImage>> = Vec::new();
        split_atlas(&mut build, &atlas_images).unwrap();
        assert!(
            build.symbols[0].frames[0].image.is_none(),
            "frame with empty verts should not get an image"
        );
    }

    #[test]
    fn split_atlas_skip_invalid_atlas_idx() {
        let verts = vec![
            make_vert(0.0, 0.0, 0.0, 0.0, 1.0, 99),
            make_vert(10.0, 0.0, 0.0, 1.0, 1.0, 99),
            make_vert(0.0, 10.0, 0.0, 0.0, 0.0, 99),
            make_vert(0.0, 10.0, 0.0, 0.0, 0.0, 99),
            make_vert(10.0, 0.0, 0.0, 1.0, 1.0, 99),
            make_vert(10.0, 10.0, 0.0, 1.0, 0.0, 99),
        ];
        let mut build = BuildFile {
            version: 6,
            name: "test".into(),
            symbols: vec![BuildSymbol {
                name: "bad_idx".into(),
                frames: vec![BuildFrame {
                    frame_num: 0,
                    duration: 1,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts,
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                }],
                frame_index: HashMap::new(),
            }],
            atlases: Vec::new(),
            symbol_index: HashMap::new(),
        };
        let atlas_images = vec![Arc::new(image::RgbaImage::new(64, 64))];
        split_atlas(&mut build, &atlas_images).unwrap();
        assert!(
            build.symbols[0].frames[0].image.is_none(),
            "frame with atlas_idx=99 should be skipped"
        );
    }

    #[test]
    fn decode_atlas_images_from_tex_missing_key() {
        let atlases = vec![BuildAtlasRef {
            name: "nonexistent.tex".into(),
        }];
        let tex_files = HashMap::new();
        let images = decode_atlas_images_from_tex(&atlases, &tex_files);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].width(), 1);
        assert_eq!(images[0].height(), 1);
    }

    #[test]
    fn gather_atlas_images_missing_key() {
        let build = BuildFile {
            version: 6,
            name: "test".into(),
            symbols: Vec::new(),
            atlases: vec![BuildAtlasRef {
                name: "missing.tex".into(),
            }],
            symbol_index: HashMap::new(),
        };
        let atlas_decoded = HashMap::new();
        let images = gather_atlas_images(&build, &atlas_decoded);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].width(), 1);
        assert_eq!(images[0].height(), 1);
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn split_atlas_frame_dimensions_positive() {
        use crate::archive::parse_zip;

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();
        let build = archive.build.as_ref().unwrap();
        for symbol in &build.symbols {
            for frame in &symbol.frames {
                if let Some(img) = &frame.image {
                    assert!(
                        img.width() >= 1 && img.height() >= 1,
                        "frame image should have positive dims, got {}x{}",
                        img.width(),
                        img.height()
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn split_atlas_multi_atlas() {
        use crate::archive::parse_zip;

        let data = std::fs::read("data/anim/abigail_shield.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        assert_eq!(atlas_images.len(), 2, "should have 2 atlas images");
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();
        let build = archive.build.as_ref().unwrap();
        let frames_with_images: usize = build
            .symbols
            .iter()
            .map(|s| s.frames.iter().filter(|f| f.image.is_some()).count())
            .sum();
        assert!(
            frames_with_images > 0,
            "multi-atlas split should produce images"
        );
    }
}
