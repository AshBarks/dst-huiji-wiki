//! M3 `knowledge sync`:代码变更 → 脏 SymbolDoc 清单 → PageSymbolMap 交叉
//! → 受影响页面与旧值锚点定位。
//!
//! v1 最小闭环(方案拍板,见 docs/KNOWLEDGE_SYNC.md):独立命令、确定性、
//! 零 LLM;`--rescan` 时级联调用 knowledge-scan-symbols 重扫脏文档。
//! Tier2 表述改写起草延后。

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
use std::path::Path;

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
}

#[derive(Debug, Clone, Serialize)]
pub struct ConstantChange {
    pub name: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageAnchor {
    pub pageid: i64,
    pub title: String,
    /// precise = 页面数值与旧常量精确匹配(旧值锚点);context = 仅提及
    pub kind: String,
    /// precise 锚点的页面原文(raw)与匹配常量
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<AnchorDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnchorDetail {
    pub raw: String,
    pub matched: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirtyDoc {
    pub path: String,
    pub category: String,
    pub constant_changes: Vec<ConstantChange>,
    pub mentioned_pages: Vec<PageAnchor>,
    pub stub_page_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuningChange {
    pub key: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrefabChange {
    pub path: String,
    pub variants: Vec<String>,
    pub pages: Vec<PageBrief>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageBrief {
    pub pageid: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
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
    pub rescan: Option<serde_json::Value>,
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
        rescan: rescan_summary,
    };
    let out_path = Path::new("output/knowledge/sync_report.json");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;

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
}
