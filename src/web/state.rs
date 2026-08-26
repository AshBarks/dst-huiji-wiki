//! Shared application state for the WebUI server.

use crate::web::jobs::JobManager;
use dst_huiji_wiki::service::dataset::{DatasetCache, SkillStringsData};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub jobs: JobManager,
    pub datasets: Arc<DatasetCache>,
    /// Cached SKILLTREE.* zh strings per snapshot selection.
    skill_strings: Mutex<HashMap<Option<String>, Arc<SkillStringsData>>>,
    /// Snapshot diff results keyed by `kind|from|to`.
    diff_cache: Mutex<HashMap<String, serde_json::Value>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: JobManager::new(),
            datasets: Arc::new(DatasetCache::new()),
            skill_strings: Mutex::new(HashMap::new()),
            diff_cache: Mutex::new(HashMap::new()),
        }
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
            if let Some(v) = cache.get(&key) {
                return Ok(v.clone());
            }
        }
        let value = compute()?;
        let mut cache = self.diff_cache.lock().await;
        // Double-check after the blocking computation.
        if let Some(v) = cache.get(&key) {
            return Ok(v.clone());
        }
        cache.insert(key, value.clone());
        Ok(value)
    }
}
