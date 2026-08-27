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

pub(crate) fn variant_pages(view: &CorpusPageView, variants: &[String]) -> BTreeSet<i64> {
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

pub(crate) fn affected_variants(artifact: &IndexArtifact, symbol: &SymbolRef) -> Vec<String> {
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
    render_symbol_annotation_prompt_for_pages(ann, code_semantics, &ann.affected_pageids)
}

/// Default per-batch rendered-input budget (`--batch-max-chars`).
///
/// Tuned so one batch stays inside a comfortable single-request size: page
/// sections average a few hundred bytes, pathological ones a few KB, so
/// ~32KB ≈ mid-single-digit-K tokens of evidence.
pub const DEFAULT_BATCH_MAX_CHARS: usize = 32_000;

/// Evidence count and rendered byte size of each page's prompt section
/// (byte-for-byte mirror of the renderer's per-page loop body).
struct PageWeights {
    counts: std::collections::HashMap<i64, usize>,
    chars: std::collections::HashMap<i64, usize>,
}

impl PageWeights {
    fn build(ann: &SymbolPageAnnotation) -> Self {
        let mut grouped: std::collections::HashMap<i64, Vec<&PageEvidence>> =
            std::collections::HashMap::new();
        for e in &ann.page_evidence {
            grouped.entry(e.pageid).or_default().push(e);
        }
        let mut counts = std::collections::HashMap::new();
        let mut chars = std::collections::HashMap::new();
        for pid in &ann.affected_pageids {
            let evs = grouped.get(pid);
            let n = evs.map_or(0, Vec::len);
            counts.insert(*pid, n);
            let mut sec = format!("### page {pid}\n");
            if n == 0 {
                sec.push_str("（无候选数值事实）\n");
            }
            if let Some(evs) = evs {
                for e in evs {
                    sec.push_str(&format!("- [{}] {}\n", e.region_id, e.snippet));
                }
            }
            sec.push('\n');
            chars.insert(*pid, sec.len());
        }
        Self { counts, chars }
    }

    fn count(&self, pid: i64) -> usize {
        self.counts.get(&pid).copied().unwrap_or(0)
    }

    fn chars(&self, pid: i64) -> usize {
        self.chars.get(&pid).copied().unwrap_or(0)
    }
}

/// Splits a symbol's affected pages into LLM-friendly annotation batches.
///
/// Pages are ordered by evidence density (candidate-fact count descending;
/// ties keep the original order) so information-rich pages land in early
/// batches, then greedily cut into consecutive chunks respecting **both**
/// caps:
///
/// - `max_pages` — structural ceiling on verdicts per request (`0` = unset);
/// - `max_chars` — rendered-input byte budget, so dense mega-pages cannot
///   inflate a page-count-bounded batch into a mega-prompt (`0` = unset).
///
/// With both caps unset the historical single-batch behaviour is preserved
/// exactly (original order, one request). Batches are non-empty and every
/// page lands in exactly one batch; a single page heavier than `max_chars`
/// still gets its own batch rather than being dropped.
pub fn paginate_affected_pages(
    ann: &SymbolPageAnnotation,
    max_pages: usize,
    max_chars: usize,
) -> Vec<Vec<i64>> {
    if max_pages == 0 && max_chars == 0 {
        return vec![ann.affected_pageids.clone()];
    }

    let weights = PageWeights::build(ann);
    let mut ranked = ann.affected_pageids.clone();
    ranked.sort_by_key(|pid| std::cmp::Reverse(weights.count(*pid)));

    let mut batches: Vec<Vec<i64>> = Vec::new();
    let mut cur: Vec<i64> = Vec::new();
    let mut cur_chars = 0usize;
    for pid in ranked {
        let w = weights.chars(pid);
        let page_full = max_pages > 0 && cur.len() >= max_pages;
        let char_full = max_chars > 0 && !cur.is_empty() && cur_chars + w > max_chars;
        if page_full || char_full {
            batches.push(std::mem::take(&mut cur));
            cur_chars = 0;
        }
        cur.push(pid);
        cur_chars += w;
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    batches
}

/// Splits the affected pages into the LLM workload (`Some`) and pages that
/// carry no candidate evidence at all (`None`-worthy leftovers) when
/// `skip_no_facts` is set; otherwise nothing is filtered.
///
/// The returned filter summary pairs every excluded pageid with a locally
/// synthesized verdict so coverage statistics remain complete without paying
/// LLM tokens for sections that would read "（无候选数值事实）" anyway.
pub fn partition_no_fact_pages(
    ann: &SymbolPageAnnotation,
    skip_no_facts: bool,
) -> (Vec<i64>, Vec<PageSymbolVerdict>) {
    if !skip_no_facts {
        return (ann.affected_pageids.clone(), Vec::new());
    }
    let with_facts: std::collections::HashSet<i64> =
        ann.page_evidence.iter().map(|e| e.pageid).collect();
    let mut sent = Vec::new();
    let mut synthesized = Vec::new();
    for pid in &ann.affected_pageids {
        if with_facts.contains(pid) {
            sent.push(*pid);
        } else {
            synthesized.push(PageSymbolVerdict {
                pageid: *pid,
                mentions: false,
                wording: None,
                semantic_consistent: None,
                missing: true,
                confidence: Some("low".to_string()),
                note: Some("页面无候选数值事实证据（未单独送审）".to_string()),
            });
        }
    }
    (sent, synthesized)
}

/// Appends locally synthesized verdicts for every requested page that never
/// appeared in any batch output, returning how many were added.
///
/// Guardrail for multi-batch runs: an LLM occasionally omits 1–2 pages from a
/// chunk's JSON array. Coverage statistics require the requested page set to
/// be fully answered, so gaps are filled with explicit low-confidence
/// placeholders instead of being silently lost.
pub fn fill_missing_requested(requested: &[i64], verdicts: &mut Vec<PageSymbolVerdict>) -> usize {
    let have: std::collections::HashSet<i64> = verdicts.iter().map(|v| v.pageid).collect();
    let mut added = 0usize;
    for pid in requested {
        if !have.contains(pid) {
            verdicts.push(PageSymbolVerdict {
                pageid: *pid,
                mentions: false,
                wording: None,
                semantic_consistent: None,
                missing: true,
                confidence: Some("low".to_string()),
                note: Some("该页未出现在批次输出中（本地补判）".to_string()),
            });
            added += 1;
        }
    }
    added
}

/// Renders the annotation prompt for an explicit subset of the symbol's
/// affected pages (see [`paginate_affected_pages`] for the batching policy).
pub fn render_symbol_annotation_prompt_for_pages(
    ann: &SymbolPageAnnotation,
    code_semantics: &str,
    pageids: &[i64],
) -> String {
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
    out.push_str(&format!(
        "## Affected pages and candidate evidence（本批 {} 页 / 全部 {} 页）\n",
        pageids.len(),
        ann.affected_pageids.len(),
    ));
    for pageid in pageids {
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
        "## 输出要求\n请输出 JSON 数组，每个元素包含：\n         {\"pageid\": 数字, \"mentions\": true/false, \"wording\": 字符串或null, \"semantic_consistent\": true/false或null, \"missing\": true/false, \"confidence\": \"high|medium|low\", \"note\": 字符串或null}\n为节省输出，可选项（wording/semantic_consistent/note）若无内容请直接省略该键，不要输出 null；仅输出本批给出的页面。\n",
    );
    out
}

/// P3: parse an LLM response into page verdicts.
///
/// Accepts either a bare JSON array or `{"symbol": "...", "verdicts": [...]}`.
/// Common LLM output defects are tolerated, in order:
///
/// 1. a Markdown code fence (```json ... ```) around the payload;
/// 2. prose before/after the payload (the outermost `[...]` is extracted);
/// 3. trailing commas inside objects/arrays.
pub fn parse_symbol_annotation_response(raw: &str) -> Result<Vec<PageSymbolVerdict>> {
    let trimmed = strip_code_fence(raw.trim());
    let sliced = extract_outermost_array(trimmed);
    // Independent fixes plus their combination (a payload may suffer from
    // several defects at once).
    let sliced_fixed = sliced.map(remove_trailing_commas);
    let fixed = remove_trailing_commas(trimmed);
    let candidates: Vec<&str> = Vec::from_iter(
        [
            Some(trimmed),
            Some(fixed.as_str()),
            sliced,
            sliced_fixed.as_deref(),
        ]
        .into_iter()
        .flatten(),
    );
    for cand in candidates {
        // Fast path: exact contract.
        if let Ok(v) = serde_json::from_str::<Vec<PageSymbolVerdict>>(cand) {
            return Ok(v);
        }
        // Lenient path: skip malformed/partial elements instead of failing
        // the whole batch when at least one usable verdict survives.
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(cand) {
            if let Some(kept) = salvage_verdict_elems(&v) {
                return Ok(kept);
            }
        }
    }
    Err(crate::error::Error::Config(format!(
        "SymbolAnnotationResponse 缺少 verdicts/results 数组或 JSON 无法修复：{}",
        excerpt(trimmed)
    )))
}

/// Accepts `[...]` or `{..., "verdicts"|"results": [...]}` and keeps only the
/// elements that deserialize cleanly into [`PageSymbolVerdict`].
///
/// Returns `None` when the shape does not apply or nothing usable survives.
fn salvage_verdict_elems(v: &serde_json::Value) -> Option<Vec<PageSymbolVerdict>> {
    let arr = match v {
        serde_json::Value::Array(arr) => arr.as_slice(),
        obj @ serde_json::Value::Object(_) => obj
            .get("verdicts")
            .or_else(|| obj.get("results"))?
            .as_array()?,
        _ => return None,
    };
    let kept: Vec<PageSymbolVerdict> = arr
        .iter()
        .filter_map(|e| serde_json::from_value::<PageSymbolVerdict>(e.clone()).ok())
        .collect();
    (!kept.is_empty()).then_some(kept)
}

/// Returns the substring spanning the first `[` to the last `]`, inclusive.
pub(crate) fn extract_outermost_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    (start < end).then_some(&s[start..=end])
}

/// Removes trailing commas before `}` / `]` (outside of this heuristic's
/// scope: commas that legitimately appear inside string literals immediately
/// followed by a closing bracket are vanishingly rare in practice).
pub(crate) fn remove_trailing_commas(s: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r",(\s*[}\]])").unwrap())
        .replace_all(s, "$1")
        .into_owned()
}

/// Short tail excerpt used in error messages so failures are diagnosable.
pub(crate) fn excerpt(s: &str) -> String {
    const MAX: usize = 200;
    // 先回退到 char 边界再切片(中文等多字节字符不能从中间截断)。
    let mut end = s.len().min(MAX);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = s[..end].to_string();
    if s.len() > MAX {
        cut.push('…');
    }
    cut.replace('\n', "\\n")
}

/// Strips a single leading/trailing Markdown code fence from `s`, if present.
pub(crate) fn strip_code_fence(s: &str) -> &str {
    const FENCE_STARTS: [&str; 4] = ["```json", "```JSON", "``` Json", "```"];
    let mut s = s;
    for start in FENCE_STARTS {
        if let Some(rest) = s.strip_prefix(start) {
            s = rest;
            break;
        }
    }
    if let Some(end) = s.rfind("```") {
        // Only treat it as a closing fence if nothing but whitespace follows.
        if s[end + 3..].trim().is_empty() {
            s = &s[..end];
        }
    }
    s.trim()
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

    fn test_annotation_fixture() -> SymbolPageAnnotation {
        SymbolPageAnnotation {
            symbol: SymbolRef::File {
                path: "components/health.lua".to_string(),
            },
            kind: SymbolKind::File,
            affected_variants: Vec::new(),
            affected_pageids: Vec::new(),
            visibility: SymbolPageVisibility::PageVisible,
            page_evidence: Vec::new(),
        }
    }

    #[test]
    fn paginate_affected_pages_bounds_and_ranks_by_evidence() {
        let mut ann = test_annotation_fixture();
        ann.affected_pageids = vec![1, 2, 3, 4, 5];
        // Evidence density: page 3 has 2 facts, page 1 has 1, others none.
        for pid in [1, 3, 3] {
            ann.page_evidence.push(PageEvidence {
                pageid: pid,
                region_id: "r".to_string(),
                raw: "raw".to_string(),
                snippet: "snippet".to_string(),
                matched_by: "candidate",
            });
        }

        // Both caps unset → legacy passthrough, original order untouched.
        let batches = paginate_affected_pages(&ann, 0, 0);
        assert_eq!(batches, vec![vec![1, 2, 3, 4, 5]]);
        // Any cap enabled → density ordering applies even when everything
        // fits in one batch.
        let batches = paginate_affected_pages(&ann, 10, 0);
        assert_eq!(batches, vec![vec![3, 1, 2, 4, 5]]);

        // Structural cap only: dense pages first (3, then 1), ties stable.
        let batches = paginate_affected_pages(&ann, 2, 0);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec![3, 1]);
        assert_eq!(batches[2], vec![5]);

        // Union covers every page exactly once.
        let mut seen: Vec<i64> = batches.iter().flatten().copied().collect();
        seen.sort_unstable();
        assert_eq!(seen, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn paginate_respects_char_budget_and_mega_page_singleton() {
        let mut ann = test_annotation_fixture();
        ann.affected_pageids = vec![1, 2, 3];
        for (pid, snippet_len) in [(1, 40usize), (2, 300), (3, 10)] {
            ann.page_evidence.push(PageEvidence {
                pageid: pid,
                region_id: "r".to_string(),
                raw: "x".to_string(),
                snippet: "s".repeat(snippet_len),
                matched_by: "candidate",
            });
        }
        let w = |pids: &[Vec<i64>]| -> Vec<usize> {
            let weights = PageWeights::build(&ann);
            pids.iter()
                .map(|b| b.iter().map(|p| weights.chars(*p)).sum())
                .collect()
        };

        // Char cap forces a split even though max_pages alone would not.
        let batches = paginate_affected_pages(&ann, 0, DEFAULT_BATCH_MAX_CHARS.min(120));
        assert!(batches.len() >= 2);
        // No batch may exceed the budget unless it is a singleton holding an
        // over-weight page.
        let ws = w(&batches);
        let singletons_over = batches
            .iter()
            .zip(&ws)
            .all(|(b, &sz)| b.len() == 1 || sz <= 120);
        assert!(singletons_over, "char cap violated: {ws:?}");

        // A lone page heavier than the whole budget still survives in its
        // own singleton batch.
        let mut one = test_annotation_fixture();
        one.affected_pageids = vec![7];
        one.page_evidence.push(PageEvidence {
            pageid: 7,
            region_id: "r".to_string(),
            raw: "x".to_string(),
            snippet: "s".repeat(500),
            matched_by: "candidate",
        });
        let batches = paginate_affected_pages(&one, 0, 50);
        assert_eq!(batches, vec![vec![7]]);
    }

    #[test]
    fn partition_no_fact_pages_filters_and_synthesizes() {
        let mut ann = test_annotation_fixture();
        ann.affected_pageids = vec![10, 11, 12];
        ann.page_evidence.push(PageEvidence {
            pageid: 11,
            region_id: "r".to_string(),
            raw: "x".to_string(),
            snippet: "s".to_string(),
            matched_by: "candidate",
        });

        // Flag off → passthrough, nothing synthesized.
        let (sent, synth) = partition_no_fact_pages(&ann, false);
        assert_eq!(sent, vec![10, 11, 12]);
        assert!(synth.is_empty());

        // Flag on → only fact-bearing pages go out; the rest get local
        // low-confidence "missing" verdicts.
        let (sent, synth) = partition_no_fact_pages(&ann, true);
        assert_eq!(sent, vec![11]);
        assert_eq!(synth.len(), 2);
        assert!(synth.iter().all(|v| v.missing && !v.mentions));
        assert_eq!(synth[0].confidence.as_deref(), Some("low"));
    }

    #[test]
    fn fill_missing_requested_covers_batch_gaps() {
        let requested = [1, 2, 3];
        let mut verdicts = vec![
            PageSymbolVerdict {
                pageid: 1,
                mentions: true,
                wording: None,
                semantic_consistent: None,
                missing: false,
                confidence: Some("high".to_string()),
                note: None,
            },
            PageSymbolVerdict {
                pageid: 2,
                mentions: false,
                wording: None,
                semantic_consistent: None,
                missing: true,
                confidence: Some("low".to_string()),
                note: None,
            },
        ];
        let added = fill_missing_requested(&requested, &mut verdicts);
        assert_eq!(added, 1);
        let ids: Vec<i64> = verdicts.iter().map(|v| v.pageid).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        let third = verdicts.last().unwrap();
        assert!(third.missing && !third.mentions);
        assert_eq!(
            third.note.as_deref(),
            Some("该页未出现在批次输出中（本地补判）")
        );

        // Nothing missing → no-op.
        let mut complete = vec![PageSymbolVerdict {
            pageid: 1,
            mentions: true,
            wording: None,
            semantic_consistent: None,
            missing: false,
            confidence: None,
            note: None,
        }];
        assert_eq!(fill_missing_requested(&[1], &mut complete), 0);
    }

    #[test]
    fn prompt_for_pages_renders_only_requested_subset() {
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
        assert!(!ann.affected_pageids.is_empty());
        let first = ann.affected_pageids[0];
        let full = render_symbol_annotation_prompt(&ann, "semantics");
        let subset = render_symbol_annotation_prompt_for_pages(&ann, "semantics", &[first]);
        assert!(subset.contains(&format!("### page {first}")));
        assert!(!subset.contains("### page 999999"));
        assert_eq!(
            full.contains(&format!("### page {first}")),
            subset.contains(&format!("### page {first}"))
        );
        // Subset header reports both scoped and total counts.
        assert!(subset.contains("本批 1 页"));
        assert!(subset.contains(&format!("全部 {} 页", ann.affected_pageids.len())));
    }

    #[test]
    fn excerpt_handles_multibyte_boundary() {
        let s = "状态".repeat(200); // '态' 正好跨 200 字节边界
        let out = excerpt(&s);
        assert!(out.ends_with('…'));
        assert!(!out.is_empty());
        assert!(out.starts_with("状态"));
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
    fn parse_symbol_annotation_response_strips_code_fence() {
        let fenced = "```json\n[\n  {\"pageid\": 1, \"mentions\": true, \"missing\": false, \"confidence\": \"high\"}\n]\n```";
        let parsed = parse_symbol_annotation_response(fenced).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].mentions);

        // Bare fence without language tag.
        let bare = "```\n[{\"pageid\": 2, \"mentions\": false, \"missing\": true}]\n```";
        let parsed = parse_symbol_annotation_response(bare).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].missing);

        // Fence + surrounding prose-free whitespace only is tolerated.
        let wrapped = "```json\n{\"verdicts\": [{\"pageid\": 3, \"mentions\": true, \"missing\": false}]}\n```";
        let parsed = parse_symbol_annotation_response(wrapped).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].pageid, 3);

        // Plain JSON keeps working (no regression).
        let plain = "[{\"pageid\": 4, \"mentions\": true, \"missing\": false}]";
        let parsed = parse_symbol_annotation_response(plain).unwrap();
        assert_eq!(parsed[0].pageid, 4);
    }

    #[test]
    fn parse_symbol_annotation_response_repairs_common_defects() {
        // Prose around the payload.
        let prose = "好的，以下是标注结果：\n[{\"pageid\": 5, \"mentions\": true, \"missing\": false}]\n如需调整请告知。";
        let parsed = parse_symbol_annotation_response(prose).unwrap();
        assert_eq!(parsed[0].pageid, 5);

        // Trailing comma inside an object.
        let trailing =
            "[{\"pageid\": 6, \"mentions\": true, \"missing\": false, \"confidence\": \"high\",}]";
        let parsed = parse_symbol_annotation_response(trailing).unwrap();
        assert_eq!(parsed[0].pageid, 6);

        // Trailing comma + fence + prose combined.
        let combo = "```json\n[{\"verdicts_level\": 1},\n {\"pageid\": 7, \"mentions\": false, \"missing\": true},]\n```\n以上。";
        let parsed = parse_symbol_annotation_response(combo).unwrap();
        assert_eq!(parsed[0].pageid, 7);

        // Genuinely broken JSON now reports a diagnostic excerpt instead of a
        // bare serde message.
        let broken = "[{\"pageid\": 8,\nline391 has unescaped \" here}";
        let err = parse_symbol_annotation_response(broken)
            .unwrap_err()
            .to_string();
        assert!(err.contains("无法修复"), "{}", err);

        // Partial elements are salvaged element-wise; usable ones survive.
        let partial = "```json\n[{\"symbol\": \"components/health.lua\"},\n {\"pageid\": 9, \"mentions\": true, \"missing\": false},\n {\"pageid\": \"oops\"}]\n```";
        let parsed = parse_symbol_annotation_response(partial).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].pageid, 9);
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
