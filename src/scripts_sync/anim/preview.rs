//! Animation preview/export helpers built on `dst-anim-tool`.
//!
//! This module is the bridge between `anim-index` data and the WebUI render
//! API. It loads a DST animation archive, renders a selected bank/animation
//! to PNG frames, and exports either a GIF or a PNG sequence zip.

use crate::error::{Error, Result};
use dst_anim_tool::render::{prepare_animation_frames, render_frame_with_elements, BuildRef};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

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

    struct LoadedBuild {
        name: String,
        build: dst_anim_tool::build_file::BuildFile,
        disabled_symbols: HashSet<String>,
    }
    let mut builds: Vec<LoadedBuild> = Vec::new();
    for file in &safe_files {
        let path = params.anim_root.join(file);
        if !path.is_file() {
            continue;
        }
        let data = std::fs::read(&path)?;
        let mut archive = dst_anim_tool::archive::parse_file_by_path(&path, &data)?;
        if archive.build.is_none() {
            continue;
        }
        let tex = archive.tex_files().clone();
        let atlases = archive.build.as_ref().unwrap().atlases.clone();
        let atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(&atlases, &tex);
        let build = archive.build.as_mut().unwrap();
        dst_anim_tool::atlas::split_atlas(build, &atlas)?;
        let build = archive.build.take().unwrap();
        builds.push(LoadedBuild {
            name: file.clone(),
            build,
            disabled_symbols: HashSet::new(),
        });
    }
    if builds.is_empty() {
        return Err(Error::Config(format!(
            "no build.bin found in any provided file for {}",
            selected
        )));
    }

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

    let (bounds, prepared) = prepare_animation_frames(
        &animation.frames,
        &build_refs,
        1.0,
        (0.0, 0.0),
        &HashSet::new(),
        &HashSet::new(),
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
    match params.format {
        RenderFormat::Gif => {
            let mut out = Vec::new();
            dst_anim_tool::gif_export::export_gif(&rendered.frames, rendered.frame_rate, &mut out)?;
            Ok(RenderOutput {
                bytes: out,
                content_type: "image/gif",
                filename: format!(
                    "{}-{}.gif",
                    safe_file_base(&rendered.selected),
                    rendered.animation_name
                ),
            })
        }
        RenderFormat::Png => {
            let bytes = frames_to_zip(&rendered.frames, &rendered.animation_name)?;
            Ok(RenderOutput {
                bytes,
                content_type: "application/zip",
                filename: format!(
                    "{}-{}-frames.zip",
                    safe_file_base(&rendered.selected),
                    rendered.animation_name
                ),
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
    for file in &safe_files {
        let path = params.anim_root.join(file);
        if !path.is_file() {
            continue;
        }
        let fdata = std::fs::read(&path)?;
        let mut farch = dst_anim_tool::archive::parse_file_by_path(&path, &fdata)?;
        if farch.build.is_none() {
            continue;
        }
        let tex = farch.tex_files().clone();
        let atlases = farch.build.as_ref().unwrap().atlases.clone();
        let atlas = dst_anim_tool::atlas::decode_atlas_images_from_tex(&atlases, &tex);
        let build = farch.build.as_mut().unwrap();
        dst_anim_tool::atlas::split_atlas(build, &atlas)?;
        let build = farch.build.as_ref().unwrap();
        build_info.push(serde_json::json!({
            "file": file,
            "name": build.name,
            "symbols": build.symbols.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "atlases": build.atlases.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
        }));
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

/// Search all animation archives under `anim_root` for build files that
/// provide any of the requested symbols.
pub fn find_builds_for_symbols(
    anim_root: &Path,
    symbols: &[String],
) -> Result<Vec<serde_json::Value>> {
    let wanted: HashSet<&str> = symbols.iter().map(|s| s.as_str()).collect();
    let mut files = Vec::new();
    collect_anim_files(anim_root, &mut files)?;
    files.par_sort();

    let found: Vec<serde_json::Value> = files
        .par_iter()
        .filter_map(|path| {
            let data = std::fs::read(path).ok()?;
            let archive = dst_anim_tool::archive::parse_file_by_path(path, &data).ok()?;
            let build = archive.build.as_ref()?;
            let matched: Vec<String> = build
                .symbols
                .iter()
                .map(|s| s.name.clone())
                .filter(|name| wanted.iter().any(|w| name.eq_ignore_ascii_case(w)))
                .collect();
            if matched.is_empty() {
                return None;
            }
            let rel = path
                .strip_prefix(anim_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            Some(serde_json::json!({
                "file": rel,
                "name": build.name,
                "symbols": build.symbols.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
                "atlases": build.atlases.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
                "matched_symbols": matched,
            }))
        })
        .collect();
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
}
