//! Compiled cooking data cache for the WebUI.
//!
//! The actual Lua parsing lives in [`crate::parser::cooking`]. This module
//! reads the snapshot-aware game files, compiles them once per snapshot and
//! keeps the result in memory, mirroring [`crate::service::dataset::DatasetCache`].

use crate::error::{Error, Result};
use crate::models::CookingData;
use crate::parser::cooking::{compile_from_sources, CookingSources};
use crate::platform::game_source::GameSource;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Load and compile cooking data for `snapshot` (None = latest scripts).
pub fn load_cooking_data(snapshot: Option<String>) -> Result<CookingData> {
    let source = GameSource::from_env(snapshot.clone())?;
    let cooking = source.read("cooking.lua")?;
    let oceanfish = source.read("prefabs/oceanfishdef.lua")?;
    let scrapbook_prefabs = source.read("scrapbook_prefabs.lua")?;
    let preparedfoods = source.read("preparedfoods.lua")?;
    let preparedfoods_warly = source.read("preparedfoods_warly.lua")?;
    let preparednonfoods = source.read("preparednonfoods.lua")?;

    let mut data = compile_from_sources(&CookingSources {
        cooking: &cooking,
        oceanfish: &oceanfish,
        scrapbook_prefabs: &scrapbook_prefabs,
        preparedfoods: &preparedfoods,
        preparedfoods_warly: &preparedfoods_warly,
        preparednonfoods: &preparednonfoods,
    })?;
    data.snapshot = snapshot.clone();
    data.label = match &snapshot {
        Some(name) => format!("scripts snapshot {}", name),
        None => latest_label(source.dst_root()),
    };
    Ok(data)
}

fn latest_label(dst_root: &std::path::Path) -> String {
    let version = std::fs::read_to_string(dst_root.join("version.txt"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    match version {
        Some(v) => format!("live scripts / patch {}", v),
        None => "live scripts".to_string(),
    }
}

/// Thread-safe cache of compiled cooking payloads, keyed by snapshot selection.
#[derive(Default)]
pub struct CookingDataCache {
    entries: Mutex<HashMap<Option<String>, Arc<CookingData>>>,
}

impl CookingDataCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_or_load(&self, snapshot: Option<String>) -> Result<Arc<CookingData>> {
        {
            let entries = self.entries.lock().await;
            if let Some(data) = entries.get(&snapshot) {
                return Ok(Arc::clone(data));
            }
        }

        let key = snapshot.clone();
        let loaded = tokio::task::spawn_blocking(move || load_cooking_data(key))
            .await
            .map_err(|e| Error::Config(format!("cooking data loader panicked: {}", e)))??;

        let mut entries = self.entries.lock().await;
        let data = entries.entry(snapshot).or_insert_with(|| Arc::new(loaded));
        Ok(Arc::clone(data))
    }

    pub async fn invalidate(&self) {
        self.entries.lock().await.clear();
    }
}
