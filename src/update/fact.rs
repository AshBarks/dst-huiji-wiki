//! FactChange unified model (docs/UPDATE_IMPACT_PLAN.md §4.2.5).
//!
//! One fact transition between two game snapshots, anchored to the page
//! surface it may invalidate. Extractor families (F1 loot today; F2 stats
//! and F3 behavior constants next) all emit this shape so the grading and
//! report layers stay family-agnostic.
//!
//! Draft status: field set is final per plan v3.1 review; `old`/`new`
//! pairing semantics are filled by snapshot diffing — extractors emit
//! current-state facts with both sides `None` until then.

use serde::{Deserialize, Serialize};

use super::index::loot::LootRecord;

/// Which extractor family produced the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    Loot,
    Stat,
    Behavior,
    Backref,
}

/// A literal as it appears in code or on the page. Numbers keep their
/// source scale (fraction vs percent) — matching applies the tolerance
/// rules from `corpus::facts` at lookup time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Literal {
    Num(f64),
    Str(String),
}

/// Where to look in the tree: file plus line anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub file: String,
    pub line: u32,
}

/// How many hops between the fact anchor and the page-facing value.
/// 1 = setter inside the prefab's own fn; 2 = constant/config in an
/// associated file; ≥3 = cross-entity chains (report-only, never rewritten).
pub type DerivationDepth = u8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactChange {
    /// Owning prefab **variant** — the atlas join key.
    pub prefab: String,
    pub kind: FactKind,
    /// Page-semantic field name (`"loot[houndstooth]"`, `"combat.damage"`);
    /// code symbols never leak into target text (§4.2.4 rule 1).
    pub field: String,
    /// Conditional context ("野生" / "有猎犬丘"); `None` = unconditional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub source_file: String,
    /// `None` = addition (or current-state anchor before diff pairing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Literal>,
    /// `None` = removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Literal>,
    pub derivation_depth: DerivationDepth,
    pub evidence: Vec<EvidenceRef>,
}

impl FactChange {
    /// Expands one attributed loot record into per-item changes.
    ///
    /// Records with empty `variants` are file-level (non-prefab sources)
    /// and yield nothing — they have no page join key.
    /// Chances are not carried by [`LootRecord`] yet; each item becomes a
    /// presence-style fact (`old`/`new` filled by snapshot pairing later).
    pub fn from_loot_record(file: &str, record: &LootRecord) -> Vec<Self> {
        if record.variants.is_empty() {
            return Vec::new();
        }
        record
            .items
            .iter()
            .flat_map(|item| {
                record.variants.iter().map(move |variant| Self {
                    prefab: variant.clone(),
                    kind: FactKind::Loot,
                    field: format!("loot[{item}]"),
                    context: None,
                    source_file: file.to_string(),
                    old: None,
                    new: None,
                    derivation_depth: 1,
                    evidence: vec![EvidenceRef {
                        file: file.to_string(),
                        line: record.line,
                    }],
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(variants: Vec<&str>, items: Vec<&str>) -> LootRecord {
        LootRecord {
            kind: super::super::index::loot::LootKind::SetLoot,
            items: items.into_iter().map(String::from).collect(),
            line: 42,
            variants: variants.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn loot_record_expands_per_item_and_variant() {
        let changes = FactChange::from_loot_record(
            "prefabs/hound.lua",
            &record(
                vec!["hound", "firehound"],
                vec!["monstermeat", "houndstooth"],
            ),
        );
        assert_eq!(changes.len(), 4);
        assert!(changes.iter().all(|c| c.kind == FactKind::Loot));
        assert_eq!(changes[0].field, "loot[monstermeat]");
        assert_eq!(changes[0].prefab, "hound");
        assert_eq!(changes[3].prefab, "firehound");
        assert_eq!(changes[3].field, "loot[houndstooth]");
        assert_eq!(changes[0].derivation_depth, 1);
        assert_eq!(
            changes[0].evidence,
            vec![EvidenceRef {
                file: "prefabs/hound.lua".into(),
                line: 42
            }]
        );
    }

    #[test]
    fn file_level_records_yield_nothing() {
        let changes = FactChange::from_loot_record("prefabs/x.lua", &record(vec![], vec!["meat"]));
        assert!(changes.is_empty());
    }

    #[test]
    fn serde_roundtrip_preserves_optional_semantics() {
        let json = r#"{"prefab":"hound","kind":"loot","field":"loot[meat]",
            "source_file":"prefabs/hound.lua","derivation_depth":1,"evidence":[]}"#;
        let c: FactChange = serde_json::from_str(json).unwrap();
        assert_eq!(c.old, None);
        assert_eq!(c.context, None);
        let back = serde_json::to_string(&c).unwrap();
        assert!(!back.contains("\"old\""));
    }
}
