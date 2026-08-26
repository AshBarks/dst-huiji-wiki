//! Structured tree diff between two game-script snapshots.
//!
//! Produces per-file [`FileDiff`] with line-range hunks (no context lines —
//! the ranges feed hunk→prefab attribution in M1b) plus a human-readable
//! unified patch. Line diffing uses `similar`'s Histogram algorithm, matching
//! `crate::utils::diff_lines`.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;
use similar::{DiffOp, TextDiff};

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    Added,
    Removed,
    Modified,
}

/// One contiguous changed range pair (context-free).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Hunk {
    /// 1-based start line in the old file (0 for pure additions).
    pub old_start: u32,
    pub old_lines: u32,
    /// 1-based start line in the new file (0 for pure deletions).
    pub new_start: u32,
    pub new_lines: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDiff {
    /// Path relative to the scripts root, `/`-separated.
    pub path: String,
    pub status: DiffStatus,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TreeDiff {
    pub files: Vec<FileDiff>,
}

impl TreeDiff {
    /// Diff every `.lua` file under both roots.
    pub fn diff_trees(old_root: &Path, new_root: &Path) -> Result<TreeDiff> {
        let old_files = collect_lua_files(old_root)?;
        let new_files = collect_lua_files(new_root)?;

        let mut files = Vec::new();
        for path in old_files.union(&new_files) {
            let status = match (old_files.contains(path), new_files.contains(path)) {
                (true, true) => DiffStatus::Modified,
                (false, true) => DiffStatus::Added,
                (true, false) => DiffStatus::Removed,
                _ => continue,
            };
            let hunks = if status == DiffStatus::Modified {
                let old_text = read_lossy(&old_root.join(path))?;
                let new_text = read_lossy(&new_root.join(path))?;
                extract_hunks(&old_text, &new_text)
            } else {
                Vec::new()
            };
            // Content-identical entries are not changes.
            if status == DiffStatus::Modified && hunks.is_empty() {
                continue;
            }
            files.push(FileDiff {
                path: path.clone(),
                status,
                hunks,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(TreeDiff { files })
    }

    /// Set of all changed paths (added + removed + modified).
    pub fn changed_paths(&self) -> BTreeSet<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// Human-readable multi-file unified patch (`changes.patch`).
    pub fn to_patch(&self, old_root: &Path, new_root: &Path) -> Result<String> {
        let mut out = String::new();
        for file in &self.files {
            out.push_str(&format!("--- a/{}\n+++ b/{}\n", file.path, file.path));
            match file.status {
                DiffStatus::Modified => {
                    let old = read_lossy(&old_root.join(&file.path))?;
                    let new = read_lossy(&new_root.join(&file.path))?;
                    out.push_str(&crate::utils::diff_lines(&old, &new));
                }
                DiffStatus::Added => {
                    let new = read_lossy(&new_root.join(&file.path))?;
                    out.push_str(&crate::utils::diff_lines("", &new));
                }
                DiffStatus::Removed => {
                    let old = read_lossy(&old_root.join(&file.path))?;
                    out.push_str(&crate::utils::diff_lines(&old, ""));
                }
            }
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
        Ok(out)
    }

    /// Paths of modified files whose hunk ranges touch `line` (1-based, new
    /// file). Used by M1b attribution; kept here to co-locate hunk logic.
    pub fn is_modified_at(&self, path: &str, new_line: u32) -> bool {
        self.files.iter().any(|f| {
            f.path == path
                && f.status == DiffStatus::Modified
                && f.hunks.iter().any(|h| {
                    h.new_lines > 0
                        && new_line >= h.new_start
                        && new_line < h.new_start + h.new_lines
                })
        })
    }
}

/// Read a script file tolerating non-UTF-8 bytes (a few DST files are not
/// valid UTF-8); line diffing operates on the lossy text.
fn read_lossy(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn extract_hunks(old_text: &str, new_text: &str) -> Vec<Hunk> {
    let diff = TextDiff::configure()
        .algorithm(similar::Algorithm::Histogram)
        .diff_lines(old_text, new_text);
    diff.ops()
        .iter()
        .filter_map(|op| match op {
            DiffOp::Equal { .. } => None,
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
                ..
            } => Some(Hunk {
                old_start: *old_index as u32 + 1,
                old_lines: *old_len as u32,
                new_start: *new_index as u32,
                new_lines: 0,
            }),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
                ..
            } => Some(Hunk {
                old_start: *old_index as u32 + 1,
                old_lines: 0,
                new_start: *new_index as u32 + 1,
                new_lines: *new_len as u32,
            }),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
                ..
            } => Some(Hunk {
                old_start: *old_index as u32 + 1,
                old_lines: *old_len as u32,
                new_start: *new_index as u32 + 1,
                new_lines: *new_len as u32,
            }),
        })
        .collect()
}

/// Recursive `.lua` collection with normalized `/` separators.
fn collect_lua_files(root: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    fn walk(dir: &Path, rel: &str, out: &mut BTreeSet<String>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), &child_rel, out)?;
            } else if name.ends_with(".lua") {
                out.insert(child_rel);
            }
        }
        Ok(())
    }
    walk(root, "", &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_A: &str = "local a = 1\nlocal b = 2\nlocal c = 3\nlocal keep = 4\n";
    const NEW_A: &str = "local a = 1\nlocal b = 20\nlocal c = 3\nlocal keep = 4\nlocal d = 5\n";

    fn write_tree(dir: &Path, files: &[(&str, &str)]) {
        for (path, content) in files {
            let abs = dir.join(path);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(abs, content).unwrap();
        }
    }

    #[test]
    fn statuses_and_hunks_are_structured() {
        let tmp = std::env::temp_dir().join(format!("dst_diff_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let old = tmp.join("old");
        let new = tmp.join("new");
        write_tree(&old, &[("a.lua", OLD_A), ("gone.lua", "x=1\n")]);
        write_tree(&new, &[("a.lua", NEW_A), ("fresh.lua", "y=2\n")]);

        let td = TreeDiff::diff_trees(&old, &new).unwrap();
        assert_eq!(td.files.len(), 3);

        let a = td.files.iter().find(|f| f.path == "a.lua").unwrap();
        assert_eq!(a.status, DiffStatus::Modified);
        assert_eq!(a.hunks.len(), 2);
        assert!(td.is_modified_at("a.lua", 2)); // changed line
        assert!(!td.is_modified_at("a.lua", 4)); // untouched line

        assert_eq!(
            td.files
                .iter()
                .find(|f| f.path == "gone.lua")
                .unwrap()
                .status,
            DiffStatus::Removed
        );
        assert_eq!(
            td.files
                .iter()
                .find(|f| f.path == "fresh.lua")
                .unwrap()
                .status,
            DiffStatus::Added
        );

        assert_eq!(
            td.changed_paths(),
            BTreeSet::from(["a.lua", "gone.lua", "fresh.lua"])
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn identical_trees_produce_empty_diff() {
        let tmp = std::env::temp_dir().join(format!("dst_diff_same_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        write_tree(&tmp, &[("x/y.lua", "return 1\n")]);
        let td = TreeDiff::diff_trees(&tmp, &tmp).unwrap();
        assert!(td.files.is_empty());
        assert_eq!(td.to_patch(&tmp, &tmp).unwrap(), "");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn patch_contains_file_headers() {
        let tmp = std::env::temp_dir().join(format!("dst_diff_patch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let old = tmp.join("old");
        let new = tmp.join("new");
        write_tree(&old, &[("m.lua", "a\n")]);
        write_tree(&new, &[("m.lua", "b\n")]);
        let td = TreeDiff::diff_trees(&old, &new).unwrap();
        let patch = td.to_patch(&old, &new).unwrap();
        assert!(patch.contains("--- a/m.lua"));
        assert!(patch.contains("+++ b/m.lua"));
        assert!(patch.contains("-a"));
        assert!(patch.contains("+b"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
