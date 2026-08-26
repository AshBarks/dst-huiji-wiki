//! Page → Symbol annotation (P0/P1).
//!
//! Determines, for a code-side symbol, which prefab variants and wiki pages
//! it can influence, and collects the candidate page evidence (regions/facts)
//! that will later drive consistency checks and Code → Page prompts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::grade::CorpusPageView;
use super::index::edges::IndexArtifact;
use crate::Result;

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

/// P3 output contract: one page's verdict for a symbol annotation task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageSymbolVerdict {
    pub pageid: i64,
    pub mentions: bool,
    #[serde(default)]
    pub wording: Option<String>,
    #[serde(default)]
    pub semantic_consistent: Option<bool>,
    #[serde(default)]
    pub missing: bool,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// P3 output contract: full LLM response for one symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolAnnotationResponse {
    pub symbol: String,
    pub verdicts: Vec<PageSymbolVerdict>,
}

pub fn symbol_display(symbol: &SymbolRef) -> String {
    match symbol {
        SymbolRef::File { path } => path.clone(),
        SymbolRef::Fn { file, name } => format!("{file}#{name}"),
        SymbolRef::Const { file, name } => format!("{file}#{name}"),
        SymbolRef::Behaviour { name } => format!("behaviours/{name}"),
    }
}

/// P3: render a Page → Symbol annotation prompt from one evidence pack.
pub fn render_symbol_annotation_prompt(ann: &SymbolPageAnnotation, code_semantics: &str) -> String {
    let symbol = symbol_display(&ann.symbol);
    let mut out = String::new();
    out.push_str("你是 DST Huiji Wiki 的代码符号页面影响标注助手。\n");
    out.push_str("请判断给定代码符号是否影响以下 wiki 页面，并检测页面是否提及、是否语义一致、是否缺失。\n\n");
    out.push_str(&format!("## Symbol\n`{symbol}`\n\n"));
    out.push_str(&format!("## Code semantics\n{code_semantics}\n\n"));
    out.push_str(&format!(
        "## Affected variants\n{}\n\n",
        ann.affected_variants.join(", ")
    ));
    out.push_str("## Affected pages and candidate evidence\n");
    for pageid in &ann.affected_pageids {
        out.push_str(&format!("### page {pageid}\n"));
        let evs: Vec<_> = ann
            .page_evidence
            .iter()
            .filter(|e| e.pageid == *pageid)
            .collect();
        if evs.is_empty() {
            out.push_str("（无候选数值事实）\n");
        }
        for e in evs {
            out.push_str(&format!("- [{}] {}\n", e.region_id, e.snippet));
        }
        out.push('\n');
    }
    out.push_str(
        "## 输出要求\n请输出 JSON 数组，每个元素包含：\n         {\"pageid\": 数字, \"mentions\": true/false, \"wording\": 字符串或null, \"semantic_consistent\": true/false或null, \"missing\": true/false, \"confidence\": \"high|medium|low\", \"note\": 字符串或null}\n",
    );
    out
}

/// P3: parse an LLM response into page verdicts.
///
/// Accepts either a bare JSON array or `{"symbol": "...", "verdicts": [...]}`.
pub fn parse_symbol_annotation_response(raw: &str) -> Result<Vec<PageSymbolVerdict>> {
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str::<Vec<PageSymbolVerdict>>(trimmed) {
        return Ok(v);
    }
    let v: serde_json::Value = serde_json::from_str(trimmed)?;
    if let Some(arr) = v
        .get("verdicts")
        .or_else(|| v.get("results"))
        .and_then(|x| x.as_array())
    {
        let arr = serde_json::Value::Array(arr.clone());
        return Ok(serde_json::from_value(arr)?);
    }
    Err(crate::error::Error::Config(
        "SymbolAnnotationResponse 缺少 verdicts/results 数组".to_string(),
    ))
}

/// P4: one page that should mention a symbol but does not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissingPage {
    pub pageid: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// P4: one page whose wording semantically disagrees with the code symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InconsistentPage {
    pub pageid: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wording: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
}

/// P4: cross-page coverage report for a single symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolCoverageReport {
    pub symbol: String,
    pub total_pages: usize,
    pub mentioned_pages: Vec<i64>,
    pub missing_pages: Vec<MissingPage>,
    pub inconsistent_pages: Vec<InconsistentPage>,
}

/// P4: aggregate per-page verdicts into a cross-page coverage report.
pub fn build_coverage_report(symbol: &str, verdicts: &[PageSymbolVerdict]) -> SymbolCoverageReport {
    let mut mentioned: Vec<i64> = verdicts
        .iter()
        .filter(|v| v.mentions)
        .map(|v| v.pageid)
        .collect();
    mentioned.sort_unstable();
    mentioned.dedup();

    let mut missing: Vec<MissingPage> = verdicts
        .iter()
        .filter(|v| v.missing)
        .map(|v| MissingPage {
            pageid: v.pageid,
            note: v.note.clone(),
        })
        .collect();
    missing.sort_by_key(|m| m.pageid);

    let mut inconsistent: Vec<InconsistentPage> = verdicts
        .iter()
        .filter(|v| v.mentions && v.semantic_consistent == Some(false))
        .map(|v| InconsistentPage {
            pageid: v.pageid,
            wording: v.wording.clone(),
            note: v.note.clone(),
            confidence: v.confidence.clone(),
        })
        .collect();
    inconsistent.sort_by_key(|i| i.pageid);

    SymbolCoverageReport {
        symbol: symbol.to_string(),
        total_pages: verdicts.len(),
        mentioned_pages: mentioned,
        missing_pages: missing,
        inconsistent_pages: inconsistent,
    }
}

/// P4: build coverage reports from a batch of symbol annotation responses.
pub fn build_coverage_reports(responses: &[SymbolAnnotationResponse]) -> Vec<SymbolCoverageReport> {
    responses
        .iter()
        .map(|r| build_coverage_report(&r.symbol, &r.verdicts))
        .collect()
}

/// P4: render a coverage report as Markdown for human review.
pub fn render_coverage_report_md(report: &SymbolCoverageReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("## `{}`\n\n", report.symbol));
    out.push_str(&format!("- total_pages: {}\n", report.total_pages));
    out.push_str(&format!(
        "- mentioned_pages: {} ({})\n",
        report.mentioned_pages.len(),
        report
            .mentioned_pages
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!(
        "- missing_pages: {}\n",
        report.missing_pages.len()
    ));
    for m in &report.missing_pages {
        out.push_str(&format!(
            "  - {} {}\n",
            m.pageid,
            m.note.as_deref().unwrap_or("")
        ));
    }
    out.push_str(&format!(
        "- inconsistent_pages: {}\n",
        report.inconsistent_pages.len()
    ));
    for i in &report.inconsistent_pages {
        out.push_str(&format!(
            "  - {} {}{}\n",
            i.pageid,
            i.wording.as_deref().unwrap_or(""),
            i.note
                .as_deref()
                .map(|n| format!(" — {n}"))
                .unwrap_or_default()
        ));
    }
    out.push('\n');
    out
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

    #[test]
    fn prompt_renders_symbol_and_evidence() {
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
        let prompt = render_symbol_annotation_prompt(&ann, "hound 在 30 距离单位内寻找目标");
        assert!(prompt.contains("## Symbol"));
        assert!(prompt.contains("brains/houndbrain.lua#SEE_DIST"));
        assert!(prompt.contains("page 13857"));
        assert!(prompt.contains("30 距离单位"));
    }

    #[test]
    fn parse_symbol_annotation_response_accepts_array_and_object() {
        let array = r#"[
            {"pageid": 13857, "mentions": true, "wording": "30 距离单位", "semantic_consistent": true, "missing": false, "confidence": "high", "note": null}
        ]"#;
        let parsed = parse_symbol_annotation_response(array).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].pageid, 13857);
        assert!(parsed[0].mentions);

        let wrapped = r#"{"symbol": "SEE_DIST", "verdicts": [
            {"pageid": 13857, "mentions": false, "missing": true, "semantic_consistent": null, "confidence": "medium", "note": "missing"}
        ]}"#;
        let parsed = parse_symbol_annotation_response(wrapped).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(!parsed[0].mentions);
        assert!(parsed[0].missing);
    }

    #[test]
    fn coverage_report_aggregates_verdicts() {
        let verdicts = vec![
            PageSymbolVerdict {
                pageid: 13857,
                mentions: true,
                wording: Some("30 距离单位".to_string()),
                semantic_consistent: Some(true),
                missing: false,
                confidence: Some("high".to_string()),
                note: None,
            },
            PageSymbolVerdict {
                pageid: 23210,
                mentions: false,
                wording: None,
                semantic_consistent: None,
                missing: true,
                confidence: Some("medium".to_string()),
                note: Some("同组页面均描述该行为".to_string()),
            },
            PageSymbolVerdict {
                pageid: 73451,
                mentions: true,
                wording: Some("40 距离单位".to_string()),
                semantic_consistent: Some(false),
                missing: false,
                confidence: Some("high".to_string()),
                note: Some("代码为 30".to_string()),
            },
        ];
        let report = build_coverage_report("SEE_DIST", &verdicts);
        assert_eq!(report.total_pages, 3);
        assert_eq!(report.mentioned_pages, vec![13857, 73451]);
        assert_eq!(report.missing_pages.len(), 1);
        assert_eq!(report.missing_pages[0].pageid, 23210);
        assert_eq!(report.inconsistent_pages.len(), 1);
        assert_eq!(report.inconsistent_pages[0].pageid, 73451);
        assert_eq!(
            report.inconsistent_pages[0].wording.as_deref(),
            Some("40 距离单位")
        );

        let md = render_coverage_report_md(&report);
        assert!(md.contains("## `SEE_DIST`"));
        assert!(md.contains("missing_pages: 1"));
        assert!(md.contains("inconsistent_pages: 1"));
    }

    #[test]
    fn coverage_reports_batch_from_responses() {
        let responses = vec![
            SymbolAnnotationResponse {
                symbol: "SEE_DIST".to_string(),
                verdicts: vec![PageSymbolVerdict {
                    pageid: 1,
                    mentions: true,
                    wording: None,
                    semantic_consistent: Some(true),
                    missing: false,
                    confidence: None,
                    note: None,
                }],
            },
            SymbolAnnotationResponse {
                symbol: "Wander".to_string(),
                verdicts: vec![PageSymbolVerdict {
                    pageid: 2,
                    mentions: false,
                    wording: None,
                    semantic_consistent: None,
                    missing: true,
                    confidence: None,
                    note: None,
                }],
            },
        ];
        let reports = build_coverage_reports(&responses);
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].symbol, "SEE_DIST");
        assert_eq!(reports[1].symbol, "Wander");
        assert_eq!(reports[1].missing_pages.len(), 1);
    }
}
