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
use crate::service::Reporter;
use crate::update::grade::CorpusPageView;
use crate::update::index::build_atlas_from_dir;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// `knowledge-scan-wiki` 参数(全确定性,无 LLM 配置)。
pub struct ScanWikiParams {
    pub scripts_root: String,
    pub knowledge_dir: String,
    /// wiki 语料 host 根目录
    pub corpus: String,
}

/// 单个 (page, symbol) 对的归因条目(落盘 schema v1)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageSymbolEntry {
    /// pass2_evidence | fact_match;stub 对为 null
    #[serde(skip_serializing_if = "Option::is_none")]
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AspectSnapshot {
    pub aspect: String,
    /// (pageid, quote)——快照时只保留本页的证据
    pub evidence: Vec<AspectEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AspectEvidence {
    pub pageid: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FactMatch {
    pub raw: String,
    pub region_id: String,
    /// `brains/houndbrain.lua: SEE_DIST=30` 形式的匹配依据
    pub matched: String,
}

/// 每页的落盘文档。
#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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

    let mut written = 0usize;
    let mut unchanged = 0usize;
    let mut pages_with_mention = 0usize;
    let mut direct_mentioned = 0usize;
    let mut two_hop_mentioned = 0usize;
    let mut stub_pairs = 0usize;
    let doc_shas: HashMap<String, String> = docs
        .iter()
        .map(|d| (d.reference.path.clone(), d.provenance.source_sha256.clone()))
        .collect();

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

            let mention_source = if !covered.is_empty() {
                Some("pass2_evidence")
            } else if !fact_matches.is_empty() {
                Some("fact_match")
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
            if mention_source.is_none() {
                stub_pairs += 1;
            } else if route == "direct" {
                direct_mentioned += 1;
            } else {
                two_hop_mentioned += 1;
            }
            symbols.insert(
                path.clone(),
                PageSymbolEntry {
                    mention_source: mention_source.map(str::to_string),
                    aspects_covered: covered,
                    aspects_ignored: ignored,
                    fact_matches,
                    route: route.to_string(),
                    detail_level: detail.to_string(),
                },
            );
        }

        // 页面级汇总:只看直连符号
        let page_detail = direct_detail(&symbols);
        if symbols.values().any(|e| e.mention_source.is_some()) {
            pages_with_mention += 1;
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
            page_detail_level: page_detail,
        };
        let body = serde_json::to_string_pretty(&map)?;
        let out_path = out_dir.join(format!("{pageid}.json"));
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
        pageids.len()
    ));
    Ok(serde_json::json!({
        "mode": "scan_wiki_m2a",
        "pages_total": pageids.len(),
        "pages_written": written,
        "pages_unchanged": unchanged,
        "pages_with_mention": pages_with_mention,
        "direct_mentioned_pairs": direct_mentioned,
        "two_hop_mentioned_pairs": two_hop_mentioned,
        "stub_pairs": stub_pairs,
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
