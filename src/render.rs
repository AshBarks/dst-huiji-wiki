use std::collections::HashSet;

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
            && let Some(&fi) = symbol.frame_index.get(&frame_num)
        {
            return Some(&symbol.frames[fi]);
        }
    }
    None
}

pub struct ElementData {
    pub sprite: std::sync::Arc<image::RgbaImage>,
    pub bf_x: f32,
    pub bf_y: f32,
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
) -> Option<Vec<ElementData>> {
    let mut elements_data: Vec<ElementData> = Vec::new();
    for element in &anim_frame.elements {
        let bf = find_symbol_frame(build_list, &element.symbol_lower, element.frame_num);
        if let Some(bf) = bf {
            let Some(sprite) = &bf.image else {
                continue;
            };
            elements_data.push(ElementData {
                sprite: sprite.clone(),
                bf_x: bf.x,
                bf_y: bf.y,
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

fn compute_bounds_from_elements(
    elements_data: &[ElementData],
    scale: f32,
    offset: (f32, f32),
) -> Option<BoundingBox> {
    let mut top = f32::INFINITY;
    let mut left = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    let mut right = f32::NEG_INFINITY;

    for elem in elements_data {
        let elem_left = elem.tx * scale + offset.0 + elem.bf_x * elem.a + elem.bf_y * elem.c;
        let elem_top = elem.ty * scale + offset.1 + elem.bf_x * elem.b + elem.bf_y * elem.d;

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
) -> (Option<BoundingBox>, Vec<Option<PreparedFrame>>) {
    let mut prepared: Vec<Option<PreparedFrame>> = Vec::with_capacity(frames.len());
    let mut union_top = f32::INFINITY;
    let mut union_left = f32::INFINITY;
    let mut union_bottom = f32::NEG_INFINITY;
    let mut union_right = f32::NEG_INFINITY;

    for frame in frames {
        if let Some(elements) = compute_frame_elements(frame, build_list, scale)
            && let Some(bounds) = compute_bounds_from_elements(&elements, scale, offset)
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
        Some(BoundingBox {
            left: union_left,
            top: union_top,
            right: union_right,
            bottom: union_bottom,
        })
    };

    (union_bounds, prepared)
}

pub fn compute_animation_bounds(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
) -> Option<BoundingBox> {
    let (bounds, _) = prepare_animation_frames(frames, build_list, scale, offset);
    bounds
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

    Some(RenderedFrame { image: canvas })
}

pub fn render_frame(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    bounds_override: Option<&BoundingBox>,
) -> Option<RenderedFrame> {
    let elements = compute_frame_elements(anim_frame, build_list, scale)?;
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
        BuildFile {
            version: 6,
            name: "test_build".into(),
            symbols: vec![BuildSymbol {
                name: symbol_name.into(),
                frames: vec![BuildFrame {
                    frame_num,
                    x: 0.0,
                    y: 0.0,
                    width: sprite.width() as f32,
                    height: sprite.height() as f32,
                    verts: Vec::new(),
                    image: Some(std::sync::Arc::new(sprite)),
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
        let rendered = render_frame(frame, &build_list, 1.0, (0.0, 0.0), None);
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
        assert!(compute_frame_elements(&anim_frame, &build_list, 1.0).is_none());
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
            compute_frame_elements(&anim_frame, &build_list, 1.0).is_none(),
            "element with no image should be skipped, yielding None"
        );
    }

    #[test]
    fn compute_bounds_from_elements_identity_transform() {
        let sprite = image::RgbaImage::new(10, 10);
        let elements = vec![ElementData {
            sprite: std::sync::Arc::new(sprite),
            bf_x: 0.0,
            bf_y: 0.0,
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
        assert!(compute_animation_bounds(&frames, &build_list, 1.0, (0.0, 0.0)).is_none());
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
        let (union_bounds, prepared) =
            prepare_animation_frames(&frames, &build_list, 1.0, (0.0, 0.0));
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
        let result = render_frame(&anim_frame, &build_list, 1.0, (0.0, 0.0), None);
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
        let result = render_frame(&anim_frame, &build_list, 1.0, (0.0, 0.0), None);
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
        let result = render_frame(&anim_frame, &build_list, 1.0, (0.0, 0.0), None);
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
        let result = render_frame(&anim_frame, &build_list, 1.0, (0.0, 0.0), Some(&bounds));
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
        let rendered = render_frame(frame, &build_list, 1.0, (0.0, 0.0), None).unwrap();
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
                    if let Some(rendered) = render_frame(frame, &build_list, 1.0, (0.0, 0.0), None)
                    {
                        assert!(
                            rendered.image.width() > 0 && rendered.image.height() > 0,
                            "rendered frame should have positive dims"
                        );
                    }
                }
            }
        }
    }
}
