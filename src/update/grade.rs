//! Grading: classify FactChanges into intervention tiers (report-only).
//!
//! Implements docs/UPDATE_IMPACT_PLAN.md §4.2.6 step 4 as a pure function.
//! The corpus side supplies a read-only [`CorpusPageView`] (variant→page
//! mapping + per-page numeric fact candidates); this module never mutates
//! anything and never talks to the wiki.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::fact::{FactChange, FactKind, Literal};
use crate::corpus::facts::FactCandidate;

/// Intervention tier per §4.2.6. `AutoHandled`/`NotifyOnly` are reserved for
/// the Tier0 scheduler and third-party targets and are not produced here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GradeTier {
    /// F1 privileged path / stat template — deterministic draft candidate.
    SuggestDraft,
    /// No reliable landing point on the page — human review with evidence.
    Manual,
    /// Entity variant has no page — creation checklist row.
    CreateCheck,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradedChange {
    pub prefab: String,
    pub field: String,
    pub tier: GradeTier,
    /// Change payload carried through so review records are self-contained.
    pub old: Option<super::fact::Literal>,
    pub new: Option<super::fact::Literal>,
    pub pageid: Option<i64>,
    /// Landing-point evidence: the matched page fact's region and raw text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing: Option<LandingRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LandingRef {
    pub region_id: String,
    pub raw: String,
}

/// Read-only corpus lookup structures, loaded once per scan.
#[derive(Debug, Default)]
pub struct CorpusPageView {
    /// prefab variant → pageids (from `index/pages_by_prefab.json`);
    /// multi-page variants fan out into one graded row per page.
    pub pages: HashMap<String, Vec<i64>>,
    /// pageid → numeric fact candidates (from `index/facts.jsonl`).
    pub facts: HashMap<i64, Vec<FactCandidate>>,
}

impl CorpusPageView {
    /// Loads the view from a corpus host root (`wikis/<host>/`). Missing
    /// files yield an empty view rather than failing — grading degrades to
    /// CreateCheck/Manual rows and the report says so.
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

/// Locates the strongest landing point for one presence-style old value
/// (item name) on the page: first fact whose normalized raw mentions it.
fn find_name_landing<'a>(facts: &'a [FactCandidate], item: &str) -> Option<&'a FactCandidate> {
    let needle = squash(item);
    facts.iter().find(|f| squash(&f.raw).contains(&needle))
}

/// Locates the strongest landing point for one numeric old-literal on the
/// page: the first fact numerically compatible with it.
fn find_landing(facts: &[FactCandidate], old_num: f64) -> Option<&FactCandidate> {
    facts.iter().find(|f| {
        f.values
            .iter()
            .any(|v| crate::corpus::facts::numbers_compatible(old_num, *v))
    })
}

/// Grades every change. Loot-family rules only for now (M2); Stat/Behavior/
/// Backref land in `Manual` with their depth recorded until their extractor
/// families define landing semantics (§4.2.3 期次表).
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
                    out.push(grade_on_page(c, pageid, view));
                }
            }
        }
    }
    out
}

/// Grades one change against one page.
fn grade_on_page(c: &FactChange, pageid: i64, view: &CorpusPageView) -> GradedChange {
    let old = c.old.clone();
    let new = c.new.clone();
    let _ = (&old, &new);

    {
        let empty = Vec::new();
        let facts = view.facts.get(&pageid).unwrap_or(&empty);
        let tier = match c.kind {
            FactKind::Loot => {
                let num = c.old.as_ref().and_then(Literal::as_num);
                let hit = num.and_then(|n| find_landing(facts, n));
                match (&c.old, hit) {
                    // Paired numeric old-value with a page landing →
                    // deterministic draft candidate (F1 privileged path).
                    (Some(_), Some(f)) => GradedChange {
                        old: c.old.clone(),
                        new: c.new.clone(),

                        prefab: c.prefab.clone(),
                        field: c.field.clone(),
                        tier: GradeTier::SuggestDraft,
                        pageid: Some(pageid),
                        landing: Some(LandingRef {
                            region_id: f.region_id.clone(),
                            raw: f.raw.clone(),
                        }),
                    },
                    // Numeric old value with nothing on the page carrying
                    // it: either stale-page cleanup or non-transcribed
                    // fact — human decides.
                    (Some(Literal::Num(_)), None) => no_landing(c, pageid),
                    // No numeric side: paired presence change — locate by
                    // the moved item's name on the page.
                    _ => {
                        let item = c
                            .old
                            .as_ref()
                            .and_then(Literal::as_str)
                            .or_else(|| c.new.as_ref().and_then(Literal::as_str));
                        match item.and_then(|i| find_name_landing(facts, i)) {
                            Some(f) => GradedChange {
                                old: c.old.clone(),
                                new: c.new.clone(),

                                prefab: c.prefab.clone(),
                                field: c.field.clone(),
                                tier: GradeTier::SuggestDraft,
                                pageid: Some(pageid),
                                landing: Some(LandingRef {
                                    region_id: f.region_id.clone(),
                                    raw: f.raw.clone(),
                                }),
                            },
                            None => no_landing(c, pageid),
                        }
                    }
                }
            }
            _ => no_landing(c, pageid),
        };
        tier
    }
}

fn no_landing(c: &FactChange, pageid: i64) -> GradedChange {
    let old = c.old.clone();
    let new = c.new.clone();

    GradedChange {
        old: c.old.clone(),
        new: c.new.clone(),

        prefab: c.prefab.clone(),
        field: c.field.clone(),
        tier: GradeTier::Manual,
        pageid: Some(pageid),
        landing: None,
    }
}

/// Tier distribution for the report summary section.
pub fn summarize(graded: &[GradedChange]) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for g in graded {
        let key = match g.tier {
            GradeTier::SuggestDraft => "suggest_draft",
            GradeTier::Manual => "manual",
            GradeTier::CreateCheck => "create_check",
        };
        *m.entry(key).or_default() += 1;
    }
    m
}

/// Report-facing Layer B summary (attached to [`super::impact::ImpactReport`]
/// when a corpus directory was supplied).
#[derive(Debug, Clone, Serialize)]
pub struct LayerBSummary {
    /// Current-state loot anchors graded (pre-pairing).
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
    //! 猎犬页 fixture：mini RichTab 页跑通 segment→facts→grading 全链。
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
        assert!(
            facts.iter().any(|f| f.fact_kind == "loot"),
            "fixture 必须产出掉落事实"
        );
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
        let view = hound_view();
        let graded = grade_changes(&[loot_change(Some(12.5))], &view);
        assert_eq!(graded[0].tier, GradeTier::SuggestDraft);
        assert_eq!(graded[0].pageid, Some(13857));
        let l = graded[0].landing.as_ref().unwrap();
        assert!(l.region_id.starts_with("13857:"));
        assert!(
            l.raw.contains("12.5%") || l.raw.contains("犬牙"),
            "{}",
            l.raw
        );
    }

    #[test]
    fn fraction_percent_tolerance_applies() {
        let view = hound_view();
        // Code-side fraction 0.125 ↔ page percent 12.5%.
        let graded = grade_changes(&[loot_change(Some(0.125))], &view);
        assert_eq!(graded[0].tier, GradeTier::SuggestDraft);
    }

    #[test]
    fn unmatched_old_value_and_unknown_variants() {
        let view = hound_view();
        // Number present nowhere on the page → manual.
        assert_eq!(
            grade_changes(&[loot_change(Some(99.0))], &view)[0].tier,
            GradeTier::Manual
        );
        // Unpaired current-state anchor → manual row awaiting pairing.
        assert_eq!(
            grade_changes(&[loot_change(None)], &view)[0].tier,
            GradeTier::Manual
        );
        // Unknown variant → create-check.
        let mut unknown = loot_change(Some(12.5));
        unknown.prefab = "moonbeast".into();
        let graded = grade_changes(&[unknown], &view);
        assert_eq!(graded[0].tier, GradeTier::CreateCheck);
        assert_eq!(graded[0].pageid, None);
    }

    #[test]
    fn presence_change_locates_by_item_name() {
        let view = hound_view();
        // Removal of an item the page lists → draft candidate anchored there.
        let mut rm = loot_change(Some(12.5));
        rm.old = Some(Literal::Str("犬牙".into()));
        rm.new = None;
        assert_eq!(grade_changes(&[rm], &view)[0].tier, GradeTier::SuggestDraft);
        // Item absent from the page entirely → manual.
        let mut gone = loot_change(Some(12.5));
        gone.old = Some(Literal::Str("不存在的材料".into()));
        gone.new = None;
        assert_eq!(grade_changes(&[gone], &view)[0].tier, GradeTier::Manual);
    }

    #[test]
    fn summarize_counts_tiers() {
        let view = hound_view();
        let changes = vec![loot_change(Some(12.5)), loot_change(Some(99.0))];
        let graded = grade_changes(&changes, &view);
        let s = summarize(&graded);
        assert_eq!(s.get("suggest_draft"), Some(&1));
        assert_eq!(s.get("manual"), Some(&1));
    }
}
