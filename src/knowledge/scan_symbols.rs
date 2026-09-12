//! M1 / 构想 a:`knowledge scan-symbols` — LLM 逐个阅读主要 component 源码,
//! 产出 SymbolDoc 基础知识文档(docs/KNOWLEDGE_PIPELINE.md §3/§5)。
//!
//! 纪律:流式进度(Reporter)、raw 归档、失败重试 1 次、sha 增量跳过、原子落盘。

use crate::error::{Error, Result};
use crate::knowledge::auto_infobox::AutoInfoboxIndex;
use crate::knowledge::scan_wiki::{extract_named_constants, trim_num};
use crate::knowledge::store::{doc_path, is_fresh, load_doc, sha256_hex, write_atomic};
use crate::knowledge::types::{
    assemble, prompt_rev_for, SymbolDoc, SymbolDocLlm, SymbolRefKey, WikiAspect, WikiEvidence,
    WikiLinkLlm, WikiSection, SCHEMA_VERSION,
};
use crate::llm::{LlmConfig, LlmStreamEvent};
use crate::platform::progress::Reporter;
use crate::update::CorpusPageView;
use crate::update::{build_atlas_from_dir, IndexArtifact};
use futures_util::StreamExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 单文档源码输入上限;超出后采用“头 + 尾”截断并写入 truncation_note(§5)。
const SOURCE_BYTES_CAP: usize = 128 * 1024;
/// 超限时保留的头部字节数。
const SOURCE_HEAD_BYTES: usize = 96 * 1024;
/// 超限时保留的尾部字节数。
const SOURCE_TAIL_BYTES: usize = 32 * 1024;
/// pass1/pass2 每文档失败后的总尝试次数(含首次,即重试 1 次)。
const MAX_ATTEMPTS: u32 = 2;

/// 语料全文缓存:pages/<pageid>.wikitext → 全文(每 run 加载一次)。
pub struct WikiTextCache {
    pages: std::collections::HashMap<i64, String>,
}

impl WikiTextCache {
    pub fn load(corpus_root: &Path) -> Result<Self> {
        let dir = corpus_root.join("pages");
        // 只加载联机版/混合页面，避免单机版页面进入全文检索采样。
        let allowed: Option<std::collections::HashSet<i64>> =
            match std::fs::read_to_string(corpus_root.join("meta.jsonl")) {
                Ok(content) => {
                    let mut set = std::collections::HashSet::new();
                    for line in content.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        if let Ok(meta) =
                            serde_json::from_str::<crate::corpus::model::PageMeta>(line)
                        {
                            if matches!(
                                meta.game_class,
                                crate::corpus::model::GameClass::Dst
                                    | crate::corpus::model::GameClass::Mixed
                            ) {
                                set.insert(meta.pageid);
                            }
                        }
                    }
                    Some(set)
                }
                Err(_) => None,
            };
        let mut pages = std::collections::HashMap::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("wikitext") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<i64>().ok());
            if let Some(pid) = stem {
                if allowed.as_ref().is_some_and(|a| !a.contains(&pid)) {
                    continue;
                }
                let text = std::fs::read_to_string(&path)?;
                pages.insert(pid, text);
            }
        }
        Ok(Self { pages })
    }

    /// 对全部页面按检索词计分(大小写不敏感),返回 (hits, pageid, 命中行)。
    fn score(&self, terms: &[String], per_page_lines: usize) -> Vec<(usize, i64, Vec<String>)> {
        let mut out = Vec::new();
        for (pid, text) in &self.pages {
            let lower = text.to_lowercase();
            let mut total = 0usize;
            let mut lines: Vec<String> = Vec::new();
            for t in terms {
                let tl = t.to_lowercase();
                if tl.is_empty() {
                    continue;
                }
                let mut from = 0usize;
                let mut counted = 0usize;
                while let Some(pos) = lower[from..].find(&tl) {
                    let abs = from + pos;
                    total += 1;
                    counted += 1;
                    if lines.len() < per_page_lines {
                        let start = text
                            .char_indices()
                            .map(|(i, _)| i)
                            .rev()
                            .find(|&i| i <= abs)
                            .unwrap_or(0);
                        let line_start = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
                        let rest = &text[line_start..];
                        let line_end = rest
                            .find('\n')
                            .map(|i| line_start + i)
                            .unwrap_or(text.len());
                        let mut le = line_end;
                        while le > line_start && !text.is_char_boundary(le) {
                            le -= 1;
                        }
                        let line = text[line_start..le].trim();
                        if !line.is_empty() && lines.len() < per_page_lines {
                            let mut cut = line.to_string();
                            if cut.len() > 160 {
                                let mut e2 = 160;
                                while e2 > 0 && !cut.is_char_boundary(e2) {
                                    e2 -= 1;
                                }
                                cut.truncate(e2);
                                cut.push('…');
                            }
                            lines.push(cut);
                        }
                    }
                    from = abs + tl.len();
                    if counted >= 50 {
                        break;
                    }
                }
            }
            if total > 0 {
                out.push((total, *pid, lines));
            }
        }
        out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        out
    }
}

pub struct ScanSymbolsParams {
    pub scripts_root: String,
    pub knowledge_root: String,
    /// 符号类别:component | brain | behaviour(默认 component)
    pub category: String,
    /// wiki 语料 host 根目录;提供则启用 pass2 语料归因(link-wiki)
    pub corpus: Option<String>,
    /// pass2 每组件采样的页面数(默认 8,引用/fact 富裕页优先)
    pub sample_pages: usize,
    pub limit: usize,
    pub force: bool,
    /// 并行处理的组件数;1 = 串行(默认)
    pub concurrency: usize,
    /// 仅对指定文件名词干(fn stem)跑 pass2;None = 全部跑(component 兼容)
    pub pass2_names: Option<Vec<String>>,
    /// 仅选取指定文件名词干的符号(绕过引用数排序截断);None = 按排序取 limit
    pub pick_names: Option<Vec<String>>,
    /// 不调 LLM:仅用 AutoInfobox 冷数据刷新现有 component 文档的 auto_maintained
    pub refresh_auto: bool,
    /// pass2 二次确认(可选防过严):采样 ≥3 页但判空时追加一次复查(REMAINING_WORK A4)
    pub confirm_empty: bool,
}

/// P1 选择:目录扫描(反向边仅作排序信号),覆盖全部 component 文件,
/// 附带磁盘源码路径与字节数。
/// component 直接扫 components/ 目录(与 brain/behaviour 同模式):
/// 314 个从未被 prefab 引用的组件(管理器/子组件)不在反向边里,
/// top_symbols 口径会漏;反向边仅作排序信号(变体数降序)。
fn pick_components(
    scripts_root: &Path,
    index: &IndexArtifact,
    limit: usize,
) -> Vec<(SymbolRefKey, PathBuf, usize)> {
    let dir = scripts_root.join("components");
    let mut entries: Vec<(usize, String, SymbolRefKey, PathBuf, usize)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let Some(name) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let rel = format!("components/{name}.lua");
            let full = scripts_root.join(&rel);
            let bytes = std::fs::metadata(&full)
                .map(|m| m.len() as usize)
                .unwrap_or(0);
            let variants = index
                .reverse
                .get(&rel)
                .map(|vs| {
                    vs.iter()
                        .filter_map(|v| v.rsplit('#').next())
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                })
                .unwrap_or(0);
            entries.push((
                variants,
                name.clone(),
                SymbolRefKey {
                    kind: "component".to_string(),
                    path: rel,
                },
                full,
                bytes,
            ));
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    entries
        .into_iter()
        .take(limit)
        .map(|(_, _, key, full, bytes)| (key, full, bytes))
        .collect()
}

/// 按类别选择符号列表(component/brain/behaviour)。
fn pick_symbols(
    scripts_root: &Path,
    index: &IndexArtifact,
    category: &str,
    limit: usize,
    pick_names: Option<&[String]>,
) -> Vec<(SymbolRefKey, PathBuf, usize)> {
    // 名称过滤时先放开 limit,避免目标符号被引用数排序截断。
    let scan_limit = if pick_names.is_some() { 100_000 } else { limit };
    let picked = match category {
        "behaviour" => pick_behaviours(scripts_root, index, scan_limit),
        "brain" => pick_brains(scripts_root, index, scan_limit),
        "stategraph" => pick_stategraphs(scripts_root, index, scan_limit),
        _ => pick_components(scripts_root, index, scan_limit),
    };
    let Some(names) = pick_names else {
        return picked;
    };
    let wanted: std::collections::HashSet<String> =
        names.iter().map(|n| n.trim().to_lowercase()).collect();
    picked
        .into_iter()
        .filter(|(key, _, _)| {
            let stem = key
                .path
                .rsplit('/')
                .next()
                .unwrap_or("")
                .trim_end_matches(".lua")
                .to_lowercase();
            wanted.contains(&stem)
        })
        .take(limit)
        .collect()
}

/// behaviour 词典层:直接扫 behaviours/ 目录,优先已被 behaviour_calls 调用的节点。
fn pick_behaviours(
    scripts_root: &Path,
    index: &IndexArtifact,
    limit: usize,
) -> Vec<(SymbolRefKey, PathBuf, usize)> {
    let mut call_counts: std::collections::HashMap<String, usize> = Default::default();
    for call in &index.behaviour_calls {
        *call_counts.entry(call.ctor.to_lowercase()).or_insert(0) += 1;
    }
    let dir = scripts_root.join("behaviours");
    let mut entries: Vec<(usize, String, SymbolRefKey, PathBuf, usize)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let Some(name) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let rel = format!("behaviours/{name}.lua");
            let full = scripts_root.join(&rel);
            let bytes = std::fs::metadata(&full)
                .map(|m| m.len() as usize)
                .unwrap_or(0);
            let count = call_counts.get(&name.to_lowercase()).copied().unwrap_or(0);
            entries.push((
                count,
                name.clone(),
                SymbolRefKey {
                    kind: "behaviour".to_string(),
                    path: rel,
                },
                full,
                bytes,
            ));
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    entries
        .into_iter()
        .take(limit)
        .map(|(_, _, key, full, bytes)| (key, full, bytes))
        .collect()
}

/// brain 组合层:直接扫 brains/ 目录(不依赖反向边——spider/beefalo 等
/// 动态 SetBrain 的文件在 atlas 里无 Brain 边,反向边只作排序信号),
/// 按 prefab 引用变体数降序;排除共享 helper(源码不含 `Class(Brain`)。
fn pick_brains(
    scripts_root: &Path,
    index: &IndexArtifact,
    limit: usize,
) -> Vec<(SymbolRefKey, PathBuf, usize)> {
    let dir = scripts_root.join("brains");
    let mut entries: Vec<(usize, String, SymbolRefKey, PathBuf, usize)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let Ok(source) = std::fs::read(&path) else {
                continue;
            };
            if !source.windows(11).any(|w| w == b"Class(Brain") {
                continue; // braincommon 等共享 helper,非 Brain 子类
            }
            let Some(name) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let rel = format!("brains/{name}.lua");
            let variants = index
                .reverse
                .get(&rel)
                .map(|vs| {
                    vs.iter()
                        .filter_map(|v| v.rsplit('#').next())
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                })
                .unwrap_or(0);
            entries.push((
                variants,
                name.clone(),
                SymbolRefKey {
                    kind: "brain".to_string(),
                    path: rel,
                },
                path,
                source.len(),
            ));
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    entries
        .into_iter()
        .take(limit)
        .map(|(_, _, key, full, bytes)| (key, full, bytes))
        .collect()
}

/// stategraph 执行层:直接扫 stategraphs/ 目录(反向边仅作排序信号),
/// 按拍板决策排除 SGwilson* / *_client / commonstates;要求源码含
/// `StateGraph(` 构造标记。doc_id 沿用文件词干(如 stategraph__SGhound),
/// path ↔ doc_id 保持 1:1,不做前缀剥离特例。
fn pick_stategraphs(
    scripts_root: &Path,
    index: &IndexArtifact,
    limit: usize,
) -> Vec<(SymbolRefKey, PathBuf, usize)> {
    let dir = scripts_root.join("stategraphs");
    let mut entries: Vec<(usize, String, SymbolRefKey, PathBuf, usize)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let Some(name) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let excluded =
                name == "commonstates" || name.starts_with("SGwilson") || name.ends_with("_client");
            if excluded {
                continue;
            }
            let Ok(source) = std::fs::read(&path) else {
                continue;
            };
            if !source.windows(11).any(|w| w == b"StateGraph(") {
                continue; // 非状态图文件(共享工具等)
            }
            let rel = format!("stategraphs/{name}.lua");
            let variants = index
                .reverse
                .get(&rel)
                .map(|vs| {
                    vs.iter()
                        .filter_map(|v| v.rsplit('#').next())
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                })
                .unwrap_or(0);
            entries.push((
                variants,
                name.clone(),
                SymbolRefKey {
                    kind: "stategraph".to_string(),
                    path: rel,
                },
                path,
                source.len(),
            ));
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    entries
        .into_iter()
        .take(limit)
        .map(|(_, _, key, full, bytes)| (key, full, bytes))
        .collect()
}

/// 单个组件处理结果统计。
#[derive(Debug, Default, Clone, Copy)]
struct ComponentOutcome {
    written: u32,
    skipped: u32,
    failed: u32,
    wiki_written: u32,
    wiki_failed: u32,
}

/// 并行处理时在符号任务间共享的只读上下文。
struct ComponentShared<'a> {
    knowledge_root: &'a Path,
    /// 当前符号类别:component / brain / behaviour。
    category: &'a str,
    /// 当前类别对应的 prompt_rev。
    prompt_rev: &'static str,
    force: bool,
    has_corpus: bool,
    sample: usize,
    atlas: &'a IndexArtifact,
    build_id: &'a Option<String>,
    view: Option<&'a CorpusPageView>,
    cache: Option<&'a WikiTextCache>,
    config: &'a LlmConfig,
    raw_dir: &'a Path,
    confirm_empty: bool,
    auto_infobox: &'a AutoInfoboxIndex,
    /// 仅对指定文件名词干跑 pass2;None = 全部。
    pass2_names: Option<&'a [String]>,
}

pub async fn run_scan_symbols(
    params: &ScanSymbolsParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let scripts_root = Path::new(&params.scripts_root);
    let knowledge_root = Path::new(&params.knowledge_root);

    let auto_infobox = AutoInfoboxIndex::load(knowledge_root)?;
    if params.refresh_auto {
        return refresh_auto_maintained(knowledge_root, &auto_infobox, reporter);
    }
    if !auto_infobox.components.is_empty() {
        reporter.log(format!(
            "AutoInfobox 冷数据已加载: {} 个组件",
            auto_infobox.components.len()
        ));
    }

    reporter.stage("构建代码关联索引");
    let atlas = build_atlas_from_dir(scripts_root)?;
    let build_id = atlas.build_id.clone();

    let category = if params.category.is_empty() {
        "component"
    } else {
        params.category.as_str()
    };
    let prompt_rev = prompt_rev_for(category);
    reporter.stage(&format!("选择 {category} 符号"));
    let picked = pick_symbols(
        scripts_root,
        &atlas.index,
        category,
        params.limit,
        params.pick_names.as_deref(),
    );
    reporter.log(format!(
        "候选 {} 个 {category}(按关联引用降序;force={})",
        picked.len(),
        params.force
    ));

    reporter.stage("生成 SymbolDoc");
    let config = LlmConfig::from_env()
        .ok_or_else(|| Error::Config("未配置 LLM__API_KEY,无法生成符号文档".to_string()))?;
    reporter.log(format!("使用模型 {} / {}", config.model, config.base_url));

    let raw_dir = PathBuf::from("output/knowledge/raw/symbols");
    std::fs::create_dir_all(&raw_dir)?;

    // pass2 需要:语料页面视图 + 全文缓存 + 采样页数
    let (view, cache, sample) = match &params.corpus {
        Some(corpus) => {
            let v = CorpusPageView::load(Path::new(corpus))?;
            let cache = WikiTextCache::load(Path::new(corpus))?;
            reporter.log(format!(
                "pass2 语料已加载:路由映射实体 {} / 全文页 {} / facts {} 页;每组件采样 {} 页",
                v.pages.len(),
                cache.pages.len(),
                v.facts.len(),
                params.sample_pages
            ));
            (Some(v), Some(cache), params.sample_pages.max(1))
        }
        None => (None, None, 0),
    };
    let concurrency = params.concurrency.max(1);
    if concurrency > 1 {
        reporter.log(format!("并行处理: concurrency={}", concurrency));
    }
    let total = picked.len();

    let outcomes = futures_util::stream::iter(picked.into_iter().enumerate().map(
        |(idx, (key, src_path, src_bytes))| {
            let shared = ComponentShared {
                knowledge_root,
                category,
                prompt_rev,
                force: params.force,
                has_corpus: params.corpus.is_some(),
                sample,
                atlas: &atlas.index,
                build_id: &build_id,
                view: view.as_ref(),
                cache: cache.as_ref(),
                config: &config,
                raw_dir: &raw_dir,
                confirm_empty: params.confirm_empty,
                auto_infobox: &auto_infobox,
                pass2_names: params.pass2_names.as_deref(),
            };
            async move {
                process_one_component(
                    idx + 1,
                    total,
                    &key,
                    &src_path,
                    src_bytes,
                    &shared,
                    reporter,
                )
                .await
            }
        },
    ))
    .buffer_unordered(concurrency)
    .collect::<Vec<_>>()
    .await;

    let (mut written, mut skipped, mut failed, mut wiki_written, mut wiki_failed) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    for outcome in outcomes {
        match outcome {
            Ok(o) => {
                written += o.written;
                skipped += o.skipped;
                failed += o.failed;
                wiki_written += o.wiki_written;
                wiki_failed += o.wiki_failed;
            }
            Err(e) => {
                failed += 1;
                reporter.log(format!("符号处理失败: {e}"));
            }
        }
    }

    Ok(serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "prompt_rev": prompt_rev,
        "picked": total,
        "written": written,
        "skipped_fresh": skipped,
        "failed": failed,
        "wiki_enriched": wiki_written,
        "wiki_failed": wiki_failed,
    }))
}

/// 根据 AutoInfobox 冷数据注入/刷新 `auto_maintained` 字段。
fn inject_auto_maintained(doc: &mut SymbolDoc, auto: &AutoInfoboxIndex) {
    let name = doc
        .reference
        .path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".lua");
    doc.auto_maintained = auto.get(name).cloned();
}

/// 不调 LLM 的 post-action 重放:把冷数据中的 auto_maintained 刷进现有
/// component 文档(冷数据修订后无需整批重扫即可同步)。仅触碰内容有变化的文档。
fn refresh_auto_maintained(
    knowledge_root: &Path,
    auto: &AutoInfoboxIndex,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let symbols_dir = knowledge_root.join("symbols");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&symbols_dir)?
        .collect::<std::io::Result<Vec<std::fs::DirEntry>>>()?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("component__") && n.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();

    let mut updated = 0usize;
    let mut unchanged = 0usize;
    for path in &paths {
        let Some(mut doc) = load_doc(path)? else {
            continue;
        };
        let name = doc
            .reference
            .path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".lua");
        let fresh = auto.get(name).cloned();
        if doc.auto_maintained == fresh {
            unchanged += 1;
            continue;
        }
        let before = doc.auto_maintained.is_some();
        doc.auto_maintained = fresh;
        write_atomic(path, &serde_json::to_string_pretty(&doc)?)?;
        updated += 1;
        reporter.log(format!(
            "auto_maintained 已刷新: {}({})",
            name,
            if before { "更新" } else { "新增" }
        ));
    }
    reporter.log(format!(
        "AutoInfobox 注入刷新完成: 共 {} 份 component 文档,更新 {} / 无变化 {}",
        paths.len(),
        updated,
        unchanged
    ));
    Ok(serde_json::json!({
        "mode": "refresh_auto",
        "scanned": paths.len(),
        "updated": updated,
        "unchanged": unchanged,
        "cold_data_components": auto.components.len(),
    }))
}

/// 是否允许当前符号跑 pass2:未配置名单时全部允许;配置后仅白名单名称命中。
fn allow_pass2(shared: &ComponentShared<'_>, key: &SymbolRefKey) -> bool {
    let Some(names) = shared.pass2_names else {
        return true;
    };
    let stem = key
        .path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".lua");
    names.iter().any(|n| n == stem)
}

async fn process_one_component(
    idx: usize,
    total: usize,
    key: &SymbolRefKey,
    src_path: &Path,
    src_bytes: usize,
    shared: &ComponentShared<'_>,
    reporter: &dyn Reporter,
) -> Result<ComponentOutcome> {
    reporter.stage(&format!(
        "[{}/{}] {}({}B)",
        idx,
        total,
        key.doc_id(),
        src_bytes
    ));
    let doc_path = doc_path(shared.knowledge_root, key);
    let source = std::fs::read(src_path)?;
    let sha = sha256_hex(&source);
    let mut outcome = ComponentOutcome::default();

    if !shared.force {
        if let Some(existing) = load_doc(&doc_path)? {
            if is_fresh(&existing, &sha, shared.prompt_rev) {
                match (&existing.wiki, shared.has_corpus) {
                    (Some(_), _) | (None, false) => {
                        reporter.log("源码与 prompt_rev 未变,跳过".to_string());
                        outcome.skipped += 1;
                        return Ok(outcome);
                    }
                    (None, true) => {
                        if !allow_pass2(shared, key) {
                            reporter.log("文档新鲜但未列入 pass2 名单,跳过".to_string());
                            outcome.skipped += 1;
                            return Ok(outcome);
                        }
                        // 文档已是 v2 但缺 wiki 节点 → 只补 pass2
                        reporter.log("代码文档新鲜,仅补 pass2 语料归因".to_string());
                        let mut doc = existing;
                        if shared.category == "component" {
                            inject_auto_maintained(&mut doc, shared.auto_infobox);
                        }
                        match enrich_with_wiki(
                            shared.config,
                            &mut doc,
                            WikiContext {
                                index: shared.atlas,
                                view: shared.view.expect("corpus loaded"),
                                cache: shared.cache,
                                sample: shared.sample,
                                prompt_rev: shared.prompt_rev,
                                confirm_empty: shared.confirm_empty,
                                constants: source_constants(&source),
                            },
                            shared.raw_dir,
                            reporter,
                        )
                        .await
                        {
                            Ok(()) => {
                                write_atomic(&doc_path, &serde_json::to_string_pretty(&doc)?)?;
                                outcome.wiki_written += 1;
                            }
                            Err(e) => {
                                outcome.wiki_failed += 1;
                                reporter
                                    .log(format!("pass2 失败,保留 pass1 文档(可下次补跑): {e}"));
                            }
                        }
                        return Ok(outcome);
                    }
                }
            }
            reporter.log("源码或 prompt_rev 已变化,重扫".to_string());
        }
    }

    let (visible, truncated) = if source.len() > SOURCE_BYTES_CAP {
        let head = &source[..SOURCE_HEAD_BYTES];
        let tail_start = source.len().saturating_sub(SOURCE_TAIL_BYTES);
        let mut body = Vec::with_capacity(source.len());
        body.extend_from_slice(head);
        body.extend_from_slice(b"\n-- ...(middle omitted)...\n");
        body.extend_from_slice(&source[tail_start..]);
        (String::from_utf8_lossy(&body).to_string(), true)
    } else {
        (String::from_utf8_lossy(&source).to_string(), false)
    };

    let behaviour_context = build_behaviour_context(shared, key)?;

    let mut llm_outcome: Option<Result<SymbolDocLlm>> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        if attempt > 1 {
            reporter.log("重试第 2 次…".to_string());
        }
        match generate_one(
            shared.config,
            key,
            &visible,
            truncated,
            shared.raw_dir,
            reporter,
            shared.category,
            shared.prompt_rev,
            behaviour_context.as_deref(),
        )
        .await
        {
            Ok(doc) => {
                llm_outcome = Some(Ok(doc));
                break;
            }
            Err(e) => llm_outcome = Some(Err(e)),
        }
    }

    match llm_outcome.expect("at least one attempt") {
        Ok(llm_doc) => {
            if let Err(e) = llm_doc.validate() {
                outcome.failed += 1;
                reporter.log(format!("schema 校验失败,已记为失败:{e}"));
                return Ok(outcome);
            }
            let mut doc = assemble(
                &llm_doc,
                key,
                &sha,
                source.len(),
                shared.build_id.clone(),
                &shared.config.model,
                shared.prompt_rev,
            );
            if shared.category == "component" {
                inject_auto_maintained(&mut doc, shared.auto_infobox);
            }
            // search_terms 质量杠杆:语料零命中过半时单次改写
            if let Some(cache) = shared.cache {
                apply_search_terms_leverage(
                    cache,
                    shared.config,
                    &mut doc,
                    shared.raw_dir,
                    reporter,
                )
                .await;
            }
            let body = serde_json::to_string_pretty(&doc)?;
            write_atomic(&doc_path, &body)?;
            if let Some(v) = shared.view {
                if allow_pass2(shared, key) {
                    match enrich_with_wiki(
                        shared.config,
                        &mut doc,
                        WikiContext {
                            index: shared.atlas,
                            view: v,
                            cache: shared.cache,
                            sample: shared.sample,
                            prompt_rev: shared.prompt_rev,
                            confirm_empty: shared.confirm_empty,
                            constants: source_constants(&source),
                        },
                        shared.raw_dir,
                        reporter,
                    )
                    .await
                    {
                        Ok(()) => {
                            write_atomic(&doc_path, &serde_json::to_string_pretty(&doc)?)?;
                            outcome.wiki_written += 1;
                        }
                        Err(e) => {
                            outcome.wiki_failed += 1;
                            reporter.log(format!("pass2 失败,已保留 pass1 文档(可下次补跑): {e}"));
                        }
                    }
                } else {
                    reporter.log("未列入 pass2 名单,仅写 pass1 文档".to_string());
                }
            }
            reporter.log(format!(
                "文档已写入 {}(api {} 条 / wiki 节点 {})",
                doc_path.display(),
                doc.api.len(),
                if doc.wiki.is_some() {
                    "已归因"
                } else {
                    "未生成"
                }
            ));
            outcome.written += 1;
        }
        Err(e) => {
            outcome.failed += 1;
            reporter.log(format!("生成失败:{e}"));
        }
    }

    Ok(outcome)
}

/// brain 文档 pass1 的额外上下文:读取本 brain 已引用 behaviour 的 SymbolDoc,
/// 把 ctor_params 摘要注入 prompt,避免 LLM 猜测位置参数语义。
fn build_behaviour_context(
    shared: &ComponentShared<'_>,
    key: &SymbolRefKey,
) -> Result<Option<String>> {
    if shared.category != "brain" {
        return Ok(None);
    }
    let mut seen = HashSet::new();
    let mut summaries: Vec<String> = Vec::new();
    for call in &shared.atlas.behaviour_calls {
        if call.brain_file != key.path || !seen.insert(call.ctor.clone()) {
            continue;
        }
        let file_name = call.ctor.to_lowercase();
        let bkey = SymbolRefKey {
            kind: "behaviour".to_string(),
            path: format!("behaviours/{file_name}.lua"),
        };
        if let Ok(Some(doc)) = load_doc(&doc_path(shared.knowledge_root, &bkey)) {
            let params = doc
                .ctor_params
                .iter()
                .map(|p| {
                    let default = p
                        .default
                        .as_deref()
                        .map(|d| format!("(默认 {d})"))
                        .unwrap_or_default();
                    format!("  - {}: {}{}", p.name, p.semantic, default)
                })
                .collect::<Vec<_>>()
                .join("\n");
            summaries.push(format!(
                "{}({})\n{}",
                doc.display_name, doc.category, params
            ));
        }
    }
    if summaries.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "该 brain 引用到的 behaviour 参数语义(来自现有 behaviour 文档):\n{}",
        summaries.join("\n\n")
    )))
}

fn build_prompt(
    display: &str,
    source: &str,
    truncated: bool,
    category: &str,
    behaviour_context: Option<&str>,
) -> String {
    let head = if truncated {
        "【注意】源码因长度被截断,仅基于可见部分标注,并在 truncation_note 中说明。\n\n"
    } else {
        ""
    };
    let extra = behaviour_context
        .map(|ctx| format!("\n{ctx}\n"))
        .unwrap_or_default();
    match category {
        "behaviour" => format!(
            "{head}以下是饥荒联机版(DST)行为节点 `{display}` 的 Lua 源码。\
请通读后产出该行为节点的知识文档 JSON。

要求:
1. 全部用中文;不得编造源码中不存在的内容。
2. summary:2~4 句,说明该行为节点驱动实体完成什么行为、在什么场景使用。
3. api:覆盖对外暴露的方法;若确实没有对外方法,api 返回空数组,并必须用 api_note 说明原因;不得为了凑数编造方法。
4. ctor_params:覆盖构造子中 `inst` 之外的全部位置参数;每项为对象 {{\"name\":参数名,\"semantic\":玩家语义,\"default\":默认值或省略}},语义要写清值变大/变小意味着什么。
5. effects:字符串数组,每项一条该节点对组件/SG 状态标签的作用(如 `locomotor:GoToPoint`、`combat:BattleCry`、检查 `sg:HasStateTag`)。
6. success_fail_conditions:字符串数组,分别说明何时 SUCCESS / FAILED / RUNNING,如 [\"SUCCESS:…\",\"FAILED:…\",\"RUNNING:…\"]。
7. events_published / events_listened / netvars / tunables:字符串数组,严格取自源码标识符,可为空数组;tunables 放源码内的默认数值/配置(如 Wander 的 wander_dist)。
8. gameplay_tags:1~4 个玩法标签(如 生存/战斗/建造/装饰)。
9. search_terms:5~10 个用于在维基全文中检索该行为玩家表达的词汇,以中文玩家语言为主(可混英文),每词 2~6 字。
10. 只输出一个 JSON 对象,字段名与上述一致;api 确无方法时 api 为空数组,但必须提供 api_note。

源码:
```lua
{source}
```",
        ),
        "brain" => format!(
            "{head}以下是饥荒联机版(DST)大脑 `{display}` 的 Lua 源码。\
请通读后产出该大脑的知识文档 JSON。

要求:
1. 全部用中文;不得编造源码中不存在的内容。
2. summary:2~4 句,说明该大脑让实体在什么情境下采取什么行为。
3. tunables:字符串数组,文件级 local 常量名(如 SEE_DIST、HOUSE_MAX_DIST),这些常量是页面“行为”章数值事实的主要锚点;名称/值严格取自源码。
4. behaviour_invocations:对象数组,每项 {{\"ctor\":构造子名,\"args_semantic\":[各位置参数的玩家语义,顺序与构造子一致],\"context\":条件语境或省略}};如 `ChaseAndAttack(inst, 100)` → args_semantic 里写“追击最长 100 秒”。
5. context_branches:对象数组,每项 {{\"condition\":条件如 HasTag(\"clay\"),\"semantic\":该分支下行为的差异}},对应页面分句枚举。
6. bt_structure:单个字符串,优先级节点树的文字化摘要(意图粒度,不追求完整还原)。
7. api:对外方法(通常少量);若没有,api 返回空数组,并必须用 api_note 说明原因。
8. events_published / events_listened / netvars:严格取自源码标识符,可为空数组。
9. gameplay_tags:1~4 个玩法标签(如 生存/战斗/建造/装饰)。
10. search_terms:5~10 个用于在维基全文中检索该大脑行为表达的词汇,以中文玩家语言为主(可混英文),每词 2~6 字。
11. 只输出一个 JSON 对象,字段名与上述一致。

{extra}源码:
```lua
{source}
```",
        ),
        "stategraph" => format!(
            "{head}以下是饥荒联机版(DST)状态图 `{display}` 的 Lua 源码。\
请通读后产出该状态图的知识文档 JSON。

要求:
1. 全部用中文;不得编造源码中不存在的内容。
2. summary:2~4 句,说明该状态图驱动什么实体、整体状态组织方式。
3. states:字符串数组,源码中 State{{...}} 定义的全部状态名,严格取自源码,按出现顺序。
4. state_notes:对象数组,只挑与玩法/页面相关的关键状态(≤10 个),每项 {{\"state\":状态名,\"note\":1~2 句玩家语义(进入条件/表现/数值配置,如无敌帧、硬直、特殊动画窗口)}};大量纯动画/音效状态不要写。
5. api:对外方法(通常少量或没有);若没有,api 返回空数组,并必须用 api_note 说明原因。
6. events_published / events_listened / netvars / tunables:字符串数组,严格取自源码标识符(PushEvent/ListenForEvent/NetEvent 等),可为空数组。
7. gameplay_tags:1~4 个玩法标签(如 生存/战斗/建造/装饰)。
8. search_terms:5~10 个用于在维基全文中检索该实体行为表现的词汇,以中文玩家语言为主(可混英文),每词 2~6 字。
9. 只输出一个 JSON 对象,字段名与上述一致。

源码:
```lua
{source}
```",
        ),
        _ => format!(
            "{head}以下是饥荒联机版(DST)组件 `{display}` 的 Lua 源码。\
请通读后产出该组件的知识文档 JSON。

要求:
1. 全部用中文;不得编造源码中不存在的内容。
2. summary:2~4 句,说明组件管理的数据与参与的核心玩法。
3. api:覆盖全部对外(实体/其他系统)暴露的方法;effect 写清行为与副作用。不要猜测页面该如何撰写——文档只承载代码可证实的事实。
4. 若源码确实没有任何对外方法(纯数据组件),api 返回空数组,并必须用 api_note 说明原因(如“纯数据组件,无公开方法”);不得为了凑数编造方法。
5. events_published / events_listened / netvars / tunables:严格取自源码标识符,可为空数组。
6. gameplay_tags:1~4 个玩法标签(如 生存/战斗/建造/装饰)。
7. search_terms:5~10 个用于在维基全文中检索该组件能力表达的词汇,以中文玩家语言为主(可混英文),每词 2~6 字;不要照抄 API 名。
8. 只输出一个 JSON 对象,字段名与上述一致;除 api 确无方法外不得为空数组。

源码:
```lua
{source}
```",
        ),
    }
}

#[allow(clippy::too_many_arguments)]
async fn generate_one(
    config: &LlmConfig,
    key: &SymbolRefKey,
    visible_source: &str,
    truncated: bool,
    raw_dir: &Path,
    reporter: &dyn Reporter,
    category: &str,
    prompt_rev: &str,
    behaviour_context: Option<&str>,
) -> Result<SymbolDocLlm> {
    let prompt = build_prompt(
        &key.doc_id(),
        visible_source,
        truncated,
        category,
        behaviour_context,
    );
    let system = "你是饥荒联机版(DST)的资深源码分析员,为中文维基产出结构化符号知识文档。\
必须只输出一个合法 JSON 对象;除 JSON 外不得输出任何说明文字或代码围栏。";

    let raw = config
        .complete_streaming(system, &prompt, |ev| match ev {
            LlmStreamEvent::FirstToken { elapsed_secs } => {
                reporter.log(format!("首个输出分片已到达({elapsed_secs}s)"));
            }
            LlmStreamEvent::Tick { chars } => {
                if chars == 0 {
                    reporter.log("仍在等待模型输出…".to_string());
                } else {
                    reporter.log(format!("流式接收中:{chars} 字符"));
                }
            }
            LlmStreamEvent::Done {
                chars,
                elapsed_secs,
            } => {
                reporter.log(format!("输出完成:{chars} 字符 / {elapsed_secs}s"));
            }
        })
        .await?;

    let raw_path = raw_dir.join(format!("{}__{}.json", key.doc_id(), prompt_rev));
    std::fs::write(&raw_path, &raw)?;
    reporter.log(format!("raw 已归档 {}", raw_path.display()));

    parse_doc_tolerant(&raw).map_err(|e| {
        Error::Llm(format!(
            "输出无法解析为 SymbolDoc(raw 已存 {}):{e}",
            raw_path.display()
        ))
    })
}

/// 容错解析:围栏剥离 → 最外层对象 → 尾逗号修复,逐候选 serde 反序列化。
fn parse_doc_tolerant(raw: &str) -> Result<SymbolDocLlm> {
    use crate::update::symbol_page::{excerpt, remove_trailing_commas, strip_code_fence};
    let trimmed = strip_code_fence(raw.trim());
    let sliced = match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(a), Some(b)) if a < b => Some(&trimmed[a..=b]),
        _ => None,
    };
    let fixed_all = remove_trailing_commas(trimmed);
    let candidates: Vec<String> = Vec::from_iter(
        [
            Some(trimmed.to_string()),
            sliced.map(str::to_string),
            Some(fixed_all),
            sliced.map(remove_trailing_commas),
        ]
        .into_iter()
        .flatten(),
    );
    for cand in &candidates {
        if let Ok(v) = serde_json::from_str::<SymbolDocLlm>(cand) {
            return Ok(v);
        }
    }
    Err(Error::Llm(excerpt(trimmed)))
}

// ---------------------------------------------------------------------------
// pass2(link-wiki):把组件能力与真实 wiki 语料对齐,产出带证据的 aspects;
// 空结果(no_wiki_mention)是合法且常见的负结果——「哪些忽略」本身就是构想 b
// 的答案之一。
// ---------------------------------------------------------------------------

/// 组件 → 变体 → 页面路由;按 fact 富裕度取前 `k` 页(信息量高优先)。
fn sample_pages_for_component(
    index: &IndexArtifact,
    view: &CorpusPageView,
    key: &SymbolRefKey,
    k: usize,
) -> Vec<i64> {
    let symbol = crate::update::SymbolRef::File {
        path: key.path.clone(),
    };
    let variants = crate::update::symbol_page::affected_variants(index, &symbol);
    let pageids = crate::update::symbol_page::variant_pages(view, &variants);
    if pageids.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(usize, i64)> = pageids
        .iter()
        .map(|pid| (view.facts.get(pid).map_or(0, Vec::len), *pid))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    ranked
        .into_iter()
        .take(k.max(1))
        .map(|(_, pid)| pid)
        .collect()
}

/// pass2 章节优先级：只把高/中优先级章节送进 LLM，低优先级（花絮/皮肤/Bug/画廊等）完全排除。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass2SectionTier {
    High,
    Medium,
    Ignore,
}

/// 高优先级章节关键词：标题包含任一关键词即视为 High。
const HIGH_SECTION_KEYWORDS: &[&str] = &[
    "行为",
    "战斗",
    "攻击",
    "技能",
    "互动",
    "掉落",
    "战利品",
    "获取",
    "制作",
    "用途",
    "生成",
    "进食",
    "食性",
    "繁殖",
    "孵化",
    "生长",
    "建造",
    "采集",
    "工作",
    "睡眠",
    "跟随",
    "雇佣",
    "交易",
    "献祭",
    "治疗",
    "光环",
    "仇恨",
    "群体",
    "耐久",
    "温度",
    "潮湿",
    "燃烧",
    "冰冻",
    "解冻",
    "腐败",
    "烹饪",
    "容器",
    "物品栏",
    "装备",
];
const IGNORE_SECTION_TITLES: &[&str] = &[
    "花絮",
    "皮肤",
    "皮肤套装",
    "画廊",
    "脚注",
    "Bug",
    "漏洞",
    "历史",
    "注释",
    "外部链接",
    "参考",
    "你知道吗",
    "轶事",
];

fn pass2_section_tier(kind: &str, title: Option<&str>) -> Pass2SectionTier {
    match kind {
        "intro" | "infobox" | "tab" => Pass2SectionTier::High,
        "section" => {
            let t = title.map(str::trim).unwrap_or("");
            if HIGH_SECTION_KEYWORDS.iter().any(|k| t.contains(k)) {
                Pass2SectionTier::High
            } else if IGNORE_SECTION_TITLES.iter().any(|k| t.contains(k)) {
                Pass2SectionTier::Ignore
            } else {
                Pass2SectionTier::Medium
            }
        }
        _ => Pass2SectionTier::Medium,
    }
}

/// 从区域文本提取前几行实质内容，跳过纯模板行/章节标题行。
fn first_content_lines(text: &str, max_lines: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("{{") && t.ends_with("}}") {
            continue;
        }
        if t.starts_with('=') {
            continue;
        }
        out.push(t.to_string());
        if out.len() >= max_lines {
            break;
        }
    }
    out
}

/// 在给定区域文本内按检索词提取命中行；行去重并限制条数。
fn hit_lines_in_text(text: &str, terms: &[String], max_lines: usize) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut lines: Vec<String> = Vec::new();
    for t in terms {
        let tl = t.to_lowercase();
        if tl.is_empty() {
            continue;
        }
        let mut from = 0usize;
        while let Some(pos) = lower[from..].find(&tl) {
            let abs = from + pos;
            let start = text
                .char_indices()
                .map(|(i, _)| i)
                .rev()
                .find(|&i| i <= abs)
                .unwrap_or(0);
            let line_start = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let rest = &text[line_start..];
            let line_end = rest
                .find('\n')
                .map(|i| line_start + i)
                .unwrap_or(text.len());
            let mut le = line_end;
            while le > line_start && !text.is_char_boundary(le) {
                le -= 1;
            }
            let line = text[line_start..le].trim();
            if !line.is_empty() && !lines.contains(&line.to_string()) {
                let mut cut = line.to_string();
                if cut.len() > 160 {
                    let mut e2 = 160;
                    while e2 > 0 && !cut.is_char_boundary(e2) {
                        e2 -= 1;
                    }
                    cut.truncate(e2);
                    cut.push('…');
                }
                lines.push(cut);
                if lines.len() >= max_lines {
                    return lines;
                }
            }
            from = abs + tl.len();
        }
    }
    lines
}

/// 单页摘录：有全文缓存时按章节优先级输出；无缓存时才退回旧的全局 grep 行。
#[allow(clippy::too_many_arguments)]
fn out_line(
    out: &mut String,
    prefab_of: &std::collections::HashMap<i64, String>,
    view: &CorpusPageView,
    cache: Option<&WikiTextCache>,
    terms: &[String],
    hit_lines: &std::collections::HashMap<i64, Vec<String>>,
    pageid: i64,
) {
    out.push_str(&format!("### 页面 {pageid}"));
    if let Some(pf) = prefab_of.get(&pageid) {
        out.push_str(&format!("(实体:{pf})"));
    }
    out.push('\n');
    let mut pushed = false;

    if let Some(cache) = cache {
        if let Some(text) = cache.pages.get(&pageid) {
            let regions = crate::corpus::segment::segment(pageid, text);
            let mut budget = 12usize;
            for region in regions {
                let tier = pass2_section_tier(region.kind, region.title.as_deref());
                if tier == Pass2SectionTier::Ignore {
                    continue;
                }
                let region_text = &text[region.start_byte..region.end_byte];
                let high = tier == Pass2SectionTier::High;
                let hits = hit_lines_in_text(region_text, terms, if high { 3 } else { 2 });
                let facts: Vec<_> = view
                    .facts
                    .get(&pageid)
                    .map(|fs| {
                        fs.iter()
                            .filter(|f| f.region_id == region.id)
                            .take(if high { 2 } else { 1 })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                // 高优先级 prose 区域额外给开头实质内容，避免只看到检索命中单句。
                let leads: Vec<String> = if high && matches!(region.kind, "intro" | "section") {
                    first_content_lines(region_text, 2)
                } else {
                    Vec::new()
                };
                if hits.is_empty() && facts.is_empty() && leads.is_empty() {
                    continue;
                }
                let label = region
                    .title
                    .clone()
                    .unwrap_or_else(|| region.kind.to_string());
                out.push_str(&format!("- 【{label}】\n"));
                for l in &leads {
                    out.push_str(&format!("  上下文:{}\n", l));
                    pushed = true;
                    budget = budget.saturating_sub(1);
                }
                for l in &hits {
                    out.push_str(&format!("  命中:{}\n", l));
                    pushed = true;
                    budget = budget.saturating_sub(1);
                }
                for f in facts {
                    out.push_str(&format!("  [{}] {}\n", f.fact_kind, f.snippet));
                    pushed = true;
                    budget = budget.saturating_sub(1);
                }
                if budget == 0 {
                    break;
                }
            }
        }
    }

    // 无全文缓存（理论不会发生在 pass2 开启时）才使用旧逻辑。
    if !pushed && cache.is_none() {
        if let Some(lines) = hit_lines.get(&pageid) {
            for l in lines {
                out.push_str(&format!("- 命中:{l}\n"));
            }
            pushed = true;
        }
        if !pushed {
            if let Some(facts) = view.facts.get(&pageid) {
                for f in facts.iter().take(6) {
                    out.push_str(&format!("- [{}] {}\n", f.region_id, f.snippet));
                }
                pushed = true;
            }
        }
    }

    if !pushed {
        out.push_str("- (无相关章节摘录)\n");
    }
    out.push('\n');
}

/// pass2 运行上下文:索引路由 + 语料视图 + 全文缓存 + 采样上限 + prompt_rev。
struct WikiContext<'a> {
    index: &'a IndexArtifact,
    view: &'a CorpusPageView,
    cache: Option<&'a WikiTextCache>,
    sample: usize,
    prompt_rev: &'static str,
    /// pass2 二次确认开关(A4)
    confirm_empty: bool,
    /// 本符号文件的命名常量(brain caps 数值语义,A5)
    constants: Vec<(String, f64)>,
}

/// 单次 pass2 LLM 调用 + 容错解析;失败由调用方决定重试/降级。
async fn generate_wiki_once(
    config: &LlmConfig,
    doc_id: &str,
    prompt: &str,
    raw_dir: &Path,
    reporter: &dyn Reporter,
    prompt_rev: &str,
) -> Result<WikiLinkLlm> {
    let system = "你是维基语料分析员,任务是在真实页面文本中检索组件能力的表达证据。只输出一个合法 JSON 对象,除 JSON 外不得输出任何其他文字。";
    let raw = config
        .complete_streaming(system, prompt, |ev| match ev {
            LlmStreamEvent::FirstToken { elapsed_secs } => {
                reporter.log(format!("pass2 首个分片({elapsed_secs}s)"));
            }
            LlmStreamEvent::Done {
                chars,
                elapsed_secs,
            } => {
                reporter.log(format!("pass2 输出完成:{chars} 字符 / {elapsed_secs}s"));
            }
            _ => {}
        })
        .await?;

    let raw_path = raw_dir.join(format!("{}__wiki__{}.json", doc_id, prompt_rev));
    std::fs::write(&raw_path, &raw)?;
    reporter.log(format!("pass2 raw 已归档 {}", raw_path.display()));

    parse_wiki_tolerant(&raw).map_err(|e| {
        Error::Llm(format!(
            "pass2 输出无法解析(raw 已存 {}):{e}",
            raw_path.display()
        ))
    })
}

/// 零命中检索词列表(大小写不敏感子串检索,与 pass2 采样同口径)。
fn zero_hit_terms(cache: &WikiTextCache, terms: &[String]) -> Vec<String> {
    terms
        .iter()
        .filter(|t| !t.trim().is_empty() && cache.score(std::slice::from_ref(*t), 1).is_empty())
        .cloned()
        .collect()
}

/// search_terms 质量杠杆:零命中 ≥2 且过半时,单次 LLM 改写并记录溯源。
async fn apply_search_terms_leverage(
    cache: &WikiTextCache,
    config: &LlmConfig,
    doc: &mut SymbolDoc,
    raw_dir: &Path,
    reporter: &dyn Reporter,
) {
    let misses = zero_hit_terms(cache, &doc.search_terms);
    let total = doc.search_terms.len();
    if total == 0 || misses.len() < 2 || misses.len() * 2 < total {
        return;
    }
    reporter.log(format!(
        "search_terms 零命中 {}/{} ,尝试单次二次修正",
        misses.len(),
        total
    ));
    let aspect_names: Vec<String> = doc
        .wiki
        .as_ref()
        .map(|w| w.aspects.iter().map(|a| a.aspect.clone()).collect())
        .unwrap_or_default();
    let prompt = format!(
        "以下是符号 `{name}` 的知识文档摘要、页面侧 aspects 与一组维基检索词。其中这些检索词在维基(饥荒联机版页面)全文检索中零命中:{misses:?}。

请改写一组检索词:5~10 个,中文玩家语言为主(可混英文),每词 2~6 字,优先使用维基页面实际使用的表述;未被点名的词若仍然有效可保留。不得输出英文 API 名或源码标识符。

只输出一个 JSON 对象:{{\"search_terms\":[\"…\"]}}

摘要:{summary}
aspects:{aspects}
现有检索词:{terms:?}",
        name = doc.reference.doc_id(),
        misses = misses,
        summary = doc.summary,
        aspects = aspect_names.join("、"),
        terms = doc.search_terms,
    );
    let system = "你是饥荒联机版维基的检索词优化助手。必须只输出一个合法 JSON 对象。";
    let raw = match config
        .complete_streaming(system, &prompt, |ev| match ev {
            LlmStreamEvent::FirstToken { elapsed_secs } => {
                reporter.log(format!("改写首片({elapsed_secs}s)"));
            }
            LlmStreamEvent::Tick { chars } => {
                let _ = chars;
            }
            LlmStreamEvent::Done {
                chars,
                elapsed_secs,
            } => {
                reporter.log(format!("改写完成:{chars} 字符 / {elapsed_secs}s"));
            }
        })
        .await
    {
        Ok(t) => t,
        Err(e) => {
            reporter.log(format!("search_terms 二次修正失败,保留原词:{e}"));
            return;
        }
    };
    let raw_path = raw_dir.join(format!(
        "{}__terms__{}.json",
        doc.reference.doc_id(),
        doc.prompt_rev
    ));
    let _ = std::fs::write(&raw_path, &raw);
    let Some(revised) = parse_search_terms_payload(&raw) else {
        reporter.log("search_terms 改写输出无法解析,保留原词".to_string());
        return;
    };
    if revised.is_empty() || revised.len() > 15 {
        reporter.log("search_terms 改写结果数量越界,保留原词".to_string());
        return;
    }
    let still = zero_hit_terms(cache, &revised);
    if still.len() > misses.len() {
        // 改写把仍然有效的词也换掉了,零命中不降反升 → 回退原词
        reporter.log(format!(
            "search_terms 改写更差({}/{} vs 原词 {} / {}),保留原词",
            still.len(),
            revised.len(),
            misses.len(),
            total
        ));
        doc.search_terms_note = Some(format!(
            "pass1 后零命中 {}/{},改写尝试更差已回退",
            misses.len(),
            total
        ));
        return;
    }
    reporter.log(format!(
        "search_terms 已修正:零命中 {} / {} → {} / {}",
        misses.len(),
        total,
        still.len(),
        revised.len()
    ));
    doc.search_terms = revised;
    doc.search_terms_note = Some(format!(
        "pass1 后零命中 {}/{},已二次修正(修正后 {}/{})",
        misses.len(),
        total,
        still.len(),
        doc.search_terms.len()
    ));
}

/// 容错解析改写输出:围栏剥离 → 最外层对象 → 尾逗号修复。
fn parse_search_terms_payload(raw: &str) -> Option<Vec<String>> {
    use crate::update::symbol_page::{remove_trailing_commas, strip_code_fence};
    let trimmed = strip_code_fence(raw.trim());
    let sliced = match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(a), Some(b)) if a < b => Some(&trimmed[a..=b]),
        _ => None,
    };
    let fixed_all = remove_trailing_commas(trimmed);
    let candidates: Vec<String> = Vec::from_iter(
        [
            Some(trimmed.to_string()),
            sliced.map(str::to_string),
            Some(fixed_all),
            sliced.map(remove_trailing_commas),
        ]
        .into_iter()
        .flatten(),
    );
    #[derive(serde::Deserialize)]
    struct Payload {
        #[serde(default, alias = "terms", alias = "searchTerms")]
        search_terms: Vec<String>,
    }
    for cand in &candidates {
        if let Ok(p) = serde_json::from_str::<Payload>(cand) {
            let terms: Vec<String> = p
                .search_terms
                .into_iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if !terms.is_empty() {
                return Some(terms);
            }
        }
    }
    None
}

/// brain pass2 caps(§9.1):裸 ctor 名对不上页面语言,用 invocation 的
/// 参数语义文本作锚点(依赖 1a behaviour 词典);tunables 叠加数值语义
/// (常量名=值,A5 改善判空对齐缺口)。
fn brain_caps(doc: &SymbolDoc, constants: &[(String, f64)]) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    for t in &doc.tunables {
        let value = constants
            .iter()
            .find(|(n, _)| n == t)
            .map(|(_, v)| trim_num(*v));
        match value {
            Some(v) => parts.push(format!("{t}={v}")),
            None => parts.push(t.clone()),
        }
    }
    for b in &doc.behaviour_invocations {
        let mut entry = b.ctor.clone();
        if !b.args_semantic.is_empty() {
            entry.push_str(": ");
            entry.push_str(&b.args_semantic.join("; "));
        }
        if let Some(ctx) = &b.context {
            entry.push_str(&format!("({})", ctx));
        }
        parts.push(entry);
    }
    parts
}

/// 本符号源码的命名常量(brain caps 数值语义用)。
fn source_constants(source: &[u8]) -> Vec<(String, f64)> {
    extract_named_constants(&String::from_utf8_lossy(source))
}

async fn enrich_with_wiki(
    config: &LlmConfig,
    doc: &mut SymbolDoc,
    ctx: WikiContext<'_>,
    raw_dir: &Path,
    reporter: &dyn Reporter,
) -> Result<()> {
    let key = doc.reference.clone();
    reporter.log("pass2 语料归因(link-wiki)".to_string());

    // 主采样:全文检索命中(组件感知);兜底:路由链 fact 排序。
    let mut hit_lines: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    let grep_pages: Vec<i64> = match (ctx.cache, &doc.search_terms) {
        (Some(cache), terms) if !terms.is_empty() => {
            let mut scored = cache.score(terms, 3);
            scored.truncate(ctx.sample);
            let ids: Vec<i64> = scored.iter().map(|(_, pid, _)| *pid).collect();
            for (_, pid, lines) in &scored {
                hit_lines.insert(*pid, lines.clone());
            }
            if !ids.is_empty() {
                reporter.log(format!("全文检索按命中数取前 {} 页", ids.len()));
            }
            ids
        }
        _ => Vec::new(),
    };
    let routed = sample_pages_for_component(ctx.index, ctx.view, &key, ctx.sample);
    let mut pageids: Vec<i64> = Vec::new();
    for pid in &grep_pages {
        if !pageids.contains(pid) {
            pageids.push(*pid);
        }
    }
    for pid in &routed {
        if pageids.len() >= ctx.sample {
            break;
        }
        if !pageids.contains(pid) {
            pageids.push(*pid);
        }
    }
    reporter.log(format!(
        "采样 {} 页(检索命中 {} / 路由兜底 {})",
        pageids.len(),
        grep_pages.len(),
        routed.len()
    ));
    if pageids.is_empty() {
        doc.wiki = Some(WikiSection {
            scanned_pageids: Vec::new(),
            aspects: Vec::new(),
            no_evidence_reason: Some("未路由到任何页面,且全文检索零命中".to_string()),
            coverage_status: "no_wiki_mention".to_string(),
        });
        return Ok(());
    }

    // pageid → prefab(为模型提供实体名上下文)
    let mut prefab_of: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for (prefab, ids) in &ctx.view.pages {
        for pid in ids {
            prefab_of.entry(*pid).or_insert_with(|| prefab.clone());
        }
    }
    let mut excerpts = String::new();
    for pid in &pageids {
        out_line(
            &mut excerpts,
            &prefab_of,
            ctx.view,
            ctx.cache,
            &doc.search_terms,
            &hit_lines,
            *pid,
        );
    }
    let caps: Vec<String> = match doc.category.as_str() {
        "behaviour" => {
            let mut parts: Vec<String> = Vec::new();
            parts.extend(doc.ctor_params.iter().map(|p| p.name.clone()));
            parts.extend(doc.effects.clone());
            parts
        }
        "brain" => brain_caps(doc, &ctx.constants),
        "stategraph" => {
            // SG 的 api 基本为空,裸状态名也不是页面语言;
            // 用 state_notes 的玩家语义作锚点(受击硬直/惊吓/变身等)。
            doc.state_notes
                .iter()
                .map(|sn| format!("{}: {}", sn.state, sn.note))
                .collect()
        }
        _ => doc.api.iter().map(|a| a.name.clone()).collect(),
    };

    // AutoInfobox 冷数据组件:注入字段级轻量提示,避免把“自动渲染导致正文无文本”
    // 误判为页面不讨论该组件(仅 component 线注入 auto_maintained,其他类别为空)。
    let auto_hint = doc
        .auto_maintained
        .as_ref()
        .map(|info| format!("{}\n", info.pass2_hint()))
        .unwrap_or_default();

    let prompt = format!(
        "下面是 wiki 语料中与 `{name}` 相关联的实体页面摘录,以及该符号具备的代码能力/行为语义清单。

任务:在页面文本中寻找这些能力/行为的玩家语言表达。若某能力被页面以任何方式体现(数值、行为描述、策略段落),归入 aspects 并给出证据页;若扫描后认为页面根本不讨论该符号的能力,aspects 返回空数组并在 no_evidence_reason 写明判断依据。

{auto_hint}
规则:
1. aspect 用中文短语概括页面侧的写作方面(如“生命值数值标注”“死亡掉落联动”“仇恨距离描述”),不要照抄 API/常量名。
2. evidence.pageid 必须来自下方给出的页面;quote 必须逐字摘自页面原文(保留原有标点与模板标记,可截取片段但不得改写、概括或拼接不同位置),≤40 字,可省略。
3. 不得凭空发明页面中不存在的能力表达;宁空勿造。
4. 只输出一个 JSON 对象,形如:{{\"aspects\":[{{\"aspect\":\"…\",\"evidence\":[{{\"pageid\":123,\"quote\":\"…\"}}]}}],\"no_evidence_reason\":null}}

代码能力/行为语义清单:{caps}

页面摘录:
{excerpts}",
        name = key.doc_id(),
        auto_hint = auto_hint,
        caps = caps.join(", "),
        excerpts = excerpts,
    );

    let mut wiki_outcome: Option<Result<WikiLinkLlm>> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        if attempt > 1 {
            reporter.log("pass2 重试第 2 次…".to_string());
        }
        match generate_wiki_once(
            config,
            &key.doc_id(),
            &prompt,
            raw_dir,
            reporter,
            ctx.prompt_rev,
        )
        .await
        {
            Ok(llm) => {
                wiki_outcome = Some(Ok(llm));
                break;
            }
            Err(e) => wiki_outcome = Some(Err(e)),
        }
    }
    let llm = wiki_outcome.expect("at least one attempt")?;

    // 证据页必须 ∈ 采样集合;过滤后无证据的 aspect 整条丢弃(宁空勿造)。
    let allowed: HashSet<i64> = pageids.iter().copied().collect();
    let mut aspects: Vec<WikiAspect> = llm
        .aspects
        .into_iter()
        .map(|mut a| {
            a.evidence
                .retain(|e: &WikiEvidence| allowed.contains(&e.pageid));
            a
        })
        .filter(|a| !a.evidence.is_empty())
        .collect();
    aspects.sort_by(|a, b| a.aspect.cmp(&b.aspect));

    // A4 二次确认(可选防过严):采样充分但判空时,以更全面标准复查一次。
    // 仍空则维持判空(宁空勿造不变)。
    if ctx.confirm_empty && pageids.len() >= 3 && aspects.is_empty() {
        reporter.log("二次确认:采样 ≥3 页但判空,追加一次复查".to_string());
        let confirm_prompt = format!(
            "{prompt}\n\n注意:上一次判定认为页面未表达该符号的能力。请以更全面的标准复查:若页面确以任何方式表达这些能力(包括数值、行为描述、策略段落、信息框参数),输出对应 aspects(证据要求不变);若确实没有,再次输出空数组并给出 no_evidence_reason。"
        );
        if let Ok(llm2) = generate_wiki_once(
            config,
            &format!("{}__confirm", key.doc_id()),
            &confirm_prompt,
            raw_dir,
            reporter,
            ctx.prompt_rev,
        )
        .await
        {
            let mut aspects2: Vec<WikiAspect> = llm2
                .aspects
                .into_iter()
                .map(|mut a| {
                    a.evidence
                        .retain(|e: &WikiEvidence| allowed.contains(&e.pageid));
                    a
                })
                .filter(|a| !a.evidence.is_empty())
                .collect();
            aspects2.sort_by(|a, b| a.aspect.cmp(&b.aspect));
            if !aspects2.is_empty() {
                reporter.log(format!("二次确认翻案:发现 {} 条 aspects", aspects2.len()));
                let coverage_status = match aspects2.len() {
                    0 => unreachable!(),
                    1 | 2 => "partial",
                    _ => "well_documented",
                };
                doc.wiki = Some(WikiSection {
                    scanned_pageids: pageids.clone(),
                    aspects: aspects2,
                    no_evidence_reason: None,
                    coverage_status: coverage_status.to_string(),
                });
                return Ok(());
            }
        }
    }

    let coverage_status = match aspects.len() {
        0 => "no_wiki_mention",
        1 | 2 => "partial",
        _ => "well_documented",
    };
    let no_evidence_reason = if aspects.is_empty() {
        Some(llm.no_evidence_reason.unwrap_or_else(|| {
            format!(
                "扫描了 {} 个关联页面,未发现该组件能力的玩家语言表达",
                pageids.len()
            )
        }))
    } else {
        None
    };

    reporter.log(format!(
        "pass2 结果:aspects {} 条(采样 {} 页)→ {coverage_status}",
        aspects.len(),
        pageids.len(),
    ));

    doc.wiki = Some(WikiSection {
        scanned_pageids: pageids,
        aspects,
        no_evidence_reason,
        coverage_status: coverage_status.to_string(),
    });
    Ok(())
}

fn parse_wiki_tolerant(raw: &str) -> Result<WikiLinkLlm> {
    use crate::update::symbol_page::{excerpt, remove_trailing_commas, strip_code_fence};
    let trimmed = strip_code_fence(raw.trim());
    let sliced = match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(a), Some(b)) if a < b => Some(&trimmed[a..=b]),
        _ => None,
    };
    let candidates: Vec<String> = Vec::from_iter(
        [
            Some(trimmed.to_string()),
            sliced.map(str::to_string),
            Some(remove_trailing_commas(trimmed)),
            sliced.map(remove_trailing_commas),
        ]
        .into_iter()
        .flatten(),
    );
    for cand in &candidates {
        if let Ok(v) = serde_json::from_str::<WikiLinkLlm>(cand) {
            return Ok(v);
        }
    }
    Err(Error::Llm(excerpt(trimmed)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::index::edges::BehaviourCallRecord;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kn-scan-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn zero_hit_terms_filters_against_corpus() {
        let dir = temp_dir("terms-corpus");
        std::fs::create_dir_all(dir.join("pages")).unwrap();
        std::fs::write(
            dir.join("meta.jsonl"),
            r#"{"pageid":100,"title":"猎犬","touched":null,"len":10,"redirect":false,"rev_sha1":null,"categories":[],"game_class":"dst","class_signals":[],"class_confidence":"high"}
"#,
        )
        .unwrap();
        std::fs::write(dir.join("pages/100.wikitext"), "猎犬会主动追击玩家。").unwrap();
        let cache = WikiTextCache::load(&dir).unwrap();
        let misses = zero_hit_terms(&cache, &["追击".to_string(), "不存在的词".to_string()]);
        assert_eq!(misses, vec!["不存在的词".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_search_terms_payload_tolerant() {
        let bare = r#"{"search_terms":["掉落","锤子敲碎"]}"#;
        assert_eq!(
            parse_search_terms_payload(bare),
            Some(vec!["掉落".to_string(), "锤子敲碎".to_string()])
        );
        let fenced = "```json\n{\"terms\": [\"游荡\", ]}\n```";
        assert_eq!(
            parse_search_terms_payload(fenced),
            Some(vec!["游荡".to_string()])
        );
        assert_eq!(parse_search_terms_payload("不是 JSON"), None);
    }

    #[test]
    fn pick_behaviours_lists_all_files_and_prioritizes_called() {
        let root = temp_dir("behaviours");
        std::fs::create_dir_all(root.join("behaviours")).unwrap();
        std::fs::write(
            root.join("behaviours/wander.lua"),
            "Wander = Class(BehaviourNode, function(self, inst) end)",
        )
        .unwrap();
        std::fs::write(
            root.join("behaviours/approach.lua"),
            "Approach = Class(BehaviourNode, function(self, inst) end)",
        )
        .unwrap();

        let mut index = IndexArtifact::default();
        index.behaviour_calls.push(BehaviourCallRecord {
            brain_file: "brains/houndbrain.lua".to_string(),
            ctor: "Wander".to_string(),
            line: 1,
            args: Vec::new(),
            prefab_variants: vec!["prefabs/hound.lua#hound".to_string()],
        });

        let picked = pick_behaviours(&root, &index, 10);
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].0.kind, "behaviour");
        assert_eq!(picked[0].0.path, "behaviours/wander.lua");
        assert_eq!(picked[1].0.path, "behaviours/approach.lua");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pick_brains_excludes_shared_helpers() {
        let root = temp_dir("brains");
        std::fs::create_dir_all(root.join("brains")).unwrap();
        std::fs::write(
            root.join("brains/houndbrain.lua"),
            "local HoundBrain = Class(Brain, function(self, inst) end)",
        )
        .unwrap();
        std::fs::write(
            root.join("brains/braincommon.lua"),
            "local BrainCommon = { PanicTrigger = function() end }",
        )
        .unwrap();

        let mut index = IndexArtifact::default();
        index.reverse.insert(
            "brains/houndbrain.lua".to_string(),
            vec!["prefabs/hound.lua#hound".to_string()],
        );
        index
            .reverse
            .insert("brains/braincommon.lua".to_string(), Vec::new());

        let picked = pick_brains(&root, &index, 10);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].0.kind, "brain");
        assert_eq!(picked[0].0.path, "brains/houndbrain.lua");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pick_brains_covers_dynamically_bound_and_pick_names_filters() {
        let root = temp_dir("brains-dyn");
        std::fs::create_dir_all(root.join("brains")).unwrap();
        std::fs::write(
            root.join("brains/houndbrain.lua"),
            "local HoundBrain = Class(Brain, function(self, inst) end)",
        )
        .unwrap();
        // spiderbrain 无反向边(动态 SetBrain),也应入选
        std::fs::write(
            root.join("brains/spiderbrain.lua"),
            "local SpiderBrain = Class(Brain, function(self, inst) end)",
        )
        .unwrap();

        let mut index = IndexArtifact::default();
        index.reverse.insert(
            "brains/houndbrain.lua".to_string(),
            vec![
                "prefabs/hound.lua#hound".to_string(),
                "prefabs/hound.lua#icehound".to_string(),
            ],
        );

        let picked = pick_brains(&root, &index, 10);
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].0.path, "brains/houndbrain.lua"); // 有引用变体,排前
        assert_eq!(picked[1].0.path, "brains/spiderbrain.lua");

        // pick_names:绕过排序/limit 截断,精确选取目标
        let picked = pick_symbols(
            &root,
            &index,
            "brain",
            1,
            Some(["spiderbrain".to_string()].as_slice()),
        );
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].0.path, "brains/spiderbrain.lua");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pick_stategraphs_excludes_player_and_shared_files() {
        let root = temp_dir("stategraphs");
        std::fs::create_dir_all(root.join("stategraphs")).unwrap();
        let sg = "return StateGraph(\"hound\", states, events, \"idle\")";
        std::fs::write(root.join("stategraphs/SGhound.lua"), sg).unwrap();
        std::fs::write(root.join("stategraphs/SGwilson.lua"), sg).unwrap();
        std::fs::write(root.join("stategraphs/SGdeerbrain_client.lua"), sg).unwrap();
        std::fs::write(
            root.join("stategraphs/commonstates.lua"),
            "local States = {}",
        )
        .unwrap();
        // 无 StateGraph( 标记的杂项文件
        std::fs::write(
            root.join("stategraphs/SGhelpers.lua"),
            "local Helper = function() end",
        )
        .unwrap();

        let mut index = IndexArtifact::default();
        index.reverse.insert(
            "stategraphs/SGhound.lua".to_string(),
            vec![
                "prefabs/hound.lua#hound".to_string(),
                "prefabs/hound.lua#icehound".to_string(),
            ],
        );

        let picked = pick_stategraphs(&root, &index, 10);
        assert_eq!(picked.len(), 1, "只应剩下 SGhound");
        assert_eq!(picked[0].0.path, "stategraphs/SGhound.lua");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A5:brain caps 把 tunables 渲染为 NAME=值(源码常量匹配时)
    #[test]
    fn brain_caps_render_tunable_values() {
        let llm_json = r#"{"category":"brain","display_name":"t","summary":"s","tunables":["SEE_DIST","NO_VALUE_CONST"],"behaviour_invocations":[{"ctor":"ChaseAndAttack","args_semantic":["追击最长 100 秒"]}],"search_terms":[],"gameplay_tags":[]}"#;
        let llm: SymbolDocLlm = serde_json::from_str(llm_json).unwrap();
        let key = SymbolRefKey {
            kind: "brain".into(),
            path: "brains/tbrain.lua".into(),
        };
        let doc = assemble(&llm, &key, "sha", 1, None, "m", "p5-brain");
        let caps = brain_caps(&doc, &[("SEE_DIST".into(), 30.0)]);
        assert!(
            caps.contains(&"SEE_DIST=30".to_string()),
            "有值常量应带 =值"
        );
        assert!(
            caps.contains(&"NO_VALUE_CONST".to_string()),
            "无值常量保持原名"
        );
        assert!(
            caps.iter()
                .any(|c| c.starts_with("ChaseAndAttack: 追击最长 100 秒")),
            "invocation 语义文本保留"
        );
    }

    #[test]
    fn build_prompt_switches_by_category() {
        let brain = build_prompt("brain__houndbrain", "src", false, "brain", None);
        assert!(brain.contains("大脑"));
        assert!(brain.contains("behaviour_invocations"));
        assert!(brain.contains("context_branches"));

        let behaviour = build_prompt("behaviour__wander", "src", false, "behaviour", None);
        assert!(behaviour.contains("行为节点"));
        assert!(behaviour.contains("ctor_params"));
        assert!(behaviour.contains("success_fail_conditions"));

        let component = build_prompt("component__health", "src", false, "component", None);
        assert!(component.contains("组件"));
        assert!(component.contains("api"));

        let stategraph = build_prompt("stategraph__SGhound", "src", false, "stategraph", None);
        assert!(stategraph.contains("状态图"));
        assert!(stategraph.contains("state_notes"));
    }

    #[test]
    fn pick_symbols_falls_back_to_component() {
        // 未识别类别应安全回退到 component 选择逻辑(目录扫描,无反向边也可入选)。
        let root = temp_dir("fallback");
        std::fs::create_dir_all(root.join("components")).unwrap();
        std::fs::write(root.join("components/health.lua"), "Health = Class").unwrap();
        let index = IndexArtifact::default();
        let picked = pick_symbols(&root, &index, "unknown", 10, None);
        assert_eq!(picked.len(), 1); // 目录扫描覆盖未被引用的组件
        assert_eq!(picked[0].0.path, "components/health.lua");
        let _ = std::fs::remove_dir_all(&root);
    }

    fn write_component_doc(root: &Path, name: &str, auto_maintained: bool) {
        let symbols = root.join("symbols");
        std::fs::create_dir_all(&symbols).unwrap();
        let auto = if auto_maintained {
            r#","auto_maintained":{"fields":["旧字段"],"code_fields":["health.max"],"overridable_fields":[]}"#
        } else {
            ""
        };
        let body = format!(
            r#"{{
                "schema_version": 2,
                "prompt_rev": "p4",
                "reference": {{"kind":"component","path":"components/{name}.lua"}},
                "category": "component",
                "display_name": "{name}",
                "summary": "测试文档",
                "api": [],
                "api_note": "纯数据组件,无公开方法"{auto},
                "provenance": {{"source_sha256":"abc","source_bytes":1,"model":"m","generated_at_ms":1}}
            }}"#
        );
        let key = SymbolRefKey {
            kind: "component".to_string(),
            path: format!("components/{name}.lua"),
        };
        write_atomic(&doc_path(root, &key), &body).unwrap();
    }

    #[test]
    fn refresh_auto_updates_only_changed_docs() {
        let root = temp_dir("refresh-auto");
        std::fs::create_dir_all(root.join("symbols")).unwrap();
        std::fs::write(
            root.join("auto_infobox.json"),
            r#"{
                "schema_version": 2,
                "source": "Module:AutoInfobox",
                "components": {
                    "health": {
                        "fields": ["生命值"],
                        "code_fields": ["health"],
                        "overridable_fields": ["生命值"]
                    }
                }
            }"#,
        )
        .unwrap();
        write_component_doc(&root, "health", true); // 带旧 auto_maintained → 应更新
        write_component_doc(&root, "bait", false); // 冷数据无 bait → 保持 None

        let idx = AutoInfoboxIndex::load(&root).unwrap();
        let reporter = crate::platform::progress::CaptureReporter::new(true);
        let out = refresh_auto_maintained(&root, &idx, &reporter).unwrap();
        assert_eq!(out["updated"], 1);
        assert_eq!(out["unchanged"], 1);

        let key = SymbolRefKey {
            kind: "component".to_string(),
            path: "components/health.lua".to_string(),
        };
        let doc = load_doc(&doc_path(&root, &key)).unwrap().unwrap();
        let info = doc.auto_maintained.expect("health 应注入冷数据");
        assert_eq!(info.fields, vec!["生命值"]);
        assert_eq!(info.code_fields, vec!["health"]);

        let bait = load_doc(&doc_path(
            &root,
            &SymbolRefKey {
                kind: "component".to_string(),
                path: "components/bait.lua".to_string(),
            },
        ))
        .unwrap()
        .unwrap();
        assert!(bait.auto_maintained.is_none());

        // 二次执行:全部无变化
        let out = refresh_auto_maintained(&root, &idx, &reporter).unwrap();
        assert_eq!(out["updated"], 0);
        assert_eq!(out["unchanged"], 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
