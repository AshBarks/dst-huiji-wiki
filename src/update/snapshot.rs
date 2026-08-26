//! SnapshotStore: discovery of manual game-script snapshots.
//!
//! The game install keeps update history as sibling directories
//! `databundles/scripts_<yyyymmddhhmm>` next to the live `scripts` tree —
//! 40+ snapshots of free golden test data.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::Result;

/// One discovered script snapshot.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Snapshot {
    /// Directory name, e.g. `scripts_202605291134`.
    pub name: String,
    /// Timestamp part, e.g. `202605291134`.
    pub timestamp: String,
}

/// Snapshot registry rooted at the `databundles` directory.
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default store from the `DST__ROOT` env var (`<root>/data/databundles`).
    pub fn from_env() -> Result<Self> {
        let game_root = std::env::var("DST__ROOT")
            .map_err(|_| crate::error::Error::EnvVarNotFound("DST__ROOT".to_string()))?;
        Ok(Self::new(
            Path::new(&game_root).join("data").join("databundles"),
        ))
    }

    /// All snapshots sorted by timestamp (oldest first).
    pub fn list(&self) -> Result<Vec<Snapshot>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let ts = name
                .strip_prefix("scripts_")
                .filter(|ts| ts.len() == 12 && ts.chars().all(|c| c.is_ascii_digit()))
                .map(str::to_string);
            if let Some(timestamp) = ts {
                out.push(Snapshot { name, timestamp });
            }
        }
        out.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(out)
    }

    /// Path of the live scripts tree.
    pub fn current_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }

    /// Resolve a snapshot by bare timestamp or full directory name.
    pub fn resolve(&self, id: &str) -> PathBuf {
        let name = if id.starts_with("scripts_") {
            id.to_string()
        } else {
            format!("scripts_{id}")
        };
        self.root.join(name)
    }

    /// Directory containing human-made golden diffs (e.g. `0529.diff`).
    pub fn golden_diff_dir(&self) -> PathBuf {
        self.root.clone()
    }
}

/// Paths changed in a human-made git-style diff file (`diff --git
/// a/scripts_<ts>/<rel> b/scripts/<rel>` headers only).
pub fn golden_diff_paths(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("diff --git a/scripts_"))
        .filter_map(|rest| {
            let rel = rest.split_once('/')?.1.split(" b/").next()?;
            Some(rel.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::diffdata::TreeDiff;
    use std::collections::BTreeSet;

    fn make_store(tag: &str) -> SnapshotStore {
        let root = std::env::temp_dir().join(format!("dst_snap_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for name in ["scripts_202601011200", "scripts_202605291134", "scripts"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(root.join("hashes.txt"), "x").unwrap();
        SnapshotStore::new(&root)
    }

    #[test]
    fn lists_only_snapshot_dirs_sorted() {
        let store = make_store("list");
        let snaps = store.list().unwrap();
        assert_eq!(
            snaps,
            vec![
                Snapshot {
                    name: "scripts_202601011200".into(),
                    timestamp: "202601011200".into()
                },
                Snapshot {
                    name: "scripts_202605291134".into(),
                    timestamp: "202605291134".into()
                },
            ]
        );
        let _ = std::fs::remove_dir_all(store.root);
    }

    #[test]
    fn resolve_accepts_bare_and_full_names() {
        let store = make_store("resolve");
        assert_eq!(
            store.resolve("202601011200"),
            store.root.join("scripts_202601011200")
        );
        assert_eq!(
            store.resolve("scripts_202601011200"),
            store.root.join("scripts_202601011200")
        );
        assert_eq!(store.current_dir(), store.root.join("scripts"));
        let _ = std::fs::remove_dir_all(store.root);
    }

    #[test]
    fn golden_paths_parse_headers() {
        let text = "\
diff --git a/scripts_202605291134/actions.lua b/scripts/actions.lua
index 111..222 100644
--- a/scripts_202605291134/actions.lua
+++ b/scripts/actions.lua
@@ -1 +1 @@
-x
+y
diff --git a/scripts_202605291134/prefabs/hound.lua b/scripts/prefabs/hound.lua
";
        let paths = golden_diff_paths(text);
        assert_eq!(
            paths,
            BTreeSet::from(["actions.lua".to_string(), "prefabs/hound.lua".to_string()])
        );
    }

    /// Golden replay: the manual update workflow renames the pre-update
    /// tree to `scripts_<update-time>` before extracting the new one, so
    /// the human-made 0529.diff compares `scripts_202605291134` against
    /// the LIVE `scripts` tree (valid until the next official update).
    fn dst_root_from_env_or_dotenv() -> Option<String> {
        std::env::var("DST__ROOT").ok().or_else(|| {
            std::fs::read_to_string(".env").ok().and_then(|content| {
                content.lines().find_map(|line| {
                    line.strip_prefix("DST__ROOT=")
                        .map(|v| v.trim_matches('"').trim().to_string())
                        .filter(|v| !v.is_empty())
                })
            })
        })
    }

    #[test]
    fn golden_diff_replay_0529() {
        let Some(root) = dst_root_from_env_or_dotenv() else {
            eprintln!("skip: DST__ROOT not set");
            return;
        };
        let store =
            SnapshotStore::new(std::path::Path::new(&root).join("data").join("databundles"));
        let old = store.resolve("202605291134");
        let new = store.current_dir();
        if !old.is_dir() || !new.is_dir() {
            eprintln!("skip: golden snapshots not installed");
            return;
        }
        let td = TreeDiff::diff_trees(&old, &new).unwrap();
        let golden_text =
            std::fs::read_to_string(store.golden_diff_dir().join("0529.diff")).unwrap();
        let golden = golden_diff_paths(&golden_text);
        let engine: BTreeSet<String> = td.changed_paths().into_iter().map(str::to_string).collect();

        // The live tree keeps drifting after every unrecorded official
        // update, so exact equality cannot hold forever. Require that the
        // engine reproduces the overwhelming majority of the human list and
        // surface any drift explicitly.
        let covered = golden.intersection(&engine).count();
        let ratio = covered as f64 / golden.len().max(1) as f64;
        let missing: Vec<_> = golden.difference(&engine).collect();
        let extra: Vec<_> = engine.difference(&golden).collect();
        let missing_sample: Vec<_> = missing.iter().take(5).collect();
        eprintln!(
            "golden replay: golden={} engine={} covered={} ({:.3}) extra_drift={} missing_sample={:?}",
            golden.len(),
            engine.len(),
            covered,
            ratio,
            extra.len(),
            missing_sample
        );
        assert!(
            ratio >= 0.95,
            "golden coverage {ratio:.3} below threshold; missing={missing:?}"
        );
        assert!(
            engine.len() >= 100,
            "engine change count suspiciously small: {}",
            engine.len()
        );
    }
}
