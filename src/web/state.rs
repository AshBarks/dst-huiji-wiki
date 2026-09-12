//! Shared application state for the WebUI server.

use crate::web::jobs::JobManager;
use dst_huiji_wiki::scripts_sync::images::icons::{build_icons_index, IconsIndex};
use dst_huiji_wiki::scripts_sync::images::meta::{self as icon_meta, IconMeta};
use dst_huiji_wiki::service::dataset::{DatasetCache, SkillStringsData};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::sync::Mutex;

/// (manifests 目录签名, 缓存的图标索引)。
type IconsIndexCache = Option<((usize, u64), Arc<IconsIndex>)>;

/// 图标元数据文件签名（存在 / 长度 / mtime 毫秒）。
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileSignature {
    exists: bool,
    len: u64,
    mtime_ms: u64,
}

/// (icon_meta.json 签名, 缓存的图标元数据)。
type IconMetaCache = Option<(FileSignature, Arc<IconMeta>)>;

/// `diff_cache` 最大条目数；按插入顺序淘汰最旧的。
const MAX_DIFF_CACHE: usize = 64;

/// 快照 diff 结果缓存（带插入顺序，用于容量淘汰）。
#[derive(Default)]
struct DiffCache {
    map: HashMap<String, serde_json::Value>,
    order: VecDeque<String>,
}

#[derive(Default)]
pub struct AppState {
    pub jobs: JobManager,
    pub datasets: Arc<DatasetCache>,
    /// Cached SKILLTREE.* zh strings per snapshot selection.
    skill_strings: Mutex<HashMap<Option<String>, Arc<SkillStringsData>>>,
    /// Snapshot diff results keyed by `kind|from|to`.
    diff_cache: Mutex<DiffCache>,
    /// Cached inventory-icons index, invalidated by the manifests dir signature.
    icons_index: Mutex<IconsIndexCache>,
    /// Cached icon metadata (names + wiki status), invalidated by file signature.
    icon_meta: Mutex<IconMetaCache>,
}

/// `KTOOLS__OUT_DIR`（缺省 `output/ktools`）——images-sync 产物根目录。
pub fn ktools_out_dir() -> PathBuf {
    dst_huiji_wiki::platform::config::ktools_out_dir()
}

fn icons_manifests_dir() -> PathBuf {
    ktools_out_dir().join("history/manifests")
}

/// 文件签名（存在 / 长度 / mtime 毫秒），作派生 JSON 缓存键。
fn file_signature(path: &Path) -> FileSignature {
    match std::fs::metadata(path) {
        Ok(m) => FileSignature {
            exists: true,
            len: m.len(),
            mtime_ms: m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        },
        Err(_) => FileSignature {
            exists: false,
            len: 0,
            mtime_ms: 0,
        },
    }
}

/// manifests 目录签名（json 文件数 + 最大 mtime 毫秒），作图标索引缓存键。
fn manifests_signature(dir: &Path) -> (usize, u64) {
    let mut count = 0usize;
    let mut max_mtime = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            count += 1;
            let ms = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            max_mtime = max_mtime.max(ms);
        }
    }
    (count, max_mtime)
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: JobManager::new(),
            datasets: Arc::new(DatasetCache::new()),
            skill_strings: Mutex::new(HashMap::new()),
            diff_cache: Mutex::new(DiffCache::default()),
            icons_index: Mutex::new(None),
            icon_meta: Mutex::new(None),
        }
    }

    /// 物品图标索引（缓存，manifests 目录变化时重建）。
    pub async fn icons_index(&self) -> dst_huiji_wiki::error::Result<Arc<IconsIndex>> {
        let dir = icons_manifests_dir();
        let sig = manifests_signature(&dir);
        {
            let cache = self.icons_index.lock().await;
            if let Some((k, v)) = cache.as_ref() {
                if *k == sig {
                    return Ok(Arc::clone(v));
                }
            }
        }
        let dir2 = dir;
        let index = tokio::task::spawn_blocking(move || build_icons_index(&dir2))
            .await
            .map_err(|e| {
                dst_huiji_wiki::error::Error::Config(format!("icons index loader panicked: {}", e))
            })??;
        let index = Arc::new(index);
        let mut cache = self.icons_index.lock().await;
        // Double-check after the blocking computation.
        if let Some((k, v)) = cache.as_ref() {
            if *k == sig {
                return Ok(Arc::clone(v));
            }
        }
        *cache = Some((sig, Arc::clone(&index)));
        Ok(index)
    }

    /// 物品图标元数据（名称 + 维基上传状态，images-sync 产物）缓存。
    /// 文件缺失时返回空元数据；由 `icon_meta.json` 签名（长度+mtime）失效。
    pub async fn icon_meta(&self) -> dst_huiji_wiki::error::Result<Arc<IconMeta>> {
        let out_dir = ktools_out_dir();
        let path = icon_meta::meta_path(&out_dir);
        let sig = file_signature(&path);
        {
            let cache = self.icon_meta.lock().await;
            if let Some((k, v)) = cache.as_ref() {
                if *k == sig {
                    return Ok(Arc::clone(v));
                }
            }
        }
        let meta = tokio::task::spawn_blocking(move || icon_meta::load(&out_dir))
            .await
            .map_err(|e| {
                dst_huiji_wiki::error::Error::Config(format!("icon meta loader panicked: {}", e))
            })??;
        let meta = Arc::new(meta);
        let mut cache = self.icon_meta.lock().await;
        // Double-check after the blocking computation.
        if let Some((k, v)) = cache.as_ref() {
            if *k == sig {
                return Ok(Arc::clone(v));
            }
        }
        *cache = Some((sig, Arc::clone(&meta)));
        Ok(meta)
    }

    pub async fn skill_strings(
        &self,
        snapshot: Option<String>,
    ) -> dst_huiji_wiki::error::Result<Arc<SkillStringsData>> {
        let mut cache = self.skill_strings.lock().await;
        if let Some(v) = cache.get(&snapshot) {
            return Ok(Arc::clone(v));
        }
        let snap = snapshot.clone();
        let loaded = tokio::task::spawn_blocking(move || {
            dst_huiji_wiki::service::dataset::load_skill_strings(snap.as_deref())
        })
        .await
        .map_err(|e| {
            dst_huiji_wiki::error::Error::Config(format!("skill strings loader panicked: {}", e))
        })??;

        let arc = Arc::new(loaded);
        cache.insert(snapshot, Arc::clone(&arc));
        Ok(arc)
    }

    pub async fn cached_diff<F>(
        &self,
        key: String,
        compute: F,
    ) -> dst_huiji_wiki::error::Result<serde_json::Value>
    where
        F: FnOnce() -> dst_huiji_wiki::error::Result<serde_json::Value>,
    {
        {
            let cache = self.diff_cache.lock().await;
            if let Some(v) = cache.map.get(&key) {
                return Ok(v.clone());
            }
        }
        let value = compute()?;
        let mut cache = self.diff_cache.lock().await;
        // Double-check after the blocking computation.
        if let Some(v) = cache.map.get(&key) {
            return Ok(v.clone());
        }
        cache.map.insert(key.clone(), value.clone());
        cache.order.push_back(key);
        while cache.order.len() > MAX_DIFF_CACHE {
            match cache.order.pop_front() {
                Some(old) => {
                    cache.map.remove(&old);
                }
                None => break,
            }
        }
        Ok(value)
    }

    /// 任务结束后的运行时缓存失效。
    ///
    /// `datasets`（数据集）在任务完成处已有失效；`skill_strings` 与
    /// `diff_cache` 的键是快照/参数、无法从文件签名自动过期，而任务
    /// （scripts-sync、images-sync 等）可能改写其底层数据，这里统一清空。
    /// `icons_index` / `icon_meta` 自带签名键，无需处理。
    pub async fn invalidate_job_caches(&self) {
        self.skill_strings.lock().await.clear();
        let mut cache = self.diff_cache.lock().await;
        cache.map.clear();
        cache.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn diff_cache_evicts_oldest_beyond_cap() {
        let st = AppState::new();
        for i in 0..(MAX_DIFF_CACHE + 10) {
            st.cached_diff(format!("k{}", i), || Ok(serde_json::json!({"i": i})))
                .await
                .unwrap();
        }
        let cache = st.diff_cache.lock().await;
        assert_eq!(cache.map.len(), MAX_DIFF_CACHE);
        assert_eq!(cache.order.len(), MAX_DIFF_CACHE);
        // 最旧的 k0..k9 已被淘汰，最新的 k73 仍在。
        assert!(!cache.map.contains_key("k0"));
        assert!(!cache.map.contains_key("k9"));
        assert!(cache.map.contains_key(&format!("k{}", MAX_DIFF_CACHE + 9)));
    }

    #[tokio::test]
    async fn invalidate_job_caches_clears_diff_cache() {
        let st = AppState::new();
        st.cached_diff("a|b|c".into(), || Ok(serde_json::json!(1)))
            .await
            .unwrap();
        st.invalidate_job_caches().await;
        assert!(st.diff_cache.lock().await.map.is_empty());
        assert!(st.skill_strings.lock().await.is_empty());
    }
}
