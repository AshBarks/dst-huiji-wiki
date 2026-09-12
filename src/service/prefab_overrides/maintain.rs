//! `maintain-prefab-overrides`：只读维护任务（当前仅 dry-run diff）。
//!
//! 把本地推导（AST + 数据族 + 补丁）渲染为 `模块:ItemTable/PrefabOverrides`
//! 的页面文本，与线上页面做 merge-preserving diff：
//! - `updates`：线上已有且值不同的键（大小写差异不算变化）；
//! - `additions`：数据族新增的键（AST 独有键只记入 `unmatched_local`，
//!   避免把解析误报写进页面）；
//! - 不产出删除项，identity 条目原样保留。
//!
//! **本任务不写维基**：只产出 diff 报告与可选的本地页面文本。

use super::audit::{parse_wiki_overrides, read_page_raw, resolve_scripts_root, PAGE_TITLE};
use super::derive_from_scripts_root;
use crate::error::Result;
use crate::platform::progress::Reporter;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DIFF_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiffEntry {
    pub prefab: String,
    pub old: Option<String>,
    pub new: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffStats {
    pub online: usize,
    pub derived: usize,
    pub merged: usize,
    pub updates: usize,
    pub additions: usize,
    pub unmatched_local: usize,
    pub retained_identities: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrefabOverridesDiff {
    pub schema_version: u32,
    pub page: String,
    pub scripts_root: String,
    pub stats: DiffStats,
    pub updates: Vec<DiffEntry>,
    pub additions: Vec<DiffEntry>,
    pub retained_identities: Vec<String>,
    pub unmatched_local: Vec<String>,
}

/// 渲染为线上页面格式（4 空格缩进、`[[ ]]` 包裹 + `jsonDecode`）。
pub fn render_wiki_page(map: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::from("local overrides = [[\n{\n");
    for (index, (key, value)) in map.iter().enumerate() {
        let key = serde_json::to_string(key)?;
        let value = serde_json::to_string(value)?;
        out.push_str(&format!("    {key}: {value}"));
        if index + 1 < map.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("}\n]]\nreturn mw.text.jsonDecode(overrides)\n");
    Ok(out)
}

/// merge-preserving diff（纯函数）。
pub(crate) fn build_diff(
    online: &BTreeMap<String, String>,
    derived: &BTreeMap<String, serde_json::Value>,
    scripts_root: &Path,
) -> PrefabOverridesDiff {
    let mut merged = online.clone();
    let mut updates = Vec::new();
    let mut additions = Vec::new();
    let mut unmatched_local = Vec::new();
    let mut retained_identities = Vec::new();

    for (prefab, value) in online {
        if prefab == value {
            retained_identities.push(prefab.clone());
        }
    }

    for (prefab, value) in derived {
        let Some(target) = value.get("override_name").and_then(|v| v.as_str()) else {
            continue;
        };
        match merged.get(prefab) {
            Some(old) if prefab == old => {}
            Some(old) if old.eq_ignore_ascii_case(target) => {}
            Some(old) => {
                updates.push(DiffEntry {
                    prefab: prefab.clone(),
                    old: Some(old.clone()),
                    new: target.to_string(),
                });
                merged.insert(prefab.clone(), target.to_string());
            }
            None => {
                let kind = value
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if kind == "family" || kind == "patch" {
                    additions.push(DiffEntry {
                        prefab: prefab.clone(),
                        old: None,
                        new: target.to_string(),
                    });
                    merged.insert(prefab.clone(), target.to_string());
                } else {
                    unmatched_local.push(prefab.clone());
                }
            }
        }
    }

    PrefabOverridesDiff {
        schema_version: DIFF_SCHEMA_VERSION,
        page: PAGE_TITLE.to_string(),
        scripts_root: scripts_root.display().to_string(),
        stats: DiffStats {
            online: online.len(),
            derived: derived.len(),
            merged: merged.len(),
            updates: updates.len(),
            additions: additions.len(),
            unmatched_local: unmatched_local.len(),
            retained_identities: retained_identities.len(),
        },
        updates,
        additions,
        retained_identities,
        unmatched_local,
    }
}

/// `maintain-prefab-overrides` 入口：只读，产出 diff 与可选本地页面文本。
pub async fn run_maintain_prefab_overrides(
    scripts: Option<&str>,
    wiki_file: Option<&str>,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("维护预制体重定向（只读 diff）");
    let root = resolve_scripts_root(scripts)?;
    if !root.is_dir() {
        return Err(crate::error::Error::Config(format!(
            "脚本根目录不存在：{}",
            root.display()
        )));
    }
    reporter.log(format!("脚本根：{}", root.display()));

    reporter.stage("本地推导");
    let derived = derive_from_scripts_root(&root, reporter)?;

    reporter.stage("获取线上页面（只读）");
    let raw = read_page_raw(wiki_file).await?;
    let online = parse_wiki_overrides(&raw)?;

    let diff = build_diff(&online, &derived.mapping, &root);
    reporter.log(format!(
        "线上 {} / 本地 {} / 合并 {}（更新 {}，新增 {}，保留 identity {}，未匹配本地 {}）",
        diff.stats.online,
        diff.stats.derived,
        diff.stats.merged,
        diff.stats.updates,
        diff.stats.additions,
        diff.stats.retained_identities,
        diff.stats.unmatched_local
    ));
    for entry in &diff.updates {
        reporter.log(format!(
            "更新 {}：{} -> {}",
            entry.prefab,
            entry.old.as_deref().unwrap_or_default(),
            entry.new
        ));
    }
    for entry in &diff.additions {
        reporter.log(format!("新增 {}：{}", entry.prefab, entry.new));
    }

    let merged_map: BTreeMap<String, String> = diff
        .updates
        .iter()
        .chain(diff.additions.iter())
        .fold(online.clone(), |mut acc, entry| {
            acc.insert(entry.prefab.clone(), entry.new.clone());
            acc
        });

    if let Some(dir) = output {
        std::fs::create_dir_all(&dir)?;
        let page_path = dir.join("PrefabOverrides.lua");
        crate::platform::fs::write_text_atomic(&page_path, render_wiki_page(&merged_map)?)?;
        let diff_path = dir.join("diff.json");
        crate::platform::fs::write_text_atomic(&diff_path, serde_json::to_string_pretty(&diff)?)?;
        reporter.log(format!("页面文本：{}", page_path.display()));
        reporter.log(format!("diff 报告：{}", diff_path.display()));
    } else {
        reporter.log("未指定 --output，未写出本地产物".to_string());
    }

    Ok(serde_json::json!({
        "schema_version": DIFF_SCHEMA_VERSION,
        "page": PAGE_TITLE,
        "stats": diff.stats,
        "wrote_wiki": false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derived(mapping: &[(&str, &str, &str)]) -> BTreeMap<String, serde_json::Value> {
        mapping
            .iter()
            .map(|(prefab, target, kind)| {
                (
                    prefab.to_string(),
                    serde_json::json!({ "override_name": target, "type": kind }),
                )
            })
            .collect()
    }

    #[test]
    fn test_render_wiki_page_roundtrip() {
        let mut map = BTreeMap::new();
        map.insert("a".to_string(), "b".to_string());
        map.insert("c".to_string(), "d".to_string());
        let text = render_wiki_page(&map).unwrap();
        assert!(text.starts_with("local overrides = [[\n{\n"));
        assert!(text.contains("    \"a\": \"b\",\n"));
        assert!(text.ends_with("}\n]]\nreturn mw.text.jsonDecode(overrides)\n"));
        let parsed = parse_wiki_overrides(&text).unwrap();
        assert_eq!(parsed, map);
    }

    #[test]
    fn test_build_diff_merge_preserving() {
        let online: BTreeMap<String, String> = [
            ("same", "target_a"),
            ("changed", "old_target"),
            ("identity", "identity"),
            ("noop", "noop"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let derived = derived(&[
            ("same", "TARGET_A", "static"),
            ("changed", "new_target", "static"),
            ("new_family", "new_base", "family"),
            ("new_ast", "some_base", "static"),
            ("noop", "conflicting_base", "static"),
        ]);
        let diff = build_diff(&online, &derived, Path::new("/scripts"));
        assert_eq!(diff.stats.updates, 1);
        assert_eq!(diff.updates[0].prefab, "changed");
        assert_eq!(diff.updates[0].old.as_deref(), Some("old_target"));
        assert_eq!(diff.additions.len(), 1);
        assert_eq!(diff.additions[0].prefab, "new_family");
        assert_eq!(diff.unmatched_local, vec!["new_ast"]);
        assert_eq!(diff.retained_identities.len(), 2);
        assert_eq!(diff.stats.merged, online.len() + 1);
    }
}
