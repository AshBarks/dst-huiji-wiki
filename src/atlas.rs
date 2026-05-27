use crate::build_file::{BuildFile, BuildVert};
use crate::error::Result;

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
    for sy in 0..sprite.height() {
        for sx in 0..sprite.width() {
            let dx = dest_x + sx as i64;
            let dy = dest_y + sy as i64;
            if dx >= 0 && dy >= 0 && (dx as u32) < canvas.width() && (dy as u32) < canvas.height() {
                let pixel = sprite.get_pixel(sx, sy);
                canvas.put_pixel(dx as u32, dy as u32, *pixel);
            }
        }
    }
}

pub fn split_atlas(build: &mut BuildFile, atlas_images: &[image::RgbaImage]) -> Result<()> {
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
            let atlas_img = &atlas_images[atlas_idx];

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

            frame.image = Some(canvas);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_atlas_abigail_flower() {
        use crate::archive::parse_zip;
        use crate::ktex::parse_ktex;

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();

        let mut atlas_images: Vec<image::RgbaImage> = Vec::new();
        for atlas in &archive.build.as_ref().unwrap().atlases {
            let tex_data = archive.tex_files.get(&atlas.name);
            if let Some(tex_data) = tex_data {
                let ktex = parse_ktex(tex_data).unwrap();
                let img = ktex.to_image_rgba().unwrap();
                atlas_images.push(img);
            }
        }

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
