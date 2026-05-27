use crate::anim::{AnimElement, AnimFrame};
use crate::build_file::BuildFile;

#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone)]
pub struct RenderedFrame {
    pub image: image::RgbaImage,
    pub bounds: BoundingBox,
}

fn find_symbol_frame<'a>(
    build_list: &[&'a BuildFile],
    symbol_name: &str,
    frame_num: u32,
) -> Option<&'a crate::build_file::BuildFrame> {
    let resolved = symbol_name;
    for build in build_list {
        for symbol in &build.symbols {
            if symbol.name.to_lowercase() == resolved.to_lowercase() {
                for frame in &symbol.frames {
                    if frame.frame_num == frame_num {
                        return Some(frame);
                    }
                }
            }
        }
    }
    None
}

fn apply_transform(
    sprite: &image::RgbaImage,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
) -> Option<image::RgbaImage> {
    let det = a * d - b * c;
    if det == 0.0 {
        return None;
    }

    let sw = sprite.width() as f32;
    let sh = sprite.height() as f32;

    let corners_x = [0.0, sw * a, sh * c, sw * a + sh * c];
    let corners_y = [0.0, sw * b, sh * d, sw * b + sh * d];

    let min_x = corners_x.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_x = corners_x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let min_y = corners_y.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_y = corners_y.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let out_w = (max_x - min_x).round() as u32;
    let out_h = (max_y - min_y).round() as u32;
    if out_w == 0 || out_h == 0 {
        return None;
    }

    let inv_a = d / det;
    let inv_b = -b / det;
    let inv_c = -c / det;
    let inv_d = a / det;

    let mut out = image::RgbaImage::new(out_w, out_h);

    for oy in 0..out_h {
        for ox in 0..out_w {
            let px = ox as f32 + min_x;
            let py = oy as f32 + min_y;

            let src_x = px * inv_a + py * inv_c;
            let src_y = px * inv_b + py * inv_d;

            let sx = src_x.round() as i64;
            let sy = src_y.round() as i64;

            if sx >= 0 && sy >= 0 && (sx as u32) < sprite.width() && (sy as u32) < sprite.height() {
                let pixel = sprite.get_pixel(sx as u32, sy as u32);
                out.put_pixel(ox, oy, *pixel);
            }
        }
    }

    Some(out)
}

fn calc_frame_bounds(
    anim_frame: &AnimFrame,
    build_list: &[&BuildFile],
    scale: f32,
    offset: (f32, f32),
) -> BoundingBox {
    let mut top = f32::INFINITY;
    let mut left = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    let mut right = f32::NEG_INFINITY;

    for element in &anim_frame.elements {
        let bf = find_symbol_frame(build_list, &element.symbol, element.frame_num);
        if let Some(bf) = bf {
            if bf.image.is_none() {
                continue;
            }
            let sprite = bf.image.as_ref().unwrap();

            let a = element.a * scale;
            let b = element.b * scale;
            let c = element.c * scale;
            let d = element.d * scale;

            let elem_left = element.tx * scale + offset.0 + bf.x * a + bf.y * c;
            let elem_top = element.ty * scale + offset.1 + bf.x * b + bf.y * d;

            let sw = sprite.width() as f32;
            let sh = sprite.height() as f32;
            let corners_x = [0.0, sw * a, sh * c, sw * a + sh * c];
            let corners_y = [0.0, sw * b, sh * d, sw * b + sh * d];
            let min_sx = corners_x.iter().cloned().fold(f32::INFINITY, f32::min);
            let max_sx = corners_x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let min_sy = corners_y.iter().cloned().fold(f32::INFINITY, f32::min);
            let max_sy = corners_y.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let tw = max_sx - min_sx;
            let th = max_sy - min_sy;

            left = left.min(elem_left - tw / 2.0);
            right = right.max(elem_left + tw / 2.0);
            top = top.min(elem_top - th / 2.0);
            bottom = bottom.max(elem_top + th / 2.0);
        }
    }

    BoundingBox {
        left,
        top,
        right,
        bottom,
    }
}

pub fn render_frame(
    anim_frame: &AnimFrame,
    build_list: &[&BuildFile],
    scale: f32,
    offset: (f32, f32),
) -> Option<RenderedFrame> {
    let bounds = calc_frame_bounds(anim_frame, build_list, scale, offset);
    if bounds.left.is_infinite() || bounds.right.is_infinite() {
        return None;
    }

    let w = (bounds.right - bounds.left).ceil() as u32;
    let h = (bounds.bottom - bounds.top).ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let mut canvas = image::RgbaImage::new(w, h);

    let mut sorted_elements: Vec<&AnimElement> = anim_frame.elements.iter().collect();
    sorted_elements.sort_by(|a, b| {
        a.z_index
            .partial_cmp(&b.z_index)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for element in sorted_elements {
        let bf = find_symbol_frame(build_list, &element.symbol, element.frame_num);
        if let Some(bf) = bf {
            if bf.image.is_none() {
                continue;
            }
            let sprite = bf.image.as_ref().unwrap();

            let a = element.a * scale;
            let b = element.b * scale;
            let c = element.c * scale;
            let d = element.d * scale;

            if let Some(transformed) = apply_transform(sprite, a, b, c, d) {
                let elem_left = element.tx * scale
                    + offset.0
                    + bf.x * element.a * scale
                    + bf.y * element.c * scale;
                let elem_top = element.ty * scale
                    + offset.1
                    + bf.x * element.b * scale
                    + bf.y * element.d * scale;

                let dest_x =
                    (elem_left - transformed.width() as f32 / 2.0 - bounds.left).round() as i64;
                let dest_y =
                    (elem_top - transformed.height() as f32 / 2.0 - bounds.top).round() as i64;

                for sy in 0..transformed.height() {
                    for sx in 0..transformed.width() {
                        let dx = dest_x + sx as i64;
                        let dy = dest_y + sy as i64;
                        if dx >= 0
                            && dy >= 0
                            && (dx as u32) < canvas.width()
                            && (dy as u32) < canvas.height()
                        {
                            let pixel = transformed.get_pixel(sx, sy);
                            if pixel[3] > 0 {
                                canvas.put_pixel(dx as u32, dy as u32, *pixel);
                            }
                        }
                    }
                }
            }
        }
    }

    Some(RenderedFrame {
        image: canvas,
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_abigail_flower() {
        use crate::archive::parse_zip;
        use crate::atlas::split_atlas;
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

        let anim = archive.anim.as_ref().unwrap();
        assert!(!anim.banks.is_empty());

        let build_list: Vec<&BuildFile> = vec![archive.build.as_ref().unwrap()];
        let bank = &anim.banks[0];
        assert!(!bank.animations.is_empty());

        let animation = &bank.animations[0];
        assert!(!animation.frames.is_empty());

        let frame = &animation.frames[0];
        let rendered = render_frame(frame, &build_list, 1.0, (0.0, 0.0));
        assert!(rendered.is_some());
        let rendered = rendered.unwrap();
        assert!(rendered.image.width() > 0);
        assert!(rendered.image.height() > 0);
    }

    #[test]
    fn apply_transform_identity() {
        let sprite = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        let result = apply_transform(&sprite, 1.0, 0.0, 0.0, 1.0);
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.width(), 4);
        assert_eq!(result.height(), 4);
    }
}
