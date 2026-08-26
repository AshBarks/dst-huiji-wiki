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

fn symbol_variant_count(artifact: &IndexArtifact, symbol: &SymbolRef) -> usize {
    match symbol {
        SymbolRef::File { path } => {
            if path.starts_with("prefabs/") {
                artifact
                    .fn_owners
                    .get(path)
                    .map(|m| m.values().map(|vs| vs.len()).sum::<usize>())
                    .unwrap_or(0)
            } else {
                artifact.reverse.get(path).map(Vec::len).unwrap_or(0)
            }
        }
        SymbolRef::Fn { file, name } => artifact
            .fn_owners
            .get(file)
            .and_then(|m| m.get(name))
            .map(Vec::len)
            .unwrap_or(0),
        SymbolRef::Const { file, .. } => {
            if file.starts_with("prefabs/") {
                artifact
                    .fn_owners
                    .get(file)
                    .map(|m| m.values().map(|vs| vs.len()).sum::<usize>())
                    .unwrap_or(0)
            } else {
                artifact.reverse.get(file).map(Vec::len).unwrap_or(0)
            }
        }
        SymbolRef::Behaviour { name } => artifact
            .behaviour_calls
            .iter()
            .filter(|b| b.ctor == *name)
            .map(|b| b.prefab_variants.len())
            .sum(),
    }
}

/// P2: select high-reference symbols (file / fn / behaviour) by the number
/// of prefab variants they affect.
pub fn top_symbols(artifact: &IndexArtifact, limit: usize) -> Vec<SymbolRef> {
    let mut seen = BTreeSet::new();
    let mut candidates: Vec<SymbolRef> = Vec::new();

    for path in artifact.reverse.keys() {
        let sym = SymbolRef::File { path: path.clone() };
        if seen.insert(format!("file:{path}")) {
            candidates.push(sym);
        }
    }
    for (file, fns) in &artifact.fn_owners {
        for name in fns.keys() {
            let sym = SymbolRef::Fn {
                file: file.clone(),
                name: name.clone(),
            };
            if seen.insert(format!("fn:{file}:{name}")) {
                candidates.push(sym);
            }
        }
    }
    let mut behaviour_names: BTreeSet<String> = BTreeSet::new();
    for b in &artifact.behaviour_calls {
        behaviour_names.insert(b.ctor.clone());
    }
    for name in behaviour_names {
        let sym = SymbolRef::Behaviour { name: name.clone() };
        if seen.insert(format!("behaviour:{name}")) {
            candidates.push(sym);
        }
    }

    let mut with_count: Vec<(usize, SymbolRef)> = candidates
        .into_iter()
        .map(|s| (symbol_variant_count(artifact, &s), s))
        .collect();
    with_count.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| format!("{:?}", a.1).cmp(&format!("{:?}", b.1)))
    });
    with_count.into_iter().take(limit).map(|(_, s)| s).collect()
}

/// P2: build candidate evidence packs for the top `limit` symbols.
pub fn build_symbol_evidence_packs(
    artifact: &IndexArtifact,
    view: &CorpusPageView,
    limit: usize,
) -> Vec<SymbolPageAnnotation> {
    top_symbols(artifact, limit)
        .into_iter()
        .map(|symbol| annotate_symbol(artifact, view, symbol))
        .collect()
}

/// Render one annotation as a compact Markdown evidence pack for review.
pub fn render_symbol_pack_md(ann: &SymbolPageAnnotation) -> String {
    let symbol = match &ann.symbol {
        SymbolRef::File { path } => path.clone(),
        SymbolRef::Fn { file, name } => format!("{file}#{name}"),
        SymbolRef::Const { file, name } => format!("{file}#{name}"),
        SymbolRef::Behaviour { name } => format!("behaviours/{name}"),
    };
    let mut out = String::new();
    out.push_str(&format!(
        "## `{symbol}`

"
    ));
    out.push_str(&format!("- kind: {:?}\n", ann.kind));
    out.push_str(&format!("- visibility: {:?}\n", ann.visibility));
    out.push_str(&format!(
        "- affected_variants: {} ({})\n",
        ann.affected_variants.len(),
        ann.affected_variants.join(", ")
    ));
    out.push_str(&format!(
        "- affected_pageids: {} ({})\n",
        ann.affected_pageids.len(),
        ann.affected_pageids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!("- evidence: {} 条\n", ann.page_evidence.len()));
    for e in ann.page_evidence.iter().take(8) {
        out.push_str(&format!(
            "  - page {} `{}`：{}（{}）\n",
            e.pageid, e.region_id, e.raw, e.snippet
        ));
    }
    out.push('\n');
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

    #[test]
    fn top_symbols_and_evidence_packs_are_generated() {
        let files = vec![
            ("prefabs/hound.lua".to_string(), HOUND_PREFAB.to_string()),
            ("brains/houndbrain.lua".to_string(), HOUND_BRAIN.to_string()),
        ];
        let artifact = build_from_sources(&files).unwrap();
        let view = hound_view();

        assert!(!top_symbols(&artifact, 10).is_empty());
        let packs = build_symbol_evidence_packs(&artifact, &view, 10);
        assert!(!packs.is_empty());
        let md = render_symbol_pack_md(&packs[0]);
        assert!(md.contains("## `"));
        assert!(md.contains("affected_variants"));
    }
}
