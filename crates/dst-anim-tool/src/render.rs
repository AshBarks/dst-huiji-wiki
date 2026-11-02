use std::collections::{BTreeMap, HashMap, HashSet};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::anim::{AnimElement, AnimFrame};
use crate::build_file::{BuildFile, InkRect};

pub struct BuildRef<'a> {
    pub build: &'a BuildFile,
    pub disabled_symbols: &'a HashSet<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BoundsKind {
    #[default]
    Rect,
    Ink,
}

#[derive(Clone)]
pub struct RenderOptions {
    pub scale: f32,
    pub offset: (f32, f32),
    disabled_elements: HashSet<(String, String)>,
    disabled_symbols: HashSet<String>,
    disabled_layers: HashSet<String>,
    overrides: SymbolOverrideMap,
    bounds_kind: BoundsKind,
}

fn normalize_element_set(set: HashSet<(String, String)>) -> HashSet<(String, String)> {
    set.into_iter()
        .map(|(s, l)| (s.to_lowercase(), l.to_lowercase()))
        .collect()
}

fn normalize_str_set(set: HashSet<String>) -> HashSet<String> {
    set.into_iter().map(|s| s.to_lowercase()).collect()
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self::new(1.0, (0.0, 0.0))
    }
}

impl RenderOptions {
    pub fn new(scale: f32, offset: (f32, f32)) -> Self {
        Self {
            scale,
            offset,
            disabled_elements: HashSet::new(),
            disabled_symbols: HashSet::new(),
            disabled_layers: HashSet::new(),
            overrides: SymbolOverrideMap::new(),
            bounds_kind: BoundsKind::Rect,
        }
    }

    pub fn with_disabled_elements(mut self, set: HashSet<(String, String)>) -> Self {
        self.disabled_elements = normalize_element_set(set);
        self
    }

    pub fn with_disabled_symbols(mut self, set: HashSet<String>) -> Self {
        self.disabled_symbols = normalize_str_set(set);
        self
    }

    pub fn with_disabled_layers(mut self, set: HashSet<String>) -> Self {
        self.disabled_layers = normalize_str_set(set);
        self
    }

    pub fn with_overrides(mut self, overrides: SymbolOverrideMap) -> Self {
        self.overrides = overrides;
        self
    }

    pub fn with_bounds_kind(mut self, kind: BoundsKind) -> Self {
        self.bounds_kind = kind;
        self
    }

    pub fn bounds_kind(&self) -> BoundsKind {
        self.bounds_kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingSymbolKind {
    NoSymbol,
    NoFrame,
    NoImage,
}

#[derive(Debug, Clone)]
pub struct MissingSymbolInfo {
    pub symbol: String,
    pub layer: String,
    pub lookup_symbol: String,
    pub kind: MissingSymbolKind,
    pub frames: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct RenderDiagnostics {
    pub missing: BTreeMap<(String, String), MissingSymbolInfo>,
    pub skipped_frames: Vec<usize>,
}

impl RenderDiagnostics {
    pub fn record(
        &mut self,
        symbol: &str,
        layer: &str,
        lookup_symbol: &str,
        kind: MissingSymbolKind,
        frame_pos: usize,
    ) {
        let key = (symbol.to_lowercase(), layer.to_lowercase());
        let entry = self
            .missing
            .entry(key)
            .or_insert_with(|| MissingSymbolInfo {
                symbol: symbol.to_string(),
                layer: layer.to_string(),
                lookup_symbol: lookup_symbol.to_string(),
                kind,
                frames: Vec::new(),
            });
        entry.frames.push(frame_pos);
    }

    pub fn has_missing(&self) -> bool {
        !self.missing.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolOverride {
    pub build: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolOverrideMap {
    entries: HashMap<String, SymbolOverride>,
}

impl SymbolOverrideMap {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, anim_symbol: &str, build: &str, replacement_symbol: &str) {
        self.entries.insert(
            anim_symbol.to_lowercase(),
            SymbolOverride {
                build: build.to_lowercase(),
                symbol: replacement_symbol.to_lowercase(),
            },
        );
    }

    pub fn get(&self, anim_symbol_lower: &str) -> Option<&SymbolOverride> {
        self.entries.get(anim_symbol_lower)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn parse_toml_string_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

pub fn parse_symbol_map(input: &str) -> Result<SymbolOverrideMap, String> {
    let mut map = SymbolOverrideMap::new();
    let mut current: Option<(Option<String>, Option<String>)> = None;
    let mut seen_symbol: Option<String> = None;

    for (line_no, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line_no = line_no + 1;
        if let Some(rest) = line.strip_prefix("[[") {
            if let Some((build, replacement)) = current.take() {
                flush_symbol_override(&mut map, seen_symbol.take(), build, replacement, line_no)?;
            }
            if !rest.trim_end().starts_with("symbol_override]]") {
                return Err(format!("line {line_no}: unknown table {line}"));
            }
            current = Some((None, None));
            continue;
        }
        if let Some((key_eq, _)) = line.split_once('=') {
            let key = key_eq.trim().to_lowercase();
            let value = line
                .split_once('=')
                .and_then(|(_, v)| parse_toml_string_value(v))
                .ok_or_else(|| format!("line {line_no}: expected quoted string value"))?;
            match key.as_str() {
                "symbol" => {
                    if seen_symbol.is_some() {
                        return Err(format!("line {line_no}: duplicate 'symbol'"));
                    }
                    seen_symbol = Some(value);
                }
                "build" => {
                    if let Some(entry) = current.as_mut() {
                        if entry.0.is_some() {
                            return Err(format!("line {line_no}: duplicate 'build'"));
                        }
                        entry.0 = Some(value);
                    } else {
                        return Err(format!(
                            "line {line_no}: 'build' outside [[symbol_override]]"
                        ));
                    }
                }
                "replace_with" | "replacement" => {
                    if let Some(entry) = current.as_mut() {
                        if entry.1.is_some() {
                            return Err(format!("line {line_no}: duplicate 'replace_with'"));
                        }
                        entry.1 = Some(value);
                    } else {
                        return Err(format!(
                            "line {line_no}: 'replace_with' outside [[symbol_override]]"
                        ));
                    }
                }
                _ => {
                    return Err(format!("line {line_no}: unknown key '{key}'"));
                }
            }
            continue;
        }
        return Err(format!("line {line_no}: unparseable line '{line}'"));
    }

    if let Some((build, replacement)) = current.take() {
        flush_symbol_override(
            &mut map,
            seen_symbol.take(),
            build,
            replacement,
            input.lines().count(),
        )?;
    } else if seen_symbol.is_some() {
        return Err("orphaned 'symbol' key".to_string());
    }

    Ok(map)
}

fn flush_symbol_override(
    map: &mut SymbolOverrideMap,
    seen_symbol: Option<String>,
    build: Option<String>,
    replacement: Option<String>,
    line_no: usize,
) -> Result<(), String> {
    let Some(symbol) = seen_symbol else {
        return Err(format!(
            "line {line_no}: [[symbol_override]] missing 'symbol'"
        ));
    };
    let Some(replacement) = replacement else {
        return Err(format!(
            "line {line_no}: [[symbol_override]] for '{symbol}' missing 'replace_with'"
        ));
    };
    map.insert(&symbol, build.as_deref().unwrap_or(""), replacement.trim());
    Ok(())
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
    pub ink_bounds: Option<BoundingBox>,
}

pub fn image_ink_bounds(image: &image::RgbaImage, canvas: &BoundingBox) -> Option<BoundingBox> {
    let raw = image.as_raw();
    let w = image.width() as usize;
    let h = image.height() as usize;
    let mut min_x = w;
    let mut max_x = 0usize;
    let mut min_y = h;
    let mut max_y = 0usize;
    let mut any = false;
    for y in 0..h {
        let row = &raw[y * w * 4..(y + 1) * w * 4];
        let mut row_min = w;
        let mut row_max = 0usize;
        let mut row_any = false;
        for x in 0..w {
            if row[x * 4 + 3] != 0 {
                if !row_any {
                    row_any = true;
                    row_min = x;
                }
                row_max = x;
            }
        }
        if row_any {
            any = true;
            min_y = min_y.min(y);
            max_y = y;
            min_x = min_x.min(row_min);
            max_x = max_x.max(row_max);
        }
    }
    if !any {
        return None;
    }
    Some(BoundingBox {
        left: canvas.left + min_x as f32,
        top: canvas.top + min_y as f32,
        right: canvas.left + (max_x + 1) as f32,
        bottom: canvas.top + (max_y + 1) as f32,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LookupFailure {
    NoSymbol,
    NoFrame,
}

fn find_symbol_frame_res<'a>(
    build_list: &[BuildRef<'a>],
    symbol_name_lower: &str,
    frame_num: u32,
    build_hint: Option<&str>,
) -> Result<&'a crate::build_file::BuildFrame, LookupFailure> {
    let mut saw_symbol = false;
    for br in build_list {
        if br.disabled_symbols.contains(symbol_name_lower) {
            continue;
        }
        if let Some(hint) = build_hint {
            if br.build.name.to_lowercase() != hint {
                continue;
            }
        }
        if let Some(&sym_idx) = br.build.symbol_index.get(symbol_name_lower) {
            if let Some(symbol) = br.build.symbols.get(sym_idx) {
                saw_symbol = true;
                if let Some(frame) = symbol.frame_for_anim_frame(frame_num) {
                    return Ok(frame);
                }
            }
        }
    }
    Err(if saw_symbol {
        LookupFailure::NoFrame
    } else {
        LookupFailure::NoSymbol
    })
}

#[cfg(test)]
fn find_symbol_frame<'a>(
    build_list: &[BuildRef<'a>],
    symbol_name_lower: &str,
    frame_num: u32,
    build_hint: Option<&str>,
) -> Option<&'a crate::build_file::BuildFrame> {
    find_symbol_frame_res(build_list, symbol_name_lower, frame_num, build_hint).ok()
}

#[derive(Clone)]
pub struct ElementData {
    pub sprite: std::sync::Arc<image::RgbaImage>,
    pub spans: Option<std::sync::Arc<crate::build_file::SpriteSpans>>,
    pub ink: Option<InkRect>,
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

/// Transform of one animation element, used as the parent transform for a
/// follower FX entity (`Follower:FollowSymbol`).
///
/// Attachment-point symbols such as `cape_front_swap` have no build sprite but
/// carry a per-frame transform in `anim.bin`; this is that transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorTransform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
    /// The anchor element's `z_index`; used to splice the follower into the
    /// parent's draw order.
    pub z_index: f32,
}

impl AnchorTransform {
    fn from_element(element: &AnimElement) -> Self {
        Self {
            a: element.a,
            b: element.b,
            c: element.c,
            d: element.d,
            tx: element.tx,
            ty: element.ty,
            z_index: element.z_index,
        }
    }

    /// Compose this anchor transform with a follower element's local
    /// transform.
    ///
    /// The renderer maps a local point `p` as `world = t + M·p` with
    /// `M = [[a, c], [b, d]]`. A follower composes as `M' = A·M` and
    /// `t' = T + A·t + offset`, matching the engine's `FollowSymbol`
    /// semantics (verified against Klei's baked `scrapbook` animation).
    ///
    /// `offset` is the Lua `FollowSymbol` x/y offset in animation units.
    pub fn compose_element(&self, element: &ElementData, offset: (f32, f32)) -> ElementData {
        let mut out = element.clone();
        out.a = self.a * element.a + self.c * element.b;
        out.b = self.b * element.a + self.d * element.b;
        out.c = self.a * element.c + self.c * element.d;
        out.d = self.b * element.c + self.d * element.d;
        out.tx = self.tx + offset.0 + self.a * element.tx + self.c * element.ty;
        out.ty = self.ty + offset.1 + self.b * element.tx + self.d * element.ty;
        out
    }
}

/// Find the transform of an attachment-point symbol in one animation frame.
/// `symbol_lower` must already be lowercase.
pub fn find_anchor_transform(frame: &AnimFrame, symbol_lower: &str) -> Option<AnchorTransform> {
    frame
        .elements
        .iter()
        .find(|element| element.symbol_lower == symbol_lower)
        .map(AnchorTransform::from_element)
}

/// Visual modifiers applied to a followed FX entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowStyle {
    pub offset: (f32, f32),
    /// `AnimState:SetFinalOffset`: >0 draws the whole FX in front of the
    /// parent, <0 behind it.
    pub final_offset: i32,
    /// Literal `AnimState:SetMultColour(r, g, b, a)`.
    pub mult_colour: Option<[f32; 4]>,
    /// Literal `AnimState:SetAddColour(r, g, b, a)`.
    pub add_colour: Option<[f32; 4]>,
}

impl Default for FollowStyle {
    fn default() -> Self {
        Self {
            offset: (0.0, 0.0),
            final_offset: 0,
            mult_colour: None,
            add_colour: None,
        }
    }
}

impl FollowStyle {
    fn z_key(&self, anchor_z: f32) -> f32 {
        anchor_z + self.final_offset as f32 * 1000.0
    }
}

/// Apply the follower's `SetMultColour` / `SetAddColour` to an element.
///
/// The sprite is tinted once per element/frame; alpha changes invalidate the
/// precomputed row spans so the renderer falls back to per-pixel compositing.
pub fn apply_follow_tint(element: &mut ElementData, mult: Option<[f32; 4]>, add: Option<[f32; 4]>) {
    let mult = mult.unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let add_rgb = add.map(|a| [a[0], a[1], a[2]]).unwrap_or([0.0; 3]);
    if mult == [1.0, 1.0, 1.0, 1.0] && add_rgb == [0.0; 3] {
        return;
    }
    let src = element.sprite.as_ref();
    let (width, height) = (src.width(), src.height());
    let mut tinted = image::RgbaImage::new(width, height);
    for (dst, px) in tinted.pixels_mut().zip(src.pixels()) {
        let channel = |index: usize, factor: f32, addend: f32| {
            ((px[index] as f32 / 255.0 * factor + addend) * 255.0).clamp(0.0, 255.0) as u8
        };
        *dst = image::Rgba([
            channel(0, mult[0], add_rgb[0]),
            channel(1, mult[1], add_rgb[1]),
            channel(2, mult[2], add_rgb[2]),
            ((px[3] as f32 * mult[3]).clamp(0.0, 255.0)) as u8,
        ]);
    }
    element.sprite = std::sync::Arc::new(tinted);
    if mult[3] < 1.0 {
        element.spans = None;
        element.ink = None;
    }
}

/// Splice composed follower elements into the parent's element stream.
///
/// `main` and `follower` are both z-ascending. Follower elements take the
/// anchor's `z_index` (plus the FX entity's `SetFinalOffset` bias), so a
/// stable sort places them after parent elements at the same z (a parented
/// follower draws after its parent) while preserving the follower's own order.
pub fn merge_followed_elements(
    main: Vec<(f32, ElementData)>,
    anchor: &AnchorTransform,
    follower: &[(f32, ElementData)],
    style: &FollowStyle,
) -> Vec<(f32, ElementData)> {
    let z = style.z_key(anchor.z_index);
    let mut merged = main;
    merged.extend(follower.iter().map(|(_, element)| {
        let mut composed = anchor.compose_element(element, style.offset);
        apply_follow_tint(&mut composed, style.mult_colour, style.add_colour);
        (z, composed)
    }));
    merged.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

pub fn compute_frame_elements(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    scale: f32,
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> Option<Vec<ElementData>> {
    compute_frame_elements_with_overrides(
        anim_frame,
        build_list,
        scale,
        disabled_elements,
        disabled_symbols,
        &SymbolOverrideMap::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn compute_frame_elements_with_overrides(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    scale: f32,
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
    overrides: &SymbolOverrideMap,
) -> Option<Vec<ElementData>> {
    let options = RenderOptions::new(scale, (0.0, 0.0))
        .with_disabled_elements(disabled_elements.clone())
        .with_disabled_symbols(disabled_symbols.clone())
        .with_overrides(overrides.clone());
    compute_frame_elements_core(anim_frame, build_list, &options, 0, None)
}

/// Like [`compute_frame_elements_with_overrides`] but keeps each element's
/// animation `z_index`, for splicing follower FX elements into the parent's
/// draw order.
#[allow(clippy::too_many_arguments)]
pub fn compute_frame_elements_with_z(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    scale: f32,
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
    overrides: &SymbolOverrideMap,
) -> Vec<(f32, ElementData)> {
    let options = RenderOptions::new(scale, (0.0, 0.0))
        .with_disabled_elements(disabled_elements.clone())
        .with_disabled_symbols(disabled_symbols.clone())
        .with_overrides(overrides.clone());
    compute_frame_elements_z_core(anim_frame, build_list, &options, 0, None)
}

pub fn compute_frame_elements_opts(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    options: &RenderOptions,
) -> (Option<Vec<ElementData>>, RenderDiagnostics) {
    let mut diagnostics = RenderDiagnostics::default();
    let result =
        compute_frame_elements_core(anim_frame, build_list, options, 0, Some(&mut diagnostics));
    (result, diagnostics)
}

fn compute_frame_elements_core(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    options: &RenderOptions,
    frame_pos: usize,
    diagnostics: Option<&mut RenderDiagnostics>,
) -> Option<Vec<ElementData>> {
    let elements =
        compute_frame_elements_z_core(anim_frame, build_list, options, frame_pos, diagnostics);
    if elements.is_empty() {
        None
    } else {
        Some(elements.into_iter().map(|(_, element)| element).collect())
    }
}

fn compute_frame_elements_z_core(
    anim_frame: &AnimFrame,
    build_list: &[BuildRef<'_>],
    options: &RenderOptions,
    frame_pos: usize,
    mut diagnostics: Option<&mut RenderDiagnostics>,
) -> Vec<(f32, ElementData)> {
    let mut elements_data: Vec<(f32, ElementData)> = Vec::new();
    for element in &anim_frame.elements {
        if options.disabled_symbols.contains(&element.symbol_lower)
            || options.disabled_layers.contains(&element.layer_lower)
            || (!options.disabled_elements.is_empty()
                && options
                    .disabled_elements
                    .contains(&(element.symbol_lower.clone(), element.layer_lower.clone())))
        {
            continue;
        }
        let (build_hint, lookup_symbol) = match options.overrides.get(&element.symbol_lower) {
            Some(o) if o.build.is_empty() => (None, o.symbol.as_str()),
            Some(o) => (Some(o.build.as_str()), o.symbol.as_str()),
            None => (None, element.symbol_lower.as_str()),
        };
        match find_symbol_frame_res(build_list, lookup_symbol, element.frame_num, build_hint) {
            Ok(bf) => {
                let Some(sprite) = &bf.image else {
                    if let Some(diag) = diagnostics.as_deref_mut() {
                        diag.record(
                            &element.symbol,
                            &element.layer_name,
                            lookup_symbol,
                            MissingSymbolKind::NoImage,
                            frame_pos,
                        );
                    }
                    continue;
                };
                elements_data.push((
                    element.z_index,
                    ElementData {
                        sprite: sprite.clone(),
                        spans: bf.spans.clone(),
                        ink: bf.spans.as_ref().and_then(|s| s.ink),
                        bf_x: bf.x,
                        bf_y: bf.y,
                        dest_x: bf.dest_x,
                        dest_y: bf.dest_y,
                        canvas_w: bf.canvas_w,
                        canvas_h: bf.canvas_h,
                        a: element.a * options.scale,
                        b: element.b * options.scale,
                        c: element.c * options.scale,
                        d: element.d * options.scale,
                        tx: element.tx,
                        ty: element.ty,
                    },
                ));
            }
            Err(failure) => {
                if let Some(diag) = diagnostics.as_deref_mut() {
                    let kind = match failure {
                        LookupFailure::NoSymbol => MissingSymbolKind::NoSymbol,
                        LookupFailure::NoFrame => MissingSymbolKind::NoFrame,
                    };
                    diag.record(
                        &element.symbol,
                        &element.layer_name,
                        lookup_symbol,
                        kind,
                        frame_pos,
                    );
                }
            }
        }
    }
    elements_data
}

fn extend_bbox(bbox: &mut (f32, f32, f32, f32), x: f32, y: f32) {
    bbox.0 = bbox.0.min(x);
    bbox.1 = bbox.1.min(y);
    bbox.2 = bbox.2.max(x);
    bbox.3 = bbox.3.max(y);
}

fn compute_element_bounds(
    elem: &ElementData,
    scale: f32,
    offset: (f32, f32),
    kind: BoundsKind,
) -> Option<(f32, f32, f32, f32)> {
    let w = elem.sprite.width() as f32;
    let h = elem.sprite.height() as f32;
    if w == 0.0 || h == 0.0 {
        return None;
    }
    let center_x = elem.bf_x + (elem.dest_x as f32 + w / 2.0 - elem.canvas_w / 2.0);
    let center_y = elem.bf_y + (elem.dest_y as f32 + h / 2.0 - elem.canvas_h / 2.0);
    let elem_left = elem.tx * scale + offset.0 + center_x * elem.a + center_y * elem.c;
    let elem_top = elem.ty * scale + offset.1 + center_x * elem.b + center_y * elem.d;

    let map = |px: f32, py: f32| -> (f32, f32) {
        let dx = px - w / 2.0;
        let dy = py - h / 2.0;
        (
            elem_left + dx * elem.a + dy * elem.c,
            elem_top + dx * elem.b + dy * elem.d,
        )
    };

    let mut bbox = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    let mut any = false;

    if kind == BoundsKind::Ink {
        let crop_x_lo = (-elem.dest_x).max(0) as f32;
        let crop_x_hi = (elem.canvas_w as i64 - elem.dest_x)
            .min(elem.sprite.width() as i64)
            .max(0) as f32;
        let crop_y_lo = (-elem.dest_y).max(0) as f32;
        let crop_y_hi = (elem.canvas_h as i64 - elem.dest_y)
            .min(elem.sprite.height() as i64)
            .max(0) as f32;

        if let Some(spans) = &elem.spans {
            for (y, row) in spans.rows.iter().enumerate() {
                let Some(rs) = row else { continue };
                let sx0 = (rs.start as f32).max(crop_x_lo);
                let sx1 = (rs.end as f32).min(crop_x_hi);
                let sy0 = (y as f32).max(crop_y_lo);
                let sy1 = (y as f32 + 1.0).min(crop_y_hi);
                if sx0 >= sx1 || sy0 >= sy1 {
                    continue;
                }
                any = true;
                for (px, py) in [(sx0, sy0), (sx1, sy0), (sx0, sy1), (sx1, sy1)] {
                    let (x, y) = map(px, py);
                    extend_bbox(&mut bbox, x, y);
                }
            }
        }

        if !any {
            let sw = elem.sprite.width() as i64;
            let sh = elem.sprite.height() as i64;
            let (x0, y0, x1, y1) = match elem.ink {
                Some(ink) => (
                    ink.min_x as i64,
                    ink.min_y as i64,
                    ink.max_x as i64,
                    ink.max_y as i64,
                ),
                None => (0, 0, sw, sh),
            };
            let x0 = x0.max(crop_x_lo as i64);
            let x1 = x1.min(crop_x_hi as i64);
            let y0 = y0.max(crop_y_lo as i64);
            let y1 = y1.min(crop_y_hi as i64);
            if x0 >= x1 || y0 >= y1 {
                return None;
            }
            any = true;
            for (px, py) in [
                (x0 as f32, y0 as f32),
                (x1 as f32, y0 as f32),
                (x0 as f32, y1 as f32),
                (x1 as f32, y1 as f32),
            ] {
                let (x, y) = map(px, py);
                extend_bbox(&mut bbox, x, y);
            }
        }
    } else {
        any = true;
        for (px, py) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
            let (x, y) = map(px, py);
            extend_bbox(&mut bbox, x, y);
        }
    }

    if !any {
        return None;
    }
    Some(bbox)
}

pub fn compute_bounds_from_elements(
    elements_data: &[ElementData],
    scale: f32,
    offset: (f32, f32),
) -> Option<BoundingBox> {
    compute_bounds_from_elements_kind(elements_data, scale, offset, BoundsKind::Rect)
}

pub fn compute_bounds_from_elements_kind(
    elements_data: &[ElementData],
    scale: f32,
    offset: (f32, f32),
    kind: BoundsKind,
) -> Option<BoundingBox> {
    let mut top = f32::INFINITY;
    let mut left = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    let mut right = f32::NEG_INFINITY;

    for elem in elements_data {
        let Some((el, et, er, eb)) = compute_element_bounds(elem, scale, offset, kind) else {
            continue;
        };
        left = left.min(el);
        top = top.min(et);
        right = right.max(er);
        bottom = bottom.max(eb);
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
    pub ink_bounds: Option<BoundingBox>,
}

pub fn prepare_animation_frames(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> (Option<BoundingBox>, Vec<Option<PreparedFrame>>) {
    prepare_animation_frames_with_overrides(
        frames,
        build_list,
        scale,
        offset,
        disabled_elements,
        disabled_symbols,
        &SymbolOverrideMap::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_animation_frames_with_overrides(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
    overrides: &SymbolOverrideMap,
) -> (Option<BoundingBox>, Vec<Option<PreparedFrame>>) {
    let options = RenderOptions::new(scale, offset)
        .with_disabled_elements(disabled_elements.clone())
        .with_disabled_symbols(disabled_symbols.clone())
        .with_overrides(overrides.clone());
    let (bounds, prepared, _diagnostics) =
        prepare_animation_frames_opts(frames, build_list, &options);
    (bounds, prepared)
}

pub fn prepare_animation_frames_opts(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    options: &RenderOptions,
) -> (
    Option<BoundingBox>,
    Vec<Option<PreparedFrame>>,
    RenderDiagnostics,
) {
    let mut prepared: Vec<Option<PreparedFrame>> = Vec::with_capacity(frames.len());
    let mut diagnostics = RenderDiagnostics::default();
    let mut union_top = f32::INFINITY;
    let mut union_left = f32::INFINITY;
    let mut union_bottom = f32::NEG_INFINITY;
    let mut union_right = f32::NEG_INFINITY;

    for (frame_pos, frame) in frames.iter().enumerate() {
        let elements = compute_frame_elements_core(
            frame,
            build_list,
            options,
            frame_pos,
            Some(&mut diagnostics),
        );
        if let Some(elements) = elements {
            if let Some(bounds) = compute_bounds_from_elements_kind(
                &elements,
                options.scale,
                options.offset,
                options.bounds_kind,
            ) {
                let ink_bounds = compute_bounds_from_elements_kind(
                    &elements,
                    options.scale,
                    options.offset,
                    BoundsKind::Ink,
                );
                union_left = union_left.min(bounds.left);
                union_top = union_top.min(bounds.top);
                union_right = union_right.max(bounds.right);
                union_bottom = union_bottom.max(bounds.bottom);
                prepared.push(Some(PreparedFrame {
                    elements,
                    bounds,
                    ink_bounds,
                }));
                continue;
            }
        }
        diagnostics.skipped_frames.push(frame_pos);
        prepared.push(None);
    }

    let union_bounds = stable_union_bounds(union_left, union_top, union_right, union_bottom);

    (union_bounds, prepared, diagnostics)
}

fn stable_union_bounds(
    union_left: f32,
    union_top: f32,
    union_right: f32,
    union_bottom: f32,
) -> Option<BoundingBox> {
    if union_left.is_infinite() || union_right.is_infinite() {
        return None;
    }
    let frac_x = union_left - union_left.floor();
    let frac_y = union_top - union_top.floor();
    Some(BoundingBox {
        left: (union_left - 2.0).floor() + frac_x,
        top: (union_top - 2.0).floor() + frac_y,
        right: (union_right + 2.0).ceil() + frac_x,
        bottom: (union_bottom + 2.0).ceil() + frac_y,
    })
}

/// Stable canvas box for a set of already-composed element frames.
///
/// Mirrors the union margin used by [`prepare_animation_frames_opts`] so the
/// follow-FX compositor can produce the same canvas as the plain path.
pub fn union_bounds_from_element_frames<'a, I>(
    frames: I,
    scale: f32,
    offset: (f32, f32),
) -> Option<BoundingBox>
where
    I: IntoIterator<Item = &'a [ElementData]>,
{
    let mut left = f32::INFINITY;
    let mut top = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for elements in frames {
        let Some(bounds) =
            compute_bounds_from_elements_kind(elements, scale, offset, BoundsKind::Rect)
        else {
            continue;
        };
        left = left.min(bounds.left);
        top = top.min(bounds.top);
        right = right.max(bounds.right);
        bottom = bottom.max(bounds.bottom);
    }
    stable_union_bounds(left, top, right, bottom)
}

pub fn compute_animation_bounds(
    frames: &[AnimFrame],
    build_list: &[BuildRef<'_>],
    scale: f32,
    offset: (f32, f32),
    disabled_elements: &HashSet<(String, String)>,
    disabled_symbols: &HashSet<String>,
) -> Option<BoundingBox> {
    prepare_animation_frames_with_overrides(
        frames,
        build_list,
        scale,
        offset,
        disabled_elements,
        disabled_symbols,
        &SymbolOverrideMap::new(),
    )
    .0
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
    let ink_bounds = image_ink_bounds(&canvas, bounds);
    Some(RenderedFrame {
        image: canvas,
        ink_bounds,
    })
}

#[cfg(feature = "parallel")]
const BAND_ROWS: usize = 64;

#[cfg(feature = "parallel")]
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
    let ink_bounds = image_ink_bounds(&canvas, bounds);
    Some(RenderedFrame {
        image: canvas,
        ink_bounds,
    })
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

    fn transform_elem(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> ElementData {
        ElementData {
            sprite: std::sync::Arc::new(image::RgbaImage::new(4, 4)),
            spans: None,
            ink: None,
            bf_x: 0.0,
            bf_y: 0.0,
            dest_x: 0,
            dest_y: 0,
            canvas_w: 4.0,
            canvas_h: 4.0,
            a,
            b,
            c,
            d,
            tx,
            ty,
        }
    }

    #[test]
    fn anchor_composition_matches_scrapbook_bake() {
        let anchor = AnchorTransform {
            a: -0.5940857,
            b: -0.01800537,
            c: 0.02183533,
            d: -0.7189637,
            tx: 2.649996,
            ty: -267.6501,
            z_index: 1.4285715,
        };
        let local = transform_elem(0.9781952, 0.0, 0.0, 1.0, -0.95, -0.8);
        let composed = anchor.compose_element(&local, (0.0, 0.0));
        assert!(
            (composed.tx - 3.196909).abs() < 0.001,
            "tx = {}",
            composed.tx
        );
        assert!(
            (composed.ty + 267.0578).abs() < 0.001,
            "ty = {}",
            composed.ty
        );
        assert!((composed.a + 0.5811318).abs() < 0.001, "a = {}", composed.a);
        assert!(
            (composed.b + 0.01761277).abs() < 0.001,
            "b = {}",
            composed.b
        );
        assert!(
            (composed.c - 0.02183533).abs() < 0.001,
            "c = {}",
            composed.c
        );
        assert!((composed.d + 0.7189637).abs() < 0.001, "d = {}", composed.d);
    }

    #[test]
    fn compose_applies_symbol_offset_in_anim_space() {
        let anchor = AnchorTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 10.0,
            ty: 20.0,
            z_index: 0.0,
        };
        let local = transform_elem(1.0, 0.0, 0.0, 1.0, 3.0, 4.0);
        let composed = anchor.compose_element(&local, (5.0, -6.0));
        assert_eq!((composed.tx, composed.ty), (18.0, 18.0));
    }

    #[test]
    fn merge_places_follower_after_parent_at_anchor_z() {
        let anchor = AnchorTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
            z_index: 0.5,
        };
        let main = vec![
            (0.0, transform_elem(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)),
            (1.0, transform_elem(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)),
        ];
        let follower = vec![
            (0.0, transform_elem(1.0, 0.0, 0.0, 1.0, 1.0, 0.0)),
            (0.0, transform_elem(1.0, 0.0, 0.0, 1.0, 2.0, 0.0)),
        ];
        let merged = merge_followed_elements(main, &anchor, &follower, &FollowStyle::default());
        let zs: Vec<f32> = merged.iter().map(|(z, _)| *z).collect();
        assert_eq!(zs, vec![0.0, 0.5, 0.5, 1.0]);
        assert_eq!(merged[1].1.tx, 1.0);
        assert_eq!(merged[2].1.tx, 2.0);
    }

    #[test]
    fn merge_honours_final_offset() {
        let anchor = AnchorTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
            z_index: 0.5,
        };
        let follower = vec![(0.0, transform_elem(1.0, 0.0, 0.0, 1.0, 0.0, 0.0))];
        let main = || {
            vec![
                (0.0, transform_elem(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)),
                (1.0, transform_elem(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)),
            ]
        };

        let front = merge_followed_elements(
            main(),
            &anchor,
            &follower,
            &FollowStyle {
                final_offset: 1,
                ..Default::default()
            },
        );
        assert_eq!(front.last().unwrap().0, 1000.5);

        let behind = merge_followed_elements(
            main(),
            &anchor,
            &follower,
            &FollowStyle {
                final_offset: -1,
                ..Default::default()
            },
        );
        assert_eq!(behind.first().unwrap().0, -999.5);
    }

    #[test]
    fn follow_tint_multiplies_and_adds() {
        let mut element = transform_elem(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        element.sprite = std::sync::Arc::new(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([100, 200, 50, 255]),
        ));
        apply_follow_tint(&mut element, Some([0.5, 0.5, 0.5, 0.5]), None);
        let px = element.sprite.get_pixel(0, 0);
        assert_eq!(*px, image::Rgba([50, 100, 25, 127]));
        assert!(element.spans.is_none());

        apply_follow_tint(&mut element, None, Some([0.1, 0.2, 0.0, 1.0]));
        let px = element.sprite.get_pixel(0, 0);
        assert_eq!(px[0], 75);
        assert_eq!(px[1], 151);
        assert_eq!(px[2], 25);
    }

    #[test]
    fn find_anchor_transform_reads_ghost_symbol() {
        let frame = make_anim_frame(vec![AnimElement {
            z_index: 0.25,
            symbol: "cape_front_swap".into(),
            symbol_lower: "cape_front_swap".into(),
            frame_num: 0,
            layer_name: "front".into(),
            layer_lower: "front".into(),
            a: 1.1,
            b: 0.0,
            c: 0.0,
            d: 1.2,
            tx: 7.0,
            ty: 9.0,
        }]);
        let anchor = find_anchor_transform(&frame, "cape_front_swap").expect("anchor");
        assert_eq!(anchor.z_index, 0.25);
        assert_eq!((anchor.tx, anchor.ty), (7.0, 9.0));
        assert!(find_anchor_transform(&frame, "missing").is_none());
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
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
        let result = find_symbol_frame(&build_list, "sym", 0, None);
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
        let result = find_symbol_frame(&build_list, "nonexistent", 0, None);
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
        let result = find_symbol_frame(&build_list, "sym", 99, None);
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
        let result = find_symbol_frame(&build_list, "sym", 0, None);
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
        assert!(compute_frame_elements(
            &anim_frame,
            &build_list,
            1.0,
            &HashSet::new(),
            &HashSet::new()
        )
        .is_none());
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
            layer_lower: "layer".into(),
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
            ink: None,
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
    fn compute_bounds_from_elements_ink_uses_spans() {
        let sprite = image::RgbaImage::new(10, 10);
        let elements = vec![ElementData {
            sprite: std::sync::Arc::new(sprite),
            spans: None,
            ink: Some(InkRect {
                min_x: 2,
                min_y: 3,
                max_x: 8,
                max_y: 9,
            }),
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
        let rect = compute_bounds_from_elements(&elements, 1.0, (0.0, 0.0)).unwrap();
        assert_eq!(rect.left, 45.0);
        assert_eq!(rect.top, 45.0);
        assert_eq!(rect.right, 55.0);
        assert_eq!(rect.bottom, 55.0);

        let ink =
            compute_bounds_from_elements_kind(&elements, 1.0, (0.0, 0.0), BoundsKind::Ink).unwrap();
        assert_eq!(ink.left, 47.0);
        assert_eq!(ink.top, 48.0);
        assert_eq!(ink.right, 53.0);
        assert_eq!(ink.bottom, 54.0);
    }

    #[test]
    fn render_options_disable_layers_case_insensitive() {
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
            layer_name: "HAT_Layer".into(),
            layer_lower: "hat_layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 50.0,
            ty: 50.0,
        };
        let anim_frame = make_anim_frame(vec![elem]);

        let plain = RenderOptions::new(1.0, (0.0, 0.0));
        assert!(compute_frame_elements_core(&anim_frame, &build_list, &plain, 0, None).is_some());

        let hidden = RenderOptions::new(1.0, (0.0, 0.0))
            .with_disabled_layers(HashSet::from(["HAT_LAYER".to_string()]));
        assert!(
            compute_frame_elements_core(&anim_frame, &build_list, &hidden, 0, None).is_none(),
            "disabled layer should be matched case-insensitively"
        );
    }

    #[test]
    fn diagnostics_record_missing_symbol_and_frame() {
        let build = make_build_with_symbol("body", 0, image::RgbaImage::new(10, 10));
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: &build,
            disabled_symbols: &empty,
        }];
        let make_elem = |symbol: &str, frame_num: u32| AnimElement {
            z_index: 0.0,
            symbol: symbol.into(),
            symbol_lower: symbol.to_lowercase(),
            frame_num,
            layer_name: "Layer".into(),
            layer_lower: "layer".into(),
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        };
        let frames = vec![
            make_anim_frame(vec![make_elem("nope", 0)]),
            make_anim_frame(vec![make_elem("body", 99)]),
        ];
        let options = RenderOptions::new(1.0, (0.0, 0.0));
        let (_, prepared, diag) = prepare_animation_frames_opts(&frames, &build_list, &options);
        assert!(prepared.iter().all(|p| p.is_none()));
        assert_eq!(diag.skipped_frames, vec![0, 1]);
        assert_eq!(diag.missing.len(), 2);
        let nope = diag
            .missing
            .get(&("nope".to_string(), "layer".to_string()))
            .unwrap();
        assert_eq!(nope.kind, MissingSymbolKind::NoSymbol);
        assert_eq!(nope.frames, vec![0]);
        let body = diag
            .missing
            .get(&("body".to_string(), "layer".to_string()))
            .unwrap();
        assert_eq!(body.kind, MissingSymbolKind::NoFrame);
        assert_eq!(body.frames, vec![1]);
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
    fn ink_bounds_match_rendered_bbox_tallbird() {
        use crate::archive::parse_zip;
        use crate::atlas::{decode_atlas_images_from_tex, split_atlas};

        let data = std::fs::read("data/anim/ds_tallbird_basic.zip").unwrap();
        let mut archive = parse_zip(&data).unwrap();
        let atlas_images = decode_atlas_images_from_tex(
            &archive.build.as_ref().unwrap().atlases,
            archive.tex_files(),
        );
        split_atlas(archive.build.as_mut().unwrap(), &atlas_images).unwrap();

        let anim = archive.anim.as_ref().unwrap();
        let empty = HashSet::new();
        let build_list = vec![BuildRef {
            build: archive.build.as_ref().unwrap(),
            disabled_symbols: &empty,
        }];
        let bank = anim
            .banks
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case("tallbird"))
            .unwrap();
        let animation = bank
            .animations
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case("idle_side"))
            .unwrap();

        let options = RenderOptions::new(1.0, (0.0, 0.0));
        let (union, prepared, _) =
            prepare_animation_frames_opts(&animation.frames, &build_list, &options);
        let union = union.unwrap();

        let mut compared = 0usize;
        let mut rect_vs_ink_bottom: Vec<f32> = Vec::new();
        for pf in prepared.iter().flatten() {
            let rendered =
                render_frame_with_elements(&pf.elements, &union, 1.0, (0.0, 0.0)).unwrap();
            let predicted = pf.ink_bounds.as_ref().unwrap();
            let actual = rendered.ink_bounds.as_ref().unwrap();
            assert!(
                (predicted.left - actual.left).abs() <= 2.0
                    && (predicted.top - actual.top).abs() <= 2.0
                    && (predicted.right - actual.right).abs() <= 2.0
                    && (predicted.bottom - actual.bottom).abs() <= 2.0,
                "ink prediction {predicted:?} should match rendered {actual:?} within rasterization tolerance"
            );
            rect_vs_ink_bottom.push(pf.bounds.bottom - actual.bottom);
            compared += 1;
        }
        assert_eq!(compared, 40);
        rect_vs_ink_bottom.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = rect_vs_ink_bottom[rect_vs_ink_bottom.len() / 2];
        assert!(
            median > 10.0,
            "rect bounds should be measurably looser than ink (median {median})"
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
        assert!(compute_animation_bounds(
            &frames,
            &build_list,
            1.0,
            (0.0, 0.0),
            &HashSet::new(),
            &HashSet::new()
        )
        .is_none());
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
            layer_lower: "layer".into(),
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
            layer_lower: "layer".into(),
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
            layer_lower: "layer".into(),
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
            layer_lower: "layer".into(),
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
            layer_lower: "layer".into(),
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
    #[ignore = "requires local DST game data (data/anim symlink)"]
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
        let has_opaque = rendered
            .image
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| px[3] > 0);
        assert!(
            has_opaque,
            "rendered image should have non-transparent pixels"
        );
    }

    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
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
            layer_lower: "layer".into(),
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
            layer_lower: "layer".into(),
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
    #[ignore = "requires local DST game data (data/anim symlink)"]
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

    #[cfg(feature = "parallel")]
    #[test]
    #[ignore = "requires local DST game data (data/anim symlink)"]
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
    #[ignore = "requires local DST game data (data/anim symlink)"]
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
    #[ignore = "requires local DST game data (data/anim symlink)"]
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
            for (i, px) in img.as_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
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

    #[test]
    fn parse_symbol_map_basic() {
        let input = r#"
# comment
[[symbol_override]]
symbol = "swap_object"
build = "Swap_Axe"
replace_with = "swap_axe"

[[symbol_override]]
symbol = "fx_swap"
build = "abigail_vial_fx"
replace_with = 'fx_regen_02'
"#;
        let map = parse_symbol_map(input).unwrap();
        assert_eq!(map.len(), 2);
        let o = map.get("swap_object").unwrap();
        assert_eq!(o.build, "swap_axe");
        assert_eq!(o.symbol, "swap_axe");
        let o = map.get("fx_swap").unwrap();
        assert_eq!(o.build, "abigail_vial_fx");
        assert_eq!(o.symbol, "fx_regen_02");
    }

    #[test]
    fn parse_symbol_map_empty_build() {
        let input = "[[symbol_override]]\nsymbol = \"a\"\nreplace_with = \"b\"\n";
        let map = parse_symbol_map(input).unwrap();
        let o = map.get("a").unwrap();
        assert_eq!(o.build, "");
        assert_eq!(o.symbol, "b");
    }

    #[test]
    fn parse_symbol_map_errors() {
        assert!(parse_symbol_map("[[symbol_override]]\nsymbol = \"a\"\n").is_err());
        assert!(parse_symbol_map("[[other]]\n").is_err());
        assert!(parse_symbol_map("[[symbol_override]]\nsymbol = \"a\"\nfoo = \"x\"\n").is_err());
        assert!(parse_symbol_map("[[symbol_override]]\nsymbol = \"a\"\nsymbol = \"b\"\n").is_err());
        assert!(parse_symbol_map("symbol = \"a\"\n").is_err());
        assert!(parse_symbol_map("[[symbol_override]]\nsymbol = a\n").is_err());
        assert!(parse_symbol_map("[[symbol_override]]\nreplace_with = \"a\"\n").is_err());
    }

    #[test]
    fn symbol_override_map_insert_lowercases() {
        let mut map = SymbolOverrideMap::new();
        map.insert("SWAP_OBJECT", "Swap_Axe", "Swap_Axe");
        let o = map.get("swap_object").unwrap();
        assert_eq!(o.build, "swap_axe");
        assert_eq!(o.symbol, "swap_axe");
    }

    #[test]
    fn find_symbol_frame_override_hint_selects_named_build() {
        let empty = HashSet::new();
        let mut b1 = make_build_with_symbol("sym", 0, image::RgbaImage::new(4, 4));
        b1.name = "build_a".into();
        let mut b2 = make_build_with_symbol("other", 0, image::RgbaImage::new(6, 6));
        b2.name = "build_b".into();
        let build_list = vec![
            BuildRef {
                build: &b1,
                disabled_symbols: &empty,
            },
            BuildRef {
                build: &b2,
                disabled_symbols: &empty,
            },
        ];
        assert!(find_symbol_frame(&build_list, "other", 0, Some("build_b")).is_some());
        assert!(find_symbol_frame(&build_list, "other", 0, Some("build_a")).is_none());
        assert!(find_symbol_frame(&build_list, "other", 0, Some("build_c")).is_none());
        assert!(find_symbol_frame(&build_list, "other", 0, None).is_some());
    }

    #[test]
    fn compute_frame_elements_with_overrides_replaces_symbol() {
        let empty_symbols = HashSet::new();
        let empty_elements = HashSet::new();

        let element = AnimElement {
            z_index: 0.0,
            symbol: "swap_object".into(),
            symbol_lower: "swap_object".into(),
            layer_name: "LAYER".into(),
            layer_lower: "layer".into(),
            frame_num: 0,
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        };
        let anim_frame = make_anim_frame(vec![element]);

        let mut body = make_build_with_symbol("body", 0, image::RgbaImage::new(4, 4));
        body.name = "body_build".into();
        let mut weapon = make_build_with_symbol("axe", 0, image::RgbaImage::new(8, 8));
        weapon.name = "weapon_build".into();
        let build_list = vec![
            BuildRef {
                build: &body,
                disabled_symbols: &empty_symbols,
            },
            BuildRef {
                build: &weapon,
                disabled_symbols: &empty_symbols,
            },
        ];

        let without = compute_frame_elements_with_overrides(
            &anim_frame,
            &build_list,
            1.0,
            &empty_elements,
            &empty_symbols,
            &SymbolOverrideMap::new(),
        );
        assert!(without.is_none());

        let mut overrides = SymbolOverrideMap::new();
        overrides.insert("swap_object", "weapon_build", "axe");
        let with = compute_frame_elements_with_overrides(
            &anim_frame,
            &build_list,
            1.0,
            &empty_elements,
            &empty_symbols,
            &overrides,
        )
        .unwrap();
        assert_eq!(with.len(), 1);
        assert_eq!(with[0].sprite.width(), 8);
        assert_eq!(with[0].sprite.height(), 8);
    }

    #[test]
    fn compute_frame_elements_with_overrides_same_name_replacement() {
        let empty_symbols = HashSet::new();
        let empty_elements = HashSet::new();

        let element = AnimElement {
            z_index: 0.0,
            symbol: "swap_object".into(),
            symbol_lower: "swap_object".into(),
            layer_name: "LAYER".into(),
            layer_lower: "layer".into(),
            frame_num: 0,
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        };
        let anim_frame = make_anim_frame(vec![element]);

        let weapon = make_build_with_symbol("axe", 0, image::RgbaImage::new(8, 8));
        let build_list = vec![BuildRef {
            build: &weapon,
            disabled_symbols: &empty_symbols,
        }];

        let mut overrides = SymbolOverrideMap::new();
        overrides.insert("swap_object", "", "axe");
        let with = compute_frame_elements_with_overrides(
            &anim_frame,
            &build_list,
            1.0,
            &empty_elements,
            &empty_symbols,
            &overrides,
        )
        .unwrap();
        assert_eq!(with.len(), 1);
        assert_eq!(with[0].sprite.width(), 8);
    }
}
