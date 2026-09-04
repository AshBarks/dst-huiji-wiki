//! 结构化 diff 计算。
//!
//! 第一版使用规范化 DTO 的 JSON 递归 diff：
//! - object 按 key 递归；
//! - array 按下标递归（normalize 阶段已经排序，所以下标在大多数情况下是稳定键）；
//! - 叶子变更输出 `path` + `old` + `new`。

use crate::error::Result;
use crate::scripts_sync::anim::archive::ParsedArchiveData;
use crate::scripts_sync::anim::history::{diff_file_maps, AnimFileEntry, FileDiff};
use crate::scripts_sync::anim::snapshot::FileEntry;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// 一条字段级变更。
#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub path: String,
    pub old: Value,
    pub new: Value,
}

/// 单个动画包的结构化 diff。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArchiveDiff {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anim: Option<SideDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<SideDiff>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tex_changed: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideDiff {
    pub changed: bool,
    pub summary: serde_json::Value,
    pub details: Vec<Change>,
}

/// 两个动画目录的文件级 diff。
#[derive(Debug, Clone, Serialize)]
pub struct AnimDirDiff {
    pub old_label: String,
    pub new_label: String,
    pub file_diff: FileDiff,
    pub archives: BTreeMap<String, ArchiveDiff>,
}

/// 比较两份文件清单，并对新增/修改的文件做结构化解析。
pub fn diff_directories(
    old_label: &str,
    new_label: &str,
    old_files: &BTreeMap<String, FileEntry>,
    new_files: &BTreeMap<String, FileEntry>,
    only: Option<&str>,
    parse_all: bool,
) -> Result<AnimDirDiff> {
    let old_entries: BTreeMap<String, AnimFileEntry> = old_files
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                AnimFileEntry {
                    kind: v.kind.as_str().to_string(),
                    sha256: v.sha256.clone(),
                    size: v.size,
                    anim_sha256: None,
                    build_sha256: None,
                    tex: BTreeMap::new(),
                },
            )
        })
        .collect();
    let new_entries: BTreeMap<String, AnimFileEntry> = new_files
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                AnimFileEntry {
                    kind: v.kind.as_str().to_string(),
                    sha256: v.sha256.clone(),
                    size: v.size,
                    anim_sha256: None,
                    build_sha256: None,
                    tex: BTreeMap::new(),
                },
            )
        })
        .collect();

    let file_diff = diff_file_maps(&old_entries, &new_entries);
    let mut archives = BTreeMap::new();

    // 只处理新增/修改/被指定文件；removed 文件无法解析新内容，只记录状态。
    for path in file_diff
        .added
        .iter()
        .chain(file_diff.changed.iter().map(|c| &c.path))
    {
        if let Some(only) = only {
            if only != path {
                continue;
            }
        }
        let Some(new_file) = new_files.get(path) else {
            continue;
        };
        let old_data = if file_diff.changed.iter().any(|c| &c.path == path) {
            old_files.get(path).map(parse_entry).transpose()?
        } else {
            None
        };
        let new_data = parse_entry(new_file)?;
        let status = if file_diff.added.iter().any(|p| p == path) {
            "added"
        } else {
            "modified"
        };
        let diff = diff_archive(old_data.as_ref(), Some(&new_data), status);
        archives.insert(path.clone(), diff);
    }

    for path in &file_diff.removed {
        if let Some(only) = only {
            if only != path {
                continue;
            }
        }
        let diff = ArchiveDiff {
            status: "removed",
            anim: None,
            build: None,
            tex_changed: BTreeMap::new(),
            parse_error: None,
        };
        archives.insert(path.clone(), diff);
    }

    if parse_all {
        // 未变化的文件也解析（通常用于 WebUI 浏览或历史重建）。
        for path in new_files.keys() {
            if archives.contains_key(path) {
                continue;
            }
            if let Some(only) = only {
                if only != path {
                    continue;
                }
            }
            let Some(new_file) = new_files.get(path) else {
                continue;
            };
            let new_data = parse_entry(new_file)?;
            let diff = diff_archive(None, Some(&new_data), "unchanged");
            archives.insert(path.clone(), diff);
        }
    }

    Ok(AnimDirDiff {
        old_label: old_label.to_string(),
        new_label: new_label.to_string(),
        file_diff,
        archives,
    })
}

fn parse_entry(entry: &FileEntry) -> Result<ParsedArchiveData> {
    crate::scripts_sync::anim::archive::parse_archive_file(entry)
}

fn diff_archive(
    old: Option<&ParsedArchiveData>,
    new: Option<&ParsedArchiveData>,
    status: &'static str,
) -> ArchiveDiff {
    let mut out = ArchiveDiff {
        status,
        anim: None,
        build: None,
        tex_changed: BTreeMap::new(),
        parse_error: None,
    };

    if let (Some(old), Some(new)) = (old, new) {
        out.anim = diff_side(old.anim.as_ref(), new.anim.as_ref(), "anim");
        out.build = diff_side(old.build.as_ref(), new.build.as_ref(), "build");
        for (name, hash) in &new.tex_hashes {
            if old.tex_hashes.get(name) != Some(hash) {
                out.tex_changed.insert(
                    name.clone(),
                    old.tex_hashes
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| "<added>".to_string()),
                );
            }
        }
        for name in old.tex_hashes.keys() {
            if !new.tex_hashes.contains_key(name) {
                out.tex_changed
                    .insert(name.clone(), "<removed>".to_string());
            }
        }
    } else if let Some(new) = new {
        out.anim = diff_side(None, new.anim.as_ref(), "anim");
        out.build = diff_side(None, new.build.as_ref(), "build");
    }

    out
}

fn diff_side<T: Serialize>(old: Option<&T>, new: Option<&T>, prefix: &str) -> Option<SideDiff> {
    let old_value = old.map(|v| serde_json::to_value(v).unwrap_or(Value::Null));
    let new_value = new.map(|v| serde_json::to_value(v).unwrap_or(Value::Null));
    let old_value = old_value.as_ref();
    let new_value = new_value.as_ref();
    if old_value.is_none() && new_value.is_none() {
        return None;
    }
    let mut details = Vec::new();
    match (old_value, new_value) {
        (Some(o), Some(n)) => diff_json(o, n, prefix, &mut details),
        (Some(o), None) => details.push(Change {
            path: prefix.to_string(),
            old: o.clone(),
            new: Value::Null,
        }),
        (None, Some(n)) => details.push(Change {
            path: prefix.to_string(),
            old: Value::Null,
            new: n.clone(),
        }),
        (None, None) => {}
    }
    let changed = !details.is_empty();
    Some(SideDiff {
        changed,
        summary: serde_json::json!({
            "changes": details.len(),
            "added": details.iter().filter(|c| c.old.is_null()).count(),
            "removed": details.iter().filter(|c| c.new.is_null()).count(),
        }),
        details,
    })
}

/// 递归比较两个 JSON 值，输出叶子变更。
pub fn diff_json(old: &Value, new: &Value, path: &str, out: &mut Vec<Change>) {
    if old == new {
        return;
    }
    match (old, new) {
        (Value::Object(old_map), Value::Object(new_map)) => {
            let mut keys: Vec<&String> = old_map.keys().chain(new_map.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match (old_map.get(key), new_map.get(key)) {
                    (Some(o), Some(n)) => diff_json(o, n, &child, out),
                    (Some(o), None) => out.push(Change {
                        path: child,
                        old: o.clone(),
                        new: Value::Null,
                    }),
                    (None, Some(n)) => out.push(Change {
                        path: child,
                        old: Value::Null,
                        new: n.clone(),
                    }),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(old_arr), Value::Array(new_arr)) => {
            let len = old_arr.len().max(new_arr.len());
            for i in 0..len {
                let child = format!("{path}[{i}]");
                match (old_arr.get(i), new_arr.get(i)) {
                    (Some(o), Some(n)) => diff_json(o, n, &child, out),
                    (Some(o), None) => out.push(Change {
                        path: child,
                        old: o.clone(),
                        new: Value::Null,
                    }),
                    (None, Some(n)) => out.push(Change {
                        path: child,
                        old: Value::Null,
                        new: n.clone(),
                    }),
                    (None, None) => {}
                }
            }
        }
        _ => out.push(Change {
            path: path.to_string(),
            old: old.clone(),
            new: new.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_diff_reports_leaf_change() {
        let old = json!({"a": 1, "b": {"x": 1}});
        let new = json!({"a": 2, "b": {"x": 1}});
        let mut out = Vec::new();
        diff_json(&old, &new, "", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "a");
    }
}
