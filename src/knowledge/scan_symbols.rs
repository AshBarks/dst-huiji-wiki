//! M1 / 构想 a:`knowledge scan-symbols` — LLM 逐个阅读主要 component 源码,
//! 产出 SymbolDoc 基础知识文档(docs/KNOWLEDGE_PIPELINE.md §3/§5)。
//!
//! 纪律:流式进度(Reporter)、raw 归档、失败重试 1 次、sha 增量跳过、原子落盘。

use crate::error::{Error, Result};
use crate::knowledge::store::{doc_path, is_fresh, load_doc, sha256_hex, write_atomic};
use crate::knowledge::types::{
    assemble, SymbolDoc, SymbolDocLlm, SymbolRefKey, WikiAspect, WikiEvidence, WikiLinkLlm,
    WikiSection, PROMPT_REV, SCHEMA_VERSION,
};
use crate::llm::{LlmConfig, LlmStreamEvent};
use crate::service::Reporter;
use crate::update::CorpusPageView;
use crate::update::{build_atlas_from_dir, top_symbols, IndexArtifact};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 单文档源码输入上限;超出截断并写入 truncation_note(§5)。
const SOURCE_BYTES_CAP: usize = 48 * 1024;
/// 每文档失败后的总尝试次数(含首次,即重试 1 次)。
const MAX_ATTEMPTS: u32 = 2;

/// 语料全文缓存:pages/<pageid>.wikitext → 全文(每 run 加载一次)。
pub struct WikiTextCache {
    pages: std::collections::HashMap<i64, String>,
}

impl WikiTextCache {
    pub fn load(corpus_root: &Path) -> Result<Self> {
        let dir = corpus_root.join("pages");
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
    /// wiki 语料 host 根目录;提供则启用 pass2 语料归因(link-wiki)
    pub corpus: Option<String>,
    /// pass2 每组件采样的页面数(默认 8,引用/fact 富裕页优先)
    pub sample_pages: usize,
    pub limit: usize,
    pub force: bool,
}

/// P1 选择:top_symbols(变体引用数降序)过滤出 component 文件型符号,
/// 附带磁盘源码路径与字节数。
fn pick_components(
    scripts_root: &Path,
    index: &IndexArtifact,
    limit: usize,
) -> Vec<(SymbolRefKey, PathBuf, usize)> {
    top_symbols(index, limit.max(64) * 4)
        .into_iter()
        .filter_map(|sym| match sym {
            crate::update::SymbolRef::File { path } if path.starts_with("components/") => {
                let full = scripts_root.join(&path);
                let bytes = std::fs::metadata(&full).ok()?.len() as usize;
                Some((
                    SymbolRefKey {
                        kind: "component".to_string(),
                        path,
                    },
                    full,
                    bytes,
                ))
            }
            _ => None,
        })
        .take(limit)
        .collect()
}

pub async fn run_scan_symbols(
    params: &ScanSymbolsParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let scripts_root = Path::new(&params.scripts_root);
    let knowledge_root = Path::new(&params.knowledge_root);

    reporter.stage("构建代码关联索引");
    let atlas = build_atlas_from_dir(scripts_root)?;
    let build_id = atlas.build_id.clone();

    reporter.stage("选择 component 符号");
    let picked = pick_components(scripts_root, &atlas.index, params.limit);
    reporter.log(format!(
        "候选 {} 个 component(按关联变体数降序;force={})",
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
    let mut wiki_written = 0u32;

    let (mut written, mut skipped, mut failed) = (0u32, 0u32, 0u32);
    for (idx, (key, src_path, src_bytes)) in picked.iter().enumerate() {
        reporter.stage(&format!(
            "[{}/{}] {}({}B)",
            idx + 1,
            picked.len(),
            key.doc_id(),
            src_bytes
        ));
        let doc_path = doc_path(knowledge_root, key);

        let source = std::fs::read(src_path)?;
        let sha = sha256_hex(&source);
        if !params.force {
            if let Some(existing) = load_doc(&doc_path)? {
                if is_fresh(&existing, &sha, PROMPT_REV) {
                    match (&existing.wiki, &params.corpus) {
                        (Some(_), _) | (None, None) => {
                            reporter.log("源码与 prompt_rev 未变,跳过".to_string());
                            skipped += 1;
                            continue;
                        }
                        (None, Some(_)) => {
                            // 文档已是 v2 但缺 wiki 节点 → 只补 pass2
                            reporter.log("代码文档新鲜,仅补 pass2 语料归因".to_string());
                            let mut doc = existing;
                            enrich_with_wiki(
                                &config,
                                &mut doc,
                                WikiContext {
                                    index: &atlas.index,
                                    view: view.as_ref().expect("corpus loaded"),
                                    cache: cache.as_ref(),
                                    sample,
                                },
                                &raw_dir,
                                reporter,
                            )
                            .await?;
                            write_atomic(&doc_path, &serde_json::to_string_pretty(&doc)?)?;
                            wiki_written += 1;
                            continue;
                        }
                    }
                }
                reporter.log("源码或 prompt_rev 已变化,重扫".to_string());
            }
        }

        let (visible, truncated) = if source.len() > SOURCE_BYTES_CAP {
            (&source[..SOURCE_BYTES_CAP], true)
        } else {
            (&source[..], false)
        };
        let visible = String::from_utf8_lossy(visible).to_string();

        let mut outcome: Option<Result<SymbolDocLlm>> = None;
        for attempt in 1..=MAX_ATTEMPTS {
            if attempt > 1 {
                reporter.log("重试第 2 次…".to_string());
            }
            match generate_one(&config, key, &visible, truncated, &raw_dir, reporter).await {
                Ok(doc) => {
                    outcome = Some(Ok(doc));
                    break;
                }
                Err(e) => outcome = Some(Err(e)),
            }
        }

        match outcome.expect("at least one attempt") {
            Ok(llm_doc) => {
                if let Err(e) = llm_doc.validate() {
                    failed += 1;
                    reporter.log(format!("schema 校验失败,已记为失败:{e}"));
                    continue;
                }
                let doc = assemble(
                    &llm_doc,
                    key,
                    &sha,
                    source.len(),
                    build_id.clone(),
                    &config.model,
                );
                let body = serde_json::to_string_pretty(&doc)?;
                write_atomic(&doc_path, &body)?;
                let mut doc = doc;
                if let Some(v) = &view {
                    enrich_with_wiki(
                        &config,
                        &mut doc,
                        WikiContext {
                            index: &atlas.index,
                            view: v,
                            cache: cache.as_ref(),
                            sample,
                        },
                        &raw_dir,
                        reporter,
                    )
                    .await?;
                    write_atomic(&doc_path, &serde_json::to_string_pretty(&doc)?)?;
                    wiki_written += 1;
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
                written += 1;
            }
            Err(e) => {
                failed += 1;
                reporter.log(format!("生成失败(raw 已归档):{e}"));
            }
        }
    }

    Ok(serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "prompt_rev": PROMPT_REV,
        "picked": picked.len(),
        "written": written,
        "skipped_fresh": skipped,
        "failed": failed,
        "wiki_enriched": wiki_written,
    }))
}

fn build_prompt(display: &str, source: &str, truncated: bool) -> String {
    let head = if truncated {
        "【注意】源码因长度被截断,仅基于可见部分标注,并在 truncation_note 中说明。\n\n"
    } else {
        ""
    };
    format!(
        "{head}以下是饥荒联机版(DST)组件 `{display}` 的 Lua 源码。\
请通读后产出该组件的知识文档 JSON。

要求:
1. 全部用中文;不得编造源码中不存在的内容。
2. summary:2~4 句,说明组件管理的数据与参与的核心玩法。
3. api:覆盖全部对外(实体/其他系统)暴露的方法;effect 写清行为与副作用。不要猜测页面该如何撰写——文档只承载代码可证实的事实。
4. events_published / events_listened / netvars / tunables:严格取自源码标识符,可为空数组。
5. gameplay_tags:1~4 个玩法标签(如 生存/战斗/建造/装饰)。
6. search_terms:5~10 个用于在维基全文中检索该组件能力表达的词汇,以中文玩家语言为主(可混英文),每词 2~6 字;不要照抄 API 名。
7. 只输出一个 JSON 对象,字段名与上述一致;除 api 确无方法外不得为空数组。

源码:
```lua
{source}
```"
    )
}

async fn generate_one(
    config: &LlmConfig,
    key: &SymbolRefKey,
    visible_source: &str,
    truncated: bool,
    raw_dir: &Path,
    reporter: &dyn Reporter,
) -> Result<SymbolDocLlm> {
    let prompt = build_prompt(&key.doc_id(), visible_source, truncated);
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

    let raw_path = raw_dir.join(format!("{}__{}.json", key.doc_id(), PROMPT_REV));
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

/// 单页摘录行:grep 命中行优先,否则 facts 摘要。
fn out_line(
    out: &mut String,
    prefab_of: &std::collections::HashMap<i64, String>,
    view: &CorpusPageView,
    hit_lines: &std::collections::HashMap<i64, Vec<String>>,
    pageid: i64,
) {
    out.push_str(&format!("### 页面 {pageid}"));
    if let Some(pf) = prefab_of.get(&pageid) {
        out.push_str(&format!("(实体:{pf})"));
    }
    out.push('\n');
    let mut pushed = false;
    if let Some(lines) = hit_lines.get(&pageid) {
        for l in lines {
            out.push_str(&format!("- 命中:{l}\n"));
            pushed = true;
        }
    }
    if !pushed {
        if let Some(facts) = view.facts.get(&pageid) {
            for f in facts.iter().take(6) {
                out.push_str(&format!("- [{}] {}\n", f.region_id, f.snippet));
            }
            pushed = true;
        }
    }
    if !pushed {
        out.push_str("- (无摘要行)\n");
    }
    out.push('\n');
}

/// pass2 运行上下文:索引路由 + 语料视图 + 全文缓存 + 采样上限。
struct WikiContext<'a> {
    index: &'a IndexArtifact,
    view: &'a CorpusPageView,
    cache: Option<&'a WikiTextCache>,
    sample: usize,
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
        out_line(&mut excerpts, &prefab_of, ctx.view, &hit_lines, *pid);
    }
    let caps: Vec<String> = doc.api.iter().map(|a| a.name.clone()).collect();

    let prompt = format!(
        "下面是 wiki 语料中与组件 `{name}` 相关联的实体页面摘录,以及该组件具备的代码能力清单。

任务:在页面文本中寻找这些能力的玩家语言表达。若某能力被页面以任何方式体现(数值、行为描述、策略段落),归入 aspects 并给出证据页;若扫描后认为页面根本不讨论该组件的能力,aspects 返回空数组并在 no_evidence_reason 写明判断依据。

规则:
1. aspect 用中文短语概括页面侧的写作方面(如“生命值数值标注”“死亡掉落联动”),不要照抄 API 名。
2. evidence.pageid 必须来自下方给出的页面;quote 摘录原文短句(≤40 字),可省略。
3. 不得凭空发明页面中不存在的能力表达;宁空勿造。
4. 只输出一个 JSON 对象,形如:{{\"aspects\":[{{\"aspect\":\"…\",\"evidence\":[{{\"pageid\":123,\"quote\":\"…\"}}]}}],\"no_evidence_reason\":null}}

组件能力清单:{caps}

页面摘录:
{excerpts}",
        name = key.doc_id(),
        caps = caps.join(", "),
        excerpts = excerpts,
    );

    let system = "你是维基语料分析员,任务是在真实页面文本中检索组件能力的表达证据。只输出一个合法 JSON 对象,除 JSON 外不得输出任何其他文字。";

    let raw = config
        .complete_streaming(system, &prompt, |ev| match ev {
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

    let raw_path = raw_dir.join(format!("{}__wiki__{}.json", key.doc_id(), PROMPT_REV));
    std::fs::write(&raw_path, &raw)?;
    reporter.log(format!("pass2 raw 已归档 {}", raw_path.display()));

    let llm = parse_wiki_tolerant(&raw).map_err(|e| {
        Error::Llm(format!(
            "pass2 输出无法解析(raw 已存 {}):{e}",
            raw_path.display()
        ))
    })?;

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
