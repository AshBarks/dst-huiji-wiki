pub mod cooking;
pub mod cooking_assets;
pub mod cooking_eval;
pub mod dataset;
pub mod job_spec;
pub mod prefab_overrides;
pub mod skilltree_wiki;
pub mod snapshot_diff;
pub mod strings_wiki;
pub mod template_check;
pub mod upload_icons;
pub mod upload_image;
pub mod wiki_write;
pub mod wikitext_edit;

pub use job_spec::{JobSpec, WikiAccess, JOB_SPECS};

use crate::error::{Error, Result};
use crate::platform::progress::{Reporter, WriteMode};
use crate::wiki::WikiClient;
use crate::DstContext;
use std::path::{Path, PathBuf};
use tracing::Instrument;

mod kind;
mod maintain_jobs;
mod map_jobs;
mod output;
mod redirect;

pub use kind::JobKind;

use crate::update::jobs::{
    run_symbol_annotate, run_update_index, run_update_scan, SymbolAnnotateParams,
};
use maintain_jobs::{run_maintain_copyclip, run_maintain_dst_recipes, run_maintain_item_table};
use map_jobs::{run_map_names, run_map_recipes, run_parse_po};

pub(crate) fn make_ctx(snapshot: &Option<String>) -> Result<DstContext> {
    let dst_root = crate::platform::config::dst_root_str()?;
    DstContext::new(dst_root, snapshot.clone())
}

fn opt_path(p: &Option<String>) -> Option<PathBuf> {
    p.as_ref().map(PathBuf::from)
}

fn path_str(p: &str) -> Result<&str> {
    Ok(p)
}

/// Executes a job to completion. Long running jobs should be spawned on a
/// dedicated tokio task by the caller (the WebUI JobManager does this).
pub async fn execute_job(kind: &JobKind, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    execute_job_with_mode(kind, reporter, WriteMode::Interactive).await
}

/// Like [`execute_job`], but with an explicit [`WriteMode`] controlling how
/// (and whether) wiki writes happen.
pub async fn execute_job_with_mode(
    kind: &JobKind,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    kind.validate()?;
    reporter.stage(&format!("{}（写入模式：{}）", kind.name(), mode.name()));
    // `Instrument` instead of `enter()`: an EnteredSpan is !Send and would
    // make this future unusable with tokio::spawn (WebUI job manager).
    let inner = execute_job_inner(kind, reporter, mode)
        .instrument(tracing::info_span!(
            "job",
            job = kind.name(),
            write_mode = mode.name()
        ))
        .await?;
    Ok(wrap_report(kind.name(), inner))
}

/// 报告外壳（report schema v1，一步到位新结构）：
///
/// ```json
/// {
///   "report_schema_version": 1,
///   "kind": "skilltree-wiki",
///   "status": "success",          // 内层数据带字符串 status 时上提，否则 "success"
///   "summary": {...},             // 可选：内层带 "summary" 键时上提
///   "artifacts": [...],           // 可选：内层带 "artifacts" 键时上提
///   "details": { ...原自由字段... } // 内层原样保留（含 status 原位置）
/// }
/// ```
///
/// 上层自动化只依赖外壳四键；作业各自的字段演进都收敛在 `details` 内。
pub fn wrap_report(kind: &str, inner: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    let status = inner
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("success")
        .to_string();
    let mut shell = serde_json::Map::new();
    shell.insert("report_schema_version".into(), serde_json::json!(1));
    shell.insert("kind".into(), serde_json::json!(kind));
    shell.insert("status".into(), serde_json::json!(status));
    if let Some(summary) = inner.get("summary") {
        shell.insert("summary".into(), summary.clone());
    }
    if let Some(artifacts) = inner.get("artifacts") {
        shell.insert("artifacts".into(), artifacts.clone());
    }
    shell.insert("details".into(), inner);
    serde_json::Value::Object(shell)
}

async fn execute_job_inner(
    kind: &JobKind,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    match kind {
        JobKind::ParsePo {
            input,
            output,
            category,
        } => run_parse_po(input, opt_path(output), category.clone(), reporter).await,
        JobKind::MapNames {
            input,
            output,
            compare,
            merge,
            version,
        } => {
            run_map_names(
                input,
                opt_path(output),
                compare.clone(),
                *merge,
                version.clone(),
                reporter,
            )
            .await
        }
        JobKind::MapRecipes {
            input,
            output,
            compare,
            merge,
            po_file,
            version,
        } => {
            run_map_recipes(
                input,
                opt_path(output),
                compare.clone(),
                *merge,
                po_file.clone(),
                version.clone(),
                reporter,
            )
            .await
        }
        JobKind::MaintainItemTable { output, snapshot } => {
            run_maintain_item_table(opt_path(output), snapshot.clone(), reporter, mode).await
        }
        JobKind::MaintainDstRecipes { output, snapshot } => {
            run_maintain_dst_recipes(opt_path(output), snapshot.clone(), reporter, mode).await
        }
        JobKind::MaintainCopyClip {
            r#type,
            output,
            snapshot,
        } => {
            run_maintain_copyclip(
                r#type.as_deref(),
                opt_path(output),
                snapshot.clone(),
                reporter,
                mode,
            )
            .await
        }
        JobKind::MaintainTemplateCheck {
            output,
            snapshot,
            skip_icon_status,
        } => {
            template_check::run(
                &template_check::TemplateCheckParams {
                    output: opt_path(output),
                    snapshot: snapshot.clone(),
                    skip_icon_status: *skip_icon_status,
                },
                reporter,
            )
            .await
        }
        JobKind::MaintainStrings {
            version,
            snapshot,
            output,
            offline,
            bucket_count,
            rebalance,
            limit,
        } => {
            strings_wiki::run(
                &strings_wiki::StringsWikiParams {
                    version: version.clone(),
                    snapshot: snapshot.clone(),
                    output: opt_path(output),
                    offline: *offline,
                    bucket_count: *bucket_count,
                    rebalance: *rebalance,
                    limit: *limit,
                },
                reporter,
                mode,
            )
            .await
        }
        JobKind::MaintainWikitext {
            pages,
            template,
            set,
            remove,
            output,
        } => {
            wikitext_edit::run(
                &wikitext_edit::WikitextEditParams {
                    pages: pages.clone(),
                    template: template.clone(),
                    set: set.clone(),
                    remove: remove.clone(),
                    output: opt_path(output),
                },
                reporter,
                mode,
            )
            .await
        }
        JobKind::PrefabOverrides { input, output } => {
            prefab_overrides::run_prefab_overrides(input, opt_path(output), reporter).await
        }
        JobKind::PrefabOverridesDir { input, output } => {
            prefab_overrides::run_prefab_overrides_dir(input.as_deref(), opt_path(output), reporter)
                .await
        }
        JobKind::PrefabOverridesAudit {
            scripts,
            wiki_file,
            output,
        } => {
            prefab_overrides::audit::run_prefab_overrides_audit(
                scripts.as_deref(),
                wiki_file.as_deref(),
                opt_path(output),
                reporter,
            )
            .await
        }
        JobKind::MaintainPrefabOverrides {
            scripts,
            wiki_file,
            output,
        } => {
            prefab_overrides::maintain::run_maintain_prefab_overrides(
                scripts.as_deref(),
                wiki_file.as_deref(),
                opt_path(output),
                reporter,
            )
            .await
        }
        JobKind::SkilltreeWiki {
            character,
            output,
            snapshot,
        } => {
            skilltree_wiki::run_skilltree_wiki(
                character.clone(),
                output.clone(),
                snapshot.clone(),
                reporter,
                mode,
            )
            .await
        }
        JobKind::SkilltreeExport {
            character,
            output,
            snapshot,
        } => {
            skilltree_wiki::run_skilltree_export(
                character.clone(),
                opt_path(output),
                snapshot.clone(),
                reporter,
            )
            .await
        }
        JobKind::UploadImage {
            path,
            name,
            description,
            comment,
            ignore_warnings,
        } => {
            upload_image::run_upload_image(
                std::path::Path::new(path),
                name.as_deref(),
                description.as_deref(),
                comment.as_deref(),
                *ignore_warnings,
                reporter,
                mode,
            )
            .await
        }
        JobKind::ScriptsSync {
            force,
            dry_run,
            state_path,
        } => run_scripts_sync(*force, *dry_run, state_path.as_deref(), reporter).await,
        JobKind::ImagesSync {
            force,
            dry_run,
            skip_wiki_status,
        } => run_images_sync(*force, *dry_run, *skip_wiki_status, reporter).await,
        JobKind::UploadIcons {
            build,
            source,
            file,
            title,
            only_missing,
            ignore_warnings,
            comment,
        } => {
            upload_icons::run_upload_icons(
                &upload_icons::UploadIconsParams {
                    build: build.clone(),
                    source: source.clone(),
                    file: file.clone(),
                    title: title.clone(),
                    only_missing: *only_missing,
                    ignore_warnings: *ignore_warnings,
                    comment: comment.clone(),
                },
                reporter,
                mode,
            )
            .await
        }
        JobKind::CreateRedirect { from, to, summary } => {
            redirect::run_create_redirect(from, to, summary.as_deref(), reporter, mode).await
        }
        JobKind::AnimSync {
            force,
            dry_run,
            label,
            out,
        } => run_anim_sync(*force, *dry_run, label.clone(), opt_path(out), reporter).await,
        JobKind::AnimDiff { old, new, zip } => {
            run_anim_diff(
                PathBuf::from(old),
                PathBuf::from(new),
                zip.clone(),
                reporter,
            )
            .await
        }
        JobKind::AnimIndex { scripts, anim, out } => {
            run_anim_index(scripts, opt_path(anim), opt_path(out), reporter).await
        }
        JobKind::SkinIndex { scripts, anim, out } => {
            run_skin_index(scripts.as_deref(), opt_path(anim), opt_path(out), reporter)
        }
        JobKind::CorpusFetch { full, dir, rc } => {
            run_corpus_fetch(*full, *rc, dir.as_deref(), reporter, mode).await
        }
        JobKind::CorpusIndex { dir, join } => {
            run_corpus_index(dir.as_deref(), join.as_deref(), reporter, mode).await
        }
        JobKind::UpdateIndex { root, out } => run_update_index(root, opt_path(out), reporter).await,
        JobKind::UpdateScan {
            old,
            new,
            out,
            corpus,
            annotate,
        } => {
            run_update_scan(
                old,
                new,
                opt_path(out),
                corpus.as_deref(),
                annotate.as_deref(),
                reporter,
            )
            .await
        }
        JobKind::KnowledgeScanSymbols {
            root,
            category,
            knowledge_dir,
            corpus,
            sample_pages,
            limit,
            force,
            concurrency,
            pass2_names,
            pick_names,
            refresh_auto,
            confirm_empty,
        } => {
            crate::knowledge::run_scan_symbols(
                &crate::knowledge::ScanSymbolsParams {
                    scripts_root: root.clone(),
                    category: category.clone(),
                    knowledge_root: knowledge_dir.clone(),
                    corpus: corpus.clone(),
                    sample_pages: *sample_pages,
                    limit: *limit,
                    force: *force,
                    concurrency: *concurrency,
                    pass2_names: pass2_names.clone(),
                    pick_names: pick_names.clone(),
                    refresh_auto: *refresh_auto,
                    confirm_empty: *confirm_empty,
                },
                reporter,
            )
            .await
        }
        JobKind::KnowledgeSync {
            old,
            new,
            knowledge_dir,
            rescan,
            limit,
            corpus,
            draft,
            review,
        } => {
            crate::knowledge::run_knowledge_sync(
                &crate::knowledge::SyncParams {
                    old: old.clone(),
                    new: new.clone(),
                    knowledge_dir: knowledge_dir.clone(),
                    rescan: *rescan,
                    limit: *limit,
                    corpus: corpus.clone(),
                    draft: *draft,
                    review: review.clone(),
                },
                reporter,
            )
            .await
        }
        JobKind::PageAssist {
            knowledge_dir,
            page,
            all,
            json,
            attribute,
            corpus,
        } => {
            crate::knowledge::run_page_assist(
                &crate::knowledge::PageAssistParams {
                    knowledge_dir: knowledge_dir.clone(),
                    page: page.clone(),
                    all: *all,
                    json: *json,
                    attribute: *attribute,
                    corpus: corpus.clone(),
                },
                reporter,
            )
            .await
        }
        JobKind::KnowledgeScanWiki {
            root,
            knowledge_dir,
            corpus,
            audit,
            audit_symbols,
            audit_max_pages,
            audit_batch_pages,
            audit_batch_max_chars,
            report,
            classify,
        } => {
            crate::knowledge::run_scan_wiki(
                &crate::knowledge::ScanWikiParams {
                    scripts_root: root.clone(),
                    knowledge_dir: knowledge_dir.clone(),
                    corpus: corpus.clone(),
                    audit: *audit,
                    audit_symbols: audit_symbols.clone(),
                    audit_max_pages: *audit_max_pages,
                    audit_batch_pages: *audit_batch_pages,
                    audit_batch_max_chars: *audit_batch_max_chars,
                    report: *report,
                    classify: *classify,
                },
                reporter,
            )
            .await
        }
        JobKind::SymbolAnnotate {
            root,
            corpus,
            limit,
            out,
            verdicts,
            llm,
            batch_pages,
            batch_max_chars,
            skip_no_fact_pages,
        } => {
            run_symbol_annotate(
                &SymbolAnnotateParams {
                    root: root.clone(),
                    corpus: corpus.clone(),
                    limit: *limit,
                    out: out.clone(),
                    verdicts: verdicts.clone(),
                    llm: *llm,
                    batch_pages: *batch_pages,
                    batch_max_chars: *batch_max_chars,
                    skip_no_fact_pages: *skip_no_fact_pages,
                },
                reporter,
            )
            .await
        }
    }
}

// ---------------------------------------------------------------------------
// Local-file commands (no game dir required)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 本地/薄包装作业入口
// ---------------------------------------------------------------------------

/// `scripts-sync`: archive the live scripts tree as a snapshot and extract
/// the new `scripts.zip`. Local-only; never touches the wiki.
async fn run_scripts_sync(
    force: bool,
    dry_run: bool,
    state_path: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    crate::scripts_sync::sync(
        &crate::scripts_sync::SyncParams {
            dst_root,
            state_path: state_path.map(PathBuf::from),
            force,
            dry_run,
        },
        reporter,
    )
}

/// `images-sync`: process the game image pipeline. Local-only image work;
/// afterwards refreshes `history/icon_meta.json` (names from the game PO
/// files and, unless skipped, wiki upload status via read-only queries).
async fn run_images_sync(
    force: bool,
    dry_run: bool,
    skip_wiki_status: bool,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    let out_dir = Some(crate::platform::config::ktools_out_dir());
    let mut report = crate::scripts_sync::images::run(
        &crate::scripts_sync::images::ImagesSyncParams {
            dst_root: dst_root.clone(),
            out_dir: out_dir.clone(),
            force,
            dry_run,
        },
        reporter,
    )?;

    let status = report
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if !dry_run && status != "rollback_skipped" {
        let out = out_dir
            .clone()
            .unwrap_or_else(upload_icons::out_dir_from_env);
        match upload_icons::refresh_icon_meta(
            Path::new(&dst_root),
            &out,
            force,
            !skip_wiki_status,
            reporter,
        )
        .await
        {
            Ok(summary) => report["icon_meta"] = summary,
            Err(e) => {
                reporter.log(format!("图标元数据刷新失败：{e}"));
                report["icon_meta_error"] = serde_json::json!(e.to_string());
            }
        }
    }
    Ok(report)
}

/// `anim-sync`: archive current `data/anim` into `ANIM__OUT_DIR` after a game
/// update. Local-only; never touches the wiki.
async fn run_anim_sync(
    force: bool,
    dry_run: bool,
    label: Option<String>,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    crate::scripts_sync::anim::run_sync(
        &crate::scripts_sync::anim::AnimSyncParams {
            dst_root,
            out_dir: out,
            label,
            force,
            dry_run,
        },
        reporter,
    )
}

/// `anim-diff`: compare two animation directories. Local-only.
async fn run_anim_diff(
    old_dir: PathBuf,
    new_dir: PathBuf,
    zip: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    crate::scripts_sync::anim::run_diff(
        &crate::scripts_sync::anim::AnimDiffParams {
            old_dir,
            new_dir,
            only: zip,
            parse_all: false,
        },
        reporter,
    )
}

/// `anim-index`: build a prefab ↔ animation-file association index.
/// Local-only; no wiki traffic.
async fn run_anim_index(
    scripts: &str,
    anim: Option<PathBuf>,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    crate::scripts_sync::anim::index::run_index(
        &crate::scripts_sync::anim::index::AnimIndexParams {
            scripts_root: PathBuf::from(scripts),
            anim_root: anim,
            out,
        },
        reporter,
    )
}

/// `skin-index`: parse `prefabs/skinprefabs.lua` and pair skin builds with
/// `data/anim/dynamic` zip/dyn packages. Local-only; no wiki traffic.
fn run_skin_index(
    scripts: Option<&str>,
    anim: Option<PathBuf>,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let scripts_root = match scripts.map(PathBuf::from) {
        Some(p) => p,
        None => crate::scripts_sync::anim::skin_index::default_scripts_root().ok_or_else(|| {
            Error::EnvVarNotFound(
                "scripts root not provided and DST__ROOT/data/databundles/scripts not found"
                    .to_string(),
            )
        })?,
    };
    crate::scripts_sync::anim::skin_index::run_skin_index(
        &crate::scripts_sync::anim::skin_index::SkinIndexParams {
            scripts_root,
            anim_root: anim,
            out,
        },
        reporter,
    )
}

async fn run_corpus_fetch(
    full: bool,
    rc: bool,
    dir: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    // 唯一允许匿名读的作业（读旧缓存可接受）；其余作业读也要求登录。
    // Login is deliberately skipped — this job never edits.
    let client = WikiClient::from_env_readonly()?;
    let base_dir = PathBuf::from(dir.unwrap_or("wikis"));
    if rc {
        return crate::corpus::rc::sync_rc(&client, &base_dir, mode == WriteMode::DryRun, reporter)
            .await;
    }
    crate::corpus::sync(
        &client,
        &base_dir,
        full,
        mode == WriteMode::DryRun,
        reporter,
    )
    .await
}

// ---------------------------------------------------------------------------
// Game-dir maintenance commands (require DST__ROOT, touch the wiki)
// ---------------------------------------------------------------------------

/// Rebuilds derived corpus indexes from the local tree. Purely local I/O —
/// no client, no login, `DST__ROOT` not required.
async fn run_corpus_index(
    dir: Option<&str>,
    join: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let base_dir = PathBuf::from(dir.unwrap_or("wikis"));
    crate::corpus::build_indexes(
        &base_dir,
        join.map(PathBuf::from).as_deref(),
        mode == WriteMode::DryRun,
        reporter,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_kind_names() {
        let jobs = [
            JobKind::ParsePo {
                input: "a.po".to_string(),
                output: None,
                category: None,
            },
            JobKind::MapNames {
                input: "a.po".to_string(),
                output: None,
                compare: None,
                merge: false,
                version: None,
            },
            JobKind::MaintainItemTable {
                output: None,
                snapshot: Some("scripts_202605291134".to_string()),
            },
        ];
        assert_eq!(jobs[0].name(), "parse-po");
        assert_eq!(jobs[1].name(), "map-names");
        assert_eq!(jobs[2].name(), "maintain-item-table");

        assert!(!jobs[0].touches_wiki());
        assert!(jobs[2].touches_wiki());
    }

    #[test]
    fn test_job_kind_serde_roundtrip() {
        let kind = JobKind::MaintainCopyClip {
            r#type: Some("tech".to_string()),
            output: None,
            snapshot: None,
        };
        let json = serde_json::to_string(&kind).unwrap();
        // Tagged representation keeps the variant name stable for the WebUI API.
        assert!(json.contains("\"kind\":\"maintain_copy_clip\""), "{}", json);
        let back: JobKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name(), kind.name());

        // Old payloads without new optional fields must keep deserializing.
        let legacy = r#"{"kind":"maintain_item_table"}"#;
        let back: JobKind = serde_json::from_str(legacy).unwrap();
        match back {
            JobKind::MaintainItemTable { output, snapshot } => {
                assert!(output.is_none());
                assert!(snapshot.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    /// 契约：`name()` 全集唯一，且与 serde tag 一一对应（kebab ↔ snake）。
    /// 防止再加变体时 CLI/JobKind/serde 三处名称继续漂移。
    #[test]
    fn contract_job_names_unique_and_aligned_with_serde_tags() {
        let jobs = JobKind::all_variants();
        assert_eq!(
            jobs.len(),
            33,
            "新增 JobKind 变体后请同步契约测试与前端 JOB_DEFS"
        );
        let mut names: Vec<&str> = jobs.iter().map(|j| j.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "JobKind::name() 存在重复：{names:?}");
        for j in &jobs {
            let tag = serde_json::to_value(j).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_string();
            assert_eq!(
                tag,
                j.name().replace('-', "_"),
                "serde tag `{tag}` 与 name() `{}` 不一致",
                j.name()
            );
        }
    }

    /// 契约：`touches_wiki()` 的集合固定为已知可写 job；新增可写变体必须
    /// 显式更新本测试（并同步前端 JOB_DEFS 的 wiki 标记与确认流程）。
    #[test]
    fn contract_touches_wiki_is_the_expected_set() {
        let expected: std::collections::BTreeSet<&str> = [
            "maintain-item-table",
            "maintain-dst-recipes",
            "maintain-copy-clip",
            "maintain-strings",
            "maintain-wikitext",
            "skilltree-wiki",
            "upload-image",
            "upload-icons",
            "create-redirect",
        ]
        .into_iter()
        .collect();
        let actual: std::collections::BTreeSet<&str> = JobKind::all_variants()
            .iter()
            .filter(|j| j.touches_wiki())
            .map(|j| j.name())
            .collect();
        assert_eq!(actual, expected, "touches_wiki 集合漂移");
    }

    /// 契约：前端 JOB_DEFS 的键必须是合法的 JobKind serde tag，
    /// 保证 WebUI 提交的每个任务都能被 `/api/jobs` 反序列化。
    #[test]
    fn contract_frontend_job_defs_keys_are_valid_serde_tags() {
        const JOBS_JS: &str = include_str!("../web/assets/js/jobs.js");
        let start = JOBS_JS
            .find("const JOB_DEFS = {")
            .expect("jobs.js 缺少 JOB_DEFS 定义");
        let block_end = start + JOBS_JS[start..].find("\n};").expect("JOB_DEFS 块未闭合");
        let block = &JOBS_JS[start..block_end];

        let tags: std::collections::BTreeSet<String> = JobKind::all_variants()
            .iter()
            .map(|j| {
                serde_json::to_value(j).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        let mut keys = Vec::new();
        for line in block.lines() {
            let Some(rest) = line.strip_prefix("  ") else {
                continue;
            };
            if rest.starts_with(' ') {
                continue; // 嵌套字段行
            }
            let Some(idx) = rest.find(": {") else {
                continue;
            };
            let key = &rest[..idx];
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                keys.push(key.to_string());
            }
        }
        assert!(!keys.is_empty(), "未能从 app.js 解析出 JOB_DEFS 键");
        for k in keys {
            assert!(
                tags.contains(&k),
                "前端 JOB_DEFS 键 `{k}` 不是合法的 JobKind serde tag"
            );
        }
    }

    /// 契约：report schema v1 外壳——kind/status 外提、可选 summary/
    /// artifacts 上提、details 原样保留内层全部字段。
    #[test]
    fn contract_report_shell_v1() {
        let shell = wrap_report(
            "scripts-sync",
            serde_json::json!({
                "status": "up_to_date",
                "version": "123",
                "summary": { "pages": 3 },
                "artifacts": ["a.json"],
            }),
        );
        assert_eq!(shell["report_schema_version"], 1);
        assert_eq!(shell["kind"], "scripts-sync");
        assert_eq!(shell["status"], "up_to_date");
        assert_eq!(shell["summary"]["pages"], 3);
        assert_eq!(shell["artifacts"][0], "a.json");
        assert_eq!(shell["details"]["version"], "123");
        assert_eq!(shell["details"]["status"], "up_to_date");

        let shell = wrap_report("parse-po", serde_json::json!({ "records": 5 }));
        assert_eq!(shell["status"], "success");
        assert!(shell.get("summary").is_none());
        assert_eq!(shell["details"]["records"], 5);
    }

    #[test]
    fn test_decide_write_table() {
        use crate::platform::progress::{decide_write, WriteDecision};
        use WriteMode::{AutoConfirm, DryRun, Interactive};

        // DryRun never writes, regardless of confirmation.
        assert_eq!(decide_write(DryRun, true), WriteDecision::Skip("dry_run"));
        assert_eq!(decide_write(DryRun, false), WriteDecision::Skip("dry_run"));

        // AutoConfirm always applies.
        assert_eq!(decide_write(AutoConfirm, false), WriteDecision::Apply);
        assert_eq!(decide_write(AutoConfirm, true), WriteDecision::Apply);

        // Interactive follows the reporter's answer.
        assert_eq!(decide_write(Interactive, true), WriteDecision::Apply);
        assert_eq!(
            decide_write(Interactive, false),
            WriteDecision::Skip("declined")
        );
    }

    #[test]
    fn test_write_mode_name_and_default() {
        assert_eq!(WriteMode::default(), WriteMode::Interactive);
        assert_eq!(WriteMode::Interactive.name(), "interactive");
        assert_eq!(WriteMode::AutoConfirm.name(), "auto_confirm");
        assert_eq!(WriteMode::DryRun.name(), "dry_run");
    }
}
