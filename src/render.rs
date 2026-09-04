use std::collections::HashSet;

#[cfg(any(feature = "cli", feature = "gui"))]
use rayon::prelude::*;

use crate::anim::AnimFrame;
use crate::build_file::BuildFile;

pub struct BuildRef<'a> {
    pub build: &'a BuildFile,
    pub disabled_symbols: &'a HashSet<String>,
}

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
}

fn find_symbol_frame<'a>(
    build_list: &[BuildRef<'a>],
    symbol_name_lower: &str,
    frame_num: u32,
) -> Option<&'a crate::build_file::BuildFrame> {
    for br in build_list {
        if br.disabled_symbols.contains(symbol_name_lower) {
            continue;
        }
        if let Some(&sym_idx) = br.build.symbol_index.get(symbol_name_lower)
            && let Some(symbol) = br.build.symbols.get(sym_idx)
            && let Some(frame) = symbol.frame_for_anim_frame(frame_num)
        {
            return Some(frame);
        }
    }
    None
}

#[derive(Clone)]
pub struct ElementData {
    pub sprite: std::sync::Arc<image::RgbaImage>,
    pub spans: Option<std::sync::Arc<crate::build_file::SpriteSpans>>,
    pub bf_x: f32,
    pub bf_y: f32,
    pub dest_x: i64,
    pub dest_y: i64,
    pub canvas_w: f32,
    pub canvas_h: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

pub fn compute_frame_elements(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    scale: f32,
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> Option<Vec<ElementData>> {
    let mut elements_data: Vec<ElementData> = Vec::new();
    for element in &anim_frame.elements {
        if disabled_symbols.contains(&element.symbol_lower)
            || (!disabled_elements.is_empty()
                && disabled_elements
                    .contains(&(element.symbol_lower.clone(), element.layer_name.clone())))
        {
            continue;
        }
        let bf = find_symbol_frame(build_list, &element.symbol_lower, element.frame_num);
        if let Some(bf) = bf {
            let Some(sprite) = &bf.image else {
                continue;
            };
            elements_data.push(ElementData {
                sprite: sprite.clone(),
                spans: bf.spans.clone(),
                bf_x: bf.x,
                bf_y: bf.y,
                dest_x: bf.dest_x,
                dest_y: bf.dest_y,
                canvas_w: bf.canvas_w,
                canvas_h: bf.canvas_h,
                a: element.a * scale,
                b: element.b * scale,
                c: element.c * scale,
                d: element.d * scale,
                tx: element.tx,
                ty: element.ty,
            });
        }
    }
    if elements_data.is_empty() {
        None
    } else {
        Some(elements_data)
    }
}

pub fn compute_bounds_from_elements(
    elements_data: &[ElementData],
    scale: f32,
    offset: (f32, f32),
) -> Option<BoundingBox> {
    let mut top = f32::INFINITY;
    let mut left = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    let mut right = f32::NEG_INFINITY;

    for elem in elements_data {
        let cw = elem.sprite.width() as f32;
        let ch = elem.sprite.height() as f32;
        let center_x = elem.bf_x + (elem.dest_x as f32 + cw / 2.0 - elem.canvas_w / 2.0);
        let center_y = elem.bf_y + (elem.dest_y as f32 + ch / 2.0 - elem.canvas_h / 2.0);
        let elem_left = elem.tx * scale + offset.0 + center_x * elem.a + center_y * elem.c;
        let elem_top = elem.ty * scale + offset.1 + center_x * elem.b + center_y * elem.d;

        let sw = elem.sprite.width() as f32;
        let sh = elem.sprite.height() as f32;
        let corners_x = [0.0, sw * elem.a, sh * elem.c, sw * elem.a + sh * elem.c];
        let corners_y = [0.0, sw * elem.b, sh * elem.d, sw * elem.b + sh * elem.d];
        let tw = corners_x.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            - corners_x.iter().cloned().fold(f32::INFINITY, f32::min);
        let th = corners_y.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            - corners_y.iter().cloned().fold(f32::INFINITY, f32::min);

        left = left.min(elem_left - tw / 2.0);
        right = right.max(elem_left + tw / 2.0);
        top = top.min(elem_top - th / 2.0);
        bottom = bottom.max(elem_top + th / 2.0);
    }

    if left.is_infinite() || right.is_infinite() {
        return None;
    }

    Some(BoundingBox {
        left,
        top,
        right,
        bottom,
    })
}

pub struct PreparedFrame {
    pub elements: Vec<ElementData>,
    pub bounds: BoundingBox,
}

pub fn prepare_animation_frames(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> (Option<BoundingBox>, Vec<Option<PreparedFrame>>) {
    let mut prepared: Vec<Option<PreparedFrame>> = Vec::with_capacity(frames.len());
    let mut union_top = f32::INFINITY;
    let mut union_left = f32::INFINITY;
    let mut union_bottom = f32::NEG_INFINITY;
    let mut union_right = f32::NEG_INFINITY;

    for frame in frames {
        if let Some(elements) = compute_frame_elements(
            frame,
            build_list,
            scale,
            disabled_elements,
            disabled_symbols,
        ) && let Some(bounds) = compute_bounds_from_elements(&elements, scale, offset)
        {
            union_left = union_left.min(bounds.left);
            union_top = union_top.min(bounds.top);
            union_right = union_right.max(bounds.right);
            union_bottom = union_bottom.max(bounds.bottom);
            prepared.push(Some(PreparedFrame { elements, bounds }));
            continue;
        }
        prepared.push(None);
    }

    let union_bounds = if union_left.is_infinite() || union_right.is_infinite() {
        None
    } else {
        let frac_x = union_left - union_left.floor();
        let frac_y = union_top - union_top.floor();
        Some(BoundingBox {
            left: (union_left - 2.0).floor() + frac_x,
            top: (union_top - 2.0).floor() + frac_y,
            right: (union_right + 2.0).ceil() + frac_x,
            bottom: (union_bottom + 2.0).ceil() + frac_y,
        })
    };

    (union_bounds, prepared)
}

pub fn compute_animation_bounds(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> Option<BoundingBox> {
    let (bounds, _) = prepare_animation_frames(
        frames,
        build_list,
        scale,
        offset,
        disabled_elements,
        disabled_symbols,
    );
    bounds
}

fn span_dest_range(
    span: (usize, usize),
    slope: f32,
    off: f32,
    x_start: i64,
    x_end: i64,
) -> (i64, i64) {
    if slope == 0.0 {
        return (x_start, x_end);
    }
    let s = span.0 as f32 - 0.5;
    let e = span.1 as f32 - 0.5;
    let mut lo = (s - off) / slope;
    let mut hi = (e - off) / slope;
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    (
        x_start.max(lo.floor() as i64 - 1),
        x_end.min(hi.ceil() as i64 + 1),
    )
}

pub fn snap_frame_bounds(frame: &BoundingBox, union: &BoundingBox) -> (BoundingBox, i64, i64) {
    let frac_x = union.left - union.left.floor();
    let frac_y = union.top - union.top.floor();
    let left = (frame.left - 2.0).floor() + frac_x;
    let top = (frame.top - 2.0).floor() + frac_y;
    let right = (frame.right + 2.0).ceil() + frac_x;
    let bottom = (frame.bottom + 2.0).ceil() + frac_y;
    let off_x = (left - union.left).round() as i64;
    let off_y = (top - union.top).round() as i64;
    (
        BoundingBox {
            left,
            top,
            right,
            bottom,
        },
        off_x,
        off_y,
    )
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
    if dst_a == 0 {
        canvas_buf[dst_off] = sprite_buf[src_off];
        canvas_buf[dst_off + 1] = sprite_buf[src_off + 1];
        canvas_buf[dst_off + 2] = sprite_buf[src_off + 2];
        canvas_buf[dst_off + 3] = src_a as u8;
        return;
    }
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

pub fn render_frame_with_elements(
    elements_data: &[ElementData],
    bounds: &BoundingBox,
    scale: f32,
    offset: (f32, f32),
) -> Option<RenderedFrame> {
    let w = (bounds.right - bounds.left).ceil() as u32;
    let h = (bounds.bottom - bounds.top).ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }
    let mut canvas = image::RgbaImage::new(w, h);
    render_into(
        canvas.as_mut(),
        w as usize,
        0,
        h as i64,
        elements_data,
        bounds,
        scale,
        offset,
    );
    Some(RenderedFrame { image: canvas })
}

#[cfg(any(feature = "cli", feature = "gui"))]
const BAND_ROWS: usize = 64;

#[cfg(any(feature = "cli", feature = "gui"))]
pub fn render_frame_with_elements_par(
    elements_data: &[ElementData],
    bounds: &BoundingBox,
    scale: f32,
    offset: (f32, f32),
) -> Option<RenderedFrame> {
    let w = (bounds.right - bounds.left).ceil() as u32;
    let h = (bounds.bottom - bounds.top).ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }
    let mut canvas = image::RgbaImage::new(w, h);
    let cw = w as usize;
    let band_bytes = BAND_ROWS * cw * 4;
    canvas
        .as_mut()
        .par_chunks_mut(band_bytes)
        .enumerate()
        .for_each(|(band, band_buf)| {
            let row_base = (band * BAND_ROWS) as i64;
            let row_limit = ((band + 1) * BAND_ROWS).min(h as usize) as i64;
            render_into(
                band_buf,
                cw,
                row_base,
                row_limit,
                elements_data,
                bounds,
                scale,
                offset,
            );
        });
    Some(RenderedFrame { image: canvas })
}

#[allow(clippy::too_many_arguments)]
fn render_into(
    dst: &mut [u8],
    dst_cw: usize,
    row_base: i64,
    row_limit: i64,
    elements_data: &[ElementData],
    bounds: &BoundingBox,
    scale: f32,
    offset: (f32, f32),
) {
    let w = (bounds.right - bounds.left).ceil() as u32;
    let cw = dst_cw;

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
            let dest_x =
                (elem_left - elem.canvas_w / 2.0 - bounds.left).round() as i64 + elem.dest_x;
            let dest_y = (elem_top - elem.canvas_h / 2.0 - bounds.top).round() as i64 + elem.dest_y;

            if dest_y + sh as i64 <= row_base || dest_y >= row_limit {
                continue;
            }

            let y_start = 0i64.max(row_base - dest_y) as usize;
            let y_end = sh.min((row_limit - dest_y).max(0) as usize);
            let x_start = 0i64.max(-dest_x) as usize;
            let x_end = sw.min((w as i64 - dest_x).max(0) as usize);

            let crop_y_lo = 0i64.max(-elem.dest_y) as usize;
            let crop_y_hi = (elem.canvas_h as i64 - elem.dest_y).min(sh as i64).max(0) as usize;
            let crop_x_lo = 0i64.max(-elem.dest_x) as usize;
            let crop_x_hi = (elem.canvas_w as i64 - elem.dest_x).min(sw as i64).max(0) as usize;
            let y_start = y_start.max(crop_y_lo);
            let y_end = y_end.min(crop_y_hi);
            let x_start = x_start.max(crop_x_lo);
            let x_end = x_end.min(crop_x_hi);

            if let Some(spans) = &elem.spans {
                if spans.fully_opaque {
                    for sy in y_start..y_end {
                        let dy = (dest_y + sy as i64) as usize;
                        let src_row = sy * sw * 4;
                        let dst_row = (dy as i64 - row_base) as usize * cw * 4;
                        let lo = (dest_x + x_start as i64) as usize * 4;
                        let hi = (dest_x + x_end as i64) as usize * 4;
                        dst[dst_row + lo..dst_row + hi].copy_from_slice(
                            &sprite_buf[src_row + x_start * 4..src_row + x_end * 4],
                        );
                    }
                } else {
                    for sy in y_start..y_end {
                        let Some(rs) = spans.rows.get(sy).and_then(|r| r.as_ref()) else {
                            continue;
                        };
                        let lo = x_start.max(rs.start);
                        let hi = x_end.min(rs.end);
                        if lo >= hi {
                            continue;
                        }
                        let dy = (dest_y + sy as i64) as usize;
                        let src_row = sy * sw * 4;
                        let dst_row = (dy as i64 - row_base) as usize * cw * 4;
                        let dst_lo = (dest_x + lo as i64) as usize * 4;
                        if rs.opaque {
                            dst[dst_row + dst_lo..dst_row + dst_lo + (hi - lo) * 4]
                                .copy_from_slice(&sprite_buf[src_row + lo * 4..src_row + hi * 4]);
                        } else {
                            for sx in lo..hi {
                                composite_pixel(
                                    dst,
                                    dst_row + (dest_x + sx as i64) as usize * 4,
                                    sprite_buf,
                                    src_row + sx * 4,
                                );
                            }
                        }
                    }
                }
            } else {
                for sy in y_start..y_end {
                    let dy = (dest_y + sy as i64) as usize;
                    let src_row = sy * sw * 4;
                    let dst_row = (dy as i64 - row_base) as usize * cw * 4;
                    for sx in x_start..x_end {
                        let dx = (dest_x + sx as i64) as usize;
                        let src_off = src_row + sx * 4;
                        let dst_off = dst_row + dx * 4;
                        composite_pixel(dst, dst_off, sprite_buf, src_off);
                    }
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

            let cw_canvas = elem.canvas_w;
            let ch_canvas = elem.canvas_h;
            let corners_x = [
                0.0f32,
                cw_canvas * a,
                ch_canvas * c,
                cw_canvas * a + ch_canvas * c,
            ];
            let corners_y = [
                0.0f32,
                cw_canvas * b,
                ch_canvas * d,
                cw_canvas * b + ch_canvas * d,
            ];
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

            if dest_y + out_h as i64 <= row_base || dest_y >= row_limit {
                continue;
            }

            let swf = sw as f32;
            let shf = sh as f32;
            let dx = elem.dest_x as f32;
            let dy = elem.dest_y as f32;
            let crop_cx = [dx, dx + swf, dx, dx + swf];
            let crop_cy = [dy, dy, dy + shf, dy + shf];
            let mut crop_min_x = f32::INFINITY;
            let mut crop_max_x = f32::NEG_INFINITY;
            let mut crop_min_y = f32::INFINITY;
            let mut crop_max_y = f32::NEG_INFINITY;
            for i in 0..4 {
                let sx = crop_cx[i] * a + crop_cy[i] * c;
                let sy = crop_cx[i] * b + crop_cy[i] * d;
                crop_min_x = crop_min_x.min(sx);
                crop_max_x = crop_max_x.max(sx);
                crop_min_y = crop_min_y.min(sy);
                crop_max_y = crop_max_y.max(sy);
            }

            let x_start = dest_x
                .max(0)
                .max((dest_x as f32 + crop_min_x - min_cx).floor() as i64 - 1);
            let x_end = (dest_x + out_w as i64)
                .min(w as i64)
                .min((dest_x as f32 + crop_max_x - min_cx).ceil() as i64 + 1);
            let y_start = dest_y
                .max(0)
                .max((dest_y as f32 + crop_min_y - min_cy).floor() as i64 - 1)
                .max(row_base);
            let y_end = (dest_y + out_h as i64)
                .min(row_limit)
                .min((dest_y as f32 + crop_max_y - min_cy).ceil() as i64 + 1);

            let is_uniform_scale = b.abs() < 1e-6 && c.abs() < 1e-6;

            if is_uniform_scale {
                let slope = inv_a;
                let base_off = (min_cx - dest_x as f32) * inv_a - dx;
                for cy in y_start..y_end {
                    let oy = (cy - dest_y) as f32;
                    let py = oy + min_cy;
                    let src_y = py * inv_d - dy;
                    let sy = src_y.round() as i64;
                    if sy < 0 || (sy as usize) >= sh {
                        continue;
                    }
                    let (lo, hi) = match &elem.spans {
                        Some(spans) => match spans.rows.get(sy as usize).and_then(|r| r.as_ref()) {
                            Some(rs) => {
                                span_dest_range((rs.start, rs.end), slope, base_off, x_start, x_end)
                            }
                            None => continue,
                        },
                        None => (x_start, x_end),
                    };
                    if lo >= hi {
                        continue;
                    }
                    let dst_row = (cy - row_base) as usize * cw * 4;
                    let src_row = sy as usize * sw * 4;
                    let mut src_x = (lo as f32 - dest_x as f32 + min_cx) * inv_a - dx;
                    let mut resync = 0usize;
                    for cx in lo..hi {
                        let sx = src_x.round() as i64;
                        let cxv = sx + elem.dest_x;
                        if sx >= 0
                            && (sx as usize) < sw
                            && cxv >= 0
                            && (cxv as usize) < elem.canvas_w as usize
                        {
                            let src_off = src_row + sx as usize * 4;
                            let dst_off = dst_row + cx as usize * 4;
                            composite_pixel(dst, dst_off, sprite_buf, src_off);
                        }
                        resync += 1;
                        if resync & 15 == 0 {
                            let px = (cx as f32 + 1.0 - dest_x as f32) + min_cx;
                            src_x = px * inv_a - dx;
                        } else {
                            src_x += inv_a;
                        }
                    }
                }
            } else {
                for cy in y_start..y_end {
                    let oy = (cy - dest_y) as f32;
                    let py = oy + min_cy;
                    let src_y_edge_l =
                        (x_start as f32 - dest_x as f32 + min_cx) * inv_b + py * inv_d - dy;
                    let src_y_edge_r =
                        (x_end as f32 - dest_x as f32 + min_cx) * inv_b + py * inv_d - dy;
                    let mut sy_lo = src_y_edge_l.min(src_y_edge_r).round() as i64 - 1;
                    let mut sy_hi = src_y_edge_l.max(src_y_edge_r).round() as i64 + 1;
                    if sy_hi < 0 || sy_lo >= sh as i64 {
                        continue;
                    }
                    if let Some(spans) = &elem.spans {
                        sy_lo = sy_lo.max(0);
                        sy_hi = sy_hi.min(sh as i64 - 1);
                        let mut any_content = false;
                        for s in sy_lo..=sy_hi {
                            if spans.rows[s as usize].is_some() {
                                any_content = true;
                                break;
                            }
                        }
                        if !any_content {
                            continue;
                        }
                    }
                    let dst_row = (cy - row_base) as usize * cw * 4;
                    let mut src_x =
                        (x_start as f32 - dest_x as f32 + min_cx) * inv_a + py * inv_c - dx;
                    let mut src_y =
                        (x_start as f32 - dest_x as f32 + min_cx) * inv_b + py * inv_d - dy;
                    let mut resync = 0usize;
                    for cx in x_start..x_end {
                        let sx = src_x.round() as i64;
                        let sy = src_y.round() as i64;
                        let cxv = sx + elem.dest_x;
                        let cyv = sy + elem.dest_y;
                        if sx >= 0
                            && sy >= 0
                            && (sx as usize) < sw
                            && (sy as usize) < sh
                            && cxv >= 0
                            && cyv >= 0
                            && (cxv as usize) < elem.canvas_w as usize
                            && (cyv as usize) < elem.canvas_h as usize
                        {
                            let src_off = (sy as usize * sw + sx as usize) * 4;
                            let dst_off = dst_row + cx as usize * 4;
                            composite_pixel(dst, dst_off, sprite_buf, src_off);
                        }
                        resync += 1;
                        if resync & 15 == 0 {
                            let px = (cx as f32 + 1.0 - dest_x as f32) + min_cx;
                            src_x = px * inv_a + py * inv_c - dx;
                            src_y = px * inv_b + py * inv_d - dy;
                        } else {
                            src_x += inv_a;
                            src_y += inv_b;
                        }
                    }
                }
            }
        }
    }
}

pub fn render_frame(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    bounds_override: Option<&BoundingBox>,
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> Option<RenderedFrame> {
    let elements = compute_frame_elements(
        anim_frame,
        build_list,
        scale,
        disabled_elements,
        disabled_symbols,
    )?;
    let bounds = match bounds_override {
        Some(ub) => ub.clone(),
        None => compute_bounds_from_elements(&elements, scale, offset)?,
    };
    render_frame_with_elements(&elements, &bounds, scale, offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimElement;
    use crate::build_file::{BuildFrame, BuildSymbol};
    use std::collections::HashMap;

    fn make_build_with_symbol(
        symbol_name: &str,
        frame_num: u32,
        sprite: image::RgbaImage,
    ) -> BuildFile {
        let sym_idx = 0usize;
        let sprite_w = sprite.width() as f32;
        let sprite_h = sprite.height() as f32;
        BuildFile {
            version: 6,
            name: "test_build".into(),
            symbols: vec![BuildSymbol {
                name: symbol_name.into(),
                frames: vec![BuildFrame {
                    frame_num,
                    duration: 1,
                    x: 0.0,
                    y: 0.0,
                    width: sprite_w,
                    height: sprite_h,
                    verts: Vec::new(),
                    image: Some(std::sync::Arc::new(sprite)),
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: sprite_w,
                    canvas_h: sprite_h,
                    spans: None,
                }],
                frame_index: {
                    let mut m = HashMap::new();
                    m.insert(frame_num, 0);
                    m
                },
            }],
            atlases: Vec::new(),
            symbol_index: {
                let mut m = HashMap::new();
                m.insert(symbol_name.to_lowercase(), sym_idx);
                m
            },
        }
    }

    fn make_anim_frame(elements: Vec<AnimElement>) -> AnimFrame {
        AnimFrame {
            idx: 0,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            elements,
            events: Vec::new(),
        }
    }

    #[test]
    fn render_abigail_flower() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();

        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let anim = archive.anim.as_ref().unwrap();
        assert!(!anim.banks.is_empty());

        let empty = HashSet::new();
        let build_list: Vec<BuildRef<'_>> = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty,
        }];
        let bank = &anim.banks[0];
        assert!(!bank.animations.is_empty());

        let animation = &bank.animations[0];
        assert!(!animation.frames.is_empty());

        let frame = &animation.frames[0];
        let rendered = render_frame(
            frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(rendered.is_some());
        let rendered = rendered.unwrap();
        assert!(rendered.image.width() > 0);
        assert!(rendered.image.height() > 0);
    }

    #[test]
    fn composite_pixel_opaque_overwrites() {
        let mut canvas = [100u8, 100, 100, 128];
        let sprite = [255u8, 0, 0, 255];
        composite_pixel(&mut canvas, 0, &sprite, 0);
        assert_eq!(canvas, [255, 0, 0, 255]);
    }

    #[test]
    fn composite_pixel_transparent_skips() {
        let mut canvas = [100u8, 100, 100, 128];
        let sprite = [255u8, 0, 0, 0];
        composite_pixel(&mut canvas, 0, &sprite, 0);
        assert_eq!(canvas, [100, 100, 100, 128]);
    }

    #[test]
    fn composite_pixel_alpha_blend() {
        let mut canvas = [100u8, 150, 200, 128];
        let sprite = [50u8, 100, 0, 128];
        composite_pixel(&mut canvas, 0, &sprite, 0);
        let src_a: u32 = 128;
        let dst_a: u32 = 128;
        let out_a = src_a + dst_a - (src_a * dst_a + 127) / 255;
        assert_eq!(canvas[3], out_a.min(255) as u8);
        assert!(
            canvas[0] > 50 && canvas[0] < 100,
            "blended R should be between 50 and 100, got {}",
            canvas[0]
        );
    }

    #[test]
    fn composite_pixel_src_over_empty_canvas() {
        let mut canvas = [0u8, 0, 0, 0];
        let sprite = [200u8, 100, 50, 180];
        composite_pixel(&mut canvas, 0, &sprite, 0);
        assert_eq!(canvas[0], 200);
        assert_eq!(canvas[1], 100);
        assert_eq!(canvas[2], 50);
        assert_eq!(canvas[3], 180);
    }

    #[test]
    fn find_symbol_frame_found() {
        let build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let result = find_symbol_frame(&build_list, "sym", 0);
        assert!(result.is_some());
        assert_eq!(result.unwrap().frame_num, 0);
    }

    #[test]
    fn find_symbol_frame_not_found() {
        let build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let result = find_symbol_frame(&build_list, "nonexistent", 0);
        assert!(result.is_none());
    }

    #[test]
    fn find_symbol_frame_wrong_frame_num() {
        let build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let result = find_symbol_frame(&build_list, "sym", 99);
        assert!(result.is_none());
    }

    #[test]
    fn find_symbol_frame_disabled_falls_through() {
        let build1 = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let build2 = make_build_with_symbol("sym", 0, image::RgbaImage::new(20, 20));
        let mut disabled = HashSet::new();
        disabled.insert("sym".into());
        let empty = HashSet::new();
        let build_list = vec![
            BuildRef {
                build: &build1,
                disabled_symbols: &disabled,
            },
            BuildRef {
                build: &build2,
                disabled_symbols: &empty,
            },
        ];
        let result = find_symbol_frame(&build_list, "sym", 0);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().image.as_ref().unwrap().width(),
            20,
            "should find from second build after first is disabled"
        );
    }

    #[test]
    fn compute_frame_elements_empty() {
        let anim_frame = make_anim_frame(Vec::new());
        let build = make_build_with_symbol("x", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        assert!(
            compute_frame_elements(
                &anim_frame,
                &build_list,
                1.0,
                &HashSet::new(),
                &HashSet::new()
            )
            .is_none()
        );
    }

    #[test]
    fn compute_frame_elements_skips_no_image() {
        let mut build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        build.symbols[0].frames[0].image = None;
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        assert!(
            compute_frame_elements(
                &anim_frame,
                &build_list,
                1.0,
                &HashSet::new(),
                &HashSet::new()
            )
            .is_none(),
            "element with no image should be skipped, yielding None"
        );
    }

    #[test]
    fn compute_bounds_from_elements_identity_transform() {
        let sprite = image::RgbaImage::new(10, 10);
        let elements = vec![ElementData {
            sprite: std::sync::Arc::new(sprite),
            spans: None,
            bf_x: 0.0,
            bf_y: 0.0,
            dest_x: 0,
            dest_y: 0,
            canvas_w: 10.0,
            canvas_h: 10.0,
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        }];
        let bounds = compute_bounds_from_elements(&elements, 1.0, (0.0, 0.0)).unwrap();
        let w = bounds.right - bounds.left;
        let h = bounds.bottom - bounds.top;
        assert!(
            (w - 10.0).abs() < 1.0,
            "width should be ~10 for identity, got {w}"
        );
        assert!(
            (h - 10.0).abs() < 1.0,
            "height should be ~10 for identity, got {h}"
        );
    }

    #[test]
    fn compute_animation_bounds_none_for_all_empty() {
        let frames = vec![make_anim_frame(Vec::new()), make_anim_frame(Vec::new())];
        let build = make_build_with_symbol("x", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        assert!(
            compute_animation_bounds(
                &frames,
                &build_list,
                1.0,
                (0.0, 0.0),
                &HashSet::new(),
                &HashSet::new()
            )
            .is_none()
        );
    }

    #[test]
    fn prepare_animation_frames_mixed() {
        let build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let frames = vec![
            make_anim_frame(vec![elem.clone()]),
            make_anim_frame(Vec::new()),
            make_anim_frame(vec![elem.clone()]),
        ];
        let (union_bounds, prepared) = prepare_animation_frames(
            &frames,
            &build_list,
            1.0,
            (0.0, 0.0),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(union_bounds.is_some());
        assert!(prepared[0].is_some());
        assert!(prepared[1].is_none());
        assert!(prepared[2].is_some());
    }

    #[test]
    fn render_frame_with_zero_det_skipped() {
        let sprite = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let build = make_build_with_symbol("sym", 0, sprite);
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let result = render_frame(
            &anim_frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn render_frame_with_uniform_scale() {
        let sprite = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let build = make_build_with_symbol("sym", 0, sprite);
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 2.0,
            b: 0.0,
            c: 0.0,
            d: 2.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let result = render_frame(
            &anim_frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(result.is_some());
        let rendered = result.unwrap();
        assert!(
            rendered.image.width() >= 18,
            "2x scale of 10px should produce ~20px, got {}",
            rendered.image.width()
        );
    }

    #[test]
    fn render_frame_with_full_transform() {
        let sprite = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let build = make_build_with_symbol("sym", 0, sprite);
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 1.0,
            b: 0.5,
            c: -0.5,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let result = render_frame(
            &anim_frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(result.is_some());
        let rendered = result.unwrap();
        assert!(rendered.image.width() > 0);
        assert!(rendered.image.height() > 0);
    }

    #[test]
    fn render_frame_bounds_override() {
        let sprite = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let build = make_build_with_symbol("sym", 0, sprite);
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let bounds = BoundingBox {
            left: 0.0,
            top: 0.0,
            right: 200.0,
            bottom: 200.0,
        };
        let result = render_frame(
            &anim_frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            Some(&bounds),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(result.is_some());
        let rendered = result.unwrap();
        assert_eq!(rendered.image.width(), 200);
        assert_eq!(rendered.image.height(), 200);
    }

    #[test]
    fn render_abigail_flower_has_nontransparent_pixels() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let anim = archive.anim.as_ref().unwrap();
        let empty = HashSet::new();
        let build_list: Vec<BuildRef<'_>> = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty,
        }];
        let bank = &anim.banks[0];
        let animation = &bank.animations[0];
        let frame = &animation.frames[0];
        let rendered = render_frame(
            frame,
            &build_list,
            1.0,
            (0.0, 0.0),
            None,
            &HashSet::new(),
            &HashSet::new(),
        )
        .unwrap();
        let has_opaque = rendered.image.as_raw().chunks_exact(4).any(|px| px[3] > 0);
        assert!(
            has_opaque,
            "rendered image should have non-transparent pixels"
        );
    }

    #[test]
    fn render_abigail_flower_all_frames() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let anim = archive.anim.as_ref().unwrap();
        let empty = HashSet::new();
        let build_list: Vec<BuildRef<'_>> = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty,
        }];
        for bank in &anim.banks {
            for animation in &bank.animations {
                for frame in &animation.frames {
                    if let Some(rendered) = render_frame(
                        frame,
                        &build_list,
                        1.0,
                        (0.0, 0.0),
                        None,
                        &HashSet::new(),
                        &HashSet::new(),
                    ) {
                        assert!(
                            rendered.image.width() > 0 && rendered.image.height() > 0,
                            "rendered frame should have positive dims"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn compute_frame_elements_disabled_element_skipped() {
        let build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let mut disabled = HashSet::new();
        disabled.insert(("sym".into(), "layer".into()));
        assert!(
            compute_frame_elements(&anim_frame, &build_list, 1.0, &disabled, &HashSet::new())
                .is_none(),
            "disabled element should be skipped"
        );
    }

    #[test]
    fn compute_frame_elements_disabled_symbol_skipped() {
        let build = make_build_with_symbol("sym", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let elem = AnimElement {
            z_index: 0.0,
            symbol: "sym".into(),
            symbol_lower: "sym".into(),
            frame_num: 0,
            layer_name: "layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);
        let mut disabled_symbols = HashSet::new();
        disabled_symbols.insert("sym".into());
        assert!(
            compute_frame_elements(
                &anim_frame,
                &build_list,
                1.0,
                &HashSet::new(),
                &disabled_symbols
            )
            .is_none(),
            "element referencing a disabled symbol should be skipped"
        );
    }

    #[test]
    fn crop_render_matches_canvas_render() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let build = archive.build.as_ref().unwrap();
        let mut old_build = build.clone();
        for sym in &mut old_build.symbols {
            for f in &mut sym.frames {
                f.image = crate::atlas::rebuild_frame_canvas(f).map(std::sync::Arc::new);
                f.spans = None;
                f.dest_x = 0;
                f.dest_y = 0;
            }
        }

        let anim = archive.anim.as_ref().unwrap();
        let empty_symbols: HashSet<String> = HashSet::new();
        let empty_elements: HashSet<(String, String)> = HashSet::new();
        let new_list = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty_symbols,
        }];
        let old_list = vec![BuildRef {
            build: &old_build,
            disabled_symbols: &empty_symbols,
        }];

        for bank in &anim.banks {
            for animation in &bank.animations {
                let (bounds, new_prepared) = prepare_animation_frames(
                    &animation.frames,
                    &new_list,
                    1.0,
                    (0.0, 0.0),
                    &empty_elements,
                    &empty_symbols,
                );
                let (_, old_prepared) = prepare_animation_frames(
                    &animation.frames,
                    &old_list,
                    1.0,
                    (0.0, 0.0),
                    &empty_elements,
                    &empty_symbols,
                );
                let Some(bounds) = bounds else {
                    continue;
                };
                for (np, op) in new_prepared.iter().zip(old_prepared.iter()) {
                    let (Some(np), Some(op)) = (np.as_ref(), op.as_ref()) else {
                        continue;
                    };
                    let new_img =
                        render_frame_with_elements(&np.elements, &bounds, 1.0, (0.0, 0.0))
                            .unwrap()
                            .image;
                    let old_img =
                        render_frame_with_elements(&op.elements, &bounds, 1.0, (0.0, 0.0))
                            .unwrap()
                            .image;
                    assert_eq!(new_img.dimensions(), old_img.dimensions());
                    assert_eq!(
                        new_img.as_raw(),
                        old_img.as_raw(),
                        "crop-based render must be pixel-identical to canvas-based render"
                    );
                }
            }
        }
    }

    #[cfg(any(feature = "cli", feature = "gui"))]
    #[test]
    fn par_render_matches_sequential() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let anim = archive.anim.as_ref().unwrap();
        let empty_symbols: HashSet<String> = HashSet::new();
        let empty_elements: HashSet<(String, String)> = HashSet::new();
        let build_list = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty_symbols,
        }];

        for bank in &anim.banks {
            for animation in &bank.animations {
                let (bounds, prepared) = prepare_animation_frames(
                    &animation.frames,
                    &build_list,
                    1.0,
                    (0.0, 0.0),
                    &empty_elements,
                    &empty_symbols,
                );
                let Some(union) = bounds else {
                    continue;
                };
                for pf in prepared.iter().flatten() {
                    let seq = render_frame_with_elements(&pf.elements, &union, 1.0, (0.0, 0.0))
                        .unwrap()
                        .image;
                    let par = render_frame_with_elements_par(&pf.elements, &union, 1.0, (0.0, 0.0))
                        .unwrap()
                        .image;
                    assert_eq!(seq.dimensions(), par.dimensions());
                    assert_eq!(
                        seq.as_raw(),
                        par.as_raw(),
                        "band-parallel render must be pixel-identical to sequential"
                    );
                }
            }
        }
    }

    #[test]
    fn snapped_frame_bounds_match_union_render() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let anim = archive.anim.as_ref().unwrap();
        let empty_symbols: HashSet<String> = HashSet::new();
        let empty_elements: HashSet<(String, String)> = HashSet::new();
        let build_list = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty_symbols,
        }];

        for bank in &anim.banks {
            for animation in &bank.animations {
                let (bounds, prepared) = prepare_animation_frames(
                    &animation.frames,
                    &build_list,
                    1.0,
                    (0.0, 0.0),
                    &empty_elements,
                    &empty_symbols,
                );
                let Some(union) = bounds else {
                    continue;
                };
                let uw = (union.right - union.left).ceil() as u32;
                let uh = (union.bottom - union.top).ceil() as u32;
                for pf in prepared.iter().flatten() {
                    let union_img =
                        render_frame_with_elements(&pf.elements, &union, 1.0, (0.0, 0.0))
                            .unwrap()
                            .image;
                    let (snapped, off_x, off_y) = snap_frame_bounds(&pf.bounds, &union);
                    let snapped_img =
                        render_frame_with_elements(&pf.elements, &snapped, 1.0, (0.0, 0.0))
                            .unwrap()
                            .image;
                    let mut pasted = image::RgbaImage::new(uw, uh);
                    let dst_x = off_x.max(0) as usize;
                    let dst_y = off_y.max(0) as usize;
                    let src_x0 = (-off_x).max(0) as usize;
                    let src_y0 = (-off_y).max(0) as usize;
                    let src_buf = snapped_img.as_raw();
                    let dst_buf = pasted.as_mut();
                    let sw = snapped_img.width() as usize;
                    for y in src_y0..snapped_img.height() as usize {
                        let dy = dst_y + (y - src_y0);
                        if dy >= uh as usize {
                            continue;
                        }
                        let src_row = y * sw * 4;
                        let dst_row = dy * uw as usize * 4;
                        for x in src_x0..sw {
                            let dx = dst_x + (x - src_x0);
                            if dx >= uw as usize {
                                break;
                            }
                            let src_off = src_row + x * 4;
                            let dst_off = dst_row + dx * 4;
                            if src_buf[src_off + 3] != 0 {
                                dst_buf[dst_off..dst_off + 4]
                                    .copy_from_slice(&src_buf[src_off..src_off + 4]);
                            }
                        }
                    }
                    assert_eq!(
                        pasted.as_raw(),
                        union_img.as_raw(),
                        "snapped per-frame render pasted at its offset must equal the union render"
                    );
                }
            }
        }
    }

    #[test]
    fn spans_fast_path_matches_fallback() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let build = archive.build.as_ref().unwrap();
        let mut no_spans_build = build.clone();
        for sym in &mut no_spans_build.symbols {
            for f in &mut sym.frames {
                f.spans = None;
            }
        }

        let anim = archive.anim.as_ref().unwrap();
        let empty_symbols: HashSet<String> = HashSet::new();
        let empty_elements: HashSet<(String, String)> = HashSet::new();
        let spans_list = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty_symbols,
        }];
        let fallback_list = vec![BuildRef {
            build: &no_spans_build,
            disabled_symbols: &empty_symbols,
        }];

        for bank in &anim.banks {
            for animation in &bank.animations {
                let (bounds, spans_prepared) = prepare_animation_frames(
                    &animation.frames,
                    &spans_list,
                    1.0,
                    (0.0, 0.0),
                    &empty_elements,
                    &empty_symbols,
                );
                let (_, fallback_prepared) = prepare_animation_frames(
                    &animation.frames,
                    &fallback_list,
                    1.0,
                    (0.0, 0.0),
                    &empty_elements,
                    &empty_symbols,
                );
                let Some(bounds) = bounds else {
                    continue;
                };
                for (sp, fp) in spans_prepared.iter().zip(fallback_prepared.iter()) {
                    let (Some(sp), Some(fp)) = (sp.as_ref(), fp.as_ref()) else {
                        continue;
                    };
                    let fast = render_frame_with_elements(&sp.elements, &bounds, 1.0, (0.0, 0.0))
                        .unwrap()
                        .image;
                    let slow = render_frame_with_elements(&fp.elements, &bounds, 1.0, (0.0, 0.0))
                        .unwrap()
                        .image;
                    assert_eq!(fast.dimensions(), slow.dimensions());
                    assert_eq!(
                        fast.as_raw(),
                        slow.as_raw(),
                        "span fast path must match per-pixel fallback"
                    );
                }
            }
        }
    }

    #[test]
    fn split_atlas_dedupes_shared_crops() {
        use crate::atlas::split_atlas;

        let verts = vec![
            crate::build_file::BuildVert {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                u: 0.0,
                v: 1.0,
                w: 0,
            },
            crate::build_file::BuildVert {
                x: 10.0,
                y: 0.0,
                z: 0.0,
                u: 1.0,
                v: 1.0,
                w: 0,
            },
            crate::build_file::BuildVert {
                x: 0.0,
                y: 10.0,
                z: 0.0,
                u: 0.0,
                v: 0.0,
                w: 0,
            },
            crate::build_file::BuildVert {
                x: 0.0,
                y: 10.0,
                z: 0.0,
                u: 0.0,
                v: 0.0,
                w: 0,
            },
            crate::build_file::BuildVert {
                x: 10.0,
                y: 0.0,
                z: 0.0,
                u: 1.0,
                v: 1.0,
                w: 0,
            },
            crate::build_file::BuildVert {
                x: 10.0,
                y: 10.0,
                z: 0.0,
                u: 1.0,
                v: 0.0,
                w: 0,
            },
        ];
        let atlas_img = {
            let mut img = image::RgbaImage::new(16, 16);
            for (i, px) in img.as_mut().chunks_exact_mut(4).enumerate() {
                px.copy_from_slice(&[i as u8, 0, 0, 255]);
            }
            img
        };
        let mut build = crate::build_file::BuildFile {
            version: 6,
            name: "test".into(),
            symbols: vec![
                crate::build_file::BuildSymbol {
                    name: "s1".into(),
                    frames: vec![BuildFrame {
                        frame_num: 0,
                        duration: 1,
                        x: 5.0,
                        y: 5.0,
                        width: 10.0,
                        height: 10.0,
                        verts: verts.clone(),
                        image: None,
                        dest_x: 0,
                        dest_y: 0,
                        canvas_w: 10.0,
                        canvas_h: 10.0,
                        spans: None,
                    }],
                    frame_index: HashMap::new(),
                },
                crate::build_file::BuildSymbol {
                    name: "s2".into(),
                    frames: vec![BuildFrame {
                        frame_num: 0,
                        duration: 1,
                        x: 5.0,
                        y: 5.0,
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
                },
            ],
            atlases: Vec::new(),
            symbol_index: HashMap::new(),
        };
        let atlas_images = vec![std::sync::Arc::new(atlas_img)];
        split_atlas(&mut build, &atlas_images).unwrap();
        let img1 = build.symbols[0].frames[0].image.as_ref().unwrap();
        let img2 = build.symbols[1].frames[0].image.as_ref().unwrap();
        assert!(
            std::sync::Arc::ptr_eq(img1, img2),
            "identical UV rects should share one Arc sprite"
        );
        assert_eq!(
            build.symbols[0].frames[0].dest_x,
            build.symbols[1].frames[0].dest_x
        );
        assert_eq!(
            build.symbols[0].frames[0].dest_y,
            build.symbols[1].frames[0].dest_y
        );
    }
}
