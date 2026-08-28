//! AutoInfobox 冷数据加载与查询。
//!
//! 该文件不是 LLM 产物，而是从 `Module:AutoInfobox` / `Module:AutoInfobox/Data`
//! / `帮助:维基实体数据更新`(游戏内导出脚本)逐键核实的确定性冷数据。
//! post-action 会把它注入到 SymbolDoc 的 `auto_maintained` 字段，用于标识
//! “哪些组件数据由自动模板维护、哪些字段仍可能手写覆盖”，并生成 pass2 轻量提示。

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

impl AutoMaintainedInfo {
    /// pass2 轻量提示：具体到字段地告知 LLM 该组件哪些信息框数据由
    /// AutoInfobox 自动渲染，页面正文缺少对应文本属正常现象。
    pub fn pass2_hint(&self) -> String {
        let fields = if self.fields.is_empty() {
            "该组件的信息框数据".to_string()
        } else {
            self.fields.join("、")
        };
        let mut hint = format!(
            "【AutoInfobox 背景】该组件的以下实体信息框数据由 Module:AutoInfobox 读取游戏导出数据自动渲染:{fields}。\
这些数据通常不再手写在页面里,页面正文缺少对应文本属正常现象,不要仅据此断定页面不讨论该组件;\
若某页面确实手写了这些数值(或以 n/a 清除自动值),以页面原文为准并正常归因。"
        );
        if !self.overridable_fields.is_empty() {
            hint.push_str(&format!(
                "其中{}在部分页面存在手写覆盖,遇到时优先依据页面原文。",
                self.overridable_fields.join("、")
            ));
        }
        hint
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

    #[test]
    fn pass2_hint_names_fields_and_overridables() {
        let info = AutoMaintainedInfo {
            source: "Module:AutoInfobox".to_string(),
            fields: vec!["漂浮".to_string(), "分类:漂浮".to_string()],
            code_fields: vec!["floater".to_string()],
            overridable_fields: vec![],
            note: None,
        };
        let hint = info.pass2_hint();
        assert!(hint.contains("漂浮"));
        assert!(hint.contains("分类:漂浮"));
        assert!(hint.contains("Module:AutoInfobox"));
        assert!(hint.contains("正常现象"));
        assert!(!hint.contains("其中"));
    }

    #[test]
    fn pass2_hint_mentions_overridable_fields() {
        let info = AutoMaintainedInfo {
            source: "Module:AutoInfobox".to_string(),
            fields: vec!["伤害".to_string()],
            code_fields: vec!["combat_damage".to_string()],
            overridable_fields: vec!["伤害".to_string()],
            note: None,
        };
        let hint = info.pass2_hint();
        assert!(hint.contains("其中伤害"));
    }
}
