//! 动画目录扫描：递归收集 `.zip` 与 `.dyn`，计算文件级 SHA-256。
//!
//! 相对路径统一使用 `/` 分隔，便于作为 manifest 的稳定 key。

use crate::error::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 动画文件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AnimFileKind {
    Zip,
    Dyn,
}

impl AnimFileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AnimFileKind::Zip => "zip",
            AnimFileKind::Dyn => "dyn",
        }
    }
}

/// 一个动画文件在快照中的记录。
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    /// 相对 `data/anim` 的路径，使用 `/` 分隔；例如 `dynamic/abigail_ice.dyn`。
    pub rel_path: String,
    /// 磁盘上的实际路径。
    #[serde(skip)]
    pub path: PathBuf,
    pub kind: AnimFileKind,
    pub sha256: String,
    pub size: u64,
}

impl FileEntry {
    pub fn is_dyn(&self) -> bool {
        self.kind == AnimFileKind::Dyn
    }
}

/// 扫描 `data/anim`（或任意动画目录），返回相对路径 → 文件记录。
///
/// 只收集 `.zip` 和 `.dyn`；递归遍历子目录，因此 `dynamic/*.dyn` 也会被包含。
pub fn scan_anim_dir(root: &Path) -> Result<BTreeMap<String, FileEntry>> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return Ok(files);
    }
    collect_dir(root, root, &mut files)?;
    Ok(files)
}

fn collect_dir(root: &Path, dir: &Path, out: &mut BTreeMap<String, FileEntry>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_dir(root, &path, out)?;
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let kind = match ext.as_str() {
            "zip" => AnimFileKind::Zip,
            "dyn" => AnimFileKind::Dyn,
            _ => continue,
        };
        let rel_path = path
            .strip_prefix(root)
            .map_err(|e| crate::error::Error::InvalidPath(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let sha256 = crate::scripts_sync::anim::history::hash_file(&path)?;
        let size = std::fs::metadata(&path)?.len();
        out.insert(
            rel_path.clone(),
            FileEntry {
                rel_path,
                path,
                kind,
                sha256,
                size,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_picks_zip_and_dyn() {
        let dir = std::env::temp_dir().join(format!("anim_scan_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("dynamic")).unwrap();
        std::fs::write(dir.join("a.zip"), b"zip").unwrap();
        std::fs::write(dir.join("dynamic/b.dyn"), b"dyn").unwrap();
        std::fs::write(dir.join("ignore.txt"), b"txt").unwrap();
        let files = scan_anim_dir(&dir).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains_key("a.zip"));
        assert!(files.contains_key("dynamic/b.dyn"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
