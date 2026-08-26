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

use super::index::loot::{LootKind, LootRecord};

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

impl Literal {
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Literal::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Literal::Str(s) => Some(s),
            _ => None,
        }
    }
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

/// Attributed stat fact (F2): one numeric setter joined to owning variants.
#[derive(Debug, Clone, Serialize)]
pub struct StatRecord {
    pub field: &'static str,
    pub value: f64,
    pub raw_arg: String,
    pub line: u32,
    pub variants: Vec<String>,
}

/// Joins raw stat facts with variant ownership (mirrors `attribute_loot`).
pub fn attribute_stats(
    path: &str,
    facts: &[super::stats::StatFact],
    scan: &super::index::symbols::FileScan,
    fn_owners: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<String>>>,
) -> Vec<StatRecord> {
    use std::collections::BTreeSet;
    let owners_of_file = fn_owners.get(path);
    let all: BTreeSet<String> = owners_of_file
        .map(|m| m.values().flatten().cloned().collect())
        .unwrap_or_default();
    let ranges: Vec<(&str, u32, u32)> = scan
        .fns
        .iter()
        .filter(|f| f.start_line > 0)
        .map(|f| (f.name.as_str(), f.start_line, f.end_line))
        .collect();
    facts
        .iter()
        .map(|fact| {
            let mut variants: BTreeSet<String> = BTreeSet::new();
            for (name, start, end) in &ranges {
                if fact.line >= *start && fact.line <= *end {
                    if let Some(owners) = owners_of_file.and_then(|m| m.get(*name)) {
                        variants.extend(owners.iter().cloned());
                    }
                }
            }
            if variants.is_empty() {
                variants = all.clone();
            }
            StatRecord {
                field: fact.kind.field(),
                value: fact.value,
                raw_arg: fact.raw_arg.clone(),
                line: fact.line,
                variants: variants.into_iter().collect(),
            }
        })
        .collect()
}

/// Pairs two snapshots' attributed stats into numeric changes.
/// Key = (variant, field); a (file-level) item moving files does not fire —
/// only genuine old≠new values or presence flips do.
pub fn pair_stat_changes(
    old: &std::collections::BTreeMap<String, Vec<StatRecord>>,
    new: &std::collections::BTreeMap<String, Vec<StatRecord>>,
) -> Vec<FactChange> {
    use std::collections::BTreeMap;
    fn index(
        side: &BTreeMap<String, Vec<StatRecord>>,
    ) -> BTreeMap<(String, String), (f64, EvidenceRef)> {
        let mut m = BTreeMap::new();
        for (file, records) in side {
            for r in records {
                for v in &r.variants {
                    m.entry((v.clone(), r.field.to_string()))
                        .or_insert_with(|| {
                            (
                                r.value,
                                EvidenceRef {
                                    file: file.clone(),
                                    line: r.line,
                                },
                            )
                        });
                }
            }
        }
        m
    }
    let oi = index(old);
    let ni = index(new);
    let keys: std::collections::BTreeSet<(String, String)> =
        oi.keys().chain(ni.keys()).cloned().collect();
    let mut out = Vec::new();
    for key in keys {
        let was = oi.get(&key).map(|(v, _)| *v);
        let now = ni.get(&key).map(|(v, _)| *v);
        if was == now {
            continue;
        }
        let ev = oi.get(&key).or_else(|| ni.get(&key)).unwrap().1.clone();
        out.push(FactChange {
            prefab: key.0.clone(),
            kind: FactKind::Stat,
            field: key.1.clone(),
            context: None,
            source_file: ev.file.clone(),
            old: was.map(Literal::Num),
            new: now.map(Literal::Num),
            derivation_depth: 1,
            evidence: vec![ev],
        });
    }
    out
}

/// Pairs two snapshots' attributed loot facts into presence changes.
///
/// A `(variant, item)` present on the old side only is a removal
/// (`old=Str(item)`, `new=None`); new-side only is an addition. Chance-only
/// drift is invisible until [`LootRecord`] carries weights (known F1
/// limitation, plan §4.2.3). A fact that merely moved files keeps its page
/// meaning and produces nothing.
pub fn pair_loot_changes(
    old: &std::collections::BTreeMap<String, Vec<LootRecord>>,
    new: &std::collections::BTreeMap<String, Vec<LootRecord>>,
) -> Vec<FactChange> {
    fn index(
        side: &std::collections::BTreeMap<String, Vec<LootRecord>>,
    ) -> std::collections::BTreeMap<(String, String), EvidenceRef> {
        let mut m = std::collections::BTreeMap::new();
        for (file, records) in side {
            for r in records {
                // 校准（2026-08-26 人工比对 B/D）：SpawnLootPrefab 多为内部
                // 机制产物（镶嵌摧毁、过程生成），玩家认知不算掉落，不进建议流。
                if r.kind == LootKind::SpawnPrefab {
                    continue;
                }
                for v in &r.variants {
                    for item in &r.items {
                        m.entry((v.clone(), item.clone()))
                            .or_insert_with(|| EvidenceRef {
                                file: file.clone(),
                                line: r.line,
                            });
                    }
                }
            }
        }
        m
    }

    let old_idx = index(old);
    let new_idx = index(new);
    let keys: std::collections::BTreeSet<(String, String)> =
        old_idx.keys().chain(new_idx.keys()).cloned().collect();
    let mut out = Vec::new();
    for key in keys {
        let was = old_idx.get(&key);
        let now = new_idx.get(&key);
        if was == now {
            continue;
        }
        let item = key.1.clone();
        let (old_lit, new_lit, ev) = match (was, now) {
            (Some(w), None) => (Some(Literal::Str(item.clone())), None, w.clone()),
            (None, Some(n)) => (None, Some(Literal::Str(item.clone())), n.clone()),
            _ => continue,
        };
        out.push(FactChange {
            prefab: key.0.clone(),
            kind: FactKind::Loot,
            field: format!("loot[{item}]"),
            context: None,
            source_file: ev.file.clone(),
            old: old_lit,
            new: new_lit,
            derivation_depth: 1,
            evidence: vec![ev],
        });
    }
    out
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

    #[test]
    fn stat_pairing_fires_only_on_value_change() {
        use std::collections::BTreeMap;
        let sr = |value: f64| StatRecord {
            field: "combat.damage",
            value,
            raw_arg: "TUNING.X".into(),
            line: 5,
            variants: vec!["hound".into()],
        };
        let mut o = BTreeMap::new();
        o.insert("prefabs/hound.lua".into(), vec![sr(20.0)]);
        let mut n = BTreeMap::new();
        n.insert("prefabs/hound.lua".into(), vec![sr(25.0)]);

        let changes = pair_stat_changes(&o, &n);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old, Some(Literal::Num(20.0)));
        assert_eq!(changes[0].new, Some(Literal::Num(25.0)));
        // 同值不触发
        assert!(pair_stat_changes(&o, &o).is_empty());
    }

    #[test]
    fn loot_pairing_produces_removals_and_additions() {
        use std::collections::BTreeMap;
        let rec = |items: &[&str], variants: &[&str], line: u32| LootRecord {
            kind: super::super::index::loot::LootKind::SetLoot,
            items: items.iter().map(|s| s.to_string()).collect(),
            line,
            variants: variants.iter().map(|s| s.to_string()).collect(),
        };
        let mut old = BTreeMap::new();
        old.insert(
            "prefabs/hound.lua".to_string(),
            vec![rec(&["monstermeat", "houndstooth"], &["hound"], 10)],
        );
        let mut new = BTreeMap::new();
        new.insert(
            "prefabs/hound.lua".to_string(),
            vec![rec(&["houndstooth", "rocks"], &["hound"], 12)],
        );

        let changes = pair_loot_changes(&old, &new);
        assert_eq!(changes.len(), 2);
        let removed = changes
            .iter()
            .find(|c| c.field == "loot[monstermeat]")
            .unwrap();
        assert_eq!(removed.old, Some(Literal::Str("monstermeat".into())));
        assert_eq!(removed.new, None);
        let added = changes.iter().find(|c| c.field == "loot[rocks]").unwrap();
        assert_eq!(added.old, None);
        assert_eq!(added.new, Some(Literal::Str("rocks".into())));
        // Unchanged fact (houndstooth) and file-moved-only facts produce nothing.
        assert!(!changes.iter().any(|c| c.field == "loot[houndstooth]"));
    }
}
