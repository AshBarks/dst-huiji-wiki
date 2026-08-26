//! Main-namespace wiki corpus harvesting (docs/WIKI_CORPUS_PLAN.md).
//!
//! One command keeps the local `wikis/<host>/` tree in sync with the wiki's
//! main namespace: enumerate → reconcile against `meta.jsonl` → batch-fetch
//! changed pages → classify → validate → manifest + derived indexes.
//!
//! The corpus stores raw facts only (wikitext + metadata); every view on top
//! (redirect map, class summary) is derived and rebuildable.

pub mod classify;
pub mod model;
pub mod prefab_index;
pub mod segment;
pub mod store;

pub use model::{ClassConfidence, CorpusManifest, GameClass, PageMeta};
pub use store::CorpusStore;

use crate::error::{Error, Result};
use crate::service::Reporter;
use crate::wiki::{PageListingEntry, WikiClient, TITLES_PER_QUERY};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How many batches between progress logs / metadata flushes. Flushing lets a
/// killed run resume close to where it stopped (unchanged `touched` is skipped
/// on the next round); orphaned page files from a lost race are harmless.
const FLUSH_EVERY_BATCHES: usize = 20;

/// What changed between the remote enumeration and the local corpus state.
#[derive(Debug, Default)]
struct ReconcilePlan<'a> {
    to_fetch: Vec<&'a PageListingEntry>,
    removed_ids: Vec<i64>,
    unchanged: usize,
}

/// Pure diff between the fresh enumeration and local metadata.
///
/// `local_file_missing` reports whether the wikitext file of a pageid is
/// absent locally — such pages are refetched even when `touched` matches,
/// healing partial states after an interrupted run.
fn reconcile<'a>(
    fresh: &'a [PageListingEntry],
    existing: &BTreeMap<i64, PageMeta>,
    local_file_missing: impl Fn(i64) -> bool,
    full: bool,
) -> ReconcilePlan<'a> {
    let mut plan = ReconcilePlan::default();
    let mut fresh_ids = std::collections::HashSet::with_capacity(fresh.len());
    for entry in fresh {
        fresh_ids.insert(entry.pageid);
        let stale = match existing.get(&entry.pageid) {
            None => true,
            Some(meta) => meta.touched != entry.touched,
        };
        if full || stale || local_file_missing(entry.pageid) {
            plan.to_fetch.push(entry);
        } else {
            plan.unchanged += 1;
        }
    }
    plan.removed_ids = existing
        .keys()
        .copied()
        .filter(|id| !fresh_ids.contains(id))
        .collect();
    plan
}

/// Runs one corpus harvest round. Read-only against the wiki; writes only to
/// the local corpus tree (suppressed entirely under `dry_run`).
pub async fn sync(
    client: &WikiClient,
    base_dir: &Path,
    full: bool,
    dry_run: bool,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let started = Instant::now();
    let mode = if full { "full" } else { "incremental" };
    let store = CorpusStore::new(base_dir, client.config().host());

    reporter.stage("枚举主命名空间");
    let fresh = client.enumerate_namespace(0).await?;
    reporter.log(format!("枚举到 {} 个页面（含重定向）", fresh.len()));

    let existing = store.load_meta()?;
    reporter.log(format!("本地已有 {} 页元数据", existing.len()));

    let plan = reconcile(&fresh, &existing, |id| !store.page_path(id).exists(), full);
    reporter.log(format!(
        "对账：待抓取 {}，未变化 {}，远端消失 {}",
        plan.to_fetch.len(),
        plan.unchanged,
        plan.removed_ids.len(),
    ));

    if dry_run {
        let manifest = finish_manifest(
            client.config().host(),
            mode,
            true,
            fresh.len(),
            0,
            plan.unchanged,
            plan.removed_ids.len(),
            0,
            0,
            &existing,
            started,
        );
        return Ok(manifest_summary(&manifest, store.root()));
    }

    // Remote-vanished pages: archive the wikitext, drop the metadata row.
    let mut removed = 0usize;
    let mut meta = existing;
    if !plan.removed_ids.is_empty() {
        reporter.stage("归档远端消失页面");
        let tag = now_compact_tag();
        for id in &plan.removed_ids {
            store.archive_removed(*id, &tag)?;
            meta.remove(id);
            removed += 1;
        }
        reporter.log(format!("已归档 {} 页到 _removed/{}", removed, tag));
    }

    reporter.stage("抓取页面内容");
    let by_title: HashMap<&str, &PageListingEntry> =
        fresh.iter().map(|e| (e.title.as_str(), e)).collect();
    let total_batches = plan.to_fetch.len().div_ceil(TITLES_PER_QUERY);

    let mut fetched = 0usize;
    let mut missing_on_fetch = 0usize;
    let mut len_mismatches = 0usize;

    for (batch_idx, chunk) in plan.to_fetch.chunks(TITLES_PER_QUERY).enumerate() {
        let titles: Vec<&str> = chunk.iter().map(|e| e.title.as_str()).collect();
        let results = client.get_pages_wikitext(&titles).await?;

        for content in results {
            // Prefer the API-echoed pageid; fall back to title lookup for the
            // (rare) shapes where the id comes back null.
            let entry = content
                .pageid
                .and_then(|id| fresh.iter().find(|e| e.pageid == id))
                .or_else(|| by_title.get(content.title.as_str()).copied());
            let Some(entry) = entry else {
                reporter.log(format!(
                    "警告：响应页 '{}' 不在枚举结果中，跳过",
                    content.title
                ));
                continue;
            };

            let mut page_meta = PageMeta::from_listing(entry);
            match content.wikitext.as_deref() {
                Some(text) => {
                    if let Some(expected) = entry.len {
                        if expected != text.len() as u64 {
                            len_mismatches += 1;
                            tracing::warn!(pageid = entry.pageid, title = %entry.title,
                                expected, actual = text.len(), "revision length mismatch");
                        }
                    }
                    store.write_page(entry.pageid, text)?;
                }
                None => {
                    missing_on_fetch += 1;
                }
            }

            let outcome = classify::classify(
                entry.title.as_str(),
                entry.redirect,
                content.wikitext.as_deref(),
                &content.categories,
            );
            page_meta.rev_sha1 = content.sha1.clone();
            page_meta.categories = content.categories.clone();
            page_meta.game_class = outcome.game_class;
            page_meta.class_signals = outcome.signals;
            page_meta.class_confidence = outcome.confidence;
            meta.insert(entry.pageid, page_meta);
        }

        fetched += chunk.len();
        if (batch_idx + 1) % FLUSH_EVERY_BATCHES == 0 || batch_idx + 1 == total_batches {
            store.save_meta(&meta)?;
            reporter.log(format!(
                "进度：{}/{} 批（已抓取 {} 页）",
                batch_idx + 1,
                total_batches,
                fetched
            ));
        }
    }

    store.save_meta(&meta)?;
    store.save_derived_indexes(&meta)?;

    let manifest = finish_manifest(
        client.config().host(),
        mode,
        false,
        fresh.len(),
        fetched,
        plan.unchanged,
        removed,
        missing_on_fetch,
        len_mismatches,
        &meta,
        started,
    );
    store.save_manifest(&manifest)?;
    reporter.log(format!(
        "完成：枚举 {}，抓取 {}，未变 {}，移除 {}；缺失 {}，长度不符 {}",
        manifest.enumerated,
        manifest.fetched,
        manifest.unchanged,
        manifest.removed,
        manifest.missing_on_fetch,
        manifest.len_mismatches,
    ));

    Ok(manifest_summary(&manifest, store.root()))
}

/// Builds the derived corpus indexes (docs/CORPUS_CODE_ATLAS_CONTRACT.md).
/// Local-only: reads `wikis/<host>/` and writes under its `index/`; no wiki
/// traffic at all. `dir` is the corpus base (default `wikis/`) and must
/// contain exactly one host tree.
pub async fn build_indexes(
    dir: &Path,
    dry_run: bool,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let host = detect_host(dir)?;
    reporter.stage("装载语料元数据");
    let store = CorpusStore::new(dir, &host);
    let metas = store.load_meta()?;
    reporter.log(format!("host={host}，页面 {} 条", metas.len()));

    reporter.stage("构建 prefab 注册表");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let registry = prefab_index::build_registry(&store, &metas, now_ms)?;
    reporter.log(format!(
        "实体页 {}，变体 {} 种，多变体页 {}，无信息框页 {}",
        registry.stats.entity_pages,
        registry.stats.distinct_variants,
        registry.stats.multi_variant_pages,
        registry.pages_without_prefab.len(),
    ));

    reporter.stage("切分页面区域");
    let mut regions_buf = String::new();
    let (mut pages_segmented, mut region_count) = (0usize, 0usize);
    for meta in metas.values() {
        if meta.redirect {
            continue;
        }
        let Some(text) = store.read_page(meta.pageid)? else {
            continue;
        };
        let regions = segment::segment(meta.pageid, &text);
        region_count += regions.len();
        pages_segmented += 1;
        serde_json::to_writer(
            // One JSON line per page: {"pageid","title","regions":[…]}
            LineWriter(&mut regions_buf),
            &serde_json::json!({"pageid": meta.pageid, "title": meta.title, "regions": regions}),
        )?;
        regions_buf.push('\n');
    }
    reporter.log(format!(
        "切分 {pages_segmented} 页，共 {region_count} 个区域"
    ));

    if dry_run {
        reporter.log("dry-run：不写任何工件".to_string());
        return Ok(serde_json::json!({
            "host": host,
            "dry_run": true,
            "stats": serde_json::to_value(&registry.stats)?,
            "regions": {"pages": pages_segmented, "total": region_count},
        }));
    }

    reporter.stage("写出工件");
    let reg_path = prefab_index::save_registry(store.root(), &registry)?;
    reporter.log(format!("已写 {}", reg_path.display()));
    let seg_path = save_regions(store.root(), &regions_buf)?;
    reporter.log(format!("已写 {}", seg_path.display()));

    Ok(serde_json::json!({
        "host": host,
        "dry_run": false,
        "artifacts": ["index/pages_by_prefab.json", "index/regions.jsonl"],
        "stats": serde_json::to_value(&registry.stats)?,
        "pages_without_prefab_count": registry.pages_without_prefab.len(),
        "regions": {"pages": pages_segmented, "total": region_count},
    }))
}

/// Appends JSON via `Display`-free `io::Write` on a String buffer so json!
/// values stream without intermediate allocations per field.
struct LineWriter<'a>(&'a mut String);
impl std::io::Write for LineWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Writes the regions JSONL atomically under `<root>/index/`.
fn save_regions(root: &Path, body: &str) -> Result<std::path::PathBuf> {
    let dir = root.join("index");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join("regions.jsonl");
    let tmp = dir.join(".regions.jsonl.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

/// A corpus base dir holds one tree per wiki host; index building operates on
/// the single present host.
fn detect_host(base: &Path) -> Result<String> {
    let mut hosts = std::fs::read_dir(base)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    hosts.sort();
    match hosts.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(Error::Config(format!(
            "{} 下没有任何语料目录，请先运行 corpus-fetch",
            base.display()
        ))),
        many => Err(Error::Config(format!(
            "{} 下存在多个语料目录（{}），请用 --dir 指定单一 host 树的父目录或整理后重试",
            base.display(),
            many.join("、")
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_manifest(
    host: &str,
    mode: &str,
    dry_run: bool,
    enumerated: usize,
    fetched: usize,
    unchanged: usize,
    removed: usize,
    missing_on_fetch: usize,
    len_mismatches: usize,
    meta: &BTreeMap<i64, PageMeta>,
    started: Instant,
) -> CorpusManifest {
    let mut class_counts = BTreeMap::new();
    for m in meta.values() {
        *class_counts
            .entry(m.game_class.name().to_string())
            .or_default() += 1;
    }
    CorpusManifest {
        generated_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        host: host.to_string(),
        mode: mode.to_string(),
        dry_run,
        enumerated,
        fetched,
        unchanged,
        removed,
        missing_on_fetch,
        len_mismatches,
        class_counts,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn manifest_summary(manifest: &CorpusManifest, root: &Path) -> serde_json::Value {
    let value = serde_json::to_value(manifest).unwrap_or_default();
    let mut obj = value;
    if let Some(map) = obj.as_object_mut() {
        map.insert(
            "root".to_string(),
            serde_json::json!(root.display().to_string()),
        );
    }
    obj
}

fn now_compact_tag() -> String {
    // Local-time-ish tag derived from epoch seconds is enough for archive dirs.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("round_{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::model::ClassConfidence;

    fn entry(pageid: i64, title: &str, touched: &str, redirect: bool) -> PageListingEntry {
        PageListingEntry {
            pageid,
            ns: 0,
            title: title.to_string(),
            touched: Some(touched.to_string()),
            len: Some(100),
            is_new: false,
            redirect,
        }
    }

    fn local_meta(pageid: i64, touched: &str) -> PageMeta {
        let mut m = PageMeta::from_listing(&entry(pageid, "x", touched, false));
        m.game_class = GameClass::Dst;
        m.class_confidence = ClassConfidence::High;
        m
    }

    #[test]
    fn reconcile_detects_new_changed_removed_and_unchanged() {
        let fresh = vec![
            entry(1, "A", "t1", false), // same touched -> unchanged
            entry(2, "B", "t9", false), // touched differs -> fetch
            entry(3, "C", "t1", false), // new page -> fetch
        ];
        let mut existing = BTreeMap::new();
        existing.insert(1, local_meta(1, "t1"));
        existing.insert(2, local_meta(2, "t8"));
        existing.insert(4, local_meta(4, "t1")); // vanished remotely

        let plan = reconcile(&fresh, &existing, |_| false, false);
        assert_eq!(
            plan.to_fetch.iter().map(|e| e.pageid).collect::<Vec<_>>(),
            [2, 3]
        );
        assert_eq!(plan.removed_ids, [4]);
        assert_eq!(plan.unchanged, 1);
    }

    #[test]
    fn reconcile_full_mode_refetches_everything() {
        let fresh = vec![entry(1, "A", "t1", false)];
        let mut existing = BTreeMap::new();
        existing.insert(1, local_meta(1, "t1"));
        let plan = reconcile(&fresh, &existing, |_| false, true);
        assert_eq!(plan.to_fetch.len(), 1);
        assert_eq!(plan.unchanged, 0);
    }

    #[test]
    fn reconcile_heals_locally_missing_files() {
        let fresh = vec![entry(1, "A", "t1", false)];
        let mut existing = BTreeMap::new();
        existing.insert(1, local_meta(1, "t1"));
        // touched matches, but the local file is gone (interrupted run)
        let plan = reconcile(&fresh, &existing, |id| id == 1, false);
        assert_eq!(plan.to_fetch.len(), 1);
        assert_eq!(plan.unchanged, 0);
    }

    #[test]
    fn redirect_pages_are_refetched_when_flag_flips() {
        // A non-redirect page turned into a redirect: touched changes anyway,
        // but make sure the flag path itself classifies correctly downstream.
        let fresh = vec![entry(1, "A", "t2", true)];
        let mut existing = BTreeMap::new();
        existing.insert(1, local_meta(1, "t1"));
        let plan = reconcile(&fresh, &existing, |_| false, false);
        assert_eq!(plan.to_fetch.len(), 1);
    }
}
