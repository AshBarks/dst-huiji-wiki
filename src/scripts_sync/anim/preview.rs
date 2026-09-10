//! Animation preview/export helpers built on `dst-anim-tool`.
//!
//! This module is the bridge between `anim-index` data and the WebUI render
//! API. It loads a DST animation archive, renders a selected bank/animation
//! to PNG frames, and exports either a GIF or a PNG sequence zip.

use crate::error::{Error, Result};
use crate::parser::anim_override::SymbolRemapIndex;
use dst_anim_tool::render::{
    prepare_animation_frames_with_overrides, render_frame_with_elements, BuildRef,
    SymbolOverrideMap,
};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Export format requested by the user.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RenderFormat {
    Gif,
    Png,
}

#[derive(Debug, Clone)]
pub struct RenderParams {
    pub anim_root: PathBuf,
    /// Animation/package files to load. The last entry is treated as the
    /// primary animation file; other entries provide build/tex data.
    pub files: Vec<String>,
    /// Build files that should not participate in symbol candidates.
    pub disabled_builds: Vec<String>,
    /// Symbols to hide entirely.
    pub hidden_symbols: Vec<String>,
    /// Lowercase symbol -> chosen build file.
    pub symbol_builds: HashMap<String, String>,
    /// Skin build package (`dynamic/<build>.zip`), when rendering with a skin.
    /// Loaded before every other build so skin symbols override the base
    /// build and missing skin symbols fall back to it (game `SetSkin`
    /// semantics, see docs/ANIM_SKIN_PREVIEW_PLAN.md §8).
    pub skin_zip: Option<String>,
    /// Skin texture package (`dynamic/<build>.dyn`). When omitted, the
    /// stem-paired `<stem>.dyn` next to `skin_zip` is used automatically.
    pub skin_dyn: Option<String>,
    /// 改名重映射 `(anim_symbol, build, src_symbol)`，对应
    /// `AnimState:OverrideSymbol(sym, build, src_sym)`（`build` 为空串表示
    /// 无 build 提示）。按 symbol 提取的索引
    /// （`parser::anim_override::SymbolRemapIndex`）可直接转换传入。
    pub symbol_overrides: Vec<(String, String, String)>,
    pub bank: String,
    pub animation: String,
    pub format: RenderFormat,
}

#[derive(Debug)]
pub struct RenderOutput {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub filename: String,
}

/// One loaded build (already atlas-split), either from the base files or
/// from the selected skin package.
struct LoadedBuild {
    name: String,
    build: dst_anim_tool::build_file::BuildFile,
    disabled_symbols: HashSet<String>,
}

/// Build the renderer's [`SymbolOverrideMap`] from `(anim_symbol, build,
/// src_symbol)` specs. All names are lowercased by the map, matching the
/// lowercase lookup keys used in `anim.bin` elements.
fn build_symbol_override_map(specs: &[(String, String, String)]) -> SymbolOverrideMap {
    let mut map = SymbolOverrideMap::new();
    for (symbol, build, src_symbol) in specs {
        map.insert(symbol, build, src_symbol);
    }
    map
}

/// Load archives providing builds referenced by overrides but absent from the
/// loaded set. The override build is only reached through the explicit build
/// hint in `find_symbol_frame`, so appending it does not change any other
/// lookup order.
fn ensure_override_builds_loaded(
    anim_root: &Path,
    specs: &[(String, String, String)],
    builds: &mut Vec<LoadedBuild>,
) -> Result<()> {
    let existing: HashSet<String> = builds.iter().map(|b| b.build.name.to_lowercase()).collect();
    let missing: HashSet<String> = specs
        .iter()
        .filter(|(_, build, _)| !build.is_empty() && !existing.contains(&build.to_lowercase()))
        .map(|(_, build, _)| build.to_lowercase())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let found = find_archives_with_build_names(anim_root, &missing)?;
    for (name, path) in found {
        if let Some(loaded) = load_build_from_archive(&path, name)? {
            builds.push(loaded);
        }
    }
    Ok(())
}

/// 单个归档的索引元数据。
struct ArchiveMeta {
    path: PathBuf,
    build_name: Option<String>,
    has_anim: bool,
    banks: Vec<ArchiveBank>,
    /// 小写 symbol 列表（来自 build.bin）。
    symbols: Vec<String>,
}

struct ArchiveBank {
    name: String,
    animations: Vec<String>,
}

/// 全树归档索引：一次扫描同时服务 override build 自动加载、同名/映射
/// 候选扫描与手动文件检索。
#[derive(Default)]
struct AnimArchiveIndex {
    archives: Vec<ArchiveMeta>,
    by_name: HashMap<String, Vec<usize>>,
    by_symbol: HashMap<String, Vec<usize>>,
}

type ArchiveIndexCache = Mutex<HashMap<PathBuf, ((usize, u64), Arc<AnimArchiveIndex>)>>;

/// 进程级归档索引缓存：避免重复全树解析 `data/anim`。键为动画树路径，
/// 失效信号为文件数 + 最大 mtime。
static ARCHIVE_INDEX_CACHE: OnceLock<ArchiveIndexCache> = OnceLock::new();

/// 动画树签名（zip/dyn 文件数 + 最大 mtime 毫秒）。
fn anim_tree_signature(anim_root: &Path) -> (usize, u64) {
    let mut files = Vec::new();
    if collect_anim_files(anim_root, &mut files).is_err() {
        return (0, 0);
    }
    let max_mtime = files
        .iter()
        .filter_map(|path| {
            std::fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
        })
        .max()
        .unwrap_or(0);
    (files.len(), max_mtime)
}

fn rel_path(anim_root: &Path, path: &Path) -> String {
    path.strip_prefix(anim_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn scan_archive_index(anim_root: &Path) -> Result<AnimArchiveIndex> {
    let mut files = Vec::new();
    collect_anim_files(anim_root, &mut files)?;
    files.par_sort();
    let entries: Vec<ArchiveMeta> = files
        .par_iter()
        .filter_map(|path| {
            let data = std::fs::read(path).ok()?;
            let archive = dst_anim_tool::archive::parse_file_by_path(path, &data).ok()?;
            let build_name = archive.build.as_ref().map(|b| b.name.to_lowercase());
            let symbols = archive
                .build
                .as_ref()
                .map(|b| {
                    b.symbols
                        .iter()
                        .map(|s| s.name.to_lowercase())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let banks = archive
                .anim
                .as_ref()
                .map(|anim| {
                    anim.banks
                        .iter()
                        .map(|bank| ArchiveBank {
                            name: bank.name.clone(),
                            animations: bank.animations.iter().map(|a| a.name.clone()).collect(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some(ArchiveMeta {
                path: path.clone(),
                build_name,
                has_anim: !banks.is_empty(),
                banks,
                symbols,
            })
        })
        .collect();
    let mut index = AnimArchiveIndex {
        archives: entries,
        ..AnimArchiveIndex::default()
    };
    for (idx, meta) in index.archives.iter().enumerate() {
        if let Some(name) = &meta.build_name {
            index.by_name.entry(name.clone()).or_default().push(idx);
        }
        for symbol in &meta.symbols {
            index.by_symbol.entry(symbol.clone()).or_default().push(idx);
        }
    }
    Ok(index)
}

/// 取（或重建）归档索引，签名未变时命中缓存。
fn archive_index(anim_root: &Path) -> Result<Arc<AnimArchiveIndex>> {
    let cache_key = anim_root.to_path_buf();
    let signature = anim_tree_signature(anim_root);
    let cache = ARCHIVE_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached_signature, index)) = guard.get(&cache_key) {
            if *cached_signature == signature {
                return Ok(Arc::clone(index));
            }
        }
    }
    let index = Arc::new(scan_archive_index(anim_root)?);
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(cache_key, (signature, Arc::clone(&index)));
    Ok(index)
}

/// Look up archives whose `build.bin` name is in `wanted`.
/// Returns `(rel_path, path)` pairs, at most one per wanted name, in
/// lexicographic rel-path order for determinism.
fn find_archives_with_build_names(
    anim_root: &Path,
    wanted: &HashSet<String>,
) -> Result<Vec<(String, PathBuf)>> {
    let index = archive_index(anim_root)?;
    let mut out: Vec<(String, PathBuf)> = wanted
        .iter()
        .filter_map(|name| {
            let &first = index.by_name.get(name)?.first()?;
            let path = &index.archives[first].path;
            Some((rel_path(anim_root, path), path.clone()))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// 手动文件检索：按相对路径子串过滤 `data/anim` 下的 zip/dyn。
pub fn list_archives(
    anim_root: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<serde_json::Value>> {
    let index = archive_index(anim_root)?;
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    for meta in &index.archives {
        let rel = rel_path(anim_root, &meta.path);
        if !q.is_empty() && !rel.to_lowercase().contains(&q) {
            continue;
        }
        out.push(serde_json::json!({
            "path": rel,
            "build_name": meta.build_name,
            "has_anim": meta.has_anim,
            "banks": meta.banks.iter().map(|b| serde_json::json!({
                "name": b.name,
                "animations": b.animations,
            })).collect::<Vec<_>>(),
        }));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// Parse the `symbol_overrides` query parameter: a CSV of
/// `anim_symbol:build:src_symbol` (build may be empty, meaning "no build
/// hint"; the 2-part form `anim_symbol:src_symbol` also means no hint).
pub fn parse_symbol_overrides(raw: &str) -> Vec<(String, String, String)> {
    raw.split(',')
        .filter_map(|pair| {
            let mut parts = pair.trim().splitn(3, ':');
            let symbol = parts.next()?.trim().to_lowercase();
            if symbol.is_empty() {
                return None;
            }
            let second = parts.next()?.trim().to_string();
            match parts.next() {
                Some(src) => Some((symbol, second, src.trim().to_string())),
                None => Some((symbol, String::new(), second)),
            }
        })
        .collect()
}

/// Load the skin package (build `.zip` + paired `.dyn` textures) as a build.
///
/// Pairing rule (docs/ANIM_SKIN_PREVIEW_PLAN.md §6.2): the `.zip` and `.dyn`
/// are stem-paired. When `skin_dyn` is omitted, the `<stem>.dyn` next to
/// `skin_zip` is used automatically when it exists. Returns `Ok(None)` when
/// no skin was requested.
fn load_skin_build(
    anim_root: &Path,
    skin_zip: Option<&str>,
    skin_dyn: Option<&str>,
) -> Result<Option<LoadedBuild>> {
    let Some(zip_rel) = skin_zip.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let zip_rel = normalize_file_argument(zip_rel)?;
    let zip_path = anim_root.join(&zip_rel);
    if !zip_path.is_file() {
        return Err(Error::Config(format!(
            "skin build file not found: {}",
            zip_path.display()
        )));
    }
    let zip_data = std::fs::read(&zip_path)?;
    let mut archive = dst_anim_tool::archive::parse_file_by_path(&zip_path, &zip_data)?;

    let dyn_rel = match skin_dyn.map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => Some(normalize_file_argument(d)?),
        None => {
            // Auto-pair: same stem, same directory as the zip.
            let candidate = paired_dyn_candidate(&zip_rel);
            anim_root.join(&candidate).is_file().then_some(candidate)
        }
    };
    if let Some(dyn_rel) = dyn_rel {
        let dyn_path = anim_root.join(&dyn_rel);
        if !dyn_path.is_file() {
            return Err(Error::Config(format!(
                "skin texture file not found: {}",
                dyn_path.display()
            )));
        }
        let dyn_data = std::fs::read(&dyn_path)?;
        let dyn_archive = dst_anim_tool::archive::parse_file_by_path(&dyn_path, &dyn_data)?;
        archive.merge(dyn_archive);
    }

    let atlases = archive
        .build
        .as_ref()
        .map(|b| b.atlases.clone())
        .unwrap_or_default();
    let tex = archive.tex_files().clone();
    let atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(&atlases, &tex);
    if !atlases.is_empty() && atlas.is_empty() {
        return Err(Error::Config(format!(
            "skin build {zip_rel} has atlases but no decodable tex (companion .dyn missing?)"
        )));
    }
    let Some(build) = archive.build.as_mut() else {
        return Err(Error::Config(format!(
            "skin package has no build.bin: {zip_rel}"
        )));
    };
    dst_anim_tool::atlas::split_atlas(build, &atlas)?;
    let build = archive.build.take().unwrap();
    Ok(Some(LoadedBuild {
        name: zip_rel,
        build,
        disabled_symbols: HashSet::new(),
    }))
}

/// Load one archive file as a [`LoadedBuild`], decoding and splitting its
/// atlases. Returns `Ok(None)` when the archive has no build.bin.
fn load_build_from_archive(path: &Path, name: String) -> Result<Option<LoadedBuild>> {
    let data = std::fs::read(path)?;
    let mut archive = dst_anim_tool::archive::parse_file_by_path(path, &data)?;
    if archive.build.is_none() {
        return Ok(None);
    }
    let tex = archive.tex_files().clone();
    let atlases = archive.build.as_ref().unwrap().atlases.clone();
    let atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(&atlases, &tex);
    let build = archive.build.as_mut().unwrap();
    dst_anim_tool::atlas::split_atlas(build, &atlas)?;
    Ok(Some(LoadedBuild {
        name,
        build: archive.build.take().unwrap(),
        disabled_symbols: HashSet::new(),
    }))
}

/// The stem-paired `.dyn` path for a skin build `.zip`
/// (`dynamic/foo.zip` -> `dynamic/foo.dyn`).
fn paired_dyn_candidate(zip_rel: &str) -> String {
    let path = std::path::Path::new(zip_rel);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    match path.parent().and_then(|p| p.to_str()) {
        Some(dir) if !dir.is_empty() => format!("{dir}/{stem}.dyn"),
        _ => format!("{stem}.dyn"),
    }
}

/// Serialize one loaded build for the `info` endpoint response.
fn build_info_json(file: &str, build: &dst_anim_tool::build_file::BuildFile) -> serde_json::Value {
    serde_json::json!({
        "file": file,
        "name": build.name,
        "symbols": build.symbols.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
        "atlases": build.atlases.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
    })
}

/// Render a selected animation and return GIF or PNG-sequence zip bytes.
/// A fully rendered animation sequence.
pub struct RenderedAnimation {
    pub frames: Vec<image::RgbaImage>,
    pub frame_rate: f32,
    pub animation_name: String,
    pub selected: String,
}

/// Render an animation to in-memory frames.
pub fn render_animation_frames(params: &RenderParams) -> Result<RenderedAnimation> {
    if params.files.is_empty() {
        return Err(Error::Config("no animation files provided".to_string()));
    }

    let mut safe_files = Vec::new();
    for f in &params.files {
        safe_files.push(normalize_file_argument(f)?);
    }
    let selected = safe_files.last().cloned().unwrap();
    let selected_path = params.anim_root.join(&selected);
    if !selected_path.is_file() {
        return Err(Error::Config(format!(
            "animation file not found: {}",
            selected_path.display()
        )));
    }

    let selected_data = std::fs::read(&selected_path)?;
    let selected_archive =
        dst_anim_tool::archive::parse_file_by_path(&selected_path, &selected_data)?;
    let anim = selected_archive
        .anim
        .as_ref()
        .ok_or_else(|| Error::Config(format!("{} has no anim.bin", selected)))?;
    let bank = anim
        .banks
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case(&params.bank))
        .ok_or_else(|| Error::Config(format!("bank not found: {}", params.bank)))?;
    let animation = bank
        .animations
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&params.animation))
        .ok_or_else(|| {
            Error::Config(format!(
                "animation not found: {}/{}",
                params.bank, params.animation
            ))
        })?;

    let mut builds: Vec<LoadedBuild> = Vec::new();
    for file in &safe_files {
        let path = params.anim_root.join(file);
        if !path.is_file() {
            continue;
        }
        if let Some(loaded) = load_build_from_archive(&path, file.clone())? {
            builds.push(loaded);
        }
    }
    if builds.is_empty() {
        return Err(Error::Config(format!(
            "no build.bin found in any provided file for {}",
            selected
        )));
    }

    // The skin build is loaded first so its symbols override the base build
    // while missing skin symbols still fall back to the base (first-match
    // resolution, matching the game's SetSkin semantics).
    if let Some(skin) = load_skin_build(
        &params.anim_root,
        params.skin_zip.as_deref(),
        params.skin_dyn.as_deref(),
    )? {
        builds.insert(0, skin);
    }

    // Overrides redirect lookups to builds that may not be among the provided
    // files; scan the anim tree for archives providing them and append.
    ensure_override_builds_loaded(&params.anim_root, &params.symbol_overrides, &mut builds)?;

    let disabled_builds: HashSet<&str> =
        params.disabled_builds.iter().map(|s| s.as_str()).collect();
    let hidden_symbols: HashSet<&str> = params.hidden_symbols.iter().map(|s| s.as_str()).collect();

    for b in &mut builds {
        if disabled_builds.contains(b.name.as_str()) {
            continue;
        }
        for sym in &hidden_symbols {
            b.disabled_symbols.insert(sym.to_string());
        }
    }

    for (symbol, chosen) in &params.symbol_builds {
        for b in &mut builds {
            if disabled_builds.contains(b.name.as_str()) {
                continue;
            }
            if !b.name.eq_ignore_ascii_case(chosen) {
                b.disabled_symbols.insert(symbol.clone());
            }
        }
    }

    let build_refs: Vec<BuildRef<'_>> = builds
        .iter()
        .filter(|b| !disabled_builds.contains(b.name.as_str()))
        .map(|b| BuildRef {
            build: &b.build,
            disabled_symbols: &b.disabled_symbols,
        })
        .collect();
    if build_refs.is_empty() {
        return Err(Error::Config("all builds are disabled".to_string()));
    }

    let (bounds, prepared) = prepare_animation_frames_with_overrides(
        &animation.frames,
        &build_refs,
        1.0,
        (0.0, 0.0),
        &HashSet::new(),
        &HashSet::new(),
        &build_symbol_override_map(&params.symbol_overrides),
    );

    let mut frames = Vec::new();
    for pf in prepared.into_iter().flatten() {
        let render_bounds = bounds.as_ref().unwrap_or(&pf.bounds);
        if let Some(rendered) =
            render_frame_with_elements(&pf.elements, render_bounds, 1.0, (0.0, 0.0))
        {
            frames.push(rendered.image);
        }
    }
    if frames.is_empty() {
        return Err(Error::Config("no renderable frames produced".to_string()));
    }

    Ok(RenderedAnimation {
        frames,
        frame_rate: animation.frame_rate,
        animation_name: animation.name.clone(),
        selected,
    })
}

/// Render a selected animation and return GIF or PNG-sequence zip bytes.
pub fn render_animation(params: &RenderParams) -> Result<RenderOutput> {
    let rendered = render_animation_frames(params)?;
    // Prefix the skin stem so skinned exports are distinguishable.
    let base = match params.skin_zip.as_deref().map(safe_file_base) {
        Some(skin) => format!("{skin}-{}", safe_file_base(&rendered.selected)),
        None => safe_file_base(&rendered.selected),
    };
    match params.format {
        RenderFormat::Gif => {
            let mut out = Vec::new();
            dst_anim_tool::gif_export::export_gif(&rendered.frames, rendered.frame_rate, &mut out)?;
            Ok(RenderOutput {
                bytes: out,
                content_type: "image/gif",
                filename: format!("{}-{}.gif", base, rendered.animation_name),
            })
        }
        RenderFormat::Png => {
            let bytes = frames_to_zip(&rendered.frames, &rendered.animation_name)?;
            Ok(RenderOutput {
                bytes,
                content_type: "application/zip",
                filename: format!("{}-{}-frames.zip", base, rendered.animation_name),
            })
        }
    }
}

/// Render an animation as base64 PNG frames for in-browser preview.
pub fn render_animation_preview_json(params: &RenderParams) -> Result<serde_json::Value> {
    let rendered = render_animation_frames(params)?;
    let mut frames = Vec::new();
    for frame in rendered.frames {
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(frame)
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| Error::Config(format!("png encode: {e}")))?;
        frames.push(serde_json::json!({
            "data": format!("data:image/png;base64,{}", base64_encode(png.get_ref())),
        }));
    }
    Ok(serde_json::json!({
        "frame_rate": rendered.frame_rate,
        "animation": rendered.animation_name,
        "frames": frames,
    }))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Return metadata for an animation: frame element summaries, symbol
/// dependencies, and build symbols/atlases. Used by the WebUI animation
/// detail page.
pub fn animation_info(params: &RenderParams) -> Result<serde_json::Value> {
    if params.files.is_empty() {
        return Err(Error::Config("no animation files provided".to_string()));
    }
    let mut safe_files = Vec::new();
    for f in &params.files {
        safe_files.push(normalize_file_argument(f)?);
    }
    let selected = safe_files.last().cloned().unwrap();
    let selected_path = params.anim_root.join(&selected);
    let data = std::fs::read(&selected_path)?;
    let archive = dst_anim_tool::archive::parse_file_by_path(&selected_path, &data)?;
    let anim = archive
        .anim
        .as_ref()
        .ok_or_else(|| Error::Config(format!("{} has no anim.bin", selected)))?;
    let bank = anim
        .banks
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case(&params.bank))
        .ok_or_else(|| Error::Config(format!("bank not found: {}", params.bank)))?;
    let animation = bank
        .animations
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&params.animation))
        .ok_or_else(|| {
            Error::Config(format!(
                "animation not found: {}/{}",
                params.bank, params.animation
            ))
        })?;

    let mut symbols: BTreeSet<String> = BTreeSet::new();
    let frames: Vec<serde_json::Value> = animation
        .frames
        .iter()
        .map(|f| {
            let elements: Vec<serde_json::Value> = f
                .elements
                .iter()
                .map(|e| {
                    symbols.insert(e.symbol_lower.clone());
                    serde_json::json!({
                        "symbol": e.symbol,
                        "symbol_lower": e.symbol_lower,
                        "layer": e.layer_name,
                        "z": e.z_index,
                    })
                })
                .collect();
            serde_json::json!({
                "idx": f.idx,
                "elements": elements,
                "events": f.events,
            })
        })
        .collect();

    let mut build_info = Vec::new();
    // Skin build first so the UI treats it as the default symbol provider.
    if let Some(skin) = load_skin_build(
        &params.anim_root,
        params.skin_zip.as_deref(),
        params.skin_dyn.as_deref(),
    )? {
        build_info.push(serde_json::json!({
            "file": skin.name,
            "name": skin.build.name,
            "skin": true,
            "symbols": skin.build.symbols.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "atlases": skin.build.atlases.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
        }));
    }
    for file in &safe_files {
        let path = params.anim_root.join(file);
        if !path.is_file() {
            continue;
        }
        if let Some(loaded) = load_build_from_archive(&path, file.clone())? {
            build_info.push(build_info_json(file, &loaded.build));
        }
    }

    Ok(serde_json::json!({
        "file": selected,
        "bank": bank.name,
        "animation": animation.name,
        "frame_rate": animation.frame_rate,
        "frame_count": animation.frames.len(),
        "frames": frames,
        "symbols": symbols.into_iter().collect::<Vec<_>>(),
        "builds": build_info,
    }))
}

/// Search animation archives for builds that can serve the requested symbols,
/// either by providing them directly (same-name) or through a Tier-A symbol
/// map entry (`anim symbol -> build/src_symbol`). Entries carry `via` and
/// `remaps` provenance so the UI can distinguish the two sources.
pub fn find_builds_for_symbols(
    anim_root: &Path,
    symbols: &[String],
    remaps: Option<&SymbolRemapIndex>,
) -> Result<Vec<serde_json::Value>> {
    let index = archive_index(anim_root)?;
    let wanted: BTreeSet<String> = symbols
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    type Slot = (BTreeSet<String>, BTreeMap<String, serde_json::Value>);
    let mut matched: BTreeMap<usize, Slot> = BTreeMap::new();

    for symbol in &wanted {
        if let Some(indices) = index.by_symbol.get(symbol) {
            for &ai in indices {
                matched.entry(ai).or_default().0.insert(symbol.clone());
            }
        }
    }
    if let Some(remaps) = remaps {
        for symbol in &wanted {
            let Some(entries) = remaps.get(symbol) else {
                continue;
            };
            for entry in entries {
                let src = entry.src_symbol.to_lowercase();
                let targets: Vec<usize> = if entry.build.is_empty() {
                    index.by_symbol.get(&src).cloned().unwrap_or_default()
                } else {
                    index
                        .by_name
                        .get(&entry.build.to_lowercase())
                        .cloned()
                        .unwrap_or_default()
                };
                for ai in targets {
                    if !index.archives[ai].symbols.contains(&src) {
                        continue;
                    }
                    let remap = serde_json::json!({
                        "symbol": symbol,
                        "build": entry.build,
                        "src_symbol": entry.src_symbol,
                        "api": entry.api,
                        "confidence": entry.confidence,
                        "prefabs": entry.prefabs,
                    });
                    let key = format!(
                        "{}|{}|{}|{:?}",
                        symbol, entry.build, entry.src_symbol, entry.api
                    );
                    matched.entry(ai).or_default().1.entry(key).or_insert(remap);
                }
            }
        }
    }

    let mut found = Vec::new();
    for (ai, (same_name, remap_map)) in matched {
        let meta = &index.archives[ai];
        let Ok(data) = std::fs::read(&meta.path) else {
            continue;
        };
        let Some(build) = dst_anim_tool::archive::parse_file_by_path(&meta.path, &data)
            .ok()
            .and_then(|archive| archive.build)
        else {
            continue;
        };
        let remaps: Vec<serde_json::Value> = remap_map.into_values().collect();
        let via = match (same_name.is_empty(), remaps.is_empty()) {
            (false, true) => "same_name",
            (true, false) => "symbol_map",
            _ => "both",
        };
        found.push(serde_json::json!({
            "file": rel_path(anim_root, &meta.path),
            "name": build.name,
            "symbols": build.symbols.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "atlases": build.atlases.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
            "matched_symbols": same_name.into_iter().collect::<Vec<_>>(),
            "via": via,
            "remaps": remaps,
        }));
    }
    Ok(found)
}

fn collect_anim_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_anim_files(&path, out)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("dyn") {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn normalize_file_argument(file: &str) -> Result<String> {
    let file = file.trim().replace('\\', "/");
    if file.is_empty()
        || file.starts_with('/')
        || file.starts_with("..")
        || file.contains("..")
        || file.contains(':')
    {
        return Err(Error::Config(format!(
            "invalid animation file path: {file}"
        )));
    }
    // Keep `dynamic/foo.zip` or `foo.zip`; disallow nested traversal.
    Ok(file)
}

fn safe_file_base(file: &str) -> String {
    Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("anim")
        .to_string()
}

fn frames_to_zip(frames: &[image::RgbaImage], animation: &str) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default();
        for (i, frame) in frames.iter().enumerate() {
            let mut png = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(frame.clone())
                .write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| Error::Config(format!("png encode: {e}")))?;
            zw.start_file(format!("{animation}_frame_{i:03}.png"), options)?;
            zw.write_all(png.get_ref())?;
        }
        zw.finish()?;
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rejects_traversal() {
        assert!(normalize_file_argument("../evil.zip").is_err());
        assert!(normalize_file_argument("/abs/evil.zip").is_err());
        assert!(normalize_file_argument("a/../../evil.zip").is_err());
        assert!(normalize_file_argument("hound.zip").is_ok());
        assert!(normalize_file_argument("dynamic/foo.zip").is_ok());
    }

    #[test]
    fn safe_base() {
        assert_eq!(safe_file_base("hound.zip"), "hound");
        assert_eq!(safe_file_base("dynamic/foo.zip"), "foo");
    }

    #[test]
    fn skin_dyn_candidate_pairs_by_stem() {
        assert_eq!(
            paired_dyn_candidate("dynamic/abigail_ice.zip"),
            "dynamic/abigail_ice.dyn"
        );
        assert_eq!(paired_dyn_candidate("wilson_ice.zip"), "wilson_ice.dyn");
    }

    #[test]
    fn parse_symbol_overrides_specs() {
        assert!(parse_symbol_overrides("").is_empty());
        assert!(parse_symbol_overrides(" , ").is_empty());
        assert_eq!(
            parse_symbol_overrides("swap_hat:hat_beehive:swap_hat"),
            vec![(
                "swap_hat".to_string(),
                "hat_beehive".to_string(),
                "swap_hat".to_string()
            )]
        );
        // 空 build 段与两段形式都表示“无 build 提示”。
        assert_eq!(
            parse_symbol_overrides("face::skin_face, torso:skin_torso"),
            vec![
                ("face".to_string(), String::new(), "skin_face".to_string()),
                ("torso".to_string(), String::new(), "skin_torso".to_string()),
            ]
        );
        // 非法段（缺 src）被丢弃，symbol 归一小写。
        assert_eq!(
            parse_symbol_overrides("swap_hat, Swap_Face:mybuild:skin_face"),
            vec![(
                "swap_face".to_string(),
                "mybuild".to_string(),
                "skin_face".to_string()
            )]
        );
    }

    #[test]
    fn override_map_build_lowercases_via_tool() {
        let map = build_symbol_override_map(&[(
            "Swap_Hat".to_string(),
            "Hat_Beehive".to_string(),
            "swap_hat".to_string(),
        )]);
        assert_eq!(map.len(), 1);
        let o = map.get("swap_hat").unwrap();
        assert_eq!(o.build, "hat_beehive");
        assert_eq!(o.symbol, "swap_hat");
    }

    #[test]
    fn ensure_override_builds_noop_without_specs() {
        let root = std::env::temp_dir();
        let mut builds: Vec<LoadedBuild> = Vec::new();
        ensure_override_builds_loaded(&root, &[], &mut builds).unwrap();
        assert!(builds.is_empty());
    }

    #[test]
    fn ensure_override_builds_noop_when_builds_loaded() {
        let root = std::env::temp_dir();
        let specs = vec![(
            "swap_hat".to_string(),
            "hat_beehive".to_string(),
            "swap_hat".to_string(),
        )];
        let mut builds = vec![LoadedBuild {
            name: "hat.zip".to_string(),
            build: dst_anim_tool::build_file::BuildFile {
                version: 0,
                name: "hat_beehive".to_string(),
                symbols: Vec::new(),
                atlases: Vec::new(),
                symbol_index: std::collections::HashMap::new(),
            },
            disabled_symbols: HashSet::new(),
        }];
        ensure_override_builds_loaded(&root, &specs, &mut builds).unwrap();
        assert_eq!(builds.len(), 1);
    }

    #[test]
    fn archive_index_hits_cache_until_signature_changes() {
        let root = std::env::temp_dir().join(format!("archive_index_cache_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let first = archive_index(&root).unwrap();
        let second = archive_index(&root).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        std::fs::write(root.join("probe.zip"), b"not an archive").unwrap();
        let third = archive_index(&root).unwrap();
        assert!(!Arc::ptr_eq(&second, &third));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skin_loader_none_when_not_requested() {
        let root = std::env::temp_dir();
        let loaded = load_skin_build(&root, None, None).unwrap();
        assert!(loaded.is_none());
        let loaded = load_skin_build(&root, Some("  "), None).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn skin_loader_rejects_missing_zip() {
        let root = std::env::temp_dir().join(format!("skin_load_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let err = match load_skin_build(&root, Some("dynamic/nope.zip"), None) {
            Err(e) => e,
            Ok(_) => panic!("expected missing-file error"),
        };
        assert!(err.to_string().contains("skin build file not found"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skin_loader_rejects_explicit_missing_dyn() {
        let root = std::env::temp_dir().join(format!("skin_load_dyn_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        // A zip that is not a valid DST archive: the missing-file check for an
        // explicitly requested dyn runs before parsing, so point both at
        // nonexistent paths to validate the explicit-dyn branch.
        let err = match load_skin_build(&root, Some("missing.zip"), Some("missing.dyn")) {
            Err(e) => e,
            Ok(_) => panic!("expected missing-file error"),
        };
        assert!(err.to_string().contains("skin build file not found"));
        std::fs::remove_dir_all(&root).ok();
    }
}
