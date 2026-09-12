//! 重映射索引的版本管理（remap history / remap-diff）。
//!
//! `anim-remap-index.json` 是 scripts 内容的纯函数（确定性排序输出），
//! 适合按 scripts 版本线快照与 diff。快照挂在 `ANIM__OUT_DIR`
//! （缺省 `output/anim`）的 `history/remaps/<label>.json`，label 取
//! DST `version.txt`（与 anim-sync 的 manifest label 同源）；不进 CAS——
//! 结构化 JSON 需要字段级 diff，CAS 只对二进制资产去重有意义。
//!
//! 解析器本身演进会让同版本重跑不一致（如变量追踪把 static 变
//! resolved），diff 时 `parser_version_changed` 单独提示；confidence 变化
//! 也单独列出，避免混入 entries_changed 制造噪声。

use crate::error::Result;
use crate::parser::anim_override::{Confidence, SymbolRemapEntry};
use crate::parser::clothing_overrides::ClothingEntry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 快照目录：`ANIM__OUT_DIR`（缺省 `output/anim`）/ `history/remaps`。
pub fn remap_history_dir() -> PathBuf {
    crate::platform::config::anim_out_dir().join("history/remaps")
}

/// 反序列化的重映射索引快照。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemapArtifact {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub parser_version: String,
    #[serde(default)]
    pub generated_at: Option<u64>,
    #[serde(default)]
    pub scripts_root: String,
    #[serde(default)]
    pub symbols: BTreeMap<String, Vec<SymbolRemapEntry>>,
    #[serde(default)]
    pub clothing: BTreeMap<String, ClothingEntry>,
}

/// 单条重映射规则的稳定标识（diff 的 key）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemapEntryRef {
    pub symbol: String,
    pub build: String,
    pub src_symbol: String,
    pub api: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemapPrefabsChange {
    #[serde(flatten)]
    pub entry: RemapEntryRef,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemapConfidenceChange {
    #[serde(flatten)]
    pub entry: RemapEntryRef,
    pub old: Confidence,
    pub new: Confidence,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemapClothingChange {
    pub name: String,
    pub old: serde_json::Value,
    pub new: serde_json::Value,
}

/// 两份重映射快照的结构化 diff。
#[derive(Debug, Clone, Serialize)]
pub struct RemapIndexDiff {
    pub from: String,
    pub to: String,
    /// 解析器版本不同：diff 结论里混有解析器升级成分。
    pub parser_version_changed: bool,
    pub symbols_added: Vec<String>,
    pub symbols_removed: Vec<String>,
    pub entries_added: Vec<RemapEntryRef>,
    pub entries_removed: Vec<RemapEntryRef>,
    pub prefabs_changed: Vec<RemapPrefabsChange>,
    pub confidence_changed: Vec<RemapConfidenceChange>,
    pub clothing_added: Vec<String>,
    pub clothing_removed: Vec<String>,
    pub clothing_changed: Vec<RemapClothingChange>,
    pub totals: serde_json::Value,
}

fn entry_ref(symbol: &str, entry: &SymbolRemapEntry) -> RemapEntryRef {
    RemapEntryRef {
        symbol: symbol.to_string(),
        build: entry.build.clone(),
        src_symbol: entry.src_symbol.clone(),
        api: entry.api.as_str().to_string(),
    }
}

/// 对比两份重映射快照。纯函数：输入相同必然输出相同。
pub fn diff_remap_indexes(old: &RemapArtifact, new: &RemapArtifact) -> RemapIndexDiff {
    let mut symbols_added = Vec::new();
    let mut symbols_removed = Vec::new();
    let mut entries_added = Vec::new();
    let mut entries_removed = Vec::new();
    let mut prefabs_changed = Vec::new();
    let mut confidence_changed = Vec::new();

    let mut old_symbols = old.symbols.clone();
    let new_symbols = &new.symbols;
    for (symbol, new_entries) in new_symbols {
        let old_entries = old_symbols.remove(symbol);
        match old_entries {
            None => {
                symbols_added.push(symbol.clone());
                for e in new_entries {
                    entries_added.push(entry_ref(symbol, e));
                }
            }
            Some(old_entries) => {
                let mut old_by_key: BTreeMap<_, &SymbolRemapEntry> = old_entries
                    .iter()
                    .map(|e| ((e.build.as_str(), e.src_symbol.as_str(), e.api), e))
                    .collect();
                for new_entry in new_entries {
                    let key = (
                        new_entry.build.as_str(),
                        new_entry.src_symbol.as_str(),
                        new_entry.api,
                    );
                    match old_by_key.remove(&key) {
                        None => entries_added.push(entry_ref(symbol, new_entry)),
                        Some(old_entry) => {
                            let added: Vec<String> = new_entry
                                .prefabs
                                .iter()
                                .filter(|p| !old_entry.prefabs.contains(p))
                                .cloned()
                                .collect();
                            let removed: Vec<String> = old_entry
                                .prefabs
                                .iter()
                                .filter(|p| !new_entry.prefabs.contains(p))
                                .cloned()
                                .collect();
                            if !added.is_empty() || !removed.is_empty() {
                                prefabs_changed.push(RemapPrefabsChange {
                                    entry: entry_ref(symbol, new_entry),
                                    added,
                                    removed,
                                });
                            }
                            if old_entry.confidence != new_entry.confidence {
                                confidence_changed.push(RemapConfidenceChange {
                                    entry: entry_ref(symbol, new_entry),
                                    old: old_entry.confidence,
                                    new: new_entry.confidence,
                                });
                            }
                        }
                    }
                }
                for (_, old_entry) in old_by_key {
                    entries_removed.push(entry_ref(symbol, old_entry));
                }
            }
        }
    }
    for (symbol, old_entries) in &old_symbols {
        symbols_removed.push(symbol.clone());
        for e in old_entries {
            entries_removed.push(entry_ref(symbol, e));
        }
    }

    let mut clothing_added = Vec::new();
    let mut clothing_removed = Vec::new();
    let mut clothing_changed = Vec::new();
    let mut old_clothing = old.clothing.clone();
    for (name, new_entry) in &new.clothing {
        match old_clothing.remove(name) {
            None => clothing_added.push(name.clone()),
            Some(old_entry) => {
                let old_value = serde_json::to_value(&old_entry).unwrap_or(json!(null));
                let new_value = serde_json::to_value(new_entry).unwrap_or(json!(null));
                if old_value != new_value {
                    clothing_changed.push(RemapClothingChange {
                        name: name.clone(),
                        old: old_value,
                        new: new_value,
                    });
                }
            }
        }
    }
    clothing_removed.extend(old_clothing.into_keys());

    let old_entries: usize = old.symbols.values().map(Vec::len).sum();
    let new_entries: usize = new.symbols.values().map(Vec::len).sum();
    RemapIndexDiff {
        from: old.label.clone(),
        to: new.label.clone(),
        parser_version_changed: old.parser_version != new.parser_version,
        symbols_added,
        symbols_removed,
        entries_added,
        entries_removed,
        prefabs_changed,
        confidence_changed,
        clothing_added,
        clothing_removed,
        clothing_changed,
        totals: json!({
            "from_symbols": old.symbols.len(),
            "to_symbols": new.symbols.len(),
            "from_entries": old_entries,
            "to_entries": new_entries,
            "from_clothing": old.clothing.len(),
            "to_clothing": new.clothing.len(),
        }),
    }
}

/// 保存一份快照（同 label 覆盖，确定性输出保证幂等）。
pub fn save_remap_snapshot(dir: &Path, artifact: &serde_json::Value) -> Result<PathBuf> {
    let label = artifact
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{label}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(artifact)?)?;
    Ok(path)
}

/// 列出快照 label（按文件名字典序升序，与 anim manifest 约定一致）。
pub fn list_remap_labels(dir: &Path) -> Result<Vec<String>> {
    let mut labels = Vec::new();
    if !dir.exists() {
        return Ok(labels);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            labels.push(stem.to_string());
        }
    }
    labels.sort();
    Ok(labels)
}

/// 读取一份快照。
pub fn load_remap_snapshot(dir: &Path, label: &str) -> Result<RemapArtifact> {
    let path = dir.join(format!("{label}.json"));
    let text = std::fs::read_to_string(&path).map_err(|e| {
        crate::error::Error::Io(std::io::Error::new(
            e.kind(),
            format!("读取 {}: {}", path.display(), e),
        ))
    })?;
    serde_json::from_str(&text).map_err(|e| {
        crate::error::Error::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("解析 {}: {}", path.display(), e),
        )))
    })
}

/// 快照汇总信息（列表接口用）。
pub fn remap_snapshot_summary(artifact: &RemapArtifact) -> serde_json::Value {
    let entries: usize = artifact.symbols.values().map(Vec::len).sum();
    json!({
        "label": artifact.label,
        "parser_version": artifact.parser_version,
        "generated_at": artifact.generated_at,
        "scripts_root": artifact.scripts_root,
        "symbols": artifact.symbols.len(),
        "entries": entries,
        "clothing": artifact.clothing.len(),
    })
}

/// Confidence / OverrideApi 用于测试比较的集合工具。
pub fn entry_key_set(artifact: &RemapArtifact) -> BTreeSet<(String, String, String, String)> {
    artifact
        .symbols
        .iter()
        .flat_map(|(symbol, entries)| {
            entries.iter().map(move |e| {
                (
                    symbol.clone(),
                    e.build.clone(),
                    e.src_symbol.clone(),
                    e.api.as_str().to_string(),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::OverrideApi;

    fn artifact(label: &str, symbols: BTreeMap<String, Vec<SymbolRemapEntry>>) -> RemapArtifact {
        RemapArtifact {
            label: label.to_string(),
            parser_version: "1.0.0".to_string(),
            generated_at: None,
            scripts_root: String::new(),
            symbols,
            clothing: BTreeMap::new(),
        }
    }

    fn entry(build: &str, src: &str, confidence: Confidence) -> SymbolRemapEntry {
        SymbolRemapEntry {
            build: build.to_string(),
            src_symbol: src.to_string(),
            api: OverrideApi::OverrideSymbol,
            confidence,
            prefabs: Vec::new(),
        }
    }

    #[test]
    fn diff_symbols_and_entries() {
        let mut old = BTreeMap::new();
        old.insert(
            "swap_hat".to_string(),
            vec![entry("hat_a", "swap_hat", Confidence::Static)],
        );
        old.insert(
            "gone".to_string(),
            vec![entry("b", "gone", Confidence::Static)],
        );
        let mut new = BTreeMap::new();
        new.insert(
            "swap_hat".to_string(),
            vec![
                entry("hat_a", "swap_hat", Confidence::Static),
                entry("hat_b", "swap_hat", Confidence::Static),
            ],
        );
        new.insert("fresh".to_string(), vec![]);

        let diff = diff_remap_indexes(&artifact("1", old), &artifact("2", new));
        assert_eq!(diff.symbols_added, vec!["fresh"]);
        assert_eq!(diff.symbols_removed, vec!["gone"]);
        assert_eq!(diff.entries_added.len(), 1);
        assert_eq!(diff.entries_added[0].build, "hat_b");
        assert_eq!(diff.entries_removed.len(), 1);
        assert_eq!(diff.entries_removed[0].symbol, "gone");
        assert!(!diff.parser_version_changed);
        assert!(diff.clothing_added.is_empty());
    }

    #[test]
    fn diff_confidence_change_is_separate() {
        let mut old = BTreeMap::new();
        old.insert("s".to_string(), vec![entry("b", "s", Confidence::Static)]);
        let mut new = BTreeMap::new();
        new.insert("s".to_string(), vec![entry("b", "s", Confidence::Resolved)]);
        let diff = diff_remap_indexes(&artifact("1", old), &artifact("2", new));
        assert!(diff.entries_added.is_empty());
        assert!(diff.entries_removed.is_empty());
        assert_eq!(diff.confidence_changed.len(), 1);
        assert_eq!(diff.confidence_changed[0].old, Confidence::Static);
        assert_eq!(diff.confidence_changed[0].new, Confidence::Resolved);
    }

    #[test]
    fn diff_prefab_changes() {
        let mut old_map = BTreeMap::new();
        old_map.insert(
            "s".to_string(),
            vec![SymbolRemapEntry {
                build: "b".into(),
                src_symbol: "s".into(),
                api: OverrideApi::OverrideSymbol,
                confidence: Confidence::Static,
                prefabs: vec!["a".into()],
            }],
        );
        let mut new_map = BTreeMap::new();
        new_map.insert(
            "s".to_string(),
            vec![SymbolRemapEntry {
                build: "b".into(),
                src_symbol: "s".into(),
                api: OverrideApi::OverrideSymbol,
                confidence: Confidence::Static,
                prefabs: vec!["b".into(), "a".into(), "c".into()],
            }],
        );
        let diff = diff_remap_indexes(&artifact("1", old_map), &artifact("2", new_map));
        assert_eq!(diff.prefabs_changed.len(), 1);
        assert_eq!(diff.prefabs_changed[0].added, vec!["b", "c"]);
        assert!(diff.prefabs_changed[0].removed.is_empty());
    }

    #[test]
    fn diff_clothing_coarse() {
        let mut old = artifact("1", BTreeMap::new());
        old.clothing.insert(
            "body_a".to_string(),
            ClothingEntry {
                clothing_type: Some("body".into()),
                build_name_override: None,
                symbol_overrides: vec!["torso".into()],
                symbol_overrides_by_character: BTreeMap::new(),
                skintype_overrides: BTreeMap::new(),
                symbol_hides: Vec::new(),
                base_fallbacks: Vec::new(),
            },
        );
        old.clothing.insert(
            "body_gone".to_string(),
            ClothingEntry {
                clothing_type: None,
                build_name_override: None,
                symbol_overrides: Vec::new(),
                symbol_overrides_by_character: BTreeMap::new(),
                skintype_overrides: BTreeMap::new(),
                symbol_hides: Vec::new(),
                base_fallbacks: Vec::new(),
            },
        );
        let mut new = artifact("2", BTreeMap::new());
        new.clothing.insert(
            "body_a".to_string(),
            ClothingEntry {
                clothing_type: Some("body".into()),
                build_name_override: None,
                symbol_overrides: vec!["torso", "arm_upper"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                symbol_overrides_by_character: BTreeMap::new(),
                skintype_overrides: BTreeMap::new(),
                symbol_hides: Vec::new(),
                base_fallbacks: Vec::new(),
            },
        );
        let diff = diff_remap_indexes(&old, &new);
        assert_eq!(diff.clothing_added, Vec::<String>::new());
        assert_eq!(diff.clothing_removed, vec!["body_gone"]);
        assert_eq!(diff.clothing_changed.len(), 1);
        assert_eq!(diff.clothing_changed[0].name, "body_a");
    }

    #[test]
    fn parser_version_change_flagged() {
        let mut old = artifact("1", BTreeMap::new());
        old.parser_version = "0.9.0".into();
        let new = artifact("2", BTreeMap::new());
        let diff = diff_remap_indexes(&old, &new);
        assert!(diff.parser_version_changed);
    }

    #[test]
    fn snapshot_roundtrip() {
        let dir = std::env::temp_dir().join(format!("remap_hist_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let artifact = json!({
            "label": "100",
            "parser_version": "1.0.0",
            "symbols": {},
            "clothing": {},
        });
        save_remap_snapshot(&dir, &artifact).unwrap();
        assert_eq!(list_remap_labels(&dir).unwrap(), vec!["100"]);
        let loaded = load_remap_snapshot(&dir, "100").unwrap();
        assert_eq!(loaded.label, "100");
        std::fs::remove_dir_all(&dir).ok();
    }
}
