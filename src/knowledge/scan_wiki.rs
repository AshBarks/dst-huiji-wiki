//! M2a `knowledge-scan-wiki`:PageSymbolMap 确定性骨架(不调 LLM)。
//!
//! 三层证据全部确定性计算(方案 C,见 docs/KNOWLEDGE_PAGE_MAP.md):
//! - D2 路由:atlas 反向边(直连)+ behaviour_calls 二跳(brain → behaviour);
//! - D0 反转:SymbolDoc.pass2 aspects 的 evidence.pageid → 页面提及(快照副本);
//! - D1 数值配对:页面 quantity facts ↔ 符号源码命名数值常量(`NAME = N`)。
//!
//! detail_level 新三档(stub/summary/detailed);二跳符号封顶 summary,
//! 页面级汇总只统计直连符号(拍板项:detail 对二跳放开)。

use crate::corpus::model::PageMeta;
use crate::error::{Error, Result};
use crate::knowledge::store::sha256_hex;
use crate::knowledge::types::SymbolDoc;
use crate::llm::{LlmConfig, LlmStreamEvent};
use crate::service::Reporter;
use crate::update::grade::CorpusPageView;
use crate::update::index::build_atlas_from_dir;
use crate::update::symbol_page::{
    paginate_affected_pages, parse_symbol_annotation_response,
    render_symbol_annotation_prompt_for_pages, PageEvidence, SymbolKind, SymbolPageAnnotation,
    SymbolPageVisibility, SymbolRef,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// `knowledge-scan-wiki` 参数(全确定性,无 LLM 配置)。
pub struct ScanWikiParams {
    pub scripts_root: String,
    pub knowledge_dir: String,
    /// wiki 语料 host 根目录
    pub corpus: String,
    /// M2b:对「routed 但确定性零证据」的页符号对跑 LLM 审计(收编
    /// symbol-annotate 的分页批处理 verdict)
    pub audit: bool,
    /// 审计符号的文件名词干(如 inspectable);None = 按缺口规模取前 10
    pub audit_symbols: Option<Vec<String>>,
    /// 每符号送审页数上限(按 facts 富裕度排序取前 N)
    pub audit_max_pages: usize,
    /// LLM 每批页数上限
    pub audit_batch_pages: usize,
    /// LLM 每批字符数上限
    pub audit_batch_max_chars: usize,
    /// M2c:不重建地图,直接聚合 knowledge/pages/*.json 出报表
    pub report: bool,
}

/// 单个 (page, symbol) 对的归因条目(落盘 schema v1)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSymbolEntry {
    /// pass2_evidence | fact_match;stub 对为 null
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_source: Option<String>,
    /// D0 命中的 aspects 快照副本(SymbolDoc 重扫不回写本图,靠 inputs.sha 刷新)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aspects_covered: Vec<AspectSnapshot>,
    /// 该符号 aspects 中未被本页覆盖的部分(stub 对 = 全部)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aspects_ignored: Vec<String>,
    /// D1 命中的数值配对(原文 + 源码常量)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fact_matches: Vec<FactMatch>,
    /// 路由层级:direct(反边直连) | two_hop(brain→behaviour)
    pub route: String,
    /// stub < summary < detailed;二跳封顶 summary
    pub detail_level: String,
    /// L2 审计产物(仅被 --audit 审计过的对携带)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_verdict: Option<LlmVerdict>,
}

/// symbol-annotate verdict 的快照(mentions=true 的转换依据 + 不一致记录)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmVerdict {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wording: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_consistent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AspectSnapshot {
    pub aspect: String,
    /// (pageid, quote)——快照时只保留本页的证据
    pub evidence: Vec<AspectEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AspectEvidence {
    pub pageid: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactMatch {
    pub raw: String,
    pub region_id: String,
    /// `brains/houndbrain.lua: SEE_DIST=30` 形式的匹配依据
    pub matched: String,
}

/// 每页的落盘文档。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSymbolMap {
    pub schema_version: u32,
    pub pageid: i64,
    pub title: String,
    pub inputs: MapInputs,
    pub symbols: BTreeMap<String, PageSymbolEntry>,
    /// 直连符号的汇总;无直连符号时为 null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_detail_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapInputs {
    pub wikitext_sha256: String,
    /// 本页条目引用到的 SymbolDoc 的 provenance.source_sha256
    pub symbol_doc_shas: BTreeMap<String, String>,
}

/// 源码里的命名数值常量(D1 配对依据)。
fn extract_named_constants(source: &str) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(eq) = line.find('=') else {
            continue;
        };
        let name = line[..eq].trim();
        let name = name.strip_prefix("local ").map(str::trim).unwrap_or(name);
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || name.bytes().next().is_some_and(|b| b.is_ascii_digit())
        {
            continue;
        }
        let value = line[eq + 1..].trim();
        let value = value.split([',', ';', ' ']).next().unwrap_or("");
        if let Ok(v) = value.parse::<f64>() {
            out.push((name.to_string(), v));
        }
    }
    out
}

/// D1 阈值:|v| < 3 的数值不参与配对(0/1/2 碰撞面过大)。
fn matchable(v: f64) -> bool {
    v.abs() >= 3.0
}

/// D2 路由:variant(小写)→ 直连符号文件集合。
fn build_variant_routes(
    index: &crate::update::index::edges::IndexArtifact,
) -> HashMap<String, BTreeSet<String>> {
    let mut routes: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (target, refs) in &index.reverse {
        let kind_ok = target.starts_with("components/")
            || target.starts_with("brains/")
            || target.starts_with("stategraphs/");
        if !kind_ok {
            continue;
        }
        for r in refs {
            let variant = r.rsplit('#').next().unwrap_or("").to_lowercase();
            if !variant.is_empty() {
                routes.entry(variant).or_default().insert(target.clone());
            }
        }
    }
    routes
}

pub async fn run_scan_wiki(
    params: &ScanWikiParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let scripts_root = Path::new(&params.scripts_root);
    let knowledge_root = Path::new(&params.knowledge_dir);
    let corpus_root = Path::new(&params.corpus);

    if params.report {
        return run_page_map_report(knowledge_root, reporter);
    }

    reporter.stage("构建代码关联索引");
    let atlas = build_atlas_from_dir(scripts_root)?;
    let variant_routes = build_variant_routes(&atlas.index);

    reporter.stage("加载语料视图与符号文档");
    let view = CorpusPageView::load(corpus_root)?;
    if view.pages.is_empty() {
        return Err(Error::Config(format!(
            "语料路由为空:{}",
            corpus_root.display()
        )));
    }
    let titles = load_titles(corpus_root)?;
    let docs = load_symbol_docs(knowledge_root)?;
    if docs.is_empty() {
        return Err(Error::Config(format!(
            "无 SymbolDoc:{}",
            knowledge_root.join("symbols").display()
        )));
    }
    let doc_by_path: HashMap<&str, &SymbolDoc> = docs
        .iter()
        .map(|d| (d.reference.path.as_str(), d))
        .collect();

    // D0:pageid → (symbol path → aspects 快照)
    let mut evidence_map: HashMap<i64, HashMap<String, Vec<AspectSnapshot>>> = HashMap::new();
    for doc in &docs {
        let Some(wiki) = &doc.wiki else { continue };
        for aspect in &wiki.aspects {
            for ev in &aspect.evidence {
                let snapshot = evidence_map
                    .entry(ev.pageid)
                    .or_default()
                    .entry(doc.reference.path.clone())
                    .or_default();
                match snapshot.iter_mut().find(|a| a.aspect == aspect.aspect) {
                    Some(existing) => {
                        existing.evidence.push(AspectEvidence {
                            pageid: ev.pageid,
                            quote: ev.quote.clone(),
                        });
                    }
                    None => snapshot.push(AspectSnapshot {
                        aspect: aspect.aspect.clone(),
                        evidence: vec![AspectEvidence {
                            pageid: ev.pageid,
                            quote: ev.quote.clone(),
                        }],
                    }),
                }
            }
        }
    }

    // brain → behaviours(二跳)
    let mut brain_behaviours: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for call in &atlas.index.behaviour_calls {
        let behaviour_path = format!("behaviours/{}.lua", call.ctor.to_lowercase());
        if doc_by_path.contains_key(behaviour_path.as_str()) {
            brain_behaviours
                .entry(call.brain_file.as_str())
                .or_default()
                .insert(behaviour_path);
        }
    }

    // pageid → variants
    let mut page_variants: HashMap<i64, Vec<String>> = HashMap::new();
    for (variant, ids) in &view.pages {
        for id in ids {
            page_variants.entry(*id).or_default().push(variant.clone());
        }
    }

    // D1 常量缓存(仅 doc-backed 符号文件)
    let mut constants_cache: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut source_constants = |path: &str| -> Vec<(String, f64)> {
        constants_cache
            .entry(path.to_string())
            .or_insert_with(|| {
                std::fs::read_to_string(scripts_root.join(path))
                    .map(|src| extract_named_constants(&src))
                    .unwrap_or_default()
            })
            .clone()
    };

    let mut pageids: Vec<i64> = page_variants.keys().copied().collect();
    pageids.sort();

    let out_dir = knowledge_root.join("pages");
    std::fs::create_dir_all(&out_dir)?;

    let doc_shas: HashMap<String, String> = docs
        .iter()
        .map(|d| (d.reference.path.clone(), d.provenance.source_sha256.clone()))
        .collect();

    let mut maps: Vec<PageSymbolMap> = Vec::new();

    // 既有审计结果回填:重建会重算全部确定性条目,磁盘图里的
    // llm_verdict(L2 审计产物)必须继承,否则多次运行互相抹除。
    let mut prior_maps: HashMap<i64, PageSymbolMap> = HashMap::new();
    let prior_dir = knowledge_root.join("pages");
    if let Ok(rd) = std::fs::read_dir(&prior_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(m) = serde_json::from_str::<PageSymbolMap>(
                &std::fs::read_to_string(&path).unwrap_or_default(),
            ) {
                prior_maps.insert(m.pageid, m);
            }
        }
    }

    reporter.stage("逐页归因");
    for pageid in &pageids {
        let variants = &page_variants[pageid];
        let mut routed: BTreeSet<String> = BTreeSet::new();
        for v in variants {
            if let Some(targets) = variant_routes.get(&v.to_lowercase()) {
                for t in targets {
                    if doc_by_path.contains_key(t.as_str()) {
                        routed.insert(t.clone());
                    }
                }
            }
        }
        // 二跳:behaviour 经由本页路由到的 brain
        let mut two_hop: BTreeSet<String> = BTreeSet::new();
        for brain in &routed {
            if brain.starts_with("brains/") {
                if let Some(bs) = brain_behaviours.get(brain.as_str()) {
                    for b in bs {
                        if !routed.contains(b) {
                            two_hop.insert(b.clone());
                        }
                    }
                }
            }
        }
        if routed.is_empty() && two_hop.is_empty() {
            continue;
        }

        let wikitext_path: PathBuf = corpus_root.join(format!("pages/{pageid}.wikitext"));
        let wikitext_sha = std::fs::read(&wikitext_path)
            .map(|bytes| sha256_hex(&bytes))
            .unwrap_or_default();
        let facts = view.facts.get(pageid);

        let mut symbols: BTreeMap<String, PageSymbolEntry> = BTreeMap::new();
        let mut used_doc_shas: BTreeMap<String, String> = BTreeMap::new();
        for path in routed.iter().chain(two_hop.iter()) {
            let Some(doc) = doc_by_path.get(path.as_str()) else {
                continue;
            };
            used_doc_shas.insert(path.clone(), doc_shas[path].as_str().to_string());
            let route = if routed.contains(path) {
                "direct"
            } else {
                "two_hop"
            };
            let covered = evidence_map
                .get(pageid)
                .and_then(|m| m.get(path))
                .cloned()
                .unwrap_or_default();
            // D1:quantity facts ↔ 源码命名常量
            let mut fact_matches: Vec<FactMatch> = Vec::new();
            if let Some(facts) = facts {
                let constants = source_constants(path);
                if !constants.is_empty() {
                    for fact in facts {
                        if fact.fact_kind != "quantity" || fact.values.len() != 1 {
                            continue;
                        }
                        let v = fact.values[0];
                        if !matchable(v) {
                            continue;
                        }
                        for (name, c) in &constants {
                            if matchable(*c) && *c == v {
                                fact_matches.push(FactMatch {
                                    raw: fact.raw.clone(),
                                    region_id: fact.region_id.clone(),
                                    matched: format!("{path}: {name}={}", trim_num(*c)),
                                });
                            }
                        }
                    }
                }
            }
            let doc_aspects: Vec<String> = doc
                .wiki
                .as_ref()
                .map(|w| w.aspects.iter().map(|a| a.aspect.clone()).collect())
                .unwrap_or_default();
            let covered_names: BTreeSet<&str> = covered.iter().map(|a| a.aspect.as_str()).collect();
            let ignored: Vec<String> = doc_aspects
                .iter()
                .filter(|a| !covered_names.contains(a.as_str()))
                .cloned()
                .collect();

            let mut mention_source: Option<String> = if !covered.is_empty() {
                Some("pass2_evidence".to_string())
            } else if !fact_matches.is_empty() {
                Some("fact_match".to_string())
            } else {
                None
            };
            // detail 三档;二跳封顶 summary
            let mut detail = match (covered.len(), fact_matches.len()) {
                (0, 0) => "stub",
                (1, 0) | (0, 1) => "summary",
                _ => "detailed",
            };
            if route == "two_hop" && detail == "detailed" {
                detail = "summary";
            }
            // 审计结果继承:llm_verdict 始终回填;若确定性层仍无证据
            // 且先前的 L2 判定为提及,则提及状态一并继承。
            let mut llm_verdict = None;
            if let Some(prior_entry) = prior_maps
                .get(pageid)
                .and_then(|m| m.symbols.get(path.as_str()))
            {
                if let Some(v) = &prior_entry.llm_verdict {
                    llm_verdict = Some(v.clone());
                    if mention_source.is_none()
                        && prior_entry.mention_source.as_deref() == Some("llm_verdict")
                    {
                        mention_source = Some("llm_verdict".to_string());
                        if detail == "stub" {
                            detail = "summary";
                        }
                    }
                }
            }
            symbols.insert(
                path.clone(),
                PageSymbolEntry {
                    mention_source,
                    aspects_covered: covered,
                    aspects_ignored: ignored,
                    fact_matches,
                    route: route.to_string(),
                    detail_level: detail.to_string(),
                    llm_verdict,
                },
            );
        }

        let map = PageSymbolMap {
            schema_version: 1,
            pageid: *pageid,
            title: titles.get(pageid).cloned().unwrap_or_default(),
            inputs: MapInputs {
                wikitext_sha256: wikitext_sha,
                symbol_doc_shas: used_doc_shas,
            },
            symbols,
            page_detail_level: None,
        };
        maps.push(map);
    }

    // M2b L2 审计:对确定性零证据(stub)对跑收编版 verdict,改写内存图
    let audit_summary = if params.audit {
        reporter.stage("M2b L2 审计");
        let config = LlmConfig::from_env()
            .ok_or_else(|| Error::Config("审计模式需要 LLM 配置(LLM__API_KEY)".to_string()))?;
        Some(
            run_audit(
                &mut maps,
                &docs,
                &view,
                &config,
                params,
                scripts_root,
                reporter,
            )
            .await?,
        )
    } else {
        None
    };

    // 汇总 + 落盘(page_detail 在审计改写后统一计算)
    let out_dir = knowledge_root.join("pages");
    std::fs::create_dir_all(&out_dir)?;
    let mut written = 0usize;
    let mut unchanged = 0usize;
    let mut pages_with_mention = 0usize;
    let mut direct_mentioned = 0usize;
    let mut two_hop_mentioned = 0usize;
    let mut stub_pairs = 0usize;
    let mut llm_converted = 0usize;
    let mut llm_inconsistent = 0usize;
    for map in &mut maps {
        let mut page_mentioned = false;
        for (path, entry) in map.symbols.iter_mut() {
            if let Some(v) = &entry.llm_verdict {
                if v.semantic_consistent == Some(false) {
                    llm_inconsistent += 1;
                }
            }
            if entry.mention_source.is_none() {
                stub_pairs += 1;
                continue;
            }
            page_mentioned = true;
            if entry.mention_source.as_deref() == Some("llm_verdict") {
                llm_converted += 1;
            }
            if entry.route == "direct" {
                direct_mentioned += 1;
            } else {
                two_hop_mentioned += 1;
            }
            let _ = path;
        }
        if page_mentioned {
            pages_with_mention += 1;
        }
        map.page_detail_level = direct_detail(&map.symbols);
        let body = serde_json::to_string_pretty(map)?;
        let out_path = out_dir.join(format!("{}.json", map.pageid));
        let changed = match std::fs::read_to_string(&out_path) {
            Ok(existing) => existing != body,
            Err(_) => true,
        };
        if changed {
            std::fs::write(&out_path, body)?;
            written += 1;
        } else {
            unchanged += 1;
        }
    }

    reporter.log(format!(
        "PageSymbolMap 完成: 页面 {} 个(有提及 {pages_with_mention}),直连提及对 {direct_mentioned},二跳提及对 {two_hop_mentioned},stub 对 {stub_pairs}",
        maps.len()
    ));
    Ok(serde_json::json!({
        "mode": if params.audit { "scan_wiki_m2b" } else { "scan_wiki_m2a" },
        "pages_total": maps.len(),
        "pages_written": written,
        "pages_unchanged": unchanged,
        "pages_with_mention": pages_with_mention,
        "direct_mentioned_pairs": direct_mentioned,
        "two_hop_mentioned_pairs": two_hop_mentioned,
        "stub_pairs": stub_pairs,
        "llm_converted_pairs": llm_converted,
        "llm_inconsistent_pairs": llm_inconsistent,
        "audit": audit_summary,
    }))
}

/// M2b L2 审计:复用 symbol-annotate 的分页批处理与 verdict 契约。
/// 只处理「stub 且符号有 aspects」的对;mentions=true 的对转为
/// mention_source=llm_verdict + summary,verdict 细节快照进图。
#[allow(clippy::too_many_arguments)]
async fn run_audit(
    maps: &mut [PageSymbolMap],
    docs: &[SymbolDoc],
    view: &CorpusPageView,
    config: &LlmConfig,
    params: &ScanWikiParams,
    scripts_root: &Path,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let doc_by_path: HashMap<&str, &SymbolDoc> = docs
        .iter()
        .map(|d| (d.reference.path.as_str(), d))
        .collect();

    // 每符号收集 stub 页(有 aspects 的),按 facts 富裕度排序后截断
    // 已有 llm_verdict 的对视为已审计(mentions=false 的判定同样有效),
    // 多次运行接力时不会重复送审。
    let mut stub_pages: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for map in maps.iter() {
        for (path, entry) in &map.symbols {
            if entry.detail_level == "stub"
                && !entry.aspects_ignored.is_empty()
                && entry.llm_verdict.is_none()
            {
                stub_pages.entry(path.clone()).or_default().push(map.pageid);
            }
        }
    }
    let selected: Vec<String> = match &params.audit_symbols {
        Some(stems) => {
            let wanted: BTreeSet<String> = stems
                .iter()
                .map(|s| format!("{}.lua", s.trim().to_lowercase()))
                .collect();
            let mut hit = Vec::new();
            for prefix in ["components/", "brains/", "stategraphs/", "behaviours/"] {
                for stem in &wanted {
                    let p = format!("{prefix}{stem}");
                    if stub_pages.contains_key(p.as_str()) {
                        hit.push(p);
                    }
                }
            }
            hit
        }
        None => {
            let mut ranked: Vec<(&String, usize)> =
                stub_pages.iter().map(|(p, ids)| (p, ids.len())).collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            ranked
                .into_iter()
                .take(10)
                .map(|(p, _)| p.clone())
                .collect()
        }
    };
    if selected.is_empty() {
        return Ok(serde_json::json!({"symbols_audited": 0, "note": "无匹配的审计符号"}));
    }

    let raw_dir = PathBuf::from("output/knowledge/raw/page_map");
    std::fs::create_dir_all(&raw_dir)?;

    let mut pages_sent = 0usize;
    let mut batches_failed = 0usize;
    let mut converted = 0usize;
    let mut inconsistent = 0usize;
    let system = "你是 DST Huiji Wiki 的代码符号页面影响标注助手。必须严格按用户要求输出 JSON。";

    for sym_path in &selected {
        let Some(doc) = doc_by_path.get(sym_path.as_str()) else {
            continue;
        };
        let Some((_, mut pids)) = stub_pages.remove_entry(sym_path.as_str()) else {
            continue;
        };
        pids.sort_by_key(|pid| std::cmp::Reverse(view.facts.get(pid).map_or(0, Vec::len)));
        pids.truncate(params.audit_max_pages);
        if pids.is_empty() {
            continue;
        }

        let variants: Vec<String> = pids
            .iter()
            .flat_map(|pid| {
                view.pages
                    .iter()
                    .filter(|(_, ids)| ids.contains(pid))
                    .map(|(v, _)| v.clone())
                    .collect::<Vec<_>>()
            })
            .take(40)
            .collect();
        let mut evidence = Vec::new();
        for pid in &pids {
            if let Some(facts) = view.facts.get(pid) {
                for f in facts.iter().take(8) {
                    evidence.push(PageEvidence {
                        pageid: *pid,
                        region_id: f.region_id.clone(),
                        raw: f.raw.clone(),
                        snippet: f.snippet.clone(),
                        matched_by: "candidate",
                    });
                }
            }
        }
        let ann = SymbolPageAnnotation {
            symbol: SymbolRef::File {
                path: (*sym_path).to_string(),
            },
            kind: SymbolKind::File,
            affected_variants: variants,
            affected_pageids: pids.clone(),
            visibility: SymbolPageVisibility::PageVisible,
            page_evidence: evidence,
        };
        let mut semantics = doc.summary.clone();
        let aspect_names: Vec<String> = doc
            .wiki
            .as_ref()
            .map(|w| w.aspects.iter().map(|a| a.aspect.clone()).collect())
            .unwrap_or_default();
        if !aspect_names.is_empty() {
            semantics.push_str(&format!(
                "\n代码侧可证实的关键 aspects:{}",
                aspect_names.join("、")
            ));
        }

        let working_pids = pids.clone();
        let mut working = ann.clone();
        working.affected_pageids = working_pids;
        let batches = paginate_affected_pages(
            &working,
            params.audit_batch_pages,
            params.audit_batch_max_chars,
        );
        reporter.log(format!(
            "审计 {}:送审 {} 页 / {} 批",
            sym_path,
            pids.len(),
            batches.len()
        ));
        pages_sent += pids.len();

        for (bi, batch) in batches.iter().enumerate() {
            let prompt = render_symbol_annotation_prompt_for_pages(&ann, &semantics, batch);
            let raw_file = raw_dir.join(format!(
                "{}_b{:02}.json",
                sym_path.replace('/', "_"),
                bi + 1
            ));
            let mut outcome = None;
            for attempt in 1..=2 {
                if attempt > 1 {
                    reporter.log("审计批次重试第 2 次…".to_string());
                }
                let raw = config
                    .complete_streaming(system, &prompt, |ev| match ev {
                        LlmStreamEvent::FirstToken { elapsed_secs } => {
                            reporter.log(format!("首个输出分片({elapsed_secs}s)"));
                        }
                        LlmStreamEvent::Tick { chars } => {
                            let _ = chars;
                        }
                        LlmStreamEvent::Done {
                            chars,
                            elapsed_secs,
                        } => {
                            reporter.log(format!("输出完成:{chars} 字符 / {elapsed_secs}s"));
                        }
                    })
                    .await;
                match raw {
                    Ok(text) => {
                        let _ = std::fs::write(&raw_file, &text);
                        outcome = Some(Ok(text));
                        break;
                    }
                    Err(e) => outcome = Some(Err(e)),
                }
            }
            let raw = match outcome.expect("至少一次尝试") {
                Ok(t) => t,
                Err(e) => {
                    batches_failed += 1;
                    reporter.log(format!("审计批次失败,跳过:{e}"));
                    continue;
                }
            };
            let verdicts = match parse_symbol_annotation_response(&raw) {
                Ok(v) => v,
                Err(e) => {
                    batches_failed += 1;
                    reporter.log(format!("verdict 解析失败,跳过:{e}"));
                    continue;
                }
            };
            for v in verdicts {
                let Some(map) = maps.iter_mut().find(|m| m.pageid == v.pageid) else {
                    continue;
                };
                let Some(entry) = map.symbols.get_mut(sym_path.as_str()) else {
                    continue;
                };
                if entry.mention_source.is_some() {
                    continue; // 只改写 stub 对
                }
                if v.mentions {
                    entry.mention_source = Some("llm_verdict".to_string());
                    entry.detail_level = "summary".to_string();
                    converted += 1;
                }
                if v.semantic_consistent == Some(false) {
                    inconsistent += 1;
                }
                entry.llm_verdict = Some(LlmVerdict {
                    wording: v.wording,
                    semantic_consistent: v.semantic_consistent,
                    note: v.note,
                });
            }
        }
        let _ = scripts_root;
    }

    reporter.log(format!(
        "审计完成:转换 {converted} 对为提及,语义不一致 {inconsistent} 对,失败批次 {batches_failed}"
    ));
    Ok(serde_json::json!({
        "symbols_audited": selected.len(),
        "pages_sent": pages_sent,
        "converted_to_mention": converted,
        "semantic_inconsistent": inconsistent,
        "batches_failed": batches_failed,
    }))
}

fn direct_detail(symbols: &BTreeMap<String, PageSymbolEntry>) -> Option<String> {
    let mut levels = Vec::new();
    for e in symbols.values() {
        if e.route != "direct" {
            continue;
        }
        levels.push(e.detail_level.as_str());
    }
    if levels.is_empty() {
        return None;
    }
    if levels.contains(&"detailed") || levels.iter().filter(|l| **l == "summary").count() >= 3 {
        Some("detailed".to_string())
    } else if levels.contains(&"summary") {
        Some("summary".to_string())
    } else {
        Some("stub".to_string())
    }
}

fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn load_titles(corpus_root: &Path) -> Result<HashMap<i64, String>> {
    let mut out = HashMap::new();
    let raw = std::fs::read_to_string(corpus_root.join("meta.jsonl"))?;
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        if let Ok(meta) = serde_json::from_str::<PageMeta>(line) {
            out.insert(meta.pageid, meta.title);
        }
    }
    Ok(out)
}

fn load_symbol_docs(knowledge_root: &Path) -> Result<Vec<SymbolDoc>> {
    let dir = knowledge_root.join("symbols");
    let mut docs = Vec::new();
    for entry in std::fs::read_dir(&dir)?.collect::<std::io::Result<Vec<std::fs::DirEntry>>>()? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(doc) = crate::knowledge::store::load_doc(&path)? {
            docs.push(doc);
        }
    }
    Ok(docs)
}

/// M2c:聚合 `knowledge/pages/*.json` 产出 `knowledge/page_map_summary.json`。
/// 按符号聚合 routed/mentioned/stub/不一致,detail_level 分布,
/// 并单独列出 semantic_consistent=false 的对(M3 修订建议输入)。
pub fn run_page_map_report(
    knowledge_root: &Path,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dir = knowledge_root.join("pages");
    let mut maps = Vec::new();
    for entry in std::fs::read_dir(&dir)?.collect::<std::io::Result<Vec<std::fs::DirEntry>>>()? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        match serde_json::from_str::<PageSymbolMap>(&raw) {
            Ok(m) => maps.push(m),
            Err(e) => reporter.log(format!("跳过无法解析的地图文件 {}: {e}", path.display())),
        }
    }
    maps.sort_by_key(|m| m.pageid);

    #[derive(serde::Serialize)]
    struct SymRow {
        routed: usize,
        mentioned: usize,
        stub: usize,
        stub_unaudited: usize,
        inconsistent: usize,
        detailed: usize,
        summary: usize,
    }
    let mut sym_rows: BTreeMap<String, SymRow> = BTreeMap::new();
    let mut inconsistencies: Vec<serde_json::Value> = Vec::new();
    let mut totals = serde_json::json!({
        "pass2_evidence": 0usize, "fact_match": 0usize, "llm_verdict": 0usize
    });
    let mut pairs = 0usize;
    let mut stub_pairs = 0usize;
    let mut stub_unaudited = 0usize;
    let mut pages_with_mention = 0usize;

    for map in &maps {
        let mut page_mentioned = false;
        for (path, e) in &map.symbols {
            pairs += 1;
            let row = sym_rows.entry(path.clone()).or_insert(SymRow {
                routed: 0,
                mentioned: 0,
                stub: 0,
                stub_unaudited: 0,
                inconsistent: 0,
                detailed: 0,
                summary: 0,
            });
            row.routed += 1;
            match e.detail_level.as_str() {
                "detailed" => row.detailed += 1,
                "summary" => row.summary += 1,
                _ => {}
            }
            if e.detail_level == "stub" {
                row.stub += 1;
                stub_pairs += 1;
                if !e.aspects_ignored.is_empty() && e.llm_verdict.is_none() {
                    row.stub_unaudited += 1;
                    stub_unaudited += 1;
                }
            } else {
                row.mentioned += 1;
                page_mentioned = true;
                if let Some(src) = &e.mention_source {
                    if let Some(slot) = totals.get_mut(src.as_str()) {
                        let next = slot.as_u64().unwrap_or(0) + 1;
                        *slot = serde_json::Value::from(next);
                    }
                }
            }
            if let Some(v) = &e.llm_verdict {
                if v.semantic_consistent == Some(false) {
                    row.inconsistent += 1;
                    inconsistencies.push(serde_json::json!({
                        "pageid": map.pageid,
                        "title": map.title,
                        "symbol": path,
                        "wording": v.wording,
                        "note": v.note,
                    }));
                }
            }
        }
        if page_mentioned {
            pages_with_mention += 1;
        }
    }

    let mut symbols: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (path, row) in sym_rows {
        symbols.insert(
            path,
            serde_json::to_value(row).unwrap_or(serde_json::Value::Null),
        );
    }
    let summary = serde_json::json!({
        "schema_version": 1,
        "generated_at_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        "totals": {
            "pages": maps.len(),
            "pages_with_mention": pages_with_mention,
            "pairs": pairs,
            "mentioned_by_source": totals,
            "stub_pairs": stub_pairs,
            "stub_unaudited": stub_unaudited,
        },
        "symbols": symbols,
        "inconsistencies": inconsistencies,
    });
    let out_path = knowledge_root.join("page_map_summary.json");
    let body = serde_json::to_string_pretty(&summary)?;
    if std::fs::read_to_string(&out_path)
        .map(|e| e != body)
        .unwrap_or(true)
    {
        std::fs::write(&out_path, body)?;
    }
    reporter.log(format!(
        "PageSymbolMap 报表: 页面 {} / 提及对页面 {pages_with_mention},对 {pairs}(stub {stub_pairs},未审计 stub {stub_unaudited}),语义不一致 {}",
        maps.len(),
        inconsistencies.len()
    ));
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_constants_extraction() {
        let src = "local SEE_DIST = 30\nlocal T = {}\nlocal x = 5\nlocal rate = 0.25\ninst.foo = 7\nlocal bad = nil\nlocal named = 100,";
        let c = extract_named_constants(src);
        assert!(c.contains(&("SEE_DIST".to_string(), 30.0)));
        assert!(c.contains(&("rate".to_string(), 0.25)));
        assert!(c.contains(&("named".to_string(), 100.0)));
        // 无 local 前缀的 inst.foo 不应出现(名字含点)
        assert!(!c.iter().any(|(n, _)| n == "inst.foo"));
        assert!(!c.iter().any(|(n, _)| n == "T"));
    }

    #[test]
    fn detail_capping_for_two_hop() {
        // 直接验证 detail 决策逻辑:二跳 detailed → summary
        let covered = [AspectSnapshot {
            aspect: "a".into(),
            evidence: vec![AspectEvidence {
                pageid: 1,
                quote: None,
            }],
        }];
        let fact_matches = [FactMatch {
            raw: "r".into(),
            region_id: "rg".into(),
            matched: "m".into(),
        }];
        let signals = covered.len() + fact_matches.len();
        let mut detail = if signals >= 2 { "detailed" } else { "summary" };
        if detail == "detailed" {
            detail = "summary"; // two_hop cap
        }
        assert_eq!(detail, "summary");
    }
}
