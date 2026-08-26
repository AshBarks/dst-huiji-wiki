//! Page → Symbol annotation (P0/P1).
//!
//! Determines, for a code-side symbol, which prefab variants and wiki pages
//! it can influence, and collects the candidate page evidence (regions/facts)
//! that will later drive consistency checks and Code → Page prompts.

use serde::Serialize;
use std::collections::BTreeSet;

use super::grade::CorpusPageView;
use super::index::edges::IndexArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    File,
    Fn,
    Const,
    Behaviour,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SymbolRef {
    /// A whole code file (components/x.lua, stategraphs/SGx.lua, brains/x.lua).
    File { path: String },
    /// A named function in a prefab file.
    Fn { file: String, name: String },
    /// A file-local constant (e.g. `SEE_DIST` in a brain file).
    Const { file: String, name: String },
    /// A behaviour constructor (e.g. `Wander`).
    Behaviour { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolPageVisibility {
    PageVisible,
    NotPageVisible,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageEvidence {
    pub pageid: i64,
    pub region_id: String,
    pub raw: String,
    pub snippet: String,
    /// How this evidence was found; P1 always uses `candidate`.
    pub matched_by: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolPageAnnotation {
    pub symbol: SymbolRef,
    pub kind: SymbolKind,
    pub affected_variants: Vec<String>,
    pub affected_pageids: Vec<i64>,
    pub visibility: SymbolPageVisibility,
    pub page_evidence: Vec<PageEvidence>,
}

fn variant_pages(view: &CorpusPageView, variants: &[String]) -> BTreeSet<i64> {
    let mut out = BTreeSet::new();
    for v in variants {
        let variant = v.rsplit('#').next().unwrap_or(v);
        if let Some(ids) = view.pages.get(variant) {
            out.extend(ids.iter().copied());
            continue;
        }
        let lower = variant.to_lowercase();
        if let Some((_, ids)) = view.pages.iter().find(|(k, _)| k.to_lowercase() == lower) {
            out.extend(ids.iter().copied());
        }
    }
    out
}

fn affected_variants(artifact: &IndexArtifact, symbol: &SymbolRef) -> Vec<String> {
    let mut set = BTreeSet::new();
    match symbol {
        SymbolRef::File { path } => {
            if path.starts_with("prefabs/") {
                if let Some(owners) = artifact.fn_owners.get(path) {
                    for vs in owners.values() {
                        for v in vs {
                            set.insert(format!("{path}#{v}"));
                        }
                    }
                }
            } else if let Some(entries) = artifact.reverse.get(path) {
                set.extend(entries.iter().cloned());
            }
        }
        SymbolRef::Fn { file, name } => {
            if let Some(owners) = artifact.fn_owners.get(file).and_then(|m| m.get(name)) {
                for v in owners {
                    set.insert(format!("{file}#{v}"));
                }
            }
        }
        SymbolRef::Const { file, .. } => {
            if file.starts_with("prefabs/") {
                if let Some(owners) = artifact.fn_owners.get(file) {
                    for vs in owners.values() {
                        for v in vs {
                            set.insert(format!("{file}#{v}"));
                        }
                    }
                }
            } else if let Some(entries) = artifact.reverse.get(file) {
                set.extend(entries.iter().cloned());
            }
        }
        SymbolRef::Behaviour { name } => {
            for b in &artifact.behaviour_calls {
                if b.ctor == *name {
                    set.extend(b.prefab_variants.iter().cloned());
                }
            }
        }
    }
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    out
}

/// Builds a P1 annotation for one symbol: affected variants → affected pages
/// → candidate page evidence.
pub fn annotate_symbol(
    artifact: &IndexArtifact,
    view: &CorpusPageView,
    symbol: SymbolRef,
) -> SymbolPageAnnotation {
    let kind = match &symbol {
        SymbolRef::File { .. } => SymbolKind::File,
        SymbolRef::Fn { .. } => SymbolKind::Fn,
        SymbolRef::Const { .. } => SymbolKind::Const,
        SymbolRef::Behaviour { .. } => SymbolKind::Behaviour,
    };
    let affected_variants = affected_variants(artifact, &symbol);
    let affected_pageids: Vec<i64> = variant_pages(view, &affected_variants)
        .into_iter()
        .collect();

    let mut page_evidence = Vec::new();
    for &pageid in &affected_pageids {
        if let Some(facts) = view.facts.get(&pageid) {
            for f in facts {
                page_evidence.push(PageEvidence {
                    pageid,
                    region_id: f.region_id.clone(),
                    raw: f.raw.clone(),
                    snippet: f.snippet.clone(),
                    matched_by: "candidate",
                });
            }
        }
    }

    let visibility = if affected_pageids.is_empty() {
        SymbolPageVisibility::NotPageVisible
    } else if page_evidence.is_empty() {
        SymbolPageVisibility::Ambiguous
    } else {
        SymbolPageVisibility::PageVisible
    };

    SymbolPageAnnotation {
        symbol,
        kind,
        affected_variants,
        affected_pageids,
        visibility,
        page_evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::facts::FactCandidate;
    use crate::update::index::build_from_sources;

    const HOUND_PREFAB: &str = r#"
local brain = require("brains/houndbrain")

local function fncommon(inst)
    inst:AddComponent("combat")
    inst.components.combat:SetDefaultDamage(TUNING.HOUND_DAMAGE)
    inst:SetBrain(brain)
    return inst
end

local function fndefault()
    return fncommon("hound")
end

return Prefab("hound", fndefault, {}, {})
"#;

    const HOUND_BRAIN: &str = r#"
local SEE_DIST = 30
return Brain(inst, PriorityNode({}, 1))
"#;

    fn hound_view() -> CorpusPageView {
        let mut view = CorpusPageView::default();
        view.pages.insert("hound".to_string(), vec![13857]);
        view.facts.insert(
            13857,
            vec![FactCandidate {
                pageid: 13857,
                region_id: "13857:section:0".to_string(),
                fact_kind: "quantity".to_string(),
                raw: "30 距离单位".to_string(),
                values: vec![30.0],
                unit: Some("距离单位".to_string()),
                snippet: "寻找 30 距离单位内的目标".to_string(),
            }],
        );
        view
    }

    #[test]
    fn file_symbol_resolves_pages_and_evidence() {
        let files = vec![
            ("prefabs/hound.lua".to_string(), HOUND_PREFAB.to_string()),
            ("brains/houndbrain.lua".to_string(), HOUND_BRAIN.to_string()),
        ];
        let artifact = build_from_sources(&files).unwrap();
        let view = hound_view();

        let ann = annotate_symbol(
            &artifact,
            &view,
            SymbolRef::Const {
                file: "brains/houndbrain.lua".to_string(),
                name: "SEE_DIST".to_string(),
            },
        );
        assert_eq!(ann.kind, SymbolKind::Const);
        assert!(ann
            .affected_variants
            .contains(&"prefabs/hound.lua#hound".to_string()));
        assert_eq!(ann.affected_pageids, vec![13857]);
        assert_eq!(ann.visibility, SymbolPageVisibility::PageVisible);
        assert!(!ann.page_evidence.is_empty());
        assert_eq!(ann.page_evidence[0].raw, "30 距离单位");
    }

    #[test]
    fn fn_symbol_uses_fn_owners() {
        let files = vec![("prefabs/hound.lua".to_string(), HOUND_PREFAB.to_string())];
        let artifact = build_from_sources(&files).unwrap();
        let view = hound_view();

        let ann = annotate_symbol(
            &artifact,
            &view,
            SymbolRef::Fn {
                file: "prefabs/hound.lua".to_string(),
                name: "fncommon".to_string(),
            },
        );
        assert_eq!(ann.kind, SymbolKind::Fn);
        assert!(ann
            .affected_variants
            .contains(&"prefabs/hound.lua#hound".to_string()));
        assert_eq!(ann.affected_pageids, vec![13857]);
    }
}
