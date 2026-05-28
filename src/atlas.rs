use std::sync::Arc;

use crate::build_file::{BuildFile, BuildVert};
use crate::error::Result;
use crate::ktex::parse_ktex;

fn calc_uv_bounds(verts: &[BuildVert]) -> (f32, f32, f32, f32) {
    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for chunk in verts.chunks_exact(6) {
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
    for chunk in verts.chunks_exact(6) {
        min_x = min_x.min(chunk[0].x);
        max_x = max_x.max(chunk[1].x);
        min_y = min_y.min(chunk[0].y);
        max_y = max_y.max(chunk[2].y);
    }
    (min_x, max_x, min_y, max_y)
}

fn paste(canvas: &mut image::RgbaImage, sprite: &image::RgbaImage, dest_x: i64, dest_y: i64) {
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

pub fn split_atlas(build: &mut BuildFile, atlas_images: &[Arc<image::RgbaImage>]) -> Result<()> {
    for symbol in &mut build.symbols {
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

            let mut sprite =
                image::imageops::crop_imm(atlas_img, src_x, src_y, src_w, src_h).to_image();

            let expected_w = (max_x - min_x).round().max(1.0) as u32;
            let expected_h = (max_y - min_y).round().max(1.0) as u32;
            if sprite.width() != expected_w || sprite.height() != expected_h {
                sprite = image::imageops::resize(
                    &sprite,
                    expected_w,
                    expected_h,
                    image::imageops::FilterType::Triangle,
                );
            }

            let pivot_x = frame.x - (frame.width / 2.0).floor();
            let pivot_y = frame.y - (frame.height / 2.0).floor();
            let dest_x = (min_x - pivot_x).round() as i64;
            let dest_y = (min_y - pivot_y).round() as i64;

            let canvas_w = frame.width.round().max(1.0) as u32;
            let canvas_h = frame.height.round().max(1.0) as u32;
            let mut canvas = image::RgbaImage::new(canvas_w, canvas_h);
            paste(&mut canvas, &sprite, dest_x, dest_y);

            frame.image = Some(Arc::new(canvas));
        }
    }
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
        if let Some(tex_data) = tex_data
            && let Ok(ktex) = parse_ktex(tex_data)
            && let Ok(img) = ktex.to_image_rgba()
        {
            images.push(Arc::new(img));
            continue;
        }
        images.push(Arc::new(image::RgbaImage::new(1, 1)));
    }
    images
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
}
