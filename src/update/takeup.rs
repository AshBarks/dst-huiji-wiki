//! Editorial take-up configuration (docs/UPDATE_IMPACT_PLAN.md §4.2.4, v3.2
//! section-boundary table as data).
//!
//! Declares which page regions an extractor family may draft into, which are
//! flag-only, and which are ignored — plus the cross-cutting protection
//! lists. Initial values transcribe the plan tables; every human-review
//! rejection gets classified (不该改 / 不该管 / 格式错) and folded back here.

use serde::{Deserialize, Serialize};

/// Per-region intervention tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Deterministic regeneration allowed (F1 privileged path).
    Draft,
    /// Report "verify wording" hints only; never rewrite.
    Flag,
    /// Not monitored.
    Ignore,
}

/// One region rule. `section` matches the unwrapped h2 title; the untitled
/// lead zone uses `"intro"`; infobox hand-written parameters use `"infobox"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionRule {
    pub section: String,
    pub tier: Tier,
    /// Free-text provenance for the rule (plan section / sample evidence).
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeupConfig {
    /// Regions where deterministic drafts are allowed.
    pub draft: Vec<RegionRule>,
    /// Regions that only ever receive verify-hints.
    pub flag: Vec<RegionRule>,
    /// Regions excluded from scanning entirely.
    pub ignore: Vec<RegionRule>,
    /// Template-structure elements the LLM must never alter; breaking any of
    /// these is a hard validation failure (§4.2.4 横切规则 ②).
    pub protected_templates: Vec<String>,
    /// Collaborative traces preserved verbatim through generation and
    /// validated as protected content (横切规则 ③).
    pub collaborative_traces: Vec<String>,
    /// 人审结论回填的提取规则注记（§4.2.4 维护方式）。
    #[serde(default)]
    pub calibration_notes: Vec<String>,
}

impl Default for TakeupConfig {
    fn default() -> Self {
        let r = |section: &str, tier: Tier, note: &str| RegionRule {
            section: section.to_string(),
            tier,
            note: note.to_string(),
        };
        Self {
            draft: vec![
                r(
                    "intro",
                    Tier::Draft,
                    "数值/字面量句（old_literal 命中）；单跳事实句除外",
                ),
                r("infobox", Tier::Draft, "手写参数：F1 特权路径 / F2 锚定"),
            ],
            flag: vec![
                r(
                    "行为",
                    Tier::Flag,
                    "数值句可升 draft（depth≤2）；机制叙述与取舍内容只提示",
                ),
                r(
                    "获取",
                    Tier::Flag,
                    "料理配方=规则约束，示例枚举非唯一真值；M3 后再评估",
                ),
                r("料理烹饪", Tier::Flag, "命中旧值的短句后续可升 draft"),
                r(
                    "自定义世界",
                    Tier::Flag,
                    "worldsettings_overrides 可溯源但跨文件解释多；与 Q3 决议一致",
                ),
                r("提示", Tier::Flag, "被引用实体变更时提示核对表述"),
                r("策略", Tier::Flag, "纯主观经验，不代改"),
            ],
            ignore: vec![
                r("花絮", Tier::Ignore, "重命名监听为可选低成本项"),
                r("皮肤", Tier::Ignore, "外部元信息"),
                r("Bug", Tier::Ignore, "人工维护，时效性强"),
                r("画廊", Tier::Ignore, "图片素材不在范围"),
            ],
            protected_templates: vec![
                "实体信息框".to_string(),
                "实体信息框/自动".to_string(),
                "RichTab".to_string(),
                "全角色台词".to_string(),
                "置顶导航".to_string(),
                "RRBI".to_string(),
            ],
            collaborative_traces: vec![
                "{{待补充}}".to_string(),
                "{{未完成}}".to_string(),
                "<!--".to_string(),
                "-->".to_string(),
            ],
            calibration_notes: vec!["SpawnLootPrefab 变更不进掉落建议：镶嵌摧毁/                过程生成属内部机制（telebase 紫宝石、vault_lobby_exit 绳索比对结论）"
                .to_string()],
        }
    }
}

impl TakeupConfig {
    /// Tier lookup by region kind/title. Unlisted regions default to `Flag`
    /// (safe-by-default); `ignore` wins over `draft` if both list a name.
    pub fn tier_for(&self, section: &str) -> Tier {
        if self.ignore.iter().any(|r| r.section == section) {
            return Tier::Ignore;
        }
        if self.draft.iter().any(|r| r.section == section) {
            return Tier::Draft;
        }
        Tier::Flag
    }

    /// Parses a user TOML overriding the defaults (whole-document replace).
    pub fn from_toml_str(toml_src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_encodes_the_plan_boundary_table() {
        let c = TakeupConfig::default();
        assert_eq!(c.tier_for("infobox"), Tier::Draft);
        assert_eq!(c.tier_for("intro"), Tier::Draft);
        assert_eq!(c.tier_for("行为"), Tier::Flag);
        assert_eq!(c.tier_for("花絮"), Tier::Ignore);
        // Unlisted → safe default Flag.
        assert_eq!(c.tier_for("历史"), Tier::Flag);
        // ignore beats draft on conflict.
        assert!(c.protected_templates.contains(&"RRBI".to_string()));
        assert!(c.collaborative_traces.len() >= 3);
    }

    #[test]
    fn toml_roundtrip_and_override() {
        let custom = r#"
# 顶层键必须先于任何表头（TOML 规则）
protected_templates = ["实体信息框"]
collaborative_traces = ["<!--", "-->"]

[[draft]]
section = "行为"
tier = "draft"
note = "试点：生物页数值句升 draft"

[[flag]]
section = "获取"
tier = "flag"

[[ignore]]
section = "花絮"
tier = "ignore"
"#;
        let c = TakeupConfig::from_toml_str(custom).unwrap();
        assert_eq!(c.tier_for("行为"), Tier::Draft);
        assert_eq!(c.draft[0].section, "行为");
        assert_eq!(c.ignore.len(), 1);
    }
}
