pub mod dataset;
pub mod progress;
pub mod snapshot_diff;

pub use progress::{CaptureReporter, ConfirmMode, JobEvent, Reporter, StdoutReporter};

use crate::error::{Error, Result};
use crate::mapping::{compare_and_report, WikiDataConverter, WikiMapper};
use crate::models::PoEntry;
use crate::parser::{
    extract_field_assignment_range, parse_prefab_overrides, OverrideValue, RecipeParser,
};
use crate::wiki::{EditResult, WikiClient};
use crate::{DstContext, TechReport};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::Instrument;

/// How a job may write to the wiki.
///
/// - `Interactive`: ask via [`Reporter::confirm`] before every write (CLI default).
/// - `AutoConfirm`: apply writes without asking (`--yes`).
/// - `DryRun`: never write; artifacts can still be written to `--output`
///   files so the diff can be reviewed offline (`--dry-run`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    #[default]
    Interactive,
    AutoConfirm,
    DryRun,
}

impl WriteMode {
    pub fn name(self) -> &'static str {
        match self {
            WriteMode::Interactive => "interactive",
            WriteMode::AutoConfirm => "auto_confirm",
            WriteMode::DryRun => "dry_run",
        }
    }
}

/// Terminal decision for one wiki-write opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDecision {
    Apply,
    /// Do not write; the payload explains why (machine-readable).
    Skip(&'static str),
}

/// Pure decision function shared by every wiki-write path.
///
/// Note that `confirmed` is only consulted in `Interactive` mode: the
/// reporter has already answered the prompt by the time this runs.
fn decide_write(mode: WriteMode, confirmed: bool) -> WriteDecision {
    match mode {
        WriteMode::DryRun => WriteDecision::Skip("dry_run"),
        WriteMode::AutoConfirm => WriteDecision::Apply,
        WriteMode::Interactive if confirmed => WriteDecision::Apply,
        WriteMode::Interactive => WriteDecision::Skip("declined"),
    }
}

/// Everything the WebUI (or CLI) needs to describe one unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobKind {
    ParsePo {
        input: String,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    MapNames {
        input: String,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        compare: Option<String>,
        #[serde(default)]
        merge: bool,
        #[serde(default)]
        version: Option<String>,
    },
    MapRecipes {
        input: String,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        compare: Option<String>,
        #[serde(default)]
        merge: bool,
        #[serde(default)]
        po_file: Option<String>,
        #[serde(default)]
        version: Option<String>,
    },
    MaintainItemTable {
        #[serde(default)]
        output: Option<String>,
        /// Optional scripts snapshot directory name.
        #[serde(default)]
        snapshot: Option<String>,
    },
    MaintainDstRecipes {
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
    },
    MaintainCopyClip {
        #[serde(default)]
        r#type: Option<String>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
    },
    PrefabOverrides {
        input: String,
        #[serde(default)]
        output: Option<String>,
    },
    /// Harvest the wiki main namespace into the local corpus tree
    /// (docs/WIKI_CORPUS_PLAN.md). Read-only against the wiki; `full`
    /// ignores `touched`-based incremental skipping.
    /// Snapshot diff + impact report (M1): read-only pipeline.
    UpdateScan {
        old: String,
        new: String,
        #[serde(default)]
        out: Option<String>,
        /// 语料 host 根目录（wikis/<host>/）：提供时附加 Layer B 定级摘要
        #[serde(default)]
        corpus: Option<String>,
        /// 输出 fn 标注骨架到该目录（prefabs 前 50 文件 + hound.lua）
        #[serde(default)]
        annotate: Option<String>,
    },
    /// Build the code association atlas (index + tuning) from a scripts root.
    UpdateIndex {
        root: String,
        #[serde(default)]
        out: Option<String>,
    },
    CorpusSync {
        #[serde(default)]
        full: bool,
        #[serde(default)]
        dir: Option<String>,
    },
    /// Rebuild derived corpus indexes from the local corpus tree
    /// (docs/CORPUS_CODE_ATLAS_CONTRACT.md). Local-only, no wiki traffic.
    CorpusIndex {
        #[serde(default)]
        dir: Option<String>,
        /// Optional code-side index.json for the join calibration report.
        #[serde(default)]
        join: Option<String>,
    },
}

impl JobKind {
    /// Human readable name used in job listings.
    pub fn name(&self) -> &'static str {
        match self {
            JobKind::ParsePo { .. } => "parse-po",
            JobKind::MapNames { .. } => "map-names",
            JobKind::MapRecipes { .. } => "map-recipes",
            JobKind::MaintainItemTable { .. } => "maintain-item-table",
            JobKind::MaintainDstRecipes { .. } => "maintain-dst-recipes",
            JobKind::MaintainCopyClip { .. } => "maintain-copyclip",
            JobKind::PrefabOverrides { .. } => "prefab-overrides",
            JobKind::CorpusSync { .. } => "corpus-sync",
            JobKind::CorpusIndex { .. } => "corpus-index",
            JobKind::UpdateIndex { .. } => "update-index",
            JobKind::UpdateScan { .. } => "update-scan",
        }
    }

    /// Whether this job may edit pages on the wiki (needs explicit confirm).
    pub fn touches_wiki(&self) -> bool {
        matches!(
            self,
            JobKind::MaintainItemTable { .. }
                | JobKind::MaintainDstRecipes { .. }
                | JobKind::MaintainCopyClip { .. }
        )
    }
}

fn make_ctx(snapshot: &Option<String>) -> Result<DstContext> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|e| Error::EnvVarNotFound(format!("DST__ROOT: {}", e)))?;
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
    reporter.stage(&format!("{}（写入模式：{}）", kind.name(), mode.name()));
    // `Instrument` instead of `enter()`: an EnteredSpan is !Send and would
    // make this future unusable with tokio::spawn (WebUI job manager).
    execute_job_inner(kind, reporter, mode)
        .instrument(tracing::info_span!(
            "job",
            job = kind.name(),
            write_mode = mode.name()
        ))
        .await
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
        JobKind::PrefabOverrides { input, output } => {
            run_prefab_overrides(input, opt_path(output), reporter).await
        }
        JobKind::CorpusSync { full, dir } => {
            run_corpus_sync(*full, dir.as_deref(), reporter, mode).await
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
    }
}

// ---------------------------------------------------------------------------
// Local-file commands (no game dir required)
// ---------------------------------------------------------------------------

/// `update-scan`: snapshot diff -> association attribution -> Tier0 rules.
///
/// Read-only: writes `impact.json` + `changes.patch` under the output dir.
async fn run_update_scan(
    old_id: &str,
    new_id: &str,
    out: Option<PathBuf>,
    corpus: Option<&str>,
    annotate: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("快照差异与影响评估");
    let store = crate::update::SnapshotStore::from_env()?;
    let old_root = store.resolve(old_id);
    let new_root = if new_id == "current" {
        store.current_dir()
    } else {
        store.resolve(new_id)
    };
    if !old_root.is_dir() || !new_root.is_dir() {
        return Err(crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "snapshot dirs missing: {} or {}",
                old_root.display(),
                new_root.display()
            ),
        )));
    }

    reporter.log(format!(
        "diff: {} <-> {}",
        old_root.display(),
        new_root.display()
    ));
    let diff = crate::update::TreeDiff::diff_trees(&old_root, &new_root)?;
    reporter.log(format!("变更文件 {} 个", diff.files.len()));

    reporter.stage("构建关联索引（新树）");
    let atlas = crate::update::build_atlas_from_dir(&new_root)?;

    let old_tuning_src = std::fs::read_to_string(old_root.join("tuning.lua")).ok();
    let old_tuning = old_tuning_src
        .as_deref()
        .map(crate::update::index::tuning::build_tuning)
        .transpose()?;
    let mut report = crate::update::build_report(
        &diff,
        &atlas.index,
        old_tuning.as_ref(),
        Some(&atlas.tuning),
        old_id,
        new_id,
    );

    // Layer B (report-only): pair loot facts across snapshots and grade the
    // changes against the local corpus. Read-only; skipped when no corpus
    // dir was supplied.
    let mut graded_layer_b: Option<Vec<crate::update::GradedChange>> = None;
    if let Some(corpus_root) = corpus {
        reporter.stage("Layer B 定级（loot 配对，只读）");
        reporter.log("构建旧树索引（loot 配对需要）".to_string());
        let old_atlas = crate::update::build_atlas_from_dir(&old_root)?;
        let mut changes =
            crate::update::pair_loot_changes(&old_atlas.index.loot, &atlas.index.loot);
        let old_stats =
            crate::update::collect_stat_records(&old_root, &old_atlas.index, &old_atlas.tuning);
        let new_stats = crate::update::collect_stat_records(&new_root, &atlas.index, &atlas.tuning);
        let stat_changes = crate::update::pair_stat_changes(&old_stats, &new_stats);
        reporter.log(format!("配对出数值变更 {} 条", stat_changes.len()));
        let old_consts = crate::update::collect_brain_consts(&old_root)?;
        let new_consts = crate::update::collect_brain_consts(&new_root)?;
        let const_changes = crate::update::pair_const_changes(&old_consts, &new_consts);
        reporter.log(format!("配对出行为常量变更 {} 条", const_changes.len()));
        changes.extend(const_changes);
        changes.extend(stat_changes);
        reporter.log(format!("配对变更合计 {} 条", changes.len()));
        let view = crate::update::CorpusPageView::load(std::path::Path::new(corpus_root))?;
        let graded = crate::update::grade_changes(&changes, &view);
        report.layer_b = Some(crate::update::grade::LayerBSummary::from(graded.as_slice()));
        reporter.log(format!(
            "Layer B: {} 条定级 {:?}",
            graded.len(),
            crate::update::summarize_grades(&graded)
        ));
        graded_layer_b = Some(graded);
    }

    let changed_paths: Vec<String> = report.files.iter().map(|f| f.path.clone()).collect();
    let tier0 = crate::update::evaluate_rules(&crate::update::default_rules(), &changed_paths);
    for hit in &tier0 {
        reporter.log(format!(
            "Tier0 [{}] {} 文件 → jobs: {}",
            hit.label,
            hit.files.len(),
            if hit.jobs.is_empty() {
                "(无自动作业)".to_string()
            } else {
                hit.jobs.join(", ")
            }
        ));
    }

    let out_dir = out.unwrap_or_else(|| {
        PathBuf::from("output")
            .join("scan")
            .join(format!("{old_id}_{new_id}"))
    });
    std::fs::create_dir_all(&out_dir)?;

    // Persist draft-eligible suggestions for human comparison (L1).
    if let Some(annotate_dir) = annotate {
        reporter.stage("fn 标注骨架生成");
        // hound.lua + 按字母序前 50 个 prefab 文件
        let mut files: Vec<&str> = atlas
            .index
            .fn_ranges
            .keys()
            .filter(|f| f.starts_with("prefabs/") && f.ends_with(".lua"))
            .map(|s| s.as_str())
            .collect();
        files.sort();
        let mut chosen: Vec<&str> = files.iter().copied().take(50).collect();
        if !chosen.contains(&"prefabs/hound.lua") {
            chosen.insert(0, "prefabs/hound.lua");
        }
        let written = crate::update::batch_annotate(
            &atlas.index,
            &std::path::PathBuf::from(annotate_dir),
            &chosen,
        )?;
        reporter.log(format!(
            "标注骨架 {} 个文件 → {}",
            written.len(),
            annotate_dir
        ));
    }

    if let Some(graded) = &graded_layer_b {
        let path = out_dir.join("layer_b_rows.json");
        std::fs::write(&path, serde_json::to_string_pretty(graded)?)?;
        let drafts = graded
            .iter()
            .filter(|g| g.tier == crate::update::GradeTier::SuggestDraft)
            .count();
        reporter.log(format!(
            "Layer B 明细 {} 条（建议 {}）→ {}",
            graded.len(),
            drafts,
            path.display()
        ));
    }

    let summary = serde_json::json!({
        "old": report.old_id,
        "new": report.new_id,
        "changed_files": report.files.len(),
        "affected_entities": report.affected_entities.len(),
        "tuning_added": report.tuning.added.len(),
        "layer_b": report.layer_b,
        "tuning_removed": report.tuning.removed.len(),
        "tuning_changed": report.tuning.changed.len(),
        "tier0_hits": tier0.len(),
    });
    std::fs::write(
        out_dir.join("impact.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "report": report,
            "tier0": tier0,
        }))?,
    )?;
    let patch = diff.to_patch(&old_root, &new_root)?;
    std::fs::write(out_dir.join("changes.patch"), &patch)?;

    reporter.log(format!("受影响实体 {}", report.affected_entities.len()));
    reporter.log(format!("已写入 {}", out_dir.display()));

    Ok(summary)
}

/// `update-index`: build the association atlas and cache it under
/// `output/atlas/<build>/` (or an explicit `--out` directory).
///
/// Purely local: reads game scripts, writes two JSON artifacts, no wiki I/O.
async fn run_update_index(
    root: &str,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("构建代码关联索引");
    let root_path = std::path::Path::new(root);
    let atlas = crate::update::build_atlas_from_dir(root_path)?;

    let build_dir = atlas
        .build_id
        .clone()
        .or_else(|| {
            // Snapshot directories follow databundles/scripts_<yyyymmddhhmm>;
            // fall back to their timestamp so caches land per-build.
            root_path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .filter(|n| n.starts_with("scripts_"))
                .map(|n| n.trim_start_matches("scripts_").to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let out_dir = out.unwrap_or_else(|| PathBuf::from("output").join("atlas").join(build_dir));
    std::fs::create_dir_all(&out_dir)?;

    let index_path = out_dir.join("index.json");
    std::fs::write(&index_path, serde_json::to_string_pretty(&atlas.index)?)?;
    let tuning_path = out_dir.join("tuning.json");
    std::fs::write(&tuning_path, serde_json::to_string_pretty(&atlas.tuning)?)?;

    reporter.log(format!(
        "{} 文件 / {} 边 / {} 行为实参 / TUNING 标量 {}",
        atlas.index.scanned_files,
        atlas.index.edges.len(),
        atlas.index.behaviour_calls.len(),
        atlas.tuning.values.len()
    ));
    reporter.log(format!("已写入 {}", out_dir.display()));

    Ok(serde_json::json!({
        "schema_version": atlas.schema_version,
        "build_id": atlas.build_id,
        "scanned_files": atlas.index.scanned_files,
        "edges": atlas.index.edges.len(),
        "behaviour_calls": atlas.index.behaviour_calls.len(),
        "unresolved": atlas.index.unresolved.len(),
        "tuning_values": atlas.tuning.values.len(),
        "output": out_dir,
    }))
}

async fn run_parse_po(
    input: &str,
    output: Option<PathBuf>,
    category: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析 PO 文件");
    let po_file = crate::parser::PoParser::parse_from_file(path_str(input)?)?;
    let entries = if let Some(cat) = category {
        po_file.filter_by_category(&cat)
    } else {
        po_file.entries.iter().collect::<Vec<_>>()
    };

    reporter.log(format!("共 {} 条目", entries.len()));

    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&entries)?;
        std::fs::write(&output_path, json)?;
        reporter.log(format!("已写入 {} 条目到 {:?}", entries.len(), output_path));
        Ok(serde_json::json!({ "entries": entries.len(), "output": output_path }))
    } else {
        for entry in entries.iter().take(10) {
            reporter.log(format!("{:?}", entry));
        }
        reporter.log(format!("... 共 {} 条目", entries.len()));
        Ok(serde_json::json!({ "entries": entries.len() }))
    }
}

async fn run_map_names(
    input: &str,
    output: Option<PathBuf>,
    compare: Option<String>,
    merge: bool,
    version: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析 PO 文件");
    let po_file = crate::parser::PoParser::parse_from_file(path_str(input)?)?;
    let names_entries: Vec<PoEntry> = po_file
        .entries
        .iter()
        .filter(|e| {
            e.msgctxt
                .as_ref()
                .map(|ctx: &String| ctx.starts_with("STRINGS.NAMES."))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    reporter.log(format!("找到 {} 条 NAMES 条目", names_entries.len()));

    let converter = WikiDataConverter::new();
    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from patch {}", version_str);
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的实体代码、中英文名及图片对照表"
    }));

    reporter.stage("生成维基数据");
    let wiki_data = if merge {
        let compare_path = compare
            .clone()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(&compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        converter.convert_with_history(&names_entries, &sources, &historical_data, description)
    } else {
        converter.convert_to_wiki_json(&names_entries, &sources, description)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            reporter.log(compare_and_report(&wiki_data, &historical_data));
        }
    }

    finish_json_output(&wiki_data, output, reporter)
}

async fn run_map_recipes(
    input: &str,
    output: Option<PathBuf>,
    compare: Option<String>,
    merge: bool,
    po_file: Option<String>,
    version: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析配方 Lua");
    let lua_content = std::fs::read_to_string(input)?;
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&lua_content, Some(input))?;

    reporter.log(format!("找到 {} 个配方", recipes.len()));

    let converter = if let Some(po_path) = &po_file {
        match crate::parser::PoParser::parse_from_file(po_path) {
            Ok(po_data) => {
                reporter.log(format!(
                    "载入 {} 条 PO 条目用于描述查询",
                    po_data.entries.len()
                ));
                WikiDataConverter::with_po_entries(po_data.entries.clone())
            }
            Err(e) => {
                reporter.log(format!("警告：PO 文件加载失败：{}", e));
                WikiDataConverter::new()
            }
        }
    } else {
        WikiDataConverter::new()
    };

    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from patch {}", version_str);
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的合成配方列表"
    }));

    reporter.stage("生成维基数据");
    let wiki_data = if merge {
        let compare_path = compare
            .clone()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(&compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        let mut data = converter.convert_recipes(&recipes, &sources, description);
        crate::models::Recipe::merge_with_history(&mut data, &historical_data);
        data
    } else {
        converter.convert_recipes(&recipes, &sources, description)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            reporter.log(compare_and_report(&wiki_data, &historical_data));
        }
    }

    finish_json_output(&wiki_data, output, reporter)
}

async fn run_prefab_overrides(
    input: &str,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析预制体重定向");
    let lua_content = std::fs::read_to_string(input)?;
    let overrides = parse_prefab_overrides(&lua_content)?;

    reporter.log(format!("找到 {} 条预制体重定向", overrides.len()));

    let mut mapping: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for override_info in overrides {
        let prefab_name = override_info.prefab_name;
        let override_value = match override_info.override_name {
            OverrideValue::Static(s) => serde_json::json!({
                "override_name": s,
                "type": "static"
            }),
            OverrideValue::Dynamic(s) => serde_json::json!({
                "override_name": s,
                "type": "dynamic"
            }),
            OverrideValue::Unknown => serde_json::json!({
                "override_name": null,
                "type": "unknown"
            }),
        };
        mapping.insert(prefab_name, override_value);
    }

    let json_output = serde_json::to_string_pretty(&mapping)?;

    if let Some(output_path) = output {
        std::fs::write(&output_path, &json_output)?;
        reporter.log(format!(
            "已写入 {} 条映射到 {:?}",
            mapping.len(),
            output_path
        ));
        Ok(serde_json::json!({ "overrides": mapping.len(), "output": output_path }))
    } else {
        reporter.log(json_output);
        Ok(serde_json::json!({ "overrides": mapping.len() }))
    }
}

// ---------------------------------------------------------------------------
// Corpus harvesting (wiki read-only, no game dir required)
// ---------------------------------------------------------------------------

async fn run_corpus_sync(
    full: bool,
    dir: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    // Anonymous reads suffice; the client only needs the HUIJI__* config to
    // know host/headers. Login is deliberately skipped — this job never edits.
    let client = WikiClient::from_env()?;
    let base_dir = PathBuf::from(dir.unwrap_or("wikis"));
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

async fn run_maintain_item_table(
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki_mut().login().await?;

    reporter.stage("解析 chinese_s.po");
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
    let names_entries: Vec<PoEntry> = po_file
        .entries
        .iter()
        .filter(|e| {
            e.msgctxt
                .as_ref()
                .map(|ctx: &String| ctx.starts_with("STRINGS.NAMES."))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    reporter.log(format!("找到 {} 条 NAMES 条目", names_entries.len()));

    let converter = WikiDataConverter::new();

    reporter.stage("拉取维基历史数据");
    let page_title = "Data:ItemTable.tabx";
    let historical_data = match ctx.wiki().get_json_data(page_title).await {
        Ok(historical_json) => Some(WikiDataConverter::parse_wiki_json(
            &historical_json.to_string(),
        )?),
        Err(e) => {
            reporter.log(format!("警告：获取历史数据失败：{}", e));
            None
        }
    };

    let sources = ctx.sources();
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的实体代码、中英文名及图片对照表"
    }));
    let mut wiki_data = converter.convert_to_wiki_json(&names_entries, &sources, description);

    let mut merged = false;
    if let Some(ref historical) = historical_data {
        PoEntry::merge_with_history(&mut wiki_data, historical);
        reporter.log(compare_and_report(&wiki_data, historical));
        merged = true;
    }

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };
    let summary = output_json_result_with_update(&wctx, page_title, &wiki_data, output).await?;
    Ok(serde_json::json!({ "merged_with_history": merged, "summary": summary }))
}

async fn run_maintain_dst_recipes(
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki_mut().login().await?;

    reporter.stage("解析 recipes.lua");
    let recipes_string = ctx.read_script_file("scripts/recipes.lua")?;

    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_string, Some("scripts/recipes.lua"))?;
    reporter.log(format!("共 {} 个配方", recipes.len()));

    reporter.stage("对比维基科技数据");
    let mut tech_report = TechReport::from_recipes(&recipes);
    match ctx.wiki().get_page("模块:RenderRecsByIngre/Data").await {
        Ok(page) => {
            if let Some(content) = &page.content {
                tech_report.compare_with_wiki(content);
                reporter.log(tech_report.generate_report());
            } else {
                reporter.log("警告：维基页面无内容".to_string());
            }
        }
        Err(e) => {
            reporter.log(format!("警告：获取科技数据失败：{}", e));
        }
    }

    reporter.stage("解析 chinese_s.po（描述查询）");
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
    reporter.log(format!("载入 {} 条 PO 条目", po_file.entries.len()));

    let converter = WikiDataConverter::with_po_entries(po_file.entries.clone());

    reporter.stage("拉取维基历史数据");
    let page_title = "Data:DSTRecipes.tabx";
    let historical_data = match ctx.wiki().get_json_data(page_title).await {
        Ok(historical_json) => Some(WikiDataConverter::parse_wiki_json(
            &historical_json.to_string(),
        )?),
        Err(e) => {
            reporter.log(format!("警告：获取历史数据失败：{}", e));
            None
        }
    };

    let sources = ctx.sources();
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的合成配方列表"
    }));
    let mut wiki_data = converter.convert_recipes(&recipes, &sources, description);

    let mut merged = false;
    if let Some(ref historical) = historical_data {
        crate::models::Recipe::merge_with_history(&mut wiki_data, historical);
        reporter.log(compare_and_report(&wiki_data, historical));
        merged = true;
    }

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };
    let summary = output_json_result_with_update(&wctx, page_title, &wiki_data, output).await?;
    Ok(
        serde_json::json!({ "merged_with_history": merged, "recipe_count": recipes.len(), "summary": summary }),
    )
}

async fn run_maintain_copyclip(
    r#type: Option<&str>,
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki_mut().login().await?;

    let types_to_run: Vec<String> = if let Some(t) = r#type {
        vec![t.to_lowercase()]
    } else {
        vec![
            "recipe_builder_tag_lookup".to_string(),
            "tech".to_string(),
            "crafting_filters".to_string(),
            "crafting_names".to_string(),
        ]
    };

    let mut results = serde_json::Map::new();
    for t in types_to_run {
        reporter.stage(&format!("运行 {}", t));
        let summary = match t.as_str() {
            "recipe_builder_tag_lookup" | "rbtl" => {
                maintain_recipe_builder_tag_lookup(&mut ctx, output.clone(), reporter, mode).await?
            }
            "tech" => maintain_tech(&mut ctx, output.clone(), reporter, mode).await?,
            "crafting_filters" | "filters" => {
                maintain_crafting_filters(&mut ctx, output.clone(), reporter, mode).await?
            }
            "crafting_names" | "names" => {
                maintain_crafting_names(&mut ctx, output.clone(), reporter, mode).await?
            }
            _ => {
                return Err(Error::Config(format!(
                    "未知的 copyclip 类型: {}。有效类型: recipe_builder_tag_lookup (rbtl), tech, crafting_filters (filters), crafting_names (names)",
                    t
                )));
            }
        };
        results.insert(t, summary);
    }

    Ok(serde_json::Value::Object(results))
}

async fn maintain_recipe_builder_tag_lookup(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let debugcommands_string = ctx.read_script_file("scripts/debugcommands.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/RecipeBuilderTagLookup";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 debugcommands.lua 提取 RECIPE_BUILDER_TAG_LOOKUP...".to_string());
    let result = crate::copyclip::process_copyclip(
        &debugcommands_string,
        "RECIPE_BUILDER_TAG_LOOKUP",
        &target_content,
    )?;

    reporter.log(format!(
        "CopyClip 完成！提取内容长度: {} 字节",
        result.extracted_content.len()
    ));

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &result.updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

async fn maintain_tech(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let constants_string = ctx.read_script_file("scripts/constants.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/Tech";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 constants.lua 提取 TECH...".to_string());
    let result = crate::copyclip::process_copyclip(&constants_string, "TECH", &target_content)?;

    reporter.log(format!(
        "CopyClip 完成！提取内容长度: {} 字节",
        result.extracted_content.len()
    ));

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &result.updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

async fn maintain_crafting_filters(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let filter_string = ctx.read_script_file("scripts/recipes_filter.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/CraftingFilters";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 recipes_filter.lua 提取 CRAFTING_FILTERS 配方列表...".to_string());
    let field_location = extract_field_assignment_range(
        &filter_string,
        "CRAFTING_FILTERS.CHARACTER.recipes",
        "CRAFTING_FILTERS.DECOR.recipes",
    )?;

    reporter.log(format!(
        "提取内容长度: {} 字节",
        field_location.content.len()
    ));

    let marker_range = crate::copyclip::CopyClipProcessor::find_marker_range(&target_content)?;
    let updated_content = crate::copyclip::CopyClipProcessor::replace_between_markers(
        &target_content,
        &marker_range,
        &field_location.content,
    );

    reporter.log("CopyClip 完成！".to_string());

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

async fn maintain_crafting_names(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;

    let station_prefix = "STRINGS.UI.CRAFTING_STATION_FILTERS.";
    let filter_prefix = "STRINGS.UI.CRAFTING_FILTERS.";

    let mut crafting_stations: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut craftings: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for entry in &po_file.entries {
        if let Some(ref entry_ctx) = entry.msgctxt {
            if let Some(key) = entry_ctx.strip_prefix(station_prefix) {
                crafting_stations.insert(
                    key.to_string(),
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            } else if let Some(key) = entry_ctx.strip_prefix(filter_prefix) {
                craftings.insert(
                    key.to_string(),
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            }
        }
    }

    let stations_len = crafting_stations.len();
    let craftings_len = craftings.len();

    let crafting_names = serde_json::json!({
        "crafting_stations": crafting_stations,
        "craftings": craftings
    });

    let json_content = serde_json::to_string_pretty(&crafting_names)?;
    reporter.log(format!(
        "找到 {} 个制作站点和 {} 个制作分类",
        stations_len, craftings_len
    ));

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/CraftingNames";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("定位 [[ 与 ]] 标记...".to_string());
    let start_marker = "[[";
    let end_marker = "]]";

    let start_pos = target_content
        .find(start_marker)
        .ok_or_else(|| Error::ParseError("'[[' marker not found".to_string()))?;
    let end_pos = target_content
        .rfind(end_marker)
        .ok_or_else(|| Error::ParseError("']]' marker not found".to_string()))?;

    if start_pos >= end_pos {
        return Err(Error::ParseError(
            "'[[' must appear before ']]'".to_string(),
        ));
    }

    let updated_content = format!(
        "{}{}\n{}",
        &target_content[..start_pos + start_marker.len()],
        json_content,
        &target_content[end_pos..]
    );

    reporter.log("CopyClip 完成！".to_string());

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

// ---------------------------------------------------------------------------
// Shared output helpers (wiki-write confirmation flows through the reporter)
// ---------------------------------------------------------------------------

fn finish_json_output(
    wiki_data: &crate::mapping::WikiJsonData,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    if let Some(output_path) = output {
        let json = WikiDataConverter::to_json_string(wiki_data)?;
        std::fs::write(&output_path, json)?;
        reporter.log(format!(
            "已写入 {} 条记录到 {:?}",
            wiki_data.data.len(),
            output_path
        ));
        return Ok(serde_json::json!({ "records": wiki_data.data.len(), "output": output_path }));
    }

    reporter.log(format!("共 {} 条记录", wiki_data.data.len()));
    Ok(serde_json::json!({ "records": wiki_data.data.len() }))
}

/// Shared context for the wiki-write helper paths, keeping their argument
/// lists small.
struct WriteCtx<'a> {
    client: &'a WikiClient,
    reporter: &'a dyn Reporter,
    mode: WriteMode,
}

async fn output_json_result_with_update(
    ctx: &WriteCtx<'_>,
    page_title: &str,
    wiki_data: &crate::mapping::WikiJsonData,
    output: Option<PathBuf>,
) -> Result<serde_json::Value> {
    let client = ctx.client;
    let reporter = ctx.reporter;
    let mode = ctx.mode;
    let new_json = WikiDataConverter::to_json_string(wiki_data)?;

    if let Some(output_path) = output {
        std::fs::write(&output_path, &new_json)?;
        reporter.log(format!(
            "已写入 {} 条记录到 {:?}",
            wiki_data.data.len(),
            output_path
        ));
    }

    // Fetch the current revision once: the content feeds the comparison and
    // its timestamp becomes `basetimestamp` for a conflict-safe edit.
    let page = match client.get_page(page_title).await {
        Ok(p) => p,
        Err(e) => {
            reporter.log(format!("警告：获取维基当前数据失败：{}", e));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };

    let historical_content = match page.content {
        Some(c) => c,
        None => {
            reporter.log(format!("警告：页面 '{}' 无内容", page_title));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };
    // Round-trip through serde so both sides are compared with identical
    // pretty-printing, independent of how the page was last saved.
    let historical_json: String = match serde_json::from_str::<serde_json::Value>(
        historical_content.trim(),
    ) {
        Ok(v) => serde_json::to_string_pretty(&v)?,
        Err(e) => {
            reporter.log(format!("警告：解析维基历史数据失败：{}", e));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };

    if new_json.trim() == historical_json.trim() {
        reporter.log("未检测到变化。".to_string());
        return Ok(serde_json::json!({ "status": "no_changes" }));
    }

    reporter.log("--- 检测到变化 ---".to_string());
    reporter.log(crate::diff_lines(&historical_json, &new_json));

    // Only Interactive mode consults the reporter; AutoConfirm/DryRun must
    // not block on stdin in unattended runs.
    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm("更新维基页面？")
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            reporter.log(format!("已跳过更新维基页面（{}）。", reason));
            Ok(serde_json::json!({ "status": reason, "page": page_title }))
        }
        WriteDecision::Apply => {
            reporter.log(format!("正在更新维基页面: {}", page_title));

            let edit_result = apply_wiki_edit(
                client,
                page_title,
                &new_json,
                page.last_rev_timestamp.as_deref(),
            )
            .await?;

            Ok(serde_json::json!(
                { "status": "updated", "page": page_title, "newrevid": edit_result.newrevid }))
        }
    }
}

/// Applies one wiki edit inside a structured-log span.
///
/// The edit carries `basetimestamp` (from the revision we based our change on)
/// so MediaWiki rejects it with `editconflict` instead of silently
/// overwriting concurrent edits.
async fn apply_wiki_edit(
    client: &WikiClient,
    page_title: &str,
    text: &str,
    basetimestamp: Option<&str>,
) -> Result<EditResult> {
    let span = tracing::info_span!("wiki_edit", page = %page_title);
    async move {
        let edit_result = client
            .edit_page(
                page_title,
                text,
                Some("Update via dst-huiji-wiki tool"),
                false,
                basetimestamp,
            )
            .await;

        match &edit_result {
            Ok(r) => tracing::info!(
                oldrevid = r.oldrevid,
                newrevid = r.newrevid,
                "wiki edit applied"
            ),
            Err(e) => tracing::warn!(error = %e, "wiki edit failed"),
        }

        edit_result
    }
    .instrument(span)
    .await
}

async fn output_copyclip_result_with_update(
    ctx: &WriteCtx<'_>,
    page_title: &str,
    target_content: &str,
    updated_content: &str,
    output: Option<PathBuf>,
    base_timestamp: Option<String>,
) -> Result<serde_json::Value> {
    let client = ctx.client;
    let reporter = ctx.reporter;
    let mode = ctx.mode;
    if target_content == updated_content {
        reporter.log("未检测到变化。".to_string());
        return Ok(serde_json::json!({ "status": "no_changes" }));
    }

    reporter.log("--- 检测到变化 ---".to_string());
    reporter.log(crate::diff_lines(target_content, updated_content));

    if let Some(output_path) = output {
        std::fs::write(&output_path, updated_content)?;
        reporter.log(format!("已写入更新内容到 {:?}", output_path));
    }

    // Only Interactive mode consults the reporter; AutoConfirm/DryRun must
    // not block on stdin in unattended runs.
    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm("更新维基页面？")
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            reporter.log(format!("已跳过更新维基页面（{}）。", reason));
            Ok(serde_json::json!({ "status": reason, "page": page_title }))
        }
        WriteDecision::Apply => {
            reporter.log(format!("正在更新维基页面: {}", page_title));

            let edit_result = apply_wiki_edit(
                client,
                page_title,
                updated_content,
                base_timestamp.as_deref(),
            )
            .await?;

            Ok(serde_json::json!(
                { "status": "updated", "page": page_title, "newrevid": edit_result.newrevid }))
        }
    }
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

    #[test]
    fn test_decide_write_table() {
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
