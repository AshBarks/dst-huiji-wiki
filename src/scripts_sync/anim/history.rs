//! 动画历史存储：CAS 对象仓 + manifest。
//!
//! 与 `images/history.rs` 思路一致，但这里存的是原始动画包（zip/dyn），
//! 并且 manifest 额外记录 anim/build/tex 的条目级 hash。

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 一个动画文件在 manifest 中的条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimFileEntry {
    pub kind: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_sha256: Option<String>,
    #[serde(default)]
    pub tex: BTreeMap<String, String>,
}

/// 一次 anim-sync 的机器可读 manifest。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    pub complete: bool,
    /// 解析器版本；变化时触发全量重解析。
    #[serde(default)]
    pub parser_version: String,
    #[serde(default)]
    pub synced_at: Option<u64>,
    #[serde(default)]
    pub files: BTreeMap<String, AnimFileEntry>,
    #[serde(default)]
    pub diff: FileDiff,
}

/// 文件级差异（与 images 的 Diff 形状类似）。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub changed: Vec<ChangedEntry>,
}

impl FileDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedEntry {
    pub path: String,
    pub old: String,
    pub new: String,
}

/// 当前 unix 毫秒。
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 流式计算文件 sha256。
pub fn hash_file(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// 计算内存字节 sha256。
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// CAS 对象仓：`<root>/<hash 前两位>/<hash><ext>`。
#[derive(Debug, Clone)]
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path_for(&self, hash: &str, ext: &str) -> PathBuf {
        let prefix = hash.get(..2).unwrap_or(hash);
        self.root.join(prefix).join(format!("{hash}{ext}"))
    }

    pub fn contains(&self, hash: &str, ext: &str) -> bool {
        self.path_for(hash, ext).exists()
    }

    /// 按内容入库；已存在则跳过。返回内容 hash。
    pub fn put_file(&self, src: &Path, ext: &str) -> Result<String> {
        let hash = hash_file(src)?;
        let dest = self.path_for(&hash, ext);
        if !dest.exists() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src, &dest)?;
        }
        Ok(hash)
    }
}

/// manifest 目录。
#[derive(Debug, Clone)]
pub struct ManifestStore {
    dir: PathBuf,
}

impl ManifestStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, label: &str) -> PathBuf {
        self.dir.join(format!("{label}.json"))
    }

    pub fn save(&self, manifest: &Manifest) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path(&manifest.label);
        let json = serde_json::to_string_pretty(manifest)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn load(&self, label: &str) -> Result<Option<Manifest>> {
        let path = self.path(label);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("读取 {}: {}", path.display(), e),
            ))
        })?;
        serde_json::from_str(&text).map(Some).map_err(|e| {
            Error::Json(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("解析 {}: {}", path.display(), e),
            )))
        })
    }

    /// 返回所有已保存的 manifest label（按文件名，不含 `.json`）。
    pub fn list_labels(&self) -> Result<Vec<String>> {
        let mut labels = Vec::new();
        if !self.dir.exists() {
            return Ok(labels);
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
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

    /// 加载最近一份完整且非当前 label 的 manifest 作为 diff 基线。
    pub fn load_parent(&self, current_label: &str) -> Result<Option<Manifest>> {
        if !self.dir.exists() {
            return Ok(None);
        }
        let mut best: Option<Manifest> = None;
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let manifest =
                match self.load(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))? {
                    Some(m) => m,
                    None => continue,
                };
            if !manifest.complete || manifest.label == current_label {
                continue;
            }
            let is_better = match &best {
                None => true,
                Some(b) => numeric_key(&manifest.label) > numeric_key(&b.label),
            };
            if is_better {
                best = Some(manifest);
            }
        }
        Ok(best)
    }
}

fn numeric_key(s: &str) -> (u8, u64, String) {
    match s.parse::<u64>() {
        Ok(n) => (0, n, String::new()),
        Err(_) => (1, 0, s.to_string()),
    }
}

/// 纯函数：比较两份文件清单，得到 added/removed/changed。
pub fn diff_file_maps(
    parent: &BTreeMap<String, AnimFileEntry>,
    new: &BTreeMap<String, AnimFileEntry>,
) -> FileDiff {
    let mut diff = FileDiff::default();
    for (path, old) in parent {
        match new.get(path) {
            None => diff.removed.push(path.clone()),
            Some(new_entry) if new_entry.sha256 != old.sha256 => diff.changed.push(ChangedEntry {
                path: path.clone(),
                old: old.sha256.clone(),
                new: new_entry.sha256.clone(),
            }),
            Some(_) => {}
        }
    }
    for path in new.keys() {
        if !parent.contains_key(path) {
            diff.added.push(path.clone());
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_file_maps_basic() {
        let old = BTreeMap::from([
            (
                "a.zip".to_string(),
                AnimFileEntry {
                    kind: "zip".into(),
                    sha256: "old".into(),
                    size: 1,
                    anim_sha256: None,
                    build_sha256: None,
                    tex: BTreeMap::new(),
                },
            ),
            (
                "b.zip".to_string(),
                AnimFileEntry {
                    kind: "zip".into(),
                    sha256: "same".into(),
                    size: 1,
                    anim_sha256: None,
                    build_sha256: None,
                    tex: BTreeMap::new(),
                },
            ),
        ]);
        let new = BTreeMap::from([
            (
                "b.zip".to_string(),
                AnimFileEntry {
                    kind: "zip".into(),
                    sha256: "same".into(),
                    size: 1,
                    anim_sha256: None,
                    build_sha256: None,
                    tex: BTreeMap::new(),
                },
            ),
            (
                "c.zip".to_string(),
                AnimFileEntry {
                    kind: "zip".into(),
                    sha256: "new".into(),
                    size: 1,
                    anim_sha256: None,
                    build_sha256: None,
                    tex: BTreeMap::new(),
                },
            ),
        ]);
        let diff = diff_file_maps(&old, &new);
        assert_eq!(diff.added, vec!["c.zip"]);
        assert_eq!(diff.removed, vec!["a.zip"]);
        assert_eq!(diff.changed.len(), 0);
    }
}
