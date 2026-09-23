//! 差异历史存储：内容寻址对象仓（CAS）+ 每 build 全量 manifest + 相邻 diff。
//!
//! 设计要点（详见模块 `scripts_sync::images` 文档）：
//! - 最终产物（split 切片 + 无 xml 的 kteched png）按 sha256 存入
//!   `history/objects/<h[:2]>/<h>.png`，跨版本去重；
//! - 每 build 一份 `history/manifests/<build>.json`：全量 final 清单 +
//!   相对 parent 的 diff + 输入 hash（供下次增量）；
//! - diff 由两份全量清单推导（纯函数 [`diff_final_maps`]），落盘以便下游
//!   （如"图片变更 → wiki 更新"作业）直接消费；
//! - partial（有失败）的 manifest 不作为下一个 diff 基线，
//!   避免"因失败消失"被误判为 removed。

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 最终产物路径（相对 `current/`，正斜杠）→ 内容 sha256。
pub type FinalMap = BTreeMap<String, String>;

/// 一条变更记录（path 的 old → new 内容 hash）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedEntry {
    pub path: String,
    pub old: String,
    pub new: String,
}

/// 相对 parent manifest 的差异。
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub changed: Vec<ChangedEntry>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// 差异规模（added + removed + changed 的条目总数）。
    pub fn len(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

/// 一次 images-sync 运行的机器可读记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// 游戏 build 号（`version.txt` 的值）。
    pub build: String,
    /// diff 基线 build（最近一个完整的 manifest）；首次运行为 `None`。
    pub parent_build: Option<String>,
    /// 无失败为 true；partial 的 manifest 不作为 diff 基线。
    pub complete: bool,
    /// 解码器版本（[`dst_ktex::DECODER_VERSION`]）；
    /// 与当前不一致时触发全量重处理（旧 manifest 无此字段按空串处理）。
    #[serde(default)]
    pub decoder: String,
    /// 切割/裁剪逻辑版本（[`crate::scripts_sync::images::split::SPLIT_VERSION`]）；
    /// 与当前不一致时触发全量重切割（旧 manifest 无此字段按空串处理）。
    #[serde(default)]
    pub split_version: String,
    /// 最终产物：路径（相对 `current/`）→ sha256。
    pub products: FinalMap,
    /// 相对 [`Manifest::parent_build`] 的差异。
    #[serde(default)]
    pub diff: Diff,
    /// 输入 hash：`tex/<base>`、`xml/<base>` → sha256（增量判定依据）。
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
    /// 阶段统计（ktech/split 处理量、失败、missing/conflict/stale 计数等）。
    #[serde(default)]
    pub stats: BTreeMap<String, u64>,
    /// 保存时刻（unix 毫秒）；旧 manifest 无此字段为 `None`，
    /// 下游（如 WebUI 图标历史）可用 manifest 文件 mtime 兜底。
    #[serde(default)]
    pub synced_at: Option<u64>,
}

/// 当前 unix 时间（毫秒）。
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 纯函数：比较两份最终产物清单，产出 added/removed/changed。
///
/// - `parent` 为空（首次运行）时，新清单全部计入 `added`；
/// - 同路径内容 hash 一致 → 无记录；
/// - 结果按路径排序（BTreeMap 迭代序），输出确定性可测。
pub fn diff_final_maps(parent: &FinalMap, new: &FinalMap) -> Diff {
    let mut diff = Diff::default();
    for (path, old_hash) in parent {
        match new.get(path) {
            None => diff.removed.push(path.clone()),
            Some(new_hash) if new_hash != old_hash => diff.changed.push(ChangedEntry {
                path: path.clone(),
                old: old_hash.clone(),
                new: new_hash.clone(),
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

/// 流式计算文件内容的 sha256（十六进制小写）。
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

/// 计算内存字节的 sha256（十六进制小写）。
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 内容寻址对象仓：`<root>/<hash 前两位>/<hash>`。
#[derive(Debug, Clone)]
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// hash → 对象路径（不保证存在）。
    pub fn path(&self, hash: &str) -> PathBuf {
        let prefix = hash.get(..2).unwrap_or(hash);
        self.root.join(prefix).join(hash)
    }

    pub fn contains(&self, hash: &str) -> bool {
        self.path(hash).exists()
    }

    /// 将文件按内容入库：已存在则跳过写入。返回内容 hash。
    pub fn put_file(&self, src: &Path) -> Result<String> {
        let hash = hash_file(src)?;
        let dest = self.path(&hash);
        if !dest.exists() {
            crate::platform::fs::ensure_parent(&dest)?;
            std::fs::copy(src, &dest)?;
        }
        Ok(hash)
    }
}

/// manifest 目录（`history/manifests/`）。
#[derive(Debug, Clone)]
pub struct ManifestStore {
    dir: PathBuf,
}

impl ManifestStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, build: &str) -> PathBuf {
        self.dir.join(format!("{build}.json"))
    }

    /// 保存 manifest（pretty JSON，文件名 `<build>.json`）。
    pub fn save(&self, manifest: &Manifest) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path(&manifest.build);
        let json = serde_json::to_string_pretty(manifest)?;
        crate::platform::fs::write_text_atomic(&path, json)?;
        Ok(path)
    }

    /// 读取指定 build 的 manifest；不存在返回 `None`。
    pub fn load(&self, build: &str) -> Result<Option<Manifest>> {
        let path = self.path(build);
        if !path.exists() {
            return Ok(None);
        }
        parse_manifest_file(&path).map(Some)
    }

    /// 加载最近一份**完整**（`complete == true`）且非 `current_build` 的
    /// manifest 作为 diff 基线。build 按数值取最大（非数字 build 按字典序兜底）。
    pub fn load_parent(&self, current_build: &str) -> Result<Option<Manifest>> {
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
            let manifest = match parse_manifest_file(&path) {
                Ok(m) => m,
                // 损坏的 manifest 不阻塞，跳过并告警。
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "跳过损坏的 manifest");
                    continue;
                }
            };
            if !manifest.complete || manifest.build == current_build {
                continue;
            }
            let is_better = match &best {
                None => true,
                Some(b) => numeric_key(&manifest.build) > numeric_key(&b.build),
            };
            if is_better {
                best = Some(manifest);
            }
        }
        Ok(best)
    }

    /// 已记录的最大数值 build（文件名 `<build>.json`，含 partial manifest；
    /// 非数字文件名忽略）。用于 build 回退检测：新 build 低于任何已记录
    /// build 时，管线应拒绝操作以免产出脏 diff。
    pub fn latest_recorded_build(&self) -> Result<Option<String>> {
        if !self.dir.exists() {
            return Ok(None);
        }
        let mut best: Option<(u64, String)> = None;
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Ok(n) = stem.parse::<u64>() {
                let is_better = best.as_ref().map(|(b, _)| n > *b).unwrap_or(true);
                if is_better {
                    best = Some((n, stem.to_string()));
                }
            }
        }
        Ok(best.map(|(_, build)| build))
    }
}

fn parse_manifest_file(path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("读取 {}: {}", path.display(), e),
        ))
    })?;
    serde_json::from_str(&text).map_err(|e| {
        Error::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("解析 {}: {}", path.display(), e),
        )))
    })
}

/// 数值排序键：数字 build 按数值，非数字按字典序（数字优先）。
pub(super) fn numeric_key(build: &str) -> (u8, u64, String) {
    match build.parse::<u64>() {
        Ok(n) => (0, n, String::new()),
        Err(_) => (1, 0, build.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, content: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    // -- diff ---------------------------------------------------------------

    #[test]
    fn test_diff_empty_parent_all_added() {
        let new: FinalMap = BTreeMap::from([
            ("split/a/1.png".into(), "h1".into()),
            ("kteched/b.png".into(), "h2".into()),
        ]);
        let diff = diff_final_maps(&BTreeMap::new(), &new);
        assert_eq!(diff.added, vec!["kteched/b.png", "split/a/1.png"]);
        assert!(diff.removed.is_empty() && diff.changed.is_empty());
    }

    #[test]
    fn test_diff_added_removed_changed() {
        let parent: FinalMap = BTreeMap::from([
            ("keep.png".into(), "same".into()),
            ("gone.png".into(), "old".into()),
            ("mut.png".into(), "oldh".into()),
        ]);
        let new: FinalMap = BTreeMap::from([
            ("keep.png".into(), "same".into()),
            ("mut.png".into(), "newh".into()),
            ("fresh.png".into(), "h".into()),
        ]);
        let diff = diff_final_maps(&parent, &new);
        assert_eq!(diff.added, vec!["fresh.png"]);
        assert_eq!(diff.removed, vec!["gone.png"]);
        assert_eq!(
            diff.changed,
            vec![ChangedEntry {
                path: "mut.png".into(),
                old: "oldh".into(),
                new: "newh".into()
            }]
        );
    }

    #[test]
    fn test_diff_identical_is_empty() {
        let m: FinalMap = BTreeMap::from([("a.png".into(), "h".into())]);
        assert!(diff_final_maps(&m, &m).is_empty());
    }

    // -- hashing ------------------------------------------------------------

    #[test]
    fn test_hash_bytes_known_value() {
        // echo -n "abc" | sha256sum
        assert_eq!(
            hash_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_hash_file_matches_hash_bytes() {
        let ws = std::env::temp_dir().join(format!("ktool_hist_{}", std::process::id()));
        let path = ws.join("blob.bin");
        write_file(&path, b"hello ktools");
        assert_eq!(hash_file(&path).unwrap(), hash_bytes(b"hello ktools"));
        std::fs::remove_dir_all(&ws).ok();
    }

    // -- object store ---------------------------------------------------------

    #[test]
    fn test_object_store_put_is_idempotent_and_dedup() {
        let ws = std::env::temp_dir().join(format!("ktool_cas_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let store = ObjectStore::new(ws.join("objects"));

        let a = ws.join("a.png");
        let b = ws.join("b.png");
        write_file(&a, b"same content");
        write_file(&b, b"same content");

        let h1 = store.put_file(&a).unwrap();
        let h2 = store.put_file(&b).unwrap();
        assert_eq!(h1, h2);
        assert!(store.contains(&h1));
        assert_eq!(store.path(&h1), ws.join("objects").join(&h1[..2]).join(&h1));
        // 两个对象目录只有一个文件（同内容去重）。
        let count = std::fs::read_dir(ws.join("objects").join(&h1[..2]))
            .unwrap()
            .count();
        assert_eq!(count, 1);

        let c = ws.join("c.png");
        write_file(&c, b"other");
        let h3 = store.put_file(&c).unwrap();
        assert_ne!(h1, h3);
        std::fs::remove_dir_all(&ws).ok();
    }

    // -- manifest store -------------------------------------------------------

    fn sample_manifest(build: &str, complete: bool) -> Manifest {
        Manifest {
            build: build.into(),
            parent_build: None,
            complete,
            decoder: "ktex-rs/1".into(),
            split_version: "1".into(),
            products: BTreeMap::from([("split/a/1.png".into(), "h".into())]),
            diff: Diff::default(),
            inputs: BTreeMap::from([("tex/a".into(), "h".into())]),
            stats: BTreeMap::from([("sprites".into(), 1)]),
            synced_at: Some(1234567890),
        }
    }

    #[test]
    fn test_manifest_roundtrip() {
        let ws = std::env::temp_dir().join(format!("ktool_manifest_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let store = ManifestStore::new(ws.join("manifests"));
        let m = sample_manifest("100", true);
        store.save(&m).unwrap();
        let loaded = store.load("100").unwrap().unwrap();
        assert_eq!(loaded.build, "100");
        assert!(loaded.complete);
        assert_eq!(loaded.synced_at, Some(1234567890));
        assert_eq!(
            loaded.products.get("split/a/1.png").map(String::as_str),
            Some("h")
        );
        assert!(store.load("999").unwrap().is_none());
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_manifest_without_decoder_field_parses() {
        // 旧版 manifest（无 decoder 字段）→ 兼容解析，decoder 为空串
        // （≠ 当前 DECODER_VERSION，会触发全量重处理——符合解码器切换语义）。
        let json = r#"{"build":"1","parent_build":null,"complete":true,"products":{},"diff":{"added":[],"removed":[],"changed":[]},"inputs":{},"stats":{}}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.decoder, "");
        assert!(m.complete);
    }

    #[test]
    fn test_load_parent_skips_partial_and_current() {
        let ws = std::env::temp_dir().join(format!("ktool_parent_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let store = ManifestStore::new(ws.join("manifests"));
        store.save(&sample_manifest("100", true)).unwrap();
        store.save(&sample_manifest("200", false)).unwrap(); // partial
        store.save(&sample_manifest("300", true)).unwrap(); // == current

        let parent = store.load_parent("300").unwrap().unwrap();
        assert_eq!(parent.build, "100");
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_load_parent_picks_latest_numeric() {
        let ws = std::env::temp_dir().join(format!("ktool_parent2_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let store = ManifestStore::new(ws.join("manifests"));
        store.save(&sample_manifest("900", true)).unwrap();
        store.save(&sample_manifest("1000", true)).unwrap();
        let parent = store.load_parent("2000").unwrap().unwrap();
        // 1000 > 900 数值序（字典序会错选 900）。
        assert_eq!(parent.build, "1000");
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_latest_recorded_build_includes_partial_and_ignores_non_numeric() {
        let ws = std::env::temp_dir().join(format!("ktool_latest_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let store = ManifestStore::new(ws.join("manifests"));
        assert_eq!(store.latest_recorded_build().unwrap(), None);
        store.save(&sample_manifest("100", true)).unwrap();
        store.save(&sample_manifest("300", false)).unwrap(); // partial 也算已记录
        store.save(&sample_manifest("200", true)).unwrap();
        // stray 非数字文件名不参与
        std::fs::write(ws.join("manifests").join("not-a-build.json"), "x").unwrap();
        assert_eq!(
            store.latest_recorded_build().unwrap().as_deref(),
            Some("300")
        );
        std::fs::remove_dir_all(&ws).ok();
    }
}
