use crate::anim::AnimFrame;
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
    symbol_name_lower: &str,
    frame_num: u32,
) -> Option<&'a crate::build_file::BuildFrame> {
    for build in build_list {
        if let Some(&sym_idx) = build.symbol_index.get(symbol_name_lower)
            && let Some(symbol) = build.symbols.get(sym_idx)
        {
            for frame in &symbol.frames {
                if frame.frame_num == frame_num {
                    return Some(frame);
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
    let is_identity =
        (a - 1.0).abs() < 1e-6 && b.abs() < 1e-6 && c.abs() < 1e-6 && (d - 1.0).abs() < 1e-6;
    if is_identity {
        return Some(sprite.clone());
    }

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

    let is_uniform_scale = b.abs() < 1e-6 && c.abs() < 1e-6;

    let inv_a = d / det;
    let inv_b = -b / det;
    let inv_c = -c / det;
    let inv_d = a / det;

    let mut out = image::RgbaImage::new(out_w, out_h);

    if is_uniform_scale {
        for oy in 0..out_h {
            for ox in 0..out_w {
                let px = ox as f32 + min_x;
                let py = oy as f32 + min_y;
                let src_x = px * inv_a + py * inv_c;
                let src_y = py * inv_d;
                let sx = src_x.round() as i64;
                let sy = src_y.round() as i64;
                if sx >= 0
                    && sy >= 0
                    && (sx as u32) < sprite.width()
                    && (sy as u32) < sprite.height()
                {
                    let pixel = *sprite.get_pixel(sx as u32, sy as u32);
                    out.put_pixel(ox, oy, pixel);
                }
            }
        }
    } else {
        for oy in 0..out_h {
            for ox in 0..out_w {
                let px = ox as f32 + min_x;
                let py = oy as f32 + min_y;
                let src_x = px * inv_a + py * inv_c;
                let src_y = px * inv_b + py * inv_d;
                let sx = src_x.round() as i64;
                let sy = src_y.round() as i64;
                if sx >= 0
                    && sy >= 0
                    && (sx as u32) < sprite.width()
                    && (sy as u32) < sprite.height()
                {
                    let pixel = *sprite.get_pixel(sx as u32, sy as u32);
                    out.put_pixel(ox, oy, pixel);
                }
            }
        }
    }

    Some(out)
}

struct ElementData {
    sprite: std::sync::Arc<image::RgbaImage>,
    bf_x: f32,
    bf_y: f32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    tx: f32,
    ty: f32,
}

fn compute_frame_bounds(
    anim_frame: &AnimFrame,
    build_list: &[&BuildFile],
    scale: f32,
    offset: (f32, f32),
) -> Option<(BoundingBox, Vec<ElementData>)> {
    let mut top = f32::INFINITY;
    let mut left = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    let mut right = f32::NEG_INFINITY;

    let mut elements_data: Vec<ElementData> = Vec::new();

    for element in &anim_frame.elements {
        let lower = element.symbol.to_lowercase();
        let bf = find_symbol_frame(build_list, &lower, element.frame_num);
        if let Some(bf) = bf {
            let Some(sprite) = &bf.image else {
                continue;
            };

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

            elements_data.push(ElementData {
                sprite: sprite.clone(),
                bf_x: bf.x,
                bf_y: bf.y,
                a,
                b,
                c,
                d,
                tx: element.tx,
                ty: element.ty,
            });
        }
    }

    if left.is_infinite() || right.is_infinite() {
        return None;
    }

    Some((
        BoundingBox {
            left,
            top,
            right,
            bottom,
        },
        elements_data,
    ))
}

pub fn compute_animation_bounds(
    frames: &[AnimFrame],
    build_list: &[&BuildFile],
    scale: f32,
    offset: (f32, f32),
) -> Option<BoundingBox> {
    let mut union_top = f32::INFINITY;
    let mut union_left = f32::INFINITY;
    let mut union_bottom = f32::NEG_INFINITY;
    let mut union_right = f32::NEG_INFINITY;

    for frame in frames {
        if let Some((bounds, _)) = compute_frame_bounds(frame, build_list, scale, offset) {
            union_left = union_left.min(bounds.left);
            union_top = union_top.min(bounds.top);
            union_right = union_right.max(bounds.right);
            union_bottom = union_bottom.max(bounds.bottom);
        }
    }

    if union_left.is_infinite() || union_right.is_infinite() {
        return None;
    }

    Some(BoundingBox {
        left: union_left,
        top: union_top,
        right: union_right,
        bottom: union_bottom,
    })
}

pub fn render_frame(
    anim_frame: &AnimFrame,
    build_list: &[&BuildFile],
    scale: f32,
    offset: (f32, f32),
    bounds_override: Option<&BoundingBox>,
) -> Option<RenderedFrame> {
    let (bounds, elements_data) = match bounds_override {
        Some(ub) => {
            let (_, elements_data) = compute_frame_bounds(anim_frame, build_list, scale, offset)?;
            (ub.clone(), elements_data)
        }
        None => compute_frame_bounds(anim_frame, build_list, scale, offset)?,
    };

    let w = (bounds.right - bounds.left).ceil() as u32;
    let h = (bounds.bottom - bounds.top).ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let mut canvas = image::RgbaImage::new(w, h);

    for elem in elements_data.iter().rev() {
        let sprite: &image::RgbaImage = &elem.sprite;
        if let Some(transformed) = apply_transform(sprite, elem.a, elem.b, elem.c, elem.d) {
            let elem_left = elem.tx * scale + offset.0 + elem.bf_x * elem.a + elem.bf_y * elem.c;
            let elem_top = elem.ty * scale + offset.1 + elem.bf_x * elem.b + elem.bf_y * elem.d;

            let dest_x =
                (elem_left - transformed.width() as f32 / 2.0 - bounds.left).round() as i64;
            let dest_y = (elem_top - transformed.height() as f32 / 2.0 - bounds.top).round() as i64;

            let tw = transformed.width();
            let th = transformed.height();

            for sy in 0..th {
                let dy = dest_y + sy as i64;
                if dy < 0 || dy >= h as i64 {
                    continue;
                }
                for sx in 0..tw {
                    let dx = dest_x + sx as i64;
                    if dx < 0 || dx >= w as i64 {
                        continue;
                    }
                    let pixel = *transformed.get_pixel(sx, sy);
                    if pixel[3] > 0 {
                        canvas.put_pixel(dx as u32, dy as u32, pixel);
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
    use std::sync::Arc;

    #[test]
    fn render_abigail_flower() {
        use crate::archive::parse_zip;
        use crate::atlas::split_atlas;
        use crate::ktex::parse_ktex;

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();

        let mut atlas_images: Vec<Arc<image::RgbaImage>> = Vec::new();
        for atlas in &archive.build.as_ref().unwrap().atlases {
            let tex_data = archive.tex_files.get(&atlas.name);
            if let Some(tex_data) = tex_data {
                let ktex = parse_ktex(tex_data).unwrap();
                let img = ktex.to_image_rgba().unwrap();
                atlas_images.push(Arc::new(img));
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
        let rendered = render_frame(frame, &build_list, 1.0, (0.0, 0.0), None);
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
