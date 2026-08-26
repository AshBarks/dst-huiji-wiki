//! Grading: classify FactChanges into intervention tiers (report-only).
use super::fact::{FactChange, FactKind, Literal};
use crate::corpus::facts::FactCandidate;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GradeTier {
    /// Autoinfobox/A 层已覆盖（bot 同步域）。
    AutoHandled,
    SuggestDraft,
    Manual,
    CreateCheck,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradedChange {
    pub prefab: String,
    pub field: String,
    pub tier: GradeTier,
    pub old: Option<Literal>,
    pub new: Option<Literal>,
    pub pageid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing: Option<LandingRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LandingRef {
    pub region_id: String,
    pub raw: String,
}

#[derive(Debug, Default)]
pub struct CorpusPageView {
    pub pages: HashMap<String, Vec<i64>>,
    pub facts: HashMap<i64, Vec<FactCandidate>>,
}

impl CorpusPageView {
    pub fn load(root: &std::path::Path) -> std::io::Result<Self> {
        let mut view = Self::default();
        let reg_raw = match std::fs::read_to_string(root.join("index/pages_by_prefab.json")) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(view),
            Err(e) => return Err(e),
        };
        #[derive(serde::Deserialize)]
        struct RegView {
            prefabs: BTreeMap<String, Vec<i64>>,
        }
        let reg: RegView = serde_json::from_str(&reg_raw)?;
        for (variant, mut ids) in reg.prefabs {
            ids.sort();
            if !ids.is_empty() {
                view.pages.insert(variant, ids);
            }
        }
        let facts_raw = match std::fs::read_to_string(root.join("index/facts.jsonl")) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(view),
            Err(e) => return Err(e),
        };
        for line in facts_raw.lines() {
            if line.is_empty() {
                continue;
            }
            if let Ok(f) = serde_json::from_str::<FactCandidate>(line) {
                view.facts.entry(f.pageid).or_default().push(f);
            }
        }
        Ok(view)
    }
}

fn squash(v: &str) -> String {
    v.to_lowercase().replace('_', "")
}

fn find_landing(facts: &[FactCandidate], old_num: f64) -> Option<&FactCandidate> {
    facts.iter().find(|f| {
        f.values
            .iter()
            .any(|v| crate::corpus::facts::numbers_compatible(old_num, *v))
    })
}

fn find_name_landing<'a>(facts: &'a [FactCandidate], item: &str) -> Option<&'a FactCandidate> {
    let needle = squash(item);
    facts.iter().find(|f| squash(&f.raw).contains(&needle))
}

fn no_landing(c: &FactChange, pageid: i64) -> GradedChange {
    GradedChange {
        prefab: c.prefab.clone(),
        field: c.field.clone(),
        tier: GradeTier::Manual,
        old: c.old.clone(),
        new: c.new.clone(),
        pageid: Some(pageid),
        landing: None,
    }
}

fn landed(c: &FactChange, pageid: i64, f: &FactCandidate) -> GradedChange {
    GradedChange {
        prefab: c.prefab.clone(),
        field: c.field.clone(),
        tier: GradeTier::SuggestDraft,
        old: c.old.clone(),
        new: c.new.clone(),
        pageid: Some(pageid),
        landing: Some(LandingRef {
            region_id: f.region_id.clone(),
            raw: f.raw.clone(),
        }),
    }
}

pub fn grade_changes(changes: &[FactChange], view: &CorpusPageView) -> Vec<GradedChange> {
    let mut out = Vec::new();
    for c in changes {
        match view.pages.get(&c.prefab) {
            None => out.push(GradedChange {
                prefab: c.prefab.clone(),
                field: c.field.clone(),
                tier: GradeTier::CreateCheck,
                old: c.old.clone(),
                new: c.new.clone(),
                pageid: None,
                landing: None,
            }),
            Some(pageids) => {
                for &pageid in pageids {
                    let empty = Vec::new();
                    let facts = view.facts.get(&pageid).unwrap_or(&empty);
                    let g = match c.kind {
                        FactKind::Loot => {
                            let num = c.old.as_ref().and_then(Literal::as_num);
                            let hit = num.and_then(|n| find_landing(facts, n));
                            match (&c.old, hit) {
                                (Some(_), Some(f)) => landed(c, pageid, f),
                                (Some(Literal::Num(_)), None) => no_landing(c, pageid),
                                _ => {
                                    let item = c
                                        .old
                                        .as_ref()
                                        .and_then(Literal::as_str)
                                        .or_else(|| c.new.as_ref().and_then(Literal::as_str));
                                    match item.and_then(|i| find_name_landing(facts, i)) {
                                        Some(f) => landed(c, pageid, f),
                                        None => no_landing(c, pageid),
                                    }
                                }
                            }
                        }
                        _ => no_landing(c, pageid),
                    };
                    out.push(g);
                }
            }
        }
    }
    out
}

pub fn summarize(graded: &[GradedChange]) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for g in graded {
        let key = match g.tier {
            GradeTier::AutoHandled => "auto_handled",
            GradeTier::SuggestDraft => "suggest_draft",
            GradeTier::Manual => "manual",
            GradeTier::CreateCheck => "create_check",
        };
        *m.entry(key).or_default() += 1;
    }
    m
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerBSummary {
    pub anchors: usize,
    pub tiers: BTreeMap<&'static str, usize>,
}

impl From<&[GradedChange]> for LayerBSummary {
    fn from(graded: &[GradedChange]) -> Self {
        Self {
            anchors: graded.len(),
            tiers: summarize(graded),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{facts::extract as extract_facts, segment::segment};

    fn hound_view() -> CorpusPageView {
        const PAGE: &str = "{{RichTab/信息框\n\
             |普通猎犬|\n{{实体信息框/自动|dst|hound\n|掉落 = {{Pic|32|怪物肉}}×1<br>{{Pic|32|犬牙}}×1（12.5%）\n}}\n\
             }}\n'''猎犬'''是联机版中的一种生物。\n==行为==\n成群袭击玩家。";
        let regions = segment(13857, PAGE);
        let mut view = CorpusPageView {
            pages: [("hound".to_string(), vec![13857i64])]
                .into_iter()
                .collect(),
            facts: HashMap::new(),
        };
        let mut facts = Vec::new();
        for r in &regions {
            facts.extend(extract_facts(13857, &r.id, &PAGE[r.start_byte..r.end_byte]));
        }
        view.facts.insert(13857, facts);
        view
    }

    fn loot_change(old_num: Option<f64>) -> FactChange {
        FactChange {
            prefab: "hound".into(),
            kind: FactKind::Loot,
            field: "loot[monstermeat]".into(),
            context: None,
            source_file: "prefabs/hound.lua".into(),
            old: old_num.map(Literal::Num),
            new: None,
            derivation_depth: 1,
            evidence: vec![],
        }
    }

    #[test]
    fn paired_hit_lands_on_page_loot_fact() {
        let graded = grade_changes(&[loot_change(Some(12.5))], &hound_view());
        assert_eq!(graded[0].tier, GradeTier::SuggestDraft);
        assert_eq!(graded[0].pageid, Some(13857));
        assert_eq!(graded[0].old, Some(Literal::Num(12.5)));
    }

    #[test]
    fn fraction_percent_tolerance_applies() {
        assert_eq!(
            grade_changes(&[loot_change(Some(0.125))], &hound_view())[0].tier,
            GradeTier::SuggestDraft
        );
    }

    #[test]
    fn unmatched_old_value_and_unknown_variants() {
        let v = hound_view();
        assert_eq!(
            grade_changes(&[loot_change(Some(99.0))], &v)[0].tier,
            GradeTier::Manual
        );
        assert_eq!(
            grade_changes(&[loot_change(None)], &v)[0].tier,
            GradeTier::Manual
        );
        let mut u = loot_change(Some(12.5));
        u.prefab = "moonbeast".into();
        assert_eq!(grade_changes(&[u], &v)[0].tier, GradeTier::CreateCheck);
    }

    #[test]
    fn presence_change_locates_by_item_name() {
        let v = hound_view();
        let mut rm = loot_change(Some(12.5));
        rm.old = Some(Literal::Str("犬牙".into()));
        assert_eq!(grade_changes(&[rm], &v)[0].tier, GradeTier::SuggestDraft);
        let mut gone = loot_change(Some(12.5));
        gone.old = Some(Literal::Str("不存在的材料".into()));
        assert_eq!(grade_changes(&[gone], &v)[0].tier, GradeTier::Manual);
    }

    #[test]
    fn multipage_variant_fans_out_per_page() {
        let mut v = hound_view();
        v.pages.insert("hound".into(), vec![1, 2]);
        let graded = grade_changes(&[loot_change(Some(12.5))], &v);
        assert_eq!(graded.len(), 2);
        assert_eq!(graded[1].pageid, Some(2));
    }

    #[test]
    fn summarize_counts_tiers() {
        let changes = vec![loot_change(Some(12.5)), loot_change(Some(99.0))];
        let s = summarize(&grade_changes(&changes, &hound_view()));
        assert_eq!(s.get("suggest_draft"), Some(&1));
        assert_eq!(s.get("manual"), Some(&1));
    }
}
