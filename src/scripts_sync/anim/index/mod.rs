//! Prefab → animation asset index.
//!
//! This is the first "animation asset management" milestone: scan
//! `scripts/prefabs` for `Asset("ANIM", "anim/xxx.zip")` /
//! `Asset("DYNAMIC_ANIM", "anim/dynamic/xxx.zip")` declarations, associate
//! them to prefab variants through `Prefab(name, fn, assets, deps)`, verify
//! existence against `data/anim`, and report dynamic/unresolved forms.
//!
//! Skin-related `.dyn`/`PKGREF` mapping lives in the sibling `skin_index`
//! module (`prefabs/skinprefabs.lua` → `prefab_skins` index).

use crate::error::Result;
use crate::parser::anim_override::{parse_anim_overrides_in, SymbolOverrideCall, SymbolRemapIndex};
use crate::parser::clothing_overrides::{parse_clothing_overrides, ClothingEntry};
use crate::platform::progress::Reporter;
use full_moon::ast;
use full_moon::node::Node;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One parsed `Asset("ANIM", ...)` / `Asset("DYNAMIC_ANIM", ...)` entry.
#[derive(Debug, Clone, Serialize)]
pub struct AssetEntry {
    pub kind: String,
    pub raw_path: String,
    pub line: u32,
    /// Normalized path relative to `data/anim` when statically known.
    pub normalized: Option<String>,
}

/// Template for an `Asset` entry inside an asset-factory function.
#[derive(Debug, Clone)]
struct TemplateAssetEntry {
    kind: String,
    raw: String,
    line: u32,
    path_template: Option<PathTemplate>,
}

/// A string path built from literals and factory parameters.
#[derive(Debug, Clone)]
enum PathTemplate {
    Literal(String),
    Concat(Vec<PathPart>),
}

#[derive(Debug, Clone)]
enum PathPart {
    Lit(String),
    Param(String),
}

/// A factory function that returns an asset table (`makeassetlist`, etc.).
#[derive(Debug, Clone)]
struct AssetFactory {
    params: Vec<String>,
    entries: Vec<TemplateAssetEntry>,
}

/// A factory function that returns one or more `Prefab(...)` calls.
#[derive(Debug, Clone)]
struct PrefabFactory {
    params: Vec<String>,
    templates: Vec<PrefabTemplate>,
}

#[derive(Debug, Clone)]
struct PrefabTemplate {
    name: TemplateName,
    assets: TemplateAssets,
    line: u32,
}

#[derive(Debug, Clone)]
enum TemplateName {
    Lit(String),
    Param(String),
    Other,
}

#[derive(Debug, Clone)]
enum TemplateAssets {
    /// `Prefab(name, fn, assets_var, ...)` where `assets_var` is a parameter.
    Param(String),
    /// `Prefab(name, fn, makeassetlist(arg), ...)`.
    FactoryCall {
        name: String,
        args: Vec<TemplateArg>,
    },
    /// Inline table in the Prefab call itself.
    Inline(Vec<TemplateAssetEntry>),
    Other(String),
}

#[derive(Debug, Clone)]
enum TemplateArg {
    Lit(String),
    Param(String),
    Other(String),
}

/// A concrete argument value at a factory/prefab call site.
#[derive(Debug, Clone)]
enum ResolvedArg {
    Str(String),
    Var(String),
    Table(Vec<AssetEntry>),
    Other(String),
}

/// A prefab variant and its resolved animation assets.
#[derive(Debug, Clone, Serialize)]
pub struct PrefabAnimRecord {
    pub prefab_file: String,
    pub prefab_name: Option<String>,
    pub asset_var: Option<String>,
    pub anims: Vec<AnimRef>,
    /// Related non-anim build/package files (PKGREF `.zip`) useful for rendering.
    pub related_files: Vec<String>,
    pub unresolved: Vec<UnresolvedRef>,
    pub content: AnimContent,
}

/// A resolved animation file reference attached to a prefab variant.
#[derive(Debug, Clone, Serialize)]
pub struct AnimRef {
    pub kind: String,
    pub path: String,
    pub normalized: String,
    pub exists: bool,
}

/// A dynamic/unresolved asset reference or asset-table expression.
#[derive(Debug, Clone, Serialize)]
pub struct UnresolvedRef {
    pub kind: String,
    pub raw: String,
    pub line: u32,
}

/// Unresolved entry in the global ledger, with prefab context.
#[derive(Debug, Clone, Serialize)]
pub struct GlobalUnresolvedRef {
    pub prefab_file: String,
    pub prefab_name: Option<String>,
    pub kind: String,
    pub raw: String,
    pub line: u32,
}

/// Content-level summary extracted from an animation package's `anim.bin`
/// and `build.bin`.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct AnimContent {
    pub banks: Vec<String>,
    pub animations: Vec<String>,
    pub builds: Vec<String>,
    pub symbols: Vec<String>,
    pub atlases: Vec<String>,
}

/// How often an animation file is referenced by prefab variants.
#[derive(Debug, Clone, Serialize)]
pub struct AnimFileUsage {
    pub path: String,
    pub kind: String,
    pub exists: bool,
    pub referenced_by: Vec<String>,
    pub content: AnimContent,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimIndexStats {
    pub prefab_files: usize,
    pub prefab_variants: usize,
    pub anim_refs: usize,
    pub resolved_refs: usize,
    pub unresolved_refs: usize,
    pub unique_anim_paths: usize,
    pub missing_anim_paths: usize,
    pub multi_anim_files: usize,
    pub skipped_pkgref_dyn: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimIndexArtifact {
    pub schema_version: u32,
    pub scripts_root: String,
    pub anim_root: Option<String>,
    pub stats: AnimIndexStats,
    pub prefabs: Vec<PrefabAnimRecord>,
    pub anim_files: BTreeMap<String, AnimFileUsage>,
    pub build_files: BTreeMap<String, AnimContent>,
    pub unresolved: Vec<GlobalUnresolvedRef>,
}

#[derive(Debug, Clone)]
pub struct AnimIndexParams {
    pub scripts_root: PathBuf,
    pub anim_root: Option<PathBuf>,
    pub out: Option<PathBuf>,
}

/// Run `anim-index`: scan prefab Lua and build a prefab↔animation-file JSON
/// index. Local-only, no wiki writes.
pub fn run_index(params: &AnimIndexParams, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    reporter.stage("构建 prefab × 动画资源索引");

    let prefabs_dir = params.scripts_root.join("prefabs");
    if !prefabs_dir.is_dir() {
        return Err(crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("prefabs directory not found: {}", prefabs_dir.display()),
        )));
    }

    let anim_root = params.anim_root.clone().unwrap_or_else(|| {
        // scripts/<...>/databundles/scripts -> scripts/<...>/data/anim
        params
            .scripts_root
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("anim"))
            .unwrap_or_else(|| PathBuf::from("data/anim"))
    });
    reporter.log(format!("scripts: {}", params.scripts_root.display()));
    reporter.log(format!("prefabs: {}", prefabs_dir.display()));
    reporter.log(format!("anim:    {}", anim_root.display()));

    let mut prefab_records = Vec::new();
    let mut unresolved = Vec::new();
    let mut prefab_file_count = 0usize;
    let mut skipped_pkgref_dyn = 0usize;
    let mut remap_calls: Vec<SymbolOverrideCall> = Vec::new();

    let mut content_cache: BTreeMap<String, AnimContent> = BTreeMap::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_lua_files(&prefabs_dir, &mut files)?;
    files.sort();

    for path in &files {
        let rel = path
            .strip_prefix(&params.scripts_root)
            .map_err(|e| crate::error::Error::InvalidPath(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(path)?;
        let scanner = Scanner::new(&source);
        let file_index = scanner.scan(rel.clone());
        prefab_file_count += 1;
        skipped_pkgref_dyn += file_index.skipped_pkgref_dyn;

        // Tier-A/C 提取：AnimState 符号重映射调用（常量三元组 + 变量追踪）。
        // 整文件解析失败时静默跳过——Scanner 已把 PARSE_ERROR 记进 unresolved。
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if let Ok(mut calls) = parse_anim_overrides_in(&source, Some(&stem)) {
            remap_calls.append(&mut calls);
        }

        for mut record in file_index.prefabs {
            // Resolve existence now that we know anim_root.
            for anim in &mut record.anims {
                anim.exists = anim_root.join(&anim.normalized).is_file();
            }

            // Deduplicate repeated animation paths from factory expansion.
            let mut seen = BTreeSet::new();
            record
                .anims
                .retain(|a| seen.insert((a.kind.clone(), a.normalized.clone())));

            // Content-level index from anim.bin/build.bin.
            let mut content = AnimContent::default();
            for anim in &record.anims {
                if let Some(cached) = content_cache.get(&anim.normalized) {
                    merge_content(&mut content, cached);
                    continue;
                }
                if !anim.exists {
                    continue;
                }
                if let Ok(c) = load_anim_content(&anim_root, &anim.normalized) {
                    merge_content(&mut content, &c);
                    content_cache.insert(anim.normalized.clone(), c);
                }
            }
            sort_dedup_content(&mut content);
            record.content = content;

            for ur in &record.unresolved {
                unresolved.push(GlobalUnresolvedRef {
                    prefab_file: record.prefab_file.clone(),
                    prefab_name: record.prefab_name.clone(),
                    kind: ur.kind.clone(),
                    raw: ur.raw.clone(),
                    line: ur.line,
                });
            }
            if !record.anims.is_empty() || !record.unresolved.is_empty() {
                prefab_records.push(record);
            }
        }
    }

    // Tier-C 覆盖面：重映射调用也大量存在于 stategraphs（SG*.lua）与
    // components（skinner 等）。这些目录不做 prefab 结构扫描，仅提取调用。
    for dir_name in ["stategraphs", "components"] {
        let dir = params.scripts_root.join(dir_name);
        if !dir.is_dir() {
            continue;
        }
        let mut extra_files = Vec::new();
        collect_lua_files(&dir, &mut extra_files)?;
        extra_files.sort();
        for path in &extra_files {
            let source = std::fs::read_to_string(path)?;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if let Ok(mut calls) = parse_anim_overrides_in(&source, Some(&stem)) {
                remap_calls.append(&mut calls);
            }
        }
    }

    // Build reverse index: anim file -> prefab variants.
    let mut anim_files: BTreeMap<String, AnimFileUsage> = BTreeMap::new();
    for record in &prefab_records {
        let label = match &record.prefab_name {
            Some(name) => format!("{}#{}", record.prefab_file, name),
            None => format!("{}#?", record.prefab_file),
        };
        for anim in &record.anims {
            let entry = anim_files
                .entry(anim.normalized.clone())
                .or_insert_with(|| AnimFileUsage {
                    path: anim.normalized.clone(),
                    kind: anim.kind.clone(),
                    exists: anim.exists,
                    referenced_by: Vec::new(),
                    content: AnimContent::default(),
                });
            entry.kind = anim.kind.clone();
            entry.exists = anim.exists;
            if let Some(cached) = content_cache.get(&anim.normalized) {
                entry.content = cached.clone();
            }
            entry.referenced_by.push(label.clone());
        }
    }

    let unique_anim_paths = anim_files.len();
    let missing_anim_paths = anim_files.values().filter(|v| !v.exists).count();
    let multi_anim_files = prefab_records.iter().filter(|r| r.anims.len() >= 2).count();
    let anim_refs: usize = prefab_records.iter().map(|r| r.anims.len()).sum();
    let resolved_refs = anim_refs;
    let unresolved_refs = unresolved.len();
    let prefab_variants = prefab_records.len();

    let mut build_files: BTreeMap<String, AnimContent> = BTreeMap::new();
    for record in &prefab_records {
        for rel in &record.related_files {
            if build_files.contains_key(rel) {
                continue;
            }
            if let Ok(c) = load_anim_content(&anim_root, rel) {
                build_files.insert(rel.clone(), c);
            }
        }
    }

    let artifact = AnimIndexArtifact {
        schema_version: 1,
        scripts_root: params.scripts_root.display().to_string(),
        anim_root: Some(anim_root.display().to_string()),
        stats: AnimIndexStats {
            prefab_files: prefab_file_count,
            prefab_variants,
            anim_refs,
            resolved_refs,
            unresolved_refs,
            unique_anim_paths,
            missing_anim_paths,
            multi_anim_files,
            skipped_pkgref_dyn,
        },
        prefabs: prefab_records,
        anim_files,
        build_files,
        unresolved,
    };

    let out_path = params
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("output").join("anim-index.json"));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&artifact)?)?;

    // Tier-A 重映射索引产物：anim-index.json 的伴生文件，供 WebUI 的
    // override 选择器与渲染端点查询。
    let remap_index = SymbolRemapIndex::from_calls(&remap_calls);
    let remap_entries: usize = remap_index.symbols.values().map(Vec::len).sum();

    // Tier-B：clothing.lua 数据表（脚本根目录，不在 prefabs/ 下）。
    let clothing_path = params.scripts_root.join("clothing.lua");
    let clothing_index: BTreeMap<String, ClothingEntry> = if clothing_path.is_file() {
        let source = std::fs::read_to_string(&clothing_path)?;
        parse_clothing_overrides(&source)?
    } else {
        BTreeMap::new()
    };

    let remap_path = out_path.with_file_name("anim-remap-index.json");
    // label 取 DST version.txt（与 anim-sync manifest 同源）；
    // scripts_root = <dst>/data/databundles/scripts → dst 根 = 三层祖先。
    let dst_root = params
        .scripts_root
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf);
    let label = dst_root
        .as_deref()
        .and_then(|d| crate::scripts_sync::read_new_version(d).ok())
        .unwrap_or_else(|| "unknown".to_string());
    let remap_artifact = serde_json::json!({
        "schema_version": 1,
        "label": label,
        "parser_version": super::PARSER_VERSION,
        "generated_at": super::history::now_ms(),
        "scripts_root": params.scripts_root.display().to_string(),
        "stats": {
            "calls": remap_calls.len(),
            "symbols": remap_index.len(),
            "entries": remap_entries,
            "clothing": clothing_index.len(),
        },
        "symbols": remap_index.symbols,
        "clothing": clothing_index,
    });
    std::fs::write(&remap_path, serde_json::to_string_pretty(&remap_artifact)?)?;

    // 版本快照：供 remap-diff 对比（同 label 覆盖，确定性输出保证幂等）。
    let snapshot_path = super::remap_history::save_remap_snapshot(
        &super::remap_history::remap_history_dir(),
        &remap_artifact,
    )?;

    reporter.log(format!(
        "prefab 文件 {} / 变体记录 {} / 动画引用 {} / 唯一动画 {} / unresolved {}",
        artifact.stats.prefab_files,
        artifact.stats.prefab_variants,
        artifact.stats.anim_refs,
        artifact.stats.unique_anim_paths,
        artifact.stats.unresolved_refs,
    ));
    reporter.log(format!("已写入 {}", out_path.display()));
    reporter.log(format!(
        "重映射：{} symbol / {} 条目（{} 次调用），已写入 {}",
        remap_index.len(),
        remap_entries,
        remap_calls.len(),
        remap_path.display()
    ));
    reporter.log(format!(
        "重映射快照 {}（label {label}）",
        snapshot_path.display()
    ));

    Ok(serde_json::json!({
        "schema_version": artifact.schema_version,
        "prefab_files": artifact.stats.prefab_files,
        "prefab_variants": artifact.stats.prefab_variants,
        "anim_refs": artifact.stats.anim_refs,
        "resolved_refs": artifact.stats.resolved_refs,
        "unresolved_refs": artifact.stats.unresolved_refs,
        "unique_anim_paths": artifact.stats.unique_anim_paths,
        "missing_anim_paths": artifact.stats.missing_anim_paths,
        "multi_anim_files": artifact.stats.multi_anim_files,
        "skipped_pkgref_dyn": artifact.stats.skipped_pkgref_dyn,
        "symbol_overrides": {
            "calls": remap_calls.len(),
            "symbols": remap_index.len(),
            "entries": remap_entries,
            "clothing": clothing_index.len(),
            "label": label,
            "snapshot": snapshot_path,
            "output": remap_path,
        },
        "output": out_path,
    }))
}

pub(crate) fn collect_lua_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_lua_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("lua") {
            out.push(path);
        }
    }
    Ok(())
}

mod scanner;

pub(crate) use scanner::{line_of, string_literal_text};
use scanner::{load_anim_content, merge_content, sort_dedup_content, Scanner};
#[cfg(test)]
mod tests {
    use super::scanner::{normalize_asset_path, ScannedFile};
    use super::*;

    fn scan_src(src: &str) -> ScannedFile {
        Scanner::new(src).scan("prefabs/test.lua".to_string())
    }

    #[test]
    fn simple_prefab_asset() {
        let src = r#"
local assets = {
    Asset("ANIM", "anim/foo.zip"),
}
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        let p = &scanned.prefabs[0];
        assert_eq!(p.prefab_name.as_deref(), Some("foo"));
        assert_eq!(p.asset_var.as_deref(), Some("assets"));
        assert_eq!(p.anims.len(), 1);
        assert_eq!(p.anims[0].normalized, "foo.zip");
        assert!(p.unresolved.is_empty());
    }

    #[test]
    fn multiple_prefabs_share_assets() {
        let src = r#"
local assets = { Asset("ANIM", "anim/shared.zip") }
local function a() end
local function b() end
return Prefab("a", a, assets), Prefab("b", b, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 2);
        assert!(scanned.prefabs.iter().all(|p| p.anims.len() == 1));
        assert!(scanned
            .prefabs
            .iter()
            .all(|p| p.anims[0].normalized == "shared.zip"));
    }

    #[test]
    fn dynamic_asset_is_unresolved() {
        let src = r#"
local assets = { Asset("ANIM", "anim/" .. build .. ".zip") }
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        let p = &scanned.prefabs[0];
        assert!(p.anims.is_empty());
        assert_eq!(p.unresolved.len(), 1);
        assert_eq!(p.unresolved[0].kind, "ANIM");
    }

    #[test]
    fn dynamic_anim_normalized_under_dynamic_dir() {
        let src = r#"
local assets = { Asset("DYNAMIC_ANIM", "anim/dynamic/body.zip") }
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        let p = &scanned.prefabs[0];
        assert_eq!(p.anims[0].normalized, "dynamic/body.zip");
    }

    #[test]
    fn inline_asset_table_in_prefab() {
        let src = r#"
local function fn() end
return Prefab("foo", fn, { Asset("ANIM", "anim/bar.zip") })
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        assert_eq!(scanned.prefabs[0].anims[0].normalized, "bar.zip");
    }

    #[test]
    fn pkgref_dyn_is_skipped() {
        let src = r#"
local assets = {
    Asset("PKGREF", "anim/dynamic/skin.dyn"),
    Asset("ANIM", "anim/base.zip"),
}
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.skipped_pkgref_dyn, 1);
        let p = &scanned.prefabs[0];
        assert_eq!(p.anims.len(), 1);
        assert_eq!(p.anims[0].normalized, "base.zip");
    }

    #[test]
    fn pkgref_zip_is_related_file() {
        let src = r#"
local assets = {
    Asset("PKGREF", "anim/base_build.zip"),
    Asset("ANIM", "anim/anim_only.zip"),
}
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        let p = &scanned.prefabs[0];
        assert_eq!(p.anims.len(), 1);
        assert_eq!(p.anims[0].normalized, "anim_only.zip");
        assert_eq!(p.related_files, vec!["base_build.zip"]);
    }

    #[test]
    fn global_asset_table_assignment() {
        let src = r#"
assets = { Asset("ANIM", "anim/global.zip") }
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        assert_eq!(scanned.prefabs[0].anims[0].normalized, "global.zip");
    }

    #[test]
    fn normalize_asset_paths() {
        assert_eq!(normalize_asset_path("ANIM", "anim/foo.zip"), "foo.zip");
        assert_eq!(
            normalize_asset_path("DYNAMIC_ANIM", "anim/dynamic/bar.zip"),
            "dynamic/bar.zip"
        );
    }

    #[test]
    fn make_axe_factory_expands_to_variants() {
        let src = r#"
local assets = { Asset("ANIM", "anim/axe.zip") }
local golden_assets = { Asset("ANIM", "anim/goldenaxe.zip") }
local function fn() end
local function MakeAxe(name, common, master, data, _assets, _prefabs)
    return Prefab(name, fn, _assets, _prefabs)
end
return MakeAxe("axe", nil, nil, nil, assets),
       MakeAxe("goldenaxe", nil, nil, nil, golden_assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 2);
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("axe")
                && p.anims.iter().any(|a| a.normalized == "axe.zip")));
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("goldenaxe")
                && p.anims.iter().any(|a| a.normalized == "goldenaxe.zip")));
    }

    #[test]
    fn makeassetlist_factory_expands_dynamic_path() {
        let src = r#"
local function makeassetlist(name)
    return { Asset("ANIM", "anim/" .. name .. ".zip") }
end
local function fn() end
local function item(name)
    return Prefab(name, fn, makeassetlist(name))
end
return item("pillar_ruins"), item("pillar_algae")
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 2);
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("pillar_ruins")
                && p.anims.iter().any(|a| a.normalized == "pillar_ruins.zip")));
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("pillar_algae")
                && p.anims.iter().any(|a| a.normalized == "pillar_algae.zip")));
    }

    #[test]
    fn direct_asset_factory_call_in_prefab() {
        let src = r#"
local function makeassetlist(name)
    return { Asset("ANIM", "anim/" .. name .. ".zip") }
end
local function fn() end
return Prefab("foo", fn, makeassetlist("foo"))
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        assert_eq!(scanned.prefabs[0].anims[0].normalized, "foo.zip");
    }

    #[test]
    fn sort_dedup_content_helper() {
        let mut c = AnimContent {
            banks: vec!["b".to_string(), "a".to_string(), "b".to_string()],
            animations: vec!["x".to_string(), "x".to_string()],
            builds: vec![],
            symbols: vec![],
            atlases: vec![],
        };
        sort_dedup_content(&mut c);
        assert_eq!(c.banks, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(c.animations, vec!["x".to_string()]);
    }
}
