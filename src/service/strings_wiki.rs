//! `maintain-strings`：解析游戏 `languages/strings.pot`（EN 原文）与
//! `languages/chinese_s.po`（CN 译文），生成 `模块:<V> Strings <LANG> <NN>`
//! 桶页与 `Data:<V>_Strings_Index.json`。
//!
//! - 只读（`WriteMode::DryRun`）：只写本地产物 + 现网逐桶对比报告。
//! - 写入（默认交互确认，`--yes` 免确认）：语义对比后只写变化的桶页，
//!   最后写索引。桶页彼此独立，失败页不阻塞其余页；但只要有失败页或
//!   canary 截断，就**不更新索引**，避免索引指向未更新的桶。
//! - `limit > 0` 为 canary：最多写 N 个桶页且不写索引。
//!
//! key 变换规则见 [`crate::models::strings`]：去掉 `STRINGS.` 前缀并把路径段
//! 规范化为 ASCII 大写，`STRINGS.CHARACTERS.<SPEAKER>.<REST>` 合并为
//! `CHARACTERS.<REST>` 角色表（`GENERIC` → `wilson`，与 `模块:Strings` 的
//! `resolve_character` 一致）。

use super::dataset::read_game_file;
use crate::error::{Error, Result};
use crate::models::{
    build_language_map, diff_values, plan_equal_count, plan_with_boundaries, render_module,
    validate_plan, BucketPlan, IndexEntry, MapDiff, StringValue, StringsIndexFile,
};
use crate::parser::{parse_strings_module, PoParser};
use crate::platform::progress::Reporter;
use crate::platform::progress::{decide_write, WriteDecision, WriteMode};
use crate::wiki::WikiClient;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub const DEFAULT_BUCKETS: usize = 100;
const EN_LANGUAGE_FILE: &str = "languages/strings.pot";
const CN_LANGUAGE_FILE: &str = "languages/chinese_s.po";
/// 对比报告里每个桶最多列出的示例 key 数。
const COMPARE_EXAMPLES: usize = 5;
/// 清空桶页时写入的空数据模块。
const EMPTY_MODULE: &str = "return {\n}\n";
const LANGS: [&str; 2] = ["CN", "EN"];

pub struct StringsWikiParams {
    /// 页面/索引前缀：`模块:<V> Strings ...`、`Data:<V>_Strings_Index.json`。
    pub version: String,
    /// 可选 scripts 快照目录名。
    pub snapshot: Option<String>,
    /// 本地产物输出目录；缺省 `output/strings/<VERSION>`。
    pub output: Option<PathBuf>,
    /// 跳过现网对比（无维基流量）。
    pub offline: bool,
    /// 无现有索引时的等分桶数。
    pub bucket_count: usize,
    /// 忽略现有索引边界，强制等量重切。
    pub rebalance: bool,
    /// Canary：最多写入 N 个桶页（索引不更新）；0 = 不限制。
    pub limit: usize,
}

/// `maintain-strings` 任务入口。
pub async fn run(
    params: &StringsWikiParams,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let version = params.version.trim().to_uppercase();
    if version.is_empty() {
        return Err(Error::Config("version 不能为空".to_string()));
    }

    reporter.stage("解析游戏文本");
    let en_po = PoParser::parse(&read_game_file(
        params.snapshot.as_deref(),
        EN_LANGUAGE_FILE,
    )?)?;
    let cn_po = PoParser::parse(&read_game_file(
        params.snapshot.as_deref(),
        CN_LANGUAGE_FILE,
    )?)?;
    let (en_map, en_stats) = build_language_map(&en_po.entries, false);
    let (cn_map, cn_stats) = build_language_map(&cn_po.entries, true);
    reporter.log(format!(
        "EN(pot) 条目 {} → 顶层 key {}；CN(po) 条目 {} → 顶层 key {}（空译 {}）",
        en_po.entries.len(),
        en_map.len(),
        cn_po.entries.len(),
        cn_map.len(),
        cn_stats.skipped_empty,
    ));

    let keys: Vec<String> = en_map
        .keys()
        .chain(cn_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let en_only = en_map.keys().filter(|k| !cn_map.contains_key(*k)).count();
    let cn_only = cn_map.keys().filter(|k| !en_map.contains_key(*k)).count();
    reporter.log(format!(
        "合并 key {}（EN 独有 {} / CN 独有 {}）",
        keys.len(),
        en_only,
        cn_only
    ));

    let mut client = if params.offline {
        None
    } else {
        Some(WikiClient::from_env().map_err(|e| {
            Error::Config(format!(
                "创建维基客户端失败（可用 --offline 跳过对比）：{e}"
            ))
        })?)
    };

    let index_title = format!("Data:{version}_Strings_Index.json");
    let existing_index = match client.as_ref() {
        Some(client) => fetch_index(client, &index_title, reporter).await?,
        None => ExistingIndex::default(),
    };
    let use_boundaries = !params.rebalance && !existing_index.data.is_empty();
    let old_index = if params.rebalance {
        Vec::new()
    } else {
        existing_index.data.clone()
    };

    let out_dir = params.output.clone().unwrap_or_else(|| {
        PathBuf::from("output")
            .join("strings")
            .join(version.as_str())
    });
    std::fs::create_dir_all(&out_dir)?;
    if !old_index.is_empty() {
        // 留存对比基线，便于事后审计与复现。
        std::fs::write(
            out_dir.join(format!("{version}_Strings_Index.old.json")),
            serde_json::to_string(&StringsIndexFile {
                data: old_index.clone(),
            })?,
        )?;
    }

    let plan = if use_boundaries {
        plan_with_boundaries(&keys, &existing_index.data)
    } else {
        plan_equal_count(&keys, params.bucket_count.max(1))
    };
    validate_plan(&plan).map_err(Error::Config)?;
    reporter.log(format!(
        "分桶 {} 个（{}）",
        plan.buckets.len(),
        if use_boundaries {
            "沿用现有索引边界"
        } else {
            "等量重切"
        }
    ));

    let (cn_lua_bytes, en_lua_bytes) =
        write_artifacts(&out_dir, &version, &plan, &cn_map, &en_map)?;
    reporter.log(format!(
        "本地产物已写入 {}（CN {:.1} MB / EN {:.1} MB）",
        out_dir.display(),
        cn_lua_bytes as f64 / 1_048_576.0,
        en_lua_bytes as f64 / 1_048_576.0,
    ));

    let scan_result = if let Some(wiki) = client.as_ref() {
        reporter.stage("与现网逐桶对比（只读）");
        let scans = scan_pages(wiki, reporter, &version, &plan, &cn_map, &en_map).await?;
        let mut report = CompareReport::default();
        for page in &scans {
            report.push(page.to_row());
        }
        report_index(&plan, &existing_index.data, &mut report, reporter);
        std::fs::write(
            out_dir.join("compare.json"),
            serde_json::to_string_pretty(&report)?,
        )?;
        std::fs::write(
            out_dir.join("compare.md"),
            render_compare_md(&version, &report),
        )?;
        reporter.log(format!("对比报告已写入 {}", out_dir.display()));

        let stale_pages = existing_stale_pages(wiki, &version, &existing_index.data, &plan).await?;
        let collected = collect_requests(
            &version,
            &index_title,
            &plan,
            &existing_index,
            &scans,
            &stale_pages,
        )?;
        Some((report, collected))
    } else {
        None
    };

    let (compare, apply) = match scan_result {
        Some((report, collected)) => {
            let apply = apply_edits(
                client.as_mut().expect("client checked above"),
                reporter,
                mode,
                collected,
                params.limit,
            )
            .await?;
            std::fs::write(
                out_dir.join("apply.json"),
                serde_json::to_string_pretty(&apply)?,
            )?;
            (Some(report), Some(apply))
        }
        None => (None, None),
    };

    Ok(serde_json::json!({
        "version": version,
        "snapshot": params.snapshot,
        "offline": params.offline,
        "write_mode": mode.name(),
        "source": {
            "en_entries": en_po.entries.len(),
            "cn_entries": cn_po.entries.len(),
            "en_keys": en_map.len(),
            "cn_keys": cn_map.len(),
            "cn_skipped_empty": cn_stats.skipped_empty,
            "en_skipped_not_strings": en_stats.skipped_not_strings,
            "cn_skipped_not_strings": cn_stats.skipped_not_strings,
            "en_duplicates": en_stats.duplicates,
            "cn_duplicates": cn_stats.duplicates,
        },
        "keys": {
            "total": keys.len(),
            "en_only": en_only,
            "cn_only": cn_only,
            "shared": keys.len() - en_only - cn_only,
        },
        "buckets": {
            "count": plan.buckets.len(),
            "reused_index": use_boundaries,
            "old_index_entries": old_index.len(),
        },
        "lua_bytes": { "cn": cn_lua_bytes, "en": en_lua_bytes },
        "output_dir": out_dir.to_string_lossy(),
        "compare": compare,
        "write": apply,
    }))
}

// ---------------------------------------------------------------------------
// 索引读取 / 产物写出
// ---------------------------------------------------------------------------

/// 现网索引页快照。
#[derive(Debug, Default)]
struct ExistingIndex {
    /// 页面存在（内容可解析）。
    present: bool,
    data: Vec<IndexEntry>,
    /// 编辑冲突检测用的基线时间戳。
    base_timestamp: Option<String>,
}

/// 读取现有索引；页面不存在返回空快照，内容无法解析则报错
/// （静默重切 200 页边界的风险远大于中止）。
async fn fetch_index(
    client: &WikiClient,
    title: &str,
    reporter: &dyn Reporter,
) -> Result<ExistingIndex> {
    match client.get_page(title).await {
        Ok(page) => {
            let base_timestamp = page.last_rev_timestamp.clone();
            let Some(text) = page.content.as_deref() else {
                reporter.log(format!("{title} 无内容，按无索引处理"));
                return Ok(ExistingIndex::default());
            };
            let file: StringsIndexFile = serde_json::from_str(text).map_err(|e| {
                Error::Config(format!(
                    "{title} 不是预期 JSON（{e}）；请先人工确认，勿自动重切边界"
                ))
            })?;
            reporter.log(format!("读取现有索引：{} 项", file.data.len()));
            Ok(ExistingIndex {
                present: true,
                data: file.data,
                base_timestamp,
            })
        }
        Err(Error::PageNotFound(_)) => {
            reporter.log(format!("{title} 不存在，将等量分桶"));
            Ok(ExistingIndex::default())
        }
        Err(e) => Err(e),
    }
}

/// 写出 index JSON 与全部桶页，返回 (CN, EN) Lua 总字节数。
fn write_artifacts(
    out_dir: &std::path::Path,
    version: &str,
    plan: &BucketPlan,
    cn_map: &BTreeMap<String, StringValue>,
    en_map: &BTreeMap<String, StringValue>,
) -> Result<(usize, usize)> {
    let index_file = StringsIndexFile {
        data: plan.index.clone(),
    };
    std::fs::write(
        out_dir.join(format!("{version}_Strings_Index.json")),
        serde_json::to_string(&index_file)?,
    )?;

    let mut cn_bytes = 0usize;
    let mut en_bytes = 0usize;
    for bucket in &plan.buckets {
        for (lang, map) in [("CN", cn_map), ("EN", en_map)] {
            let values = bucket_values(map, &bucket.keys);
            let text = render_module(&values);
            if lang == "CN" {
                cn_bytes += text.len();
            } else {
                en_bytes += text.len();
            }
            std::fs::write(
                out_dir.join(format!("{version}_Strings_{lang}_{}.lua", bucket.id)),
                text,
            )?;
        }
    }
    Ok((cn_bytes, en_bytes))
}

fn bucket_values(
    map: &BTreeMap<String, StringValue>,
    keys: &[String],
) -> BTreeMap<String, StringValue> {
    keys.iter()
        .filter_map(|key| map.get(key).map(|value| (key.clone(), value.clone())))
        .collect()
}

// ---------------------------------------------------------------------------
// 现网对比（扫描）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
struct CompareSummary {
    checked: usize,
    unchanged: usize,
    modified: usize,
    page_missing: usize,
    parse_errors: usize,
    added: usize,
    removed: usize,
    changed: usize,
    chars_added: usize,
    chars_removed: usize,
    chars_changed: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CompareRow {
    bucket: String,
    lang: &'static str,
    /// `unchanged` / `modified` / `page_missing` / `parse_error`
    status: &'static str,
    old_keys: usize,
    new_keys: usize,
    added: usize,
    removed: usize,
    changed: usize,
    chars_added: usize,
    chars_removed: usize,
    chars_changed: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added_examples: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed_examples: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed_examples: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct CompareReport {
    rows: Vec<CompareRow>,
    summary: CompareSummary,
    index_entries_old: usize,
    index_entries_new: usize,
    index_boundaries_changed: usize,
}

impl CompareReport {
    fn push(&mut self, row: CompareRow) {
        match row.status {
            "modified" => self.summary.modified += 1,
            "page_missing" => self.summary.page_missing += 1,
            "parse_error" => self.summary.parse_errors += 1,
            _ => self.summary.unchanged += 1,
        }
        self.summary.checked += 1;
        self.summary.added += row.added;
        self.summary.removed += row.removed;
        self.summary.changed += row.changed;
        self.summary.chars_added += row.chars_added;
        self.summary.chars_removed += row.chars_removed;
        self.summary.chars_changed += row.chars_changed;
        self.rows.push(row);
    }
}

/// 一个桶页的扫描结果（含组装好的新文本，写站阶段直接复用）。
struct PageDiff {
    bucket: String,
    lang: &'static str,
    title: String,
    status: &'static str,
    old_keys: usize,
    new_keys: usize,
    diff: MapDiff,
    base_timestamp: Option<String>,
    new_text: String,
    error: Option<String>,
}

impl PageDiff {
    fn to_row(&self) -> CompareRow {
        CompareRow {
            bucket: self.bucket.clone(),
            lang: self.lang,
            status: self.status,
            old_keys: self.old_keys,
            new_keys: self.new_keys,
            added: self.diff.added.len(),
            removed: self.diff.removed.len(),
            changed: self.diff.changed.len(),
            chars_added: self.diff.chars_added,
            chars_removed: self.diff.chars_removed,
            chars_changed: self.diff.chars_changed,
            added_examples: truncate(&self.diff.added),
            removed_examples: truncate(&self.diff.removed),
            changed_examples: truncate(&self.diff.changed),
            error: self.error.clone(),
        }
    }
}

async fn scan_pages(
    client: &WikiClient,
    reporter: &dyn Reporter,
    version: &str,
    plan: &BucketPlan,
    cn_map: &BTreeMap<String, StringValue>,
    en_map: &BTreeMap<String, StringValue>,
) -> Result<Vec<PageDiff>> {
    let mut scans = Vec::with_capacity(plan.buckets.len() * 2);

    for (i, bucket) in plan.buckets.iter().enumerate() {
        for (lang, map) in [("CN", cn_map), ("EN", en_map)] {
            let title = page_title(version, lang, &bucket.id);
            let new_values = bucket_values(map, &bucket.keys);
            let new_text = render_module(&new_values);
            let (old_values, page_missing, base_timestamp, parse_error) =
                match client.get_page(&title).await {
                    Ok(page) => {
                        let base_timestamp = page.last_rev_timestamp.clone();
                        match page.content.as_deref() {
                            Some(text) => match parse_strings_module(text) {
                                Ok(values) => (values, false, base_timestamp, None),
                                Err(e) => {
                                    reporter
                                        .log(format!("警告：{title} 解析失败（{e}），将跳过写入"));
                                    (BTreeMap::new(), false, base_timestamp, Some(e.to_string()))
                                }
                            },
                            None => (BTreeMap::new(), false, base_timestamp, None),
                        }
                    }
                    Err(Error::PageNotFound(_)) => (BTreeMap::new(), true, None, None),
                    Err(e) => return Err(e),
                };
            let diff = diff_values(&new_values, &old_values);
            let status = if parse_error.is_some() {
                "parse_error"
            } else if page_missing {
                "page_missing"
            } else if diff.is_empty() {
                "unchanged"
            } else {
                "modified"
            };
            scans.push(PageDiff {
                bucket: bucket.id.clone(),
                lang,
                title: title.clone(),
                status,
                old_keys: old_values.len(),
                new_keys: new_values.len(),
                diff,
                base_timestamp,
                new_text,
                error: parse_error,
            });
            let scan = scans.last().expect("just pushed");
            if !matches!(status, "unchanged") {
                reporter.log(format!(
                    "[{lang} {}] {status}：key {}→{}（+{} -{} ~{}，角色 +{}/-{}/~{}）",
                    bucket.id,
                    scan.old_keys,
                    scan.new_keys,
                    scan.diff.added.len(),
                    scan.diff.removed.len(),
                    scan.diff.changed.len(),
                    scan.diff.chars_added,
                    scan.diff.chars_removed,
                    scan.diff.chars_changed,
                ));
            }
        }
        if (i + 1) % 20 == 0 {
            reporter.log(format!("对比进度 {}/{}", i + 1, plan.buckets.len()));
        }
    }
    Ok(scans)
}

fn truncate(keys: &[String]) -> Vec<String> {
    keys.iter().take(COMPARE_EXAMPLES).cloned().collect()
}

fn page_title(version: &str, lang: &str, bucket: &str) -> String {
    format!("模块:{version} Strings {lang} {bucket}")
}

fn bucket_comment(version: &str, lang: &str, bucket: &str) -> String {
    format!("maintain-strings: update {version} Strings {lang} {bucket}")
}

fn index_comment(version: &str) -> String {
    format!("maintain-strings: update {version}_Strings_Index.json")
}

/// 索引边界差异并打印摘要。
fn report_index(
    plan: &BucketPlan,
    old_index: &[IndexEntry],
    report: &mut CompareReport,
    reporter: &dyn Reporter,
) {
    let old_by_bucket: BTreeMap<&str, &str> = old_index
        .iter()
        .map(|entry| (entry.bucket.as_str(), entry.key.as_str()))
        .collect();
    let changed = plan
        .index
        .iter()
        .filter(|entry| old_by_bucket.get(entry.bucket.as_str()) != Some(&entry.key.as_str()))
        .count();
    report.index_entries_old = old_index.len();
    report.index_entries_new = plan.index.len();
    report.index_boundaries_changed = changed;
    reporter.log(format!(
        "索引：{} → {} 项，边界变化 {} 项",
        old_index.len(),
        plan.index.len(),
        changed
    ));
    reporter.log(format!(
        "对比汇总：{} 对桶页；未变 {} / 修改 {} / 缺失 {} / 解析失败 {}；新增 {} 删除 {} 变更 {}（角色 +{}/-{}/~{}）[索引边界变化 {}]",
        report.summary.checked,
        report.summary.unchanged,
        report.summary.modified,
        report.summary.page_missing,
        report.summary.parse_errors,
        report.summary.added,
        report.summary.removed,
        report.summary.changed,
        report.summary.chars_added,
        report.summary.chars_removed,
        report.summary.chars_changed,
        changed,
    ));
}

fn render_compare_md(version: &str, report: &CompareReport) -> String {
    let s = &report.summary;
    let mut out = format!(
        "# {version} Strings 桶页对比\n\n\
         - 检查桶页 {} 对：未变 {} / 修改 {} / 缺失 {} / 解析失败 {}\n\
         - 新增 key {}，删除 key {}，值变更 {}；角色条目 +{}/-{}/~{}\n\
         - 索引边界变化 {} 项（{} → {}）\n\n\
         | 桶 | 语言 | 状态 | 旧 key | 新 key | 新增 | 删除 | 变更 | 角色 +/-/改 |\n\
         |---|---|---|---:|---:|---:|---:|---:|---:|\n",
        s.checked,
        s.unchanged,
        s.modified,
        s.page_missing,
        s.parse_errors,
        s.added,
        s.removed,
        s.changed,
        s.chars_added,
        s.chars_removed,
        s.chars_changed,
        report.index_boundaries_changed,
        report.index_entries_old,
        report.index_entries_new,
    );
    for row in &report.rows {
        if row.status == "unchanged" {
            continue;
        }
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {}/{}/{} |\n",
            row.bucket,
            row.lang,
            row.status,
            row.old_keys,
            row.new_keys,
            row.added,
            row.removed,
            row.changed,
            row.chars_added,
            row.chars_removed,
            row.chars_changed,
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// 写入
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
struct PageStats {
    added: usize,
    removed: usize,
    changed: usize,
}

struct EditRequest {
    title: String,
    text: String,
    comment: String,
    base_timestamp: Option<String>,
    kind: &'static str,
    lang: Option<&'static str>,
    bucket: Option<String>,
    stats: PageStats,
}

#[derive(Debug, Clone, Serialize)]
struct EditOutcome {
    title: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bucket: Option<String>,
    /// `updated` / `created` / `failed` / `skipped`
    status: &'static str,
    added: usize,
    removed: usize,
    changed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    oldrevid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    newrevid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct ApplyReport {
    /// 计划写入的页面数（桶页 + 清空页 + 索引）。
    pending: usize,
    attempted: usize,
    applied: usize,
    failed: usize,
    /// canary 截断（索引未更新）。
    truncated: bool,
    index_pending: bool,
    index_updated: bool,
    outcomes: Vec<EditOutcome>,
}

/// 扫描结果 → 写站请求集合。
struct Collected {
    requests: Vec<EditRequest>,
    index: Option<EditRequest>,
    pre_failed: Vec<EditOutcome>,
}

fn collect_requests(
    version: &str,
    index_title: &str,
    plan: &BucketPlan,
    existing_index: &ExistingIndex,
    scans: &[PageDiff],
    stale_pages: &[(String, &'static str, String)],
) -> Result<Collected> {
    let mut requests = Vec::new();
    let mut pre_failed = Vec::new();

    for page in scans {
        match page.status {
            "modified" | "page_missing" => requests.push(EditRequest {
                title: page.title.clone(),
                text: page.new_text.clone(),
                comment: bucket_comment(version, page.lang, &page.bucket),
                base_timestamp: page.base_timestamp.clone(),
                kind: "bucket",
                lang: Some(page.lang),
                bucket: Some(page.bucket.clone()),
                stats: PageStats {
                    added: page.diff.added.len(),
                    removed: page.diff.removed.len(),
                    changed: page.diff.changed.len(),
                },
            }),
            "parse_error" => pre_failed.push(EditOutcome {
                title: page.title.clone(),
                kind: "bucket",
                lang: Some(page.lang),
                bucket: Some(page.bucket.clone()),
                status: "parse_error",
                added: page.diff.added.len(),
                removed: page.diff.removed.len(),
                changed: page.diff.changed.len(),
                oldrevid: None,
                newrevid: None,
                error: page.error.clone(),
            }),
            _ => {}
        }
    }

    for (title, lang, bucket) in stale_pages {
        requests.push(EditRequest {
            title: title.clone(),
            text: EMPTY_MODULE.to_string(),
            comment: format!("maintain-strings: clear {version} Strings {lang} {bucket}"),
            base_timestamp: None,
            kind: "stale",
            lang: Some(lang),
            bucket: Some(bucket.clone()),
            stats: PageStats::default(),
        });
    }

    let index_changed = !existing_index.present || existing_index.data != plan.index;
    let index = if index_changed {
        Some(EditRequest {
            title: index_title.to_string(),
            text: serde_json::to_string(&StringsIndexFile {
                data: plan.index.clone(),
            })?,
            comment: index_comment(version),
            base_timestamp: existing_index.base_timestamp.clone(),
            kind: "index",
            lang: None,
            bucket: None,
            stats: PageStats::default(),
        })
    } else {
        None
    };

    Ok(Collected {
        requests,
        index,
        pre_failed,
    })
}

/// 旧索引里有、但新方案已不再引用的桶（整桶 key 消失）。
fn stale_bucket_ids(old_index: &[IndexEntry], plan: &BucketPlan) -> Vec<String> {
    let live: BTreeSet<&str> = plan.index.iter().map(|e| e.bucket.as_str()).collect();
    let stale: BTreeSet<&str> = old_index
        .iter()
        .map(|e| e.bucket.as_str())
        .filter(|bucket| !live.contains(bucket))
        .collect();
    stale.into_iter().map(str::to_string).collect()
}

/// 查询被淘汰桶页是否存在；只清空真实存在的页面（避免创建空页）。
async fn existing_stale_pages(
    client: &WikiClient,
    version: &str,
    old_index: &[IndexEntry],
    plan: &BucketPlan,
) -> Result<Vec<(String, &'static str, String)>> {
    let stale_ids = stale_bucket_ids(old_index, plan);
    if stale_ids.is_empty() {
        return Ok(Vec::new());
    }
    let candidates: Vec<(String, &'static str, String)> = stale_ids
        .iter()
        .flat_map(|bucket| {
            LANGS
                .iter()
                .map(move |lang| (page_title(version, lang, bucket), *lang, bucket.clone()))
        })
        .collect();
    let titles: Vec<&str> = candidates.iter().map(|(t, _, _)| t.as_str()).collect();
    let metas = client.get_pages_meta(&titles).await?;
    let existing: BTreeSet<&str> = metas
        .iter()
        .filter(|m| !m.missing)
        .map(|m| m.title.as_str())
        .collect();
    Ok(candidates
        .into_iter()
        .filter(|(title, _, _)| existing.contains(title.as_str()))
        .collect())
}

/// 应用写站请求：单次批量确认 → 桶页 → 索引（仅全部成功且未截断时）。
async fn apply_edits(
    client: &mut WikiClient,
    reporter: &dyn Reporter,
    mode: WriteMode,
    collected: Collected,
    limit: usize,
) -> Result<ApplyReport> {
    let Collected {
        requests,
        index,
        pre_failed,
    } = collected;
    let pending = requests.len() + usize::from(index.is_some());
    let index_pending = index.is_some();

    if mode == WriteMode::DryRun {
        reporter.log(format!(
            "dry-run：跳过写入（待写 {} 页，其中索引 {}）",
            pending, index_pending
        ));
        let mut outcomes = pre_failed;
        for request in &requests {
            outcomes.push(skipped_outcome(request, "dry_run"));
        }
        if let Some(request) = &index {
            outcomes.push(skipped_outcome(request, "dry_run"));
        }
        let failed = outcomes
            .iter()
            .filter(|o| o.status == "parse_error")
            .count();
        return Ok(ApplyReport {
            pending,
            failed,
            outcomes,
            index_pending,
            ..ApplyReport::default()
        });
    }

    if pending == 0 && pre_failed.is_empty() {
        reporter.log("无变化，跳过写入".to_string());
        return Ok(ApplyReport {
            index_pending,
            ..ApplyReport::default()
        });
    }

    let scope = format!(
        "桶页 {} / 清空 {} / 索引 {}",
        requests.iter().filter(|r| r.kind == "bucket").count(),
        requests.iter().filter(|r| r.kind == "stale").count(),
        usize::from(index_pending),
    );
    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm(&format!("将写入 {pending} 个页面（{scope}），继续？"))
    } else {
        false
    };
    if let WriteDecision::Skip(reason) = decide_write(mode, confirmed) {
        reporter.log(format!("已跳过写入（{reason}）"));
        let mut outcomes = pre_failed;
        for request in &requests {
            outcomes.push(skipped_outcome(request, reason));
        }
        if let Some(request) = &index {
            outcomes.push(skipped_outcome(request, reason));
        }
        let failed = outcomes
            .iter()
            .filter(|o| o.status == "parse_error")
            .count();
        return Ok(ApplyReport {
            pending,
            failed,
            outcomes,
            index_pending,
            ..ApplyReport::default()
        });
    };

    reporter.stage("写入维基");
    client.login().await?;

    let mut outcomes = pre_failed;
    let mut failed = outcomes.len();
    let mut applied = 0usize;
    let mut requests = requests;
    let truncated = limit > 0 && requests.len() > limit;
    if truncated {
        requests.truncate(limit);
    }
    let attempted = requests.len();
    for (i, request) in requests.iter().enumerate() {
        match edit_one(client, request).await {
            Ok(outcome) => {
                applied += 1;
                reporter.log(format!(
                    "{}：{}（oldrev={:?} newrev={:?}）",
                    outcome.title, outcome.status, outcome.oldrevid, outcome.newrevid
                ));
                outcomes.push(outcome);
            }
            Err(e) => {
                failed += 1;
                reporter.log(format!("{}：写入失败（{e}）", request.title));
                outcomes.push(failed_outcome(request, e.to_string()));
            }
        }
        if (i + 1) % 20 == 0 {
            reporter.log(format!("写入进度 {}/{}", i + 1, attempted));
        }
    }

    let mut index_updated = false;
    if let Some(request) = &index {
        if truncated {
            reporter.log("canary 模式：跳过索引更新".to_string());
            outcomes.push(skipped_outcome(request, "canary"));
        } else if failed > 0 {
            reporter.log(format!(
                "存在 {} 个失败/跳过页，跳过索引更新（避免索引指向未更新桶）",
                failed
            ));
            outcomes.push(skipped_outcome(request, "blocked_by_failures"));
        } else {
            match edit_one(client, request).await {
                Ok(outcome) => {
                    index_updated = true;
                    reporter.log(format!(
                        "{}：{}（oldrev={:?} newrev={:?}）",
                        outcome.title, outcome.status, outcome.oldrevid, outcome.newrevid
                    ));
                    outcomes.push(outcome);
                }
                Err(e) => {
                    failed += 1;
                    reporter.log(format!("{}：写入失败（{e}）", request.title));
                    outcomes.push(failed_outcome(request, e.to_string()));
                }
            }
        }
    }

    Ok(ApplyReport {
        pending,
        attempted,
        applied,
        failed,
        truncated,
        index_pending,
        index_updated,
        outcomes,
    })
}

async fn edit_one(client: &WikiClient, request: &EditRequest) -> Result<EditOutcome> {
    let edit = client
        .edit_page(
            &request.title,
            &request.text,
            Some(&request.comment),
            false,
            request.base_timestamp.as_deref(),
        )
        .await?;
    Ok(EditOutcome {
        title: request.title.clone(),
        kind: request.kind,
        lang: request.lang,
        bucket: request.bucket.clone(),
        status: if request.base_timestamp.is_some() {
            "updated"
        } else {
            "created"
        },
        added: request.stats.added,
        removed: request.stats.removed,
        changed: request.stats.changed,
        oldrevid: edit.oldrevid,
        newrevid: edit.newrevid,
        error: None,
    })
}

fn failed_outcome(request: &EditRequest, error: String) -> EditOutcome {
    EditOutcome {
        title: request.title.clone(),
        kind: request.kind,
        lang: request.lang,
        bucket: request.bucket.clone(),
        status: "failed",
        added: request.stats.added,
        removed: request.stats.removed,
        changed: request.stats.changed,
        oldrevid: None,
        newrevid: None,
        error: Some(error),
    }
}

fn skipped_outcome(request: &EditRequest, reason: &str) -> EditOutcome {
    EditOutcome {
        title: request.title.clone(),
        kind: request.kind,
        lang: request.lang,
        bucket: request.bucket.clone(),
        status: "skipped",
        added: request.stats.added,
        removed: request.stats.removed,
        changed: request.stats.changed,
        oldrevid: None,
        newrevid: None,
        error: Some(reason.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_values_filters_to_keys() {
        let mut map = BTreeMap::new();
        map.insert("A".to_string(), StringValue::Text("a".into()));
        map.insert("B".to_string(), StringValue::Text("b".into()));
        let values = bucket_values(&map, &["B".to_string(), "C".to_string()]);
        assert_eq!(values.len(), 1);
        assert!(values.contains_key("B"));
    }

    #[test]
    fn test_page_title_and_comments() {
        assert_eq!(page_title("DST", "CN", "07"), "模块:DST Strings CN 07");
        assert_eq!(
            bucket_comment("DST", "EN", "07"),
            "maintain-strings: update DST Strings EN 07"
        );
        assert_eq!(
            index_comment("DST"),
            "maintain-strings: update DST_Strings_Index.json"
        );
    }

    #[test]
    fn test_stale_bucket_ids() {
        let old = vec![
            IndexEntry {
                key: "A".into(),
                bucket: "00".into(),
            },
            IndexEntry {
                key: "B".into(),
                bucket: "01".into(),
            },
        ];
        let plan = BucketPlan {
            buckets: vec![crate::models::Bucket {
                id: "00".into(),
                keys: vec!["A".into()],
            }],
            index: vec![IndexEntry {
                key: "A".into(),
                bucket: "00".into(),
            }],
        };
        assert_eq!(stale_bucket_ids(&old, &plan), vec!["01".to_string()]);
    }

    #[test]
    fn test_collect_requests_skips_unchanged_and_marks_parse_errors() {
        let scans = vec![
            PageDiff {
                bucket: "00".into(),
                lang: "CN",
                title: "模块:DST Strings CN 00".into(),
                status: "unchanged",
                old_keys: 1,
                new_keys: 1,
                diff: MapDiff::default(),
                base_timestamp: Some("t".into()),
                new_text: "return {\n}\n".into(),
                error: None,
            },
            PageDiff {
                bucket: "01".into(),
                lang: "CN",
                title: "模块:DST Strings CN 01".into(),
                status: "modified",
                old_keys: 1,
                new_keys: 2,
                diff: MapDiff {
                    added: vec!["K".into()],
                    ..MapDiff::default()
                },
                base_timestamp: Some("t".into()),
                new_text: "return {\n}\n".into(),
                error: None,
            },
            PageDiff {
                bucket: "02".into(),
                lang: "EN",
                title: "模块:DST Strings EN 02".into(),
                status: "parse_error",
                old_keys: 0,
                new_keys: 1,
                diff: MapDiff::default(),
                base_timestamp: None,
                new_text: "return {\n}\n".into(),
                error: Some("bad lua".into()),
            },
        ];
        let existing = ExistingIndex {
            present: false,
            data: vec![],
            base_timestamp: None,
        };
        let plan = BucketPlan {
            buckets: vec![],
            index: vec![],
        };
        let collected = collect_requests(
            "DST",
            "Data:DST_Strings_Index.json",
            &plan,
            &existing,
            &scans,
            &[],
        )
        .unwrap();
        assert_eq!(collected.requests.len(), 1);
        assert_eq!(collected.requests[0].title, "模块:DST Strings CN 01");
        assert_eq!(collected.pre_failed.len(), 1);
        // 索引页缺失 → 需要创建。
        assert!(collected.index.is_some());
    }

    #[test]
    fn test_compare_report_aggregates() {
        let make = |status: &'static str, added: usize| CompareRow {
            bucket: "00".into(),
            lang: "CN",
            status,
            old_keys: 1,
            new_keys: 2,
            added,
            removed: 0,
            changed: 0,
            chars_added: 0,
            chars_removed: 0,
            chars_changed: 0,
            added_examples: vec![],
            removed_examples: vec![],
            changed_examples: vec![],
            error: None,
        };
        let mut report = CompareReport {
            rows: vec![],
            summary: CompareSummary::default(),
            index_entries_old: 0,
            index_entries_new: 0,
            index_boundaries_changed: 0,
        };
        report.push(make("modified", 2));
        report.push(make("unchanged", 0));
        report.push(make("page_missing", 5));
        report.push(make("parse_error", 1));
        assert_eq!(report.summary.checked, 4);
        assert_eq!(report.summary.modified, 1);
        assert_eq!(report.summary.unchanged, 1);
        assert_eq!(report.summary.page_missing, 1);
        assert_eq!(report.summary.parse_errors, 1);
        assert_eq!(report.summary.added, 8);
        assert_eq!(report.rows.len(), 4);
    }
}
