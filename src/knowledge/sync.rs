//! M3 `knowledge sync`:代码变更 → 脏 SymbolDoc 清单 → PageSymbolMap 交叉
//! → 受影响页面与旧值锚点定位。
//!
//! v1 最小闭环(方案拍板,见 docs/KNOWLEDGE_SYNC.md):独立命令、确定性、
//! 零 LLM;`--rescan` 时级联调用 knowledge-scan-symbols 重扫脏文档。
//! v1.1 接入 tuning/prefab 交叉与 Tier2 `--draft` 起草;v1.2 起草 prompt
//! 注入常量使用上下文与同值竞争常量提示,并以 `--review` 完成人工复核闭环。

use crate::error::{Error, Result};
use crate::knowledge::scan_wiki::{extract_named_constants, trim_num, PageSymbolMap};
use crate::knowledge::store::sha256_hex;
use crate::knowledge::types::SymbolDoc;
use crate::service::Reporter;
use crate::update::build_atlas_from_dir;
use crate::update::diffdata::{DiffStatus, TreeDiff};
use crate::update::snapshot::SnapshotStore;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub struct SyncParams {
    /// 旧快照(时间戳或目录名,SnapshotStore 口径)
    pub old: String,
    /// 新快照(时间戳、目录名,或 "current")
    pub new: String,
    pub knowledge_dir: String,
    /// 级联重扫脏文档(调 LLM)
    pub rescan: bool,
    /// 详列的脏文档数上限(按受影响页面数排序取前 N)
    pub limit: usize,
    /// wiki 语料根目录;提供则启用 prefab→页面交叉与标题解析
    pub corpus: Option<String>,
    /// Tier2:按常量差异与旧值锚点起草页面修订建议(需 corpus + LLM)
    pub draft: bool,
    /// 复核裁决文件路径:对既有 sync_report.json 的建议逐条 approve/reject,
    /// 产出仅含 approve 项的应用清单(不重跑 diff 与起草)
    pub review: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ConstantChange {
    pub name: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PageAnchor {
    pub pageid: i64,
    pub title: String,
    /// precise = 页面数值与旧常量精确匹配(旧值锚点);context = 仅提及
    pub kind: String,
    /// precise 锚点的页面原文(raw)与匹配常量
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorDetail>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AnchorDetail {
    pub raw: String,
    pub matched: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DirtyDoc {
    pub path: String,
    pub category: String,
    pub constant_changes: Vec<ConstantChange>,
    pub mentioned_pages: Vec<PageAnchor>,
    pub stub_page_count: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct TuningChange {
    pub key: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PrefabChange {
    pub path: String,
    pub variants: Vec<String>,
    pub pages: Vec<PageBrief>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PageBrief {
    pub pageid: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncReport {
    pub old: String,
    pub new: String,
    pub changed_files: usize,
    pub symbol_changes: usize,
    pub other_changes: usize,
    pub dirty_docs: Vec<DirtyDoc>,
    pub added_symbols: Vec<String>,
    pub removed_symbols: Vec<String>,
    pub other_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tuning_changes: Vec<TuningChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefab_changes: Vec<PrefabChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drafts: Vec<DraftSuggestion>,
    pub rescan: Option<serde_json::Value>,
}

/// Tier2 起草产物:一条页面修订建议(仅建议,人工审阅)。
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DraftSuggestion {
    /// 稳定建议 id(单次 --draft 运行内唯一,复核文件按此裁决)
    #[serde(default)]
    pub id: String,
    pub path: String,
    pub pageid: i64,
    pub title: String,
    pub old_sentence: String,
    pub new_sentence: String,
    pub reason: String,
}

fn category_of(path: &str) -> Option<&'static str> {
    if path.starts_with("components/") {
        Some("component")
    } else if path.starts_with("brains/") {
        Some("brain")
    } else if path.starts_with("stategraphs/") {
        Some("stategraph")
    } else if path.starts_with("behaviours/") {
        Some("behaviour")
    } else {
        None
    }
}

/// 命名常量差异:同名常量值变化(added/removed 不计入,v1 只关心值变更)。
fn diff_constants(old_src: &str, new_src: &str) -> Vec<ConstantChange> {
    let old_map: BTreeMap<String, String> = extract_named_constants(old_src)
        .into_iter()
        .map(|(n, v)| (n, trim_num(v)))
        .collect();
    let new_map: BTreeMap<String, String> = extract_named_constants(new_src)
        .into_iter()
        .map(|(n, v)| (n, trim_num(v)))
        .collect();
    old_map
        .iter()
        .filter_map(|(name, old_v)| {
            let new_v = new_map.get(name)?;
            (old_v != new_v).then(|| ConstantChange {
                name: name.clone(),
                old: old_v.clone(),
                new: new_v.clone(),
            })
        })
        .collect()
}

/// 脏文档判定:当前文件 sha 与文档生成时 sha 不一致。
fn is_dirty(doc: &SymbolDoc, current_root: &Path) -> bool {
    let current = std::fs::read(current_root.join(&doc.reference.path))
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_default();
    current != doc.provenance.source_sha256
}

pub async fn run_knowledge_sync(
    params: &SyncParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    // 复核模式:不重跑 diff/起草,回读既有报告 + 人工裁决 → 应用清单
    if let Some(review_path) = &params.review {
        let report_path = Path::new("output/knowledge/sync_report.json");
        return run_review(Path::new(review_path), report_path, reporter);
    }
    let store = SnapshotStore::from_env()?;
    let resolve = |id: &str| -> std::path::PathBuf {
        if id == "current" {
            store.current_dir()
        } else {
            let as_dir = Path::new(id);
            if as_dir.is_dir() {
                as_dir.to_path_buf()
            } else {
                store.resolve(id)
            }
        }
    };
    let old_root = resolve(&params.old);
    let new_root = resolve(&params.new);
    if !old_root.is_dir() || !new_root.is_dir() {
        return Err(Error::Config(format!(
            "快照目录缺失: {} 或 {}",
            old_root.display(),
            new_root.display()
        )));
    }
    let knowledge_root = Path::new(&params.knowledge_dir);

    reporter.stage("树差异(Layer A 复用)");
    let diff = TreeDiff::diff_trees(&old_root, &new_root)?;
    let symbol_changes: Vec<_> = diff
        .files
        .iter()
        .filter(|f| category_of(&f.path).is_some())
        .collect();
    let other: Vec<String> = diff
        .files
        .iter()
        .filter(|f| category_of(&f.path).is_none())
        .map(|f| {
            let status = match f.status {
                DiffStatus::Added => "+",
                DiffStatus::Removed => "-",
                DiffStatus::Modified => "~",
            };
            format!("{status} {}", f.path)
        })
        .collect();
    reporter.log(format!(
        "变更文件 {} 个(符号 {} / 其他 {})",
        diff.files.len(),
        symbol_changes.len(),
        other.len()
    ));

    reporter.stage("加载符号文档与页面图");
    let mut docs: BTreeMap<String, SymbolDoc> = BTreeMap::new();
    for entry in std::fs::read_dir(knowledge_root.join("symbols"))?
        .collect::<std::io::Result<Vec<std::fs::DirEntry>>>()?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(doc) = crate::knowledge::store::load_doc(&path)? {
            docs.insert(doc.reference.path.clone(), doc);
        }
    }
    let mut page_maps: BTreeMap<i64, PageSymbolMap> = BTreeMap::new();
    let pages_dir = knowledge_root.join("pages");
    for entry in
        std::fs::read_dir(&pages_dir)?.collect::<std::io::Result<Vec<std::fs::DirEntry>>>()?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<PageSymbolMap>(
            &std::fs::read_to_string(&path).unwrap_or_default(),
        ) {
            page_maps.insert(m.pageid, m);
        }
    }
    // symbol path → (mentioned pages, stub count),供交叉
    let mut sym_mentioned: BTreeMap<String, Vec<&PageSymbolMap>> = BTreeMap::new();
    let mut sym_stub_count: BTreeMap<String, usize> = BTreeMap::new();
    for map in page_maps.values() {
        for (path, entry) in &map.symbols {
            if entry.mention_source.is_some() {
                sym_mentioned.entry(path.clone()).or_default().push(map);
            } else if entry.detail_level == "stub" {
                *sym_stub_count.entry(path.clone()).or_default() += 1;
            }
        }
    }

    reporter.stage("脏文档与锚点交叉");
    let mut dirty_docs: Vec<DirtyDoc> = Vec::new();
    let mut added_symbols = Vec::new();
    let mut removed_symbols = Vec::new();
    for f in &symbol_changes {
        let Some(category) = category_of(&f.path) else {
            continue;
        };
        match f.status {
            DiffStatus::Added => added_symbols.push(f.path.clone()),
            DiffStatus::Removed => removed_symbols.push(f.path.clone()),
            DiffStatus::Modified => {
                let Some(doc) = docs.get(&f.path) else {
                    continue; // 尚无文档的符号,属 M1 扩量范畴
                };
                if !is_dirty(doc, &new_root) {
                    continue; // 文档生成于变更后的树,已同步
                }
                let old_src = std::fs::read_to_string(old_root.join(&f.path)).unwrap_or_default();
                let new_src = std::fs::read_to_string(new_root.join(&f.path)).unwrap_or_default();
                let constant_changes = diff_constants(&old_src, &new_src);
                let mentioned = sym_mentioned.get(&f.path).cloned().unwrap_or_default();
                let mut page_anchors: Vec<PageAnchor> = mentioned
                    .iter()
                    .map(|m| {
                        let entry = m.symbols.get(&f.path);
                        let anchors: Vec<AnchorDetail> = entry
                            .map(|e| {
                                e.fact_matches
                                    .iter()
                                    .filter(|fm| {
                                        constant_changes.iter().any(|c| {
                                            fm.matched.contains(&format!("{}={}", c.name, c.old))
                                        })
                                    })
                                    .map(|fm| AnchorDetail {
                                        raw: fm.raw.clone(),
                                        matched: fm.matched.clone(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        PageAnchor {
                            pageid: m.pageid,
                            title: m.title.clone(),
                            kind: if anchors.is_empty() {
                                "context".to_string()
                            } else {
                                "precise".to_string()
                            },
                            anchors,
                        }
                    })
                    .collect();
                page_anchors.sort_by_key(|p| std::cmp::Reverse(p.anchors.len()));
                dirty_docs.push(DirtyDoc {
                    path: f.path.clone(),
                    category: category.to_string(),
                    constant_changes,
                    mentioned_pages: page_anchors,
                    stub_page_count: *sym_stub_count.get(&f.path).unwrap_or(&0),
                });
            }
        }
    }
    dirty_docs.sort_by_key(|d| {
        std::cmp::Reverse(
            d.mentioned_pages.len() + d.constant_changes.len() * 2 + d.stub_page_count / 10,
        )
    });

    // v1.1:tuning.lua 数值差异(独立于符号文件,确定性)
    let mut tuning_changes: Vec<TuningChange> = Vec::new();
    if let (Ok(old_t), Ok(new_t)) = (
        std::fs::read_to_string(old_root.join("tuning.lua")),
        std::fs::read_to_string(new_root.join("tuning.lua")),
    ) {
        if let (Ok(a), Ok(b)) = (
            crate::update::index::tuning::build_tuning(&old_t),
            crate::update::index::tuning::build_tuning(&new_t),
        ) {
            for (key, new_v) in &b.values {
                match a.values.get(key) {
                    Some(old_v) if old_v == new_v => {}
                    _ => tuning_changes.push(TuningChange {
                        key: format!("TUNING.{key}"),
                        old: a
                            .values
                            .get(key)
                            .map(tuning_val_text)
                            .unwrap_or_else(|| "(缺失)".to_string()),
                        new: tuning_val_text(new_v),
                    }),
                }
            }
        }
    }
    if !tuning_changes.is_empty() {
        reporter.log(format!("tuning 变更 {} 项", tuning_changes.len()));
    }

    // v1.1:prefab 变更 → 变体 → 页面交叉(需要语料索引;atlas 按需构建)
    let mut prefab_changes: Vec<PrefabChange> = Vec::new();
    let prefab_files: Vec<&crate::update::diffdata::FileDiff> = diff
        .files
        .iter()
        .filter(|f| f.path.starts_with("prefabs/") && f.status != DiffStatus::Removed)
        .collect();
    if !prefab_files.is_empty() {
        if let Some(corpus) = &params.corpus {
            reporter.stage("构建关联索引(prefab 交叉)");
            let atlas = build_atlas_from_dir(&new_root)?;
            let variant_pages = load_variant_pages(Path::new(corpus))?;
            let titles = load_titles_meta(Path::new(corpus))?;
            for f in &prefab_files {
                let mut variants: BTreeSet<String> = BTreeSet::new();
                for e in &atlas.index.edges {
                    if e.prefab_file == f.path {
                        variants.insert(e.prefab_variant.clone());
                    }
                }
                let mut pages: Vec<PageBrief> = Vec::new();
                let mut seen = BTreeSet::new();
                for v in &variants {
                    for key in [v.as_str(), &v.to_lowercase()] {
                        if let Some(ids) = variant_pages.get(key) {
                            for pid in ids {
                                if seen.insert(*pid) {
                                    if let Some(t) = titles.get(pid) {
                                        pages.push(PageBrief {
                                            pageid: *pid,
                                            title: t.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                pages.sort_by_key(|p| p.pageid);
                prefab_changes.push(PrefabChange {
                    path: f.path.clone(),
                    variants: variants.into_iter().collect(),
                    pages,
                });
            }
            prefab_changes.sort_by_key(|p| std::cmp::Reverse(p.pages.len()));
            reporter.log(format!(
                "prefab 变更 {} 个,涉及页面交叉 {} 条",
                prefab_changes.len(),
                prefab_changes.iter().map(|p| p.pages.len()).sum::<usize>()
            ));
        }
    }

    // Tier2 起草:precise 锚点 + 常量差异 → 页面修订建议(仅建议,人工审阅)
    let mut drafts: Vec<DraftSuggestion> = Vec::new();
    if params.draft {
        let Some(corpus) = &params.corpus else {
            return Err(Error::Config(
                "--draft 需要 --corpus 提供页面原文".to_string(),
            ));
        };
        let config = crate::llm::LlmConfig::from_env()
            .ok_or_else(|| Error::Config("--draft 需要 LLM 配置(LLM__API_KEY)".to_string()))?;
        reporter.stage("Tier2 起草修订建议");
        drafts = draft_revision_suggestions(
            &dirty_docs,
            &new_root,
            Path::new(corpus),
            &config,
            params.limit,
            reporter,
        )
        .await;
        if !drafts.is_empty() {
            write_review_template(&drafts, reporter)?;
        }
    }

    let mut rescan_summary = None;
    if params.rescan {
        reporter.stage("级联重扫脏文档");
        let corpus: Option<&str> = params.corpus.as_deref();
        let mut by_category: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for d in &dirty_docs {
            by_category.entry(d.category.as_str()).or_default().push(
                d.path
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(".lua")
                    .to_string(),
            );
        }
        let mut results = serde_json::Map::new();
        for (category, stems) in by_category {
            let params = crate::knowledge::ScanSymbolsParams {
                scripts_root: new_root.display().to_string(),
                category: category.to_string(),
                knowledge_root: params.knowledge_dir.clone(),
                corpus: corpus.map(str::to_string),
                sample_pages: 8,
                limit: stems.len(),
                force: true,
                concurrency: 2,
                pass2_names: None,
                pick_names: Some(stems),
                refresh_auto: false,
                confirm_empty: false,
            };
            let out = crate::knowledge::run_scan_symbols(&params, reporter).await?;
            results.insert(category.to_string(), out);
        }
        rescan_summary = Some(serde_json::Value::Object(results));
    }

    let report = SyncReport {
        old: params.old.clone(),
        new: params.new.clone(),
        changed_files: diff.files.len(),
        symbol_changes: symbol_changes.len(),
        other_changes: other.len(),
        dirty_docs: dirty_docs.clone(),
        added_symbols,
        removed_symbols,
        other_files: other.clone(),
        tuning_changes,
        prefab_changes,
        drafts: drafts.clone(),
        rescan: rescan_summary,
    };
    let out_path = Path::new("output/knowledge/sync_report.json");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;

    if !drafts.is_empty() {
        reporter.log(format!("Tier2 起草 {} 条修订建议(人工审阅)", drafts.len()));
    }
    reporter.log(format!(
        "sync 完成: 脏文档 {} 个(新增符号 {} / 移除 {} / 其他文件变更 {}),报告 {}",
        dirty_docs.len(),
        report.added_symbols.len(),
        report.removed_symbols.len(),
        other.len(),
        out_path.display()
    ));
    Ok(serde_json::to_value(&report)?)
}

/// Tier2:对带 precise 锚点的脏文档,按常量差异起草页面修订建议。
/// 每文档一次 LLM 调用;页面原文行作为 grounding,输出仅入报告不写 wiki。
const DRAFT_EXAMPLE: &str = r#"{"suggestions":[{"pageid":123,"old_sentence":"原句","new_sentence":"改后的句子","reason":"理由"}]}"#;

async fn draft_revision_suggestions(
    dirty_docs: &[DirtyDoc],
    new_root: &Path,
    corpus_root: &Path,
    config: &crate::llm::LlmConfig,
    limit: usize,
    reporter: &dyn Reporter,
) -> Vec<DraftSuggestion> {
    let mut drafts = Vec::new();
    let targets: Vec<&DirtyDoc> = dirty_docs
        .iter()
        .filter(|d| d.mentioned_pages.iter().any(|p| p.kind == "precise"))
        .take(limit)
        .collect();
    let system = "你是饥荒联机版中文维基的页面修订起草助手。必须只输出一个合法 JSON 对象。";
    for doc in targets {
        let mut grounding = String::new();
        let mut titles: BTreeMap<i64, String> = BTreeMap::new();
        for p in &doc.mentioned_pages {
            titles.insert(p.pageid, p.title.clone());
            let Some(first) = p.anchors.first().map(|a| a.raw.clone()) else {
                continue;
            };
            let text =
                std::fs::read_to_string(corpus_root.join(format!("pages/{}.wikitext", p.pageid)))
                    .unwrap_or_default();
            let lines: Vec<&str> = text
                .lines()
                .filter(|l| l.contains(&first))
                .take(2)
                .collect();
            for l in lines {
                grounding.push_str(&format!(
                    "- 页面《{}》(pageid {})含句:`{}`(关联锚点 {})\n",
                    p.title,
                    p.pageid,
                    l.trim(),
                    p.anchors.first().map(|a| a.matched.as_str()).unwrap_or("-")
                ));
            }
        }
        if grounding.is_empty() {
            continue;
        }
        // v1.2:注入常量使用上下文(新源码引用行)与同值竞争常量提示,
        // 供起草时消歧"同值异义"锚点(如 SEE_DIST=30 vs SHARE_TARGET_DIST=30)
        let new_src = std::fs::read_to_string(new_root.join(&doc.path)).unwrap_or_default();
        let consts: Vec<String> = doc
            .constant_changes
            .iter()
            .map(|c| {
                let usage = constant_usage_lines(&new_src, &c.name);
                let usage_txt = if usage.is_empty() {
                    "-".to_string()
                } else {
                    usage.join(" ; ")
                };
                let comp = competing_constants(&new_src, c);
                let comp_txt = if comp.is_empty() {
                    "无".to_string()
                } else {
                    comp.iter()
                        .map(|(n, ls)| {
                            let ls_txt = if ls.is_empty() {
                                "-".to_string()
                            } else {
                                ls.join(" ; ")
                            };
                            format!("{n}(使用行: {ls_txt})")
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                format!(
                    "- {}: {} → {}(新代码使用上下文:`{}`);同值竞争常量:{}",
                    c.name, c.old, c.new, usage_txt, comp_txt
                )
            })
            .collect();
        let prompt = format!(
            "代码更新导致以下常量变化(附新代码使用上下文与同值竞争常量提示):\n{}\n\n受影响页面的相关原文:\n{grounding}\n请仅为数值语义确实来自上述变更常量的原文句起草修订(数值/表述与新代码一致,保持页面行文风格);判断依据是使用上下文的语义(例如使用行体现\"寻找食物\"时,只有描述寻找食物的句子可改)。若某句的数值语义来自同值竞争常量且该常量未变更,或你无法从使用上下文确证该句数值来自变更常量,则不要为该句输出建议——宁可漏掉也不要改错。只输出一个 JSON 对象,形如:{example}\n只覆盖上面列出的页面。",
            consts.join("\n"),
            example = DRAFT_EXAMPLE,
        );
        reporter.log(format!("起草 {}:{}", doc.path, doc.mentioned_pages.len()));
        let raw = match config.complete_streaming(system, &prompt, |_| {}).await {
            Ok(t) => t,
            Err(e) => {
                reporter.log(format!("起草失败 {}:{}", doc.path, e));
                continue;
            }
        };
        let raw_path = PathBuf::from(format!(
            "output/knowledge/raw/page_map/draft_{}_{}.json",
            doc.path.replace('/', "_"),
            chrono_free_stamp(),
        ));
        let _ = std::fs::write(&raw_path, &raw);
        let trimmed = raw.trim();
        let (Some(a), Some(b)) = (trimmed.find('{'), trimmed.rfind('}')) else {
            reporter.log(format!("起草输出无 JSON:{}", doc.path));
            continue;
        };
        if a >= b {
            continue;
        }
        #[derive(serde::Deserialize)]
        struct Payload {
            #[serde(default)]
            suggestions: Vec<serde_json::Value>,
        }
        let Ok(p) = serde_json::from_str::<Payload>(&trimmed[a..=b]) else {
            reporter.log(format!("起草输出无法解析:{}", doc.path));
            continue;
        };
        for sg in p.suggestions {
            let pageid = sg.get("pageid").and_then(|x| x.as_i64()).unwrap_or(0);
            let new_sentence = sg
                .get("new_sentence")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string();
            if pageid == 0 || new_sentence.is_empty() {
                continue;
            }
            let id = format!("D{:03}", drafts.len() + 1);
            drafts.push(DraftSuggestion {
                id,
                path: doc.path.clone(),
                pageid,
                title: titles.get(&pageid).cloned().unwrap_or_default(),
                old_sentence: sg
                    .get("old_sentence")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string(),
                new_sentence,
                reason: sg
                    .get("reason")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
    }
    drafts
}

fn chrono_free_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 常量在源码中的使用行(≤2 行,trim 后原文)。
fn constant_usage_lines(src: &str, name: &str) -> Vec<String> {
    src.lines()
        .filter(|l| l.contains(name))
        .map(|l| l.trim().to_string())
        .take(2)
        .collect()
}

/// 同值竞争常量:与变更常量旧值相等的其它命名常量(页面同数值可能源自它们)。
fn competing_constants(src: &str, changed: &ConstantChange) -> Vec<(String, Vec<String>)> {
    extract_named_constants(src)
        .into_iter()
        .map(|(n, v)| (n, trim_num(v)))
        .filter(|(n, v)| *n != changed.name && *v == changed.old)
        .map(|(n, _)| {
            let usage = constant_usage_lines(src, &n);
            (n, usage)
        })
        .collect()
}

/// 复核裁决文件中的一条记录(--draft 生成模板,人工改 decision/note)。
#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ReviewEntry {
    id: String,
    decision: String,
    #[serde(default)]
    note: String,
}

/// --draft 收尾:落盘复核裁决模板 draft_review.json(全部 pending)。
fn write_review_template(drafts: &[DraftSuggestion], reporter: &dyn Reporter) -> Result<()> {
    let entries: Vec<serde_json::Value> = drafts
        .iter()
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "decision": "pending",
                "note": "",
                "path": d.path,
                "pageid": d.pageid,
                "title": d.title,
                "old_sentence": d.old_sentence,
                "new_sentence": d.new_sentence,
                "reason": d.reason,
            })
        })
        .collect();
    let path = Path::new("output/knowledge/draft_review.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&entries)?)?;
    reporter.log(format!(
        "复核模板已写出 {},请人工填写 decision(approve/reject)后以 --review 回读",
        path.display()
    ));
    Ok(())
}

/// 复核模式:回读 sync_report.json + 裁决文件 → 仅含 approve 项的应用清单。
fn run_review(
    review_path: &Path,
    report_path: &Path,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("Tier2 复核裁决");
    let raw = std::fs::read_to_string(report_path).map_err(|_| {
        Error::Config(format!(
            "复核模式需要既有报告 {},请先运行 --draft 生成",
            report_path.display()
        ))
    })?;
    let report: SyncReport =
        serde_json::from_str(&raw).map_err(|e| Error::Config(format!("既有报告解析失败: {e}")))?;
    if report.drafts.is_empty() {
        return Err(Error::Config(
            "既有报告中无起草建议(drafts 为空),无需复核".to_string(),
        ));
    }
    let entries: Vec<ReviewEntry> =
        serde_json::from_str(&std::fs::read_to_string(review_path).map_err(|e| {
            Error::Config(format!("裁决文件读取失败 {}: {e}", review_path.display()))
        })?)
        .map_err(|e| Error::Config(format!("裁决文件解析失败: {e}")))?;
    let mut decisions: BTreeMap<String, (String, String)> = BTreeMap::new();
    let known: BTreeSet<String> = report.drafts.iter().map(|d| d.id.clone()).collect();
    for e in entries {
        let decision = match e.decision.trim().to_lowercase().as_str() {
            "approve" | "approved" => "approve".to_string(),
            "reject" | "rejected" => "reject".to_string(),
            other => {
                reporter.log(format!(
                    "裁决 {} 非法值 `{other}`(需 approve/reject),按 pending 处理",
                    e.id
                ));
                "pending".to_string()
            }
        };
        if !known.contains(&e.id) {
            reporter.log(format!("裁决 {} 不在既有报告建议中,忽略", e.id));
            continue;
        }
        decisions.insert(e.id, (decision, e.note));
    }
    let approved = report
        .drafts
        .iter()
        .filter(|d| decisions.get(&d.id).map(|(x, _)| x.as_str()) == Some("approve"))
        .count();
    let rejected = report
        .drafts
        .iter()
        .filter(|d| decisions.get(&d.id).map(|(x, _)| x.as_str()) == Some("reject"))
        .count();
    let md = render_apply_list(&report.old, &report.new, &report.drafts, &decisions);
    let out_path = Path::new("output/knowledge/sync_apply_list.md");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, &md)?;
    reporter.log(format!(
        "复核完成:approve {approved} / reject {rejected} / 未裁决 {},应用清单 {}(wiki 编辑仍需人工执行)",
        report.drafts.len() - approved - rejected,
        out_path.display()
    ));
    Ok(serde_json::json!({
        "drafts": report.drafts.len(),
        "approved": approved,
        "rejected": rejected,
        "pending": report.drafts.len() - approved - rejected,
        "apply_list": out_path.display().to_string(),
    }))
}

/// 应用清单渲染:仅列 approve 项(old/new 句对 + 理由 + 审阅注记)。
fn render_apply_list(
    old: &str,
    new: &str,
    drafts: &[DraftSuggestion],
    decisions: &BTreeMap<String, (String, String)>,
) -> String {
    let mut out = String::from("# Tier2 修订建议 · 应用清单(仅含 approve 项)\n\n");
    out.push_str(&format!("对比区间:{old} → {new};wiki 编辑需人工执行。\n\n"));
    let mut approved_count = 0usize;
    for d in drafts {
        let Some((decision, note)) = decisions.get(&d.id) else {
            continue;
        };
        if decision != "approve" {
            continue;
        }
        approved_count += 1;
        out.push_str(&format!(
            "## {} · 页面《{}》(pageid {})· {}\n\
             - 旧句:`{}`\n\
             - 新句:`{}`\n\
             - 理由:{}\n\
             - 审阅注记:{}\n\n",
            d.id,
            d.title,
            d.pageid,
            d.path,
            d.old_sentence,
            d.new_sentence,
            if d.reason.is_empty() { "-" } else { &d.reason },
            if note.is_empty() { "-" } else { note },
        ));
    }
    let rejected: Vec<&DraftSuggestion> = drafts
        .iter()
        .filter(|d| decisions.get(&d.id).map(|(x, _)| x.as_str()) == Some("reject"))
        .collect();
    if !rejected.is_empty() {
        out.push_str("## 已驳回(留痕,不应用)\n\n");
        for d in rejected {
            let note = decisions.get(&d.id).map(|(_, n)| n.as_str()).unwrap_or("-");
            out.push_str(&format!(
                "- {} · 《{}》(pageid {}):{}\n",
                d.id,
                d.title,
                d.pageid,
                if note.is_empty() { "-" } else { note },
            ));
        }
        out.push('\n');
    }
    out.push_str(&format!("合计 approve {approved_count} 条。\n"));
    out
}

fn tuning_val_text(v: &crate::update::index::tuning::TuningVal) -> String {
    match v {
        crate::update::index::tuning::TuningVal::Num(n) => trim_num(*n),
        crate::update::index::tuning::TuningVal::Str(s) => s.clone(),
    }
}

/// variant(含大小写别名)→ pageids,来自语料索引 pages_by_prefab.json。
fn load_variant_pages(corpus_root: &Path) -> Result<BTreeMap<String, Vec<i64>>> {
    #[derive(serde::Deserialize)]
    struct Idx {
        prefabs: BTreeMap<String, Vec<i64>>,
    }
    let raw = std::fs::read_to_string(corpus_root.join("index/pages_by_prefab.json"))?;
    let idx: Idx = serde_json::from_str(&raw)?;
    let mut out = BTreeMap::new();
    for (variant, ids) in idx.prefabs {
        out.insert(variant.clone(), ids.clone());
        out.entry(variant.to_lowercase())
            .or_insert_with(|| ids.clone());
    }
    Ok(out)
}

fn load_titles_meta(corpus_root: &Path) -> Result<BTreeMap<i64, String>> {
    use crate::corpus::model::{GameClass, PageMeta};
    let mut out = BTreeMap::new();
    for line in std::fs::read_to_string(corpus_root.join("meta.jsonl"))?.lines() {
        if line.is_empty() {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<PageMeta>(line) {
            if matches!(m.game_class, GameClass::Dst | GameClass::Mixed) {
                out.insert(m.pageid, m.title);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_diff_detects_value_changes() {
        let old = "local SEE_DIST = 30\nlocal OTHER = 5\nlocal SAME = 7";
        let new = "local SEE_DIST = 35\nlocal OTHER = 5\nlocal SAME = 7";
        let d = diff_constants(old, new);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "SEE_DIST");
        assert_eq!(d[0].old, "30");
        assert_eq!(d[0].new, "35");
    }

    #[test]
    fn category_dispatch() {
        assert_eq!(category_of("components/health.lua"), Some("component"));
        assert_eq!(category_of("brains/houndbrain.lua"), Some("brain"));
        assert_eq!(category_of("stategraphs/SGhound.lua"), Some("stategraph"));
        assert_eq!(category_of("behaviours/wander.lua"), Some("behaviour"));
        assert_eq!(category_of("prefabs/hound.lua"), None);
        assert_eq!(category_of("tuning.lua"), None);
    }

    #[test]
    fn dirty_detection_by_sha() {
        let dir = std::env::temp_dir().join(format!("kn-sync-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("components")).unwrap();
        std::fs::write(dir.join("components/health.lua"), "local X = 1").unwrap();
        let file_sha = crate::knowledge::store::sha256_hex(
            &std::fs::read(dir.join("components/health.lua")).unwrap(),
        );
        let key = crate::knowledge::types::SymbolRefKey {
            kind: "component".into(),
            path: "components/health.lua".into(),
        };
        let llm_json = r#"{"category":"component","display_name":"health","summary":"测试","api":[{"name":"x","effect":"e"}],"search_terms":[],"gameplay_tags":[]}"#;
        let llm: crate::knowledge::types::SymbolDocLlm = serde_json::from_str(llm_json).unwrap();
        let doc = crate::knowledge::types::assemble(&llm, &key, &file_sha, 1, None, "m", "p4");
        assert!(!is_dirty(&doc, &dir), "sha 匹配生成时内容 → 不脏");
        std::fs::write(dir.join("components/health.lua"), "local X = 2").unwrap();
        assert!(is_dirty(&doc, &dir), "内容变化 → 脏");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// v1.2:使用行提取与同值竞争常量识别(SEE_DIST vs SHARE_TARGET_DIST 场景)
    #[test]
    fn constant_usage_and_competing() {
        let src = "local SEE_DIST = 30\n\
                   local ret = FindEntity(inst, SEE_DIST, nil, {\"_combat\"})\n\
                   local SHARE_TARGET_DIST = 30\n\
                   local OTHER = 30";
        let usage = constant_usage_lines(src, "SEE_DIST");
        assert_eq!(usage.len(), 2, "定义行 + 使用行都应命中");
        assert!(usage[0].starts_with("local SEE_DIST"));
        let changed = ConstantChange {
            name: "SEE_DIST".into(),
            old: "30".into(),
            new: "33".into(),
        };
        let comp = competing_constants(src, &changed);
        let names: Vec<&str> = comp.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names.len(), 2, "SHARE_TARGET_DIST 与 OTHER 同值竞争");
        assert!(names.contains(&"SHARE_TARGET_DIST"));
        assert!(names.contains(&"OTHER"));
        assert!(!names.contains(&"SEE_DIST"), "变更常量自身不算竞争");
    }

    /// Tier2 闭环:应用清单只渲染 approve 项正文,reject 项仅留痕
    #[test]
    fn apply_list_renders_approved_only() {
        let mk = |id: &str, old: &str, new: &str| DraftSuggestion {
            id: id.into(),
            path: "brains/houndbrain.lua".into(),
            pageid: 1,
            title: "猎犬".into(),
            old_sentence: old.into(),
            new_sentence: new.into(),
            reason: "常量 SEE_DIST 30→33".into(),
        };
        let drafts = vec![mk("D001", "旧A", "新A"), mk("D002", "旧B", "新B")];
        let mut decisions = BTreeMap::new();
        decisions.insert(
            "D001".to_string(),
            (
                "approve".to_string(),
                "数值语义确认来自 SEE_DIST".to_string(),
            ),
        );
        decisions.insert(
            "D002".to_string(),
            (
                "reject".to_string(),
                "来自竞争常量 SHARE_TARGET_DIST".to_string(),
            ),
        );
        let md = render_apply_list("20260501", "current", &drafts, &decisions);
        assert!(md.contains("D001"), "approve 项应出现");
        assert!(md.contains("新A"), "approve 项新句应出现");
        assert!(md.contains("数值语义确认来自 SEE_DIST"), "审阅注记应出现");
        assert!(!md.contains("新B"), "reject 项正文不应出现");
        assert!(md.contains("已驳回"), "reject 项应留痕");
        assert!(md.contains("来自竞争常量 SHARE_TARGET_DIST"));
        assert!(md.contains("合计 approve 1 条"));
    }

    /// Tier2 闭环:run_review 回读既有报告 + 裁决文件 → 应用清单
    #[test]
    fn review_flow_reads_report_and_decisions() {
        let dir = std::env::temp_dir().join(format!("kn-review-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let report = SyncReport {
            old: "20260501".into(),
            new: "current".into(),
            changed_files: 1,
            symbol_changes: 1,
            other_changes: 0,
            dirty_docs: vec![],
            added_symbols: vec![],
            removed_symbols: vec![],
            other_files: vec![],
            tuning_changes: vec![],
            prefab_changes: vec![],
            drafts: vec![DraftSuggestion {
                id: "D001".into(),
                path: "brains/houndbrain.lua".into(),
                pageid: 42,
                title: "猎犬".into(),
                old_sentence: "旧".into(),
                new_sentence: "新".into(),
                reason: "r".into(),
            }],
            rescan: None,
        };
        let report_path = dir.join("sync_report.json");
        std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        let review_path = dir.join("review.json");
        std::fs::write(
            &review_path,
            r#"[{"id":"D001","decision":"approve","note":"确认"},{"id":"D999","decision":"approve"}]"#,
        )
        .unwrap();
        let reporter = crate::service::CaptureReporter::new(true);
        let out = run_review(&review_path, &report_path, &reporter).unwrap();
        assert_eq!(out["approved"], 1);
        assert_eq!(out["rejected"], 0);
        assert_eq!(out["pending"], 0);
        let md = std::fs::read_to_string("output/knowledge/sync_apply_list.md").unwrap();
        assert!(md.contains("D001"));
        assert!(md.contains("确认"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
