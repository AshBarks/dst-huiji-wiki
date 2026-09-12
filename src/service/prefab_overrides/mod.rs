//! 预制体重定向解析（`prefab-overrides` / `prefab-overrides-dir`）：
//! 用 full_moon AST 从 Lua 源码提取 prefab 名重定向映射，并对数据驱动的
//! 运行时生成族（spice / winter ornament）做确定性展开。
//!
//! - 单文件入口解析指定 Lua 文件；
//! - 目录入口递归扫描目录下全部 `.lua`（缺省
//!   `DST__ROOT/data/databundles/scripts/prefabs`），按文件路径排序后逐文件
//!   解析，再按 prefab 名合并；单个文件解析失败只记录不中断。
//!
//! 纯本地操作，不访问维基。

pub mod audit;
pub mod maintain;

use crate::error::{Error, Result};
use crate::parser::{
    parse_bobbers, parse_indexed_variants, parse_literal_registrations, parse_oversized_waxed,
    parse_prefab_overrides, parse_spiced_foods, parse_spike_sizes, parse_winter_ornaments,
    spice_names_from_source, OverrideValue, PrefabVariant, LITERAL_FAMILIES,
};
use crate::platform::progress::Reporter;
use crate::scripts_sync::anim::index::collect_lua_files;
use crate::scripts_sync::anim::skin_index::default_scripts_root;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 日志中逐条列出的「同名不同值」冲突上限。
const MAX_CONFLICT_LOGS: usize = 20;

/// 单文件：解析一个 Lua 文件中的预制体重定向。
pub async fn run_prefab_overrides(
    input: &str,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析预制体重定向");
    let lua_content = std::fs::read_to_string(input)?;
    let overrides = parse_prefab_overrides(&lua_content)?;

    reporter.log(format!("找到 {} 条预制体重定向", overrides.len()));

    let mut mapping: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for override_info in overrides {
        mapping.insert(
            override_info.prefab_name,
            override_json(&override_info.override_name),
        );
    }

    let written = write_mapping(&mapping, output, reporter)?;
    Ok(match written {
        Some(path) => serde_json::json!({
            "overrides": mapping.len(),
            "output": path,
        }),
        None => serde_json::json!({ "overrides": mapping.len() }),
    })
}

/// 目录：递归扫描目录内全部 Lua 文件并合并解析结果。
pub async fn run_prefab_overrides_dir(
    input: Option<&str>,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("扫描目录解析预制体重定向");
    let dir = resolve_dir(input)?;
    reporter.log(format!("扫描目录：{}", dir.display()));

    let outcome = derive_dir(&dir, &family_root(&dir), reporter)?;

    for (path, err) in &outcome.read_failures {
        reporter.log(format!("跳过读取失败：{}（{err}）", path.display()));
    }
    for (path, err) in &outcome.parse_failures {
        reporter.log(format!("跳过解析失败：{}（{err}）", path.display()));
    }
    for conflict in outcome.conflicts.iter().take(MAX_CONFLICT_LOGS) {
        reporter.log(format!(
            "冲突：{} 由 {} 改为 {} 覆盖",
            conflict.prefab_name, conflict.previous_file, conflict.new_file
        ));
    }
    if outcome.conflicts.len() > MAX_CONFLICT_LOGS {
        reporter.log(format!(
            "其余 {} 条冲突已省略",
            outcome.conflicts.len() - MAX_CONFLICT_LOGS
        ));
    }

    reporter.log(format!(
        "{} 个文件 / {} 条重定向（数据族补充 {}，补丁 {}，冲突 {}，解析失败 {}，读取失败 {}）",
        outcome.file_count,
        outcome.mapping.len(),
        outcome.family_added,
        outcome.patch_applied,
        outcome.conflicts.len(),
        outcome.parse_failures.len(),
        outcome.read_failures.len(),
    ));

    let written = write_mapping(&outcome.mapping, output, reporter)?;
    Ok(serde_json::json!({
        "files": outcome.file_count,
        "overrides": outcome.mapping.len(),
        "family_overrides": outcome.family_added,
        "patch_overrides": outcome.patch_applied,
        "conflicts": outcome.conflicts.len(),
        "parse_failures": outcome.parse_failures.len(),
        "read_failures": outcome.read_failures.len(),
        "output": written,
    }))
}

/// 一次目录推导的完整结果（AST 扫描 + 数据驱动族展开）。
pub(crate) struct DeriveOutcome {
    pub mapping: BTreeMap<String, serde_json::Value>,
    pub conflicts: Vec<Conflict>,
    pub parse_failures: Vec<(PathBuf, String)>,
    pub read_failures: Vec<(PathBuf, String)>,
    pub file_count: usize,
    pub family_added: usize,
    pub patch_applied: usize,
}

/// 扫描脚本根（`<root>/prefabs`）并展开数据驱动族。
pub(crate) fn derive_from_scripts_root(
    root: &Path,
    reporter: &dyn Reporter,
) -> Result<DeriveOutcome> {
    derive_dir(&root.join("prefabs"), root, reporter)
}

fn family_root(scan_dir: &Path) -> PathBuf {
    if scan_dir.file_name().and_then(|n| n.to_str()) == Some("prefabs") {
        scan_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| scan_dir.to_path_buf())
    } else {
        scan_dir.to_path_buf()
    }
}

fn derive_dir(
    scan_dir: &Path,
    families_root: &Path,
    reporter: &dyn Reporter,
) -> Result<DeriveOutcome> {
    let mut files = Vec::new();
    collect_lua_files(scan_dir, &mut files)?;
    files.sort();

    if files.is_empty() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("目录下没有 .lua 文件：{}", scan_dir.display()),
        )));
    }

    let mut sources: Vec<(PathBuf, String)> = Vec::with_capacity(files.len());
    let mut read_failures: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        match std::fs::read_to_string(path) {
            Ok(source) => sources.push((path.clone(), source)),
            Err(e) => read_failures.push((path.clone(), e.to_string())),
        }
    }

    let merged = merge_sources(&sources);
    let mut mapping = merged.mapping;
    let mut origins = merged.origins;
    let mut conflicts = merged.conflicts;

    let (variants, patch) = derive_families(families_root, reporter);
    let family_added = apply_families(&mut mapping, &mut origins, &mut conflicts, &variants);
    let patch_applied = apply_patch(&mut mapping, &mut origins, &patch);

    Ok(DeriveOutcome {
        mapping,
        conflicts,
        parse_failures: merged.parse_failures,
        read_failures,
        file_count: files.len(),
        family_added,
        patch_applied,
    })
}

/// 展开已知的数据驱动生成族，外加人工补丁表。
fn derive_families(
    root: &Path,
    reporter: &dyn Reporter,
) -> (Vec<PrefabVariant>, Vec<PrefabVariant>) {
    let mut variants = Vec::new();

    let spice_names = std::fs::read_to_string(root.join("spicedfoods.lua"))
        .ok()
        .map(|source| spice_names_from_source(&source))
        .unwrap_or_default();
    let mut food_sources = Vec::new();
    for name in ["preparedfoods.lua", "preparedfoods_warly.lua"] {
        if let Ok(source) = std::fs::read_to_string(root.join(name)) {
            food_sources.push((name.to_string(), source));
        }
    }
    if !spice_names.is_empty() && !food_sources.is_empty() {
        push_family(
            &mut variants,
            reporter,
            "spicedfoods",
            parse_spiced_foods(&food_sources, &spice_names),
        );
    }

    if let Ok(source) = std::fs::read_to_string(root.join("prefabs/winter_ornaments.lua")) {
        push_family(
            &mut variants,
            reporter,
            "winter_ornaments",
            parse_winter_ornaments(&source),
        );
    }

    if let Ok(source) = std::fs::read_to_string(root.join("prefabs/veggies.lua")) {
        push_family(
            &mut variants,
            reporter,
            "oversized_waxed",
            parse_oversized_waxed(&source, "prefabs/veggies.lua"),
        );
    }

    if let Ok(source) = std::fs::read_to_string(root.join("prefabs/oceanfishingbobber.lua")) {
        push_family(
            &mut variants,
            reporter,
            "oceanfishingbobber",
            parse_bobbers(&source, "prefabs/oceanfishingbobber.lua"),
        );
    }

    for (file, base) in [
        ("prefabs/glass_spike.lua", "glassspike"),
        ("prefabs/sand_spike.lua", "sandspike"),
    ] {
        if let Ok(source) = std::fs::read_to_string(root.join(file)) {
            push_family(
                &mut variants,
                reporter,
                base,
                parse_spike_sizes(&source, base, file),
            );
        }
    }

    if let Ok(source) = std::fs::read_to_string(root.join("prefabs/goosplash.lua")) {
        push_family(
            &mut variants,
            reporter,
            "goosplash",
            parse_indexed_variants(&source, "goosplash", "prefabs/goosplash.lua"),
        );
    }

    for family in LITERAL_FAMILIES {
        if let Ok(source) = std::fs::read_to_string(root.join(family.file)) {
            push_family(
                &mut variants,
                reporter,
                family.file,
                parse_literal_registrations(&source, family.base, family.names, family.file),
            );
        }
    }

    let patch = load_patch(Path::new(PATCH_PATH), reporter);

    (variants, patch)
}

/// 人工补丁表（脚本无法推导或需覆盖的条目）。
const PATCH_PATH: &str = "config/prefab_overrides_patch.json";

#[derive(Debug, Deserialize)]
struct PatchFile {
    #[serde(default)]
    mappings: BTreeMap<String, PatchEntry>,
}

#[derive(Debug, Deserialize)]
struct PatchEntry {
    base: String,
    #[serde(default)]
    source: Option<String>,
}

fn load_patch(path: &Path, reporter: &dyn Reporter) -> Vec<PrefabVariant> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match parse_patch(&text) {
        Ok(variants) => {
            reporter.log(format!("补丁表 {}：{} 条", path.display(), variants.len()));
            variants
        }
        Err(e) => {
            reporter.log(format!("补丁表 {} 解析失败：{e}", path.display()));
            Vec::new()
        }
    }
}

fn parse_patch(text: &str) -> Result<Vec<PrefabVariant>> {
    let file: PatchFile = serde_json::from_str(text)?;
    Ok(file
        .mappings
        .into_iter()
        .map(|(prefab, entry)| PrefabVariant {
            prefab,
            base: entry.base,
            origin: format!(
                "patch:{}",
                entry.source.unwrap_or_else(|| "unlabeled".to_string())
            ),
        })
        .collect())
}

fn apply_patch(
    mapping: &mut BTreeMap<String, serde_json::Value>,
    origins: &mut BTreeMap<String, String>,
    patch: &[PrefabVariant],
) -> usize {
    let mut applied = 0;
    for variant in patch {
        let value = serde_json::json!({
            "override_name": variant.base,
            "type": "patch",
        });
        if mapping.get(&variant.prefab) != Some(&value) {
            mapping.insert(variant.prefab.clone(), value);
            origins.insert(variant.prefab.clone(), variant.origin.clone());
            applied += 1;
        }
    }
    applied
}

fn push_family(
    variants: &mut Vec<PrefabVariant>,
    reporter: &dyn Reporter,
    label: &str,
    result: Result<Vec<PrefabVariant>>,
) {
    match result {
        Ok(found) => {
            reporter.log(format!("{label} 族：推导 {} 条", found.len()));
            variants.extend(found);
        }
        Err(e) => reporter.log(format!("{label} 族解析失败：{e}")),
    }
}

fn apply_families(
    mapping: &mut BTreeMap<String, serde_json::Value>,
    origins: &mut BTreeMap<String, String>,
    conflicts: &mut Vec<Conflict>,
    variants: &[PrefabVariant],
) -> usize {
    let mut added = 0;
    for variant in variants {
        let value = serde_json::json!({
            "override_name": variant.base,
            "type": "family",
        });
        if let Some(existing) = mapping.get(&variant.prefab) {
            if existing.get("override_name").and_then(|v| v.as_str()) != Some(variant.base.as_str())
            {
                conflicts.push(Conflict {
                    prefab_name: variant.prefab.clone(),
                    previous_file: origins.get(&variant.prefab).cloned().unwrap_or_default(),
                    new_file: variant.origin.clone(),
                });
            }
            continue;
        }
        mapping.insert(variant.prefab.clone(), value);
        origins.insert(variant.prefab.clone(), variant.origin.clone());
        added += 1;
    }
    added
}

/// 解析目录：显式 `input` 优先，否则取 `DST__ROOT/data/databundles/scripts/prefabs`。
fn resolve_dir(input: Option<&str>) -> Result<PathBuf> {
    match input {
        Some(path) => Ok(PathBuf::from(path)),
        None => {
            let root = default_scripts_root().ok_or_else(|| {
                Error::Config(
                    "未找到游戏脚本目录：请设置 DST__ROOT（data/databundles/scripts）\
                     或显式指定 --input"
                        .to_string(),
                )
            })?;
            Ok(root.join("prefabs"))
        }
    }
}

fn write_mapping(
    mapping: &BTreeMap<String, serde_json::Value>,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<Option<PathBuf>> {
    let json_output = serde_json::to_string_pretty(mapping)?;
    match output {
        Some(path) => {
            std::fs::write(&path, &json_output)?;
            reporter.log(format!(
                "已写入 {} 条映射到 {}",
                mapping.len(),
                path.display()
            ));
            Ok(Some(path))
        }
        None => {
            reporter.log(json_output);
            Ok(None)
        }
    }
}

fn override_json(value: &OverrideValue) -> serde_json::Value {
    match value {
        OverrideValue::Static(s) => serde_json::json!({
            "override_name": s,
            "type": "static"
        }),
        OverrideValue::Dynamic(s) => serde_json::json!({
            "override_name": s,
            "type": "dynamic"
        }),
        OverrideValue::Unknown => serde_json::json!({
            "override_name": null,
            "type": "unknown"
        }),
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Conflict {
    prefab_name: String,
    previous_file: String,
    new_file: String,
}

#[derive(Debug, Default)]
struct MergeOutcome {
    mapping: BTreeMap<String, serde_json::Value>,
    origins: BTreeMap<String, String>,
    conflicts: Vec<Conflict>,
    parse_failures: Vec<(PathBuf, String)>,
}

fn merge_sources(sources: &[(PathBuf, String)]) -> MergeOutcome {
    let mut mapping: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut origins: BTreeMap<String, String> = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut parse_failures = Vec::new();

    for (path, source) in sources {
        let overrides = match parse_prefab_overrides(source) {
            Ok(overrides) => overrides,
            Err(e) => {
                parse_failures.push((path.clone(), e.to_string()));
                continue;
            }
        };
        let file = path.display().to_string();
        for item in overrides {
            let value = override_json(&item.override_name);
            if let Some(previous) = mapping.get(&item.prefab_name) {
                if previous != &value {
                    conflicts.push(Conflict {
                        prefab_name: item.prefab_name.clone(),
                        previous_file: origins.get(&item.prefab_name).cloned().unwrap_or_default(),
                        new_file: file.clone(),
                    });
                }
            }
            origins.insert(item.prefab_name.clone(), file.clone());
            mapping.insert(item.prefab_name, value);
        }
    }

    MergeOutcome {
        mapping,
        origins,
        conflicts,
        parse_failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with_override(prefab: &str, override_name: &str) -> String {
        format!(
            r#"
local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("{override_name}")
    return inst
end

return Prefab("{prefab}", fn)
"#
        )
    }

    #[test]
    fn test_merge_sources_combines_files() {
        let sources = vec![
            (
                PathBuf::from("prefabs/a.lua"),
                source_with_override("prefab_a", "altar_a"),
            ),
            (
                PathBuf::from("prefabs/b.lua"),
                source_with_override("prefab_b", "altar_b"),
            ),
        ];
        let merged = merge_sources(&sources);
        assert_eq!(merged.mapping.len(), 2);
        assert_eq!(merged.conflicts.len(), 0);
        assert_eq!(merged.parse_failures.len(), 0);
        assert_eq!(
            merged.mapping["prefab_a"]["override_name"],
            serde_json::json!("altar_a")
        );
    }

    #[test]
    fn test_merge_sources_reports_conflicts_and_parse_failures() {
        let sources = vec![
            (
                PathBuf::from("prefabs/a.lua"),
                source_with_override("same", "first"),
            ),
            (
                PathBuf::from("prefabs/b.lua"),
                source_with_override("same", "second"),
            ),
            (
                PathBuf::from("prefabs/broken.lua"),
                "local x = ".to_string(),
            ),
        ];
        let merged = merge_sources(&sources);
        assert_eq!(merged.mapping.len(), 1);
        assert_eq!(merged.conflicts.len(), 1);
        assert_eq!(merged.conflicts[0].prefab_name, "same");
        assert_eq!(merged.conflicts[0].previous_file, "prefabs/a.lua");
        assert_eq!(merged.conflicts[0].new_file, "prefabs/b.lua");
        assert_eq!(merged.parse_failures.len(), 1);
        assert_eq!(merged.mapping["same"]["override_name"], "second");
    }

    #[test]
    fn test_override_json_variants() {
        assert_eq!(override_json(&OverrideValue::Unknown)["type"], "unknown");
        assert_eq!(
            override_json(&OverrideValue::Dynamic("x".to_string()))["type"],
            "dynamic"
        );
    }

    #[test]
    fn test_parse_patch() {
        let text = r#"{
            "schema_version": 1,
            "mappings": {
                "some_prefab": { "base": "base_prefab", "source": "prefabs/x.lua" },
                "other": { "base": "base2" }
            }
        }"#;
        let variants = parse_patch(text).unwrap();
        assert_eq!(variants.len(), 2);
        let some = variants.iter().find(|v| v.prefab == "some_prefab").unwrap();
        assert_eq!(some.base, "base_prefab");
        assert_eq!(some.origin, "patch:prefabs/x.lua");
        let other = variants.iter().find(|v| v.prefab == "other").unwrap();
        assert_eq!(other.origin, "patch:unlabeled");
        assert!(parse_patch("not json").is_err());
    }
}
