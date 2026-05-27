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
        let bf = find_symbol_frame(build_list, &element.symbol_lower, element.frame_num);
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

fn composite_pixel(canvas_buf: &mut [u8], dst_off: usize, sprite_buf: &[u8], src_off: usize) {
    let src_a = sprite_buf[src_off + 3] as u32;
    if src_a == 0 {
        return;
    }
    if src_a == 255 {
        canvas_buf[dst_off..dst_off + 4].copy_from_slice(&sprite_buf[src_off..src_off + 4]);
        return;
    }
    let dst_a = canvas_buf[dst_off + 3] as u32;
    let out_a = src_a + dst_a - (src_a * dst_a + 127) / 255;
    if out_a == 0 {
        return;
    }
    let src_a_255 = src_a * 255;
    let dst_contrib = dst_a * (255 - src_a);
    let denom = out_a * 255;
    let half = denom / 2;
    canvas_buf[dst_off] =
        ((sprite_buf[src_off] as u32 * src_a_255 + canvas_buf[dst_off] as u32 * dst_contrib + half)
            / denom)
            .min(255) as u8;
    canvas_buf[dst_off + 1] = ((sprite_buf[src_off + 1] as u32 * src_a_255
        + canvas_buf[dst_off + 1] as u32 * dst_contrib
        + half)
        / denom)
        .min(255) as u8;
    canvas_buf[dst_off + 2] = ((sprite_buf[src_off + 2] as u32 * src_a_255
        + canvas_buf[dst_off + 2] as u32 * dst_contrib
        + half)
        / denom)
        .min(255) as u8;
    canvas_buf[dst_off + 3] = out_a.min(255) as u8;
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
    let canvas_buf = canvas.as_mut();
    let cw = w as usize;

    for elem in elements_data.iter().rev() {
        let sprite: &image::RgbaImage = &elem.sprite;
        let sprite_buf = sprite.as_raw();
        let sw = sprite.width() as usize;
        let sh = sprite.height() as usize;

        let a = elem.a;
        let b = elem.b;
        let c = elem.c;
        let d = elem.d;

        let elem_left = elem.tx * scale + offset.0 + elem.bf_x * a + elem.bf_y * c;
        let elem_top = elem.ty * scale + offset.1 + elem.bf_x * b + elem.bf_y * d;

        let is_identity =
            (a - 1.0).abs() < 1e-6 && b.abs() < 1e-6 && c.abs() < 1e-6 && (d - 1.0).abs() < 1e-6;

        if is_identity {
            let dest_x = (elem_left - sw as f32 / 2.0 - bounds.left).round() as i64;
            let dest_y = (elem_top - sh as f32 / 2.0 - bounds.top).round() as i64;

            let y_start = 0i64.max(-dest_y) as usize;
            let y_end = sh.min((h as i64 - dest_y).max(0) as usize);
            let x_start = 0i64.max(-dest_x) as usize;
            let x_end = sw.min((w as i64 - dest_x).max(0) as usize);

            for sy in y_start..y_end {
                let dy = (dest_y + sy as i64) as usize;
                let src_row = sy * sw * 4;
                let dst_row = dy * cw * 4;
                for sx in x_start..x_end {
                    let dx = (dest_x + sx as i64) as usize;
                    let src_off = src_row + sx * 4;
                    let dst_off = dst_row + dx * 4;
                    composite_pixel(canvas_buf, dst_off, sprite_buf, src_off);
                }
            }
        } else {
            let det = a * d - b * c;
            if det == 0.0 {
                continue;
            }

            let inv_a = d / det;
            let inv_b = -b / det;
            let inv_c = -c / det;
            let inv_d = a / det;

            let swf = sw as f32;
            let shf = sh as f32;
            let corners_x = [0.0f32, swf * a, shf * c, swf * a + shf * c];
            let corners_y = [0.0f32, swf * b, shf * d, swf * b + shf * d];
            let min_cx = corners_x.iter().cloned().fold(f32::INFINITY, f32::min);
            let max_cx = corners_x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let min_cy = corners_y.iter().cloned().fold(f32::INFINITY, f32::min);
            let max_cy = corners_y.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

            let out_w = (max_cx - min_cx).round() as u32;
            let out_h = (max_cy - min_cy).round() as u32;
            if out_w == 0 || out_h == 0 {
                continue;
            }

            let dest_x = (elem_left - out_w as f32 / 2.0 - bounds.left).round() as i64;
            let dest_y = (elem_top - out_h as f32 / 2.0 - bounds.top).round() as i64;

            let x_start = dest_x.max(0);
            let x_end = (dest_x + out_w as i64).min(w as i64);
            let y_start = dest_y.max(0);
            let y_end = (dest_y + out_h as i64).min(h as i64);

            let is_uniform_scale = b.abs() < 1e-6 && c.abs() < 1e-6;

            if is_uniform_scale {
                for cy in y_start..y_end {
                    let oy = (cy - dest_y) as f32;
                    let py = oy + min_cy;
                    let src_y = py * inv_d;
                    let sy = src_y.round() as i64;
                    if sy < 0 || (sy as usize) >= sh {
                        continue;
                    }
                    let dst_row = cy as usize * cw * 4;
                    let src_row = sy as usize * sw * 4;
                    for cx in x_start..x_end {
                        let ox = (cx - dest_x) as f32;
                        let px = ox + min_cx;
                        let src_x = px * inv_a;
                        let sx = src_x.round() as i64;
                        if sx >= 0 && (sx as usize) < sw {
                            let src_off = src_row + sx as usize * 4;
                            let dst_off = dst_row + cx as usize * 4;
                            composite_pixel(canvas_buf, dst_off, sprite_buf, src_off);
                        }
                    }
                }
            } else {
                for cy in y_start..y_end {
                    let oy = (cy - dest_y) as f32;
                    let py = oy + min_cy;
                    let dst_row = cy as usize * cw * 4;
                    for cx in x_start..x_end {
                        let ox = (cx - dest_x) as f32;
                        let px = ox + min_cx;
                        let src_x = px * inv_a + py * inv_c;
                        let src_y = px * inv_b + py * inv_d;
                        let sx = src_x.round() as i64;
                        let sy = src_y.round() as i64;
                        if sx >= 0 && sy >= 0 && (sx as usize) < sw && (sy as usize) < sh {
                            let src_off = (sy as usize * sw + sx as usize) * 4;
                            let dst_off = dst_row + cx as usize * 4;
                            composite_pixel(canvas_buf, dst_off, sprite_buf, src_off);
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
        use crate::atlas::{decode_atlas_images, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();

        let atlas_images = decode_atlas_images(&archive);
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
}
