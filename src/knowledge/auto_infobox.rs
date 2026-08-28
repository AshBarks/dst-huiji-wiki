//! AutoInfobox 冷数据加载与查询。
//!
//! 该文件不是 LLM 产物，而是从 `Module:AutoInfobox` / `Module:AutoInfobox/Data/doc`
//! 人工/半自动整理的确定性冷数据。post-action 会把它注入到 SymbolDoc 的
//! `auto_maintained` 字段，用于标识“哪些组件数据由自动模板维护、哪些字段仍可能手写覆盖”。

use crate::error::Result;
use crate::knowledge::types::AutoMaintainedInfo;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// `knowledge/auto_infobox.json` 的根结构。
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AutoInfoboxIndex {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub generated_at_ms: u64,
    #[serde(default)]
    pub components: BTreeMap<String, AutoMaintainedInfo>,
}

impl AutoInfoboxIndex {
    /// 从 `knowledge_root/auto_infobox.json` 加载；文件不存在时返回空索引。
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("auto_infobox.json");
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.into()),
        };
        Ok(serde_json::from_str(&raw)?)
    }

    /// 按组件名（`components/health.lua` → `health`）查询。
    pub fn get(&self, component_name: &str) -> Option<&AutoMaintainedInfo> {
        self.components.get(component_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_empty_index() {
        let dir = std::env::temp_dir().join(format!("auto-infobox-none-{}", std::process::id()));
        let idx = AutoInfoboxIndex::load(&dir).unwrap();
        assert!(idx.components.is_empty());
    }

    #[test]
    fn load_json_and_query_component() {
        let dir = std::env::temp_dir().join(format!("auto-infobox-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auto_infobox.json");
        std::fs::write(
            &path,
            r#"{
                "schema_version": 1,
                "source": "Module:AutoInfobox",
                "generated_at_ms": 1,
                "components": {
                    "health": {
                        "fields": ["生命值"],
                        "code_fields": ["health.max"],
                        "overridable_fields": ["生命值"]
                    }
                }
            }"#,
        )
        .unwrap();
        let idx = AutoInfoboxIndex::load(&dir).unwrap();
        let info = idx.get("health").unwrap();
        assert_eq!(info.fields, vec!["生命值"]);
        assert_eq!(info.code_fields, vec!["health.max"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
