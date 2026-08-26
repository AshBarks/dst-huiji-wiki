//! Lightweight checkpoint state for long-running update scans.
//!
//! `update-scan` writes a `state.json` under its output directory so an
//! interrupted/completed run can be recognised on the next invocation.
//! Currently it supports whole-run idempotency: a completed state with the
//! same snapshot pair skips the expensive rebuild.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanState {
    pub schema_version: u32,
    pub old_id: String,
    pub new_id: String,
    pub status: String,
    pub stages: BTreeMap<String, bool>,
    pub updated_at_ms: u64,
}

impl ScanState {
    pub fn new(old_id: &str, new_id: &str) -> Self {
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            old_id: old_id.to_string(),
            new_id: new_id.to_string(),
            status: "running".to_string(),
            stages: BTreeMap::new(),
            updated_at_ms: now_ms(),
        }
    }

    pub fn load(dir: &Path) -> Option<ScanState> {
        let content = std::fs::read_to_string(dir.join("state.json")).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self, dir: &Path) -> crate::Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("state.json");
        let tmp = dir.join("state.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn mark_stage(&mut self, name: &str, done: bool, dir: &Path) -> crate::Result<()> {
        self.stages.insert(name.to_string(), done);
        self.updated_at_ms = now_ms();
        self.save(dir)
    }

    pub fn complete(&mut self, dir: &Path) -> crate::Result<()> {
        self.status = "completed".to_string();
        self.updated_at_ms = now_ms();
        self.save(dir)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_preserves_checkpoints() {
        let dir = std::env::temp_dir().join(format!("dst_state_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut state = ScanState::new("old", "new");
        state.mark_stage("diff", true, &dir).unwrap();
        state.mark_stage("atlas", true, &dir).unwrap();
        state.complete(&dir).unwrap();

        let loaded = ScanState::load(&dir).unwrap();
        assert_eq!(loaded.old_id, "old");
        assert_eq!(loaded.new_id, "new");
        assert!(loaded.stages["diff"]);
        assert!(loaded.stages["atlas"]);
        assert_eq!(loaded.status, "completed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
