//! `update` 域的作业入口（自 service/mod.rs 下沉）。
//!
//! run_* 只依赖本模块与 platform 层，不含 JobKind/分发知识。

use crate::error::Result;
use crate::platform::progress::Reporter;
use std::path::PathBuf;

fn opt_path(p: &Option<String>) -> Option<PathBuf> {
    p.as_ref().map(PathBuf::from)
}

pub(crate) fn cached_scan_summary(out_dir: &std::path::Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(out_dir.join("impact.json"))?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let report = v.get("report").cloned().unwrap_or_default();
    let tier0 = v.get("tier0").cloned().unwrap_or_default();
    Ok(serde_json::json!({
        "cached": true,
        "output_dir": out_dir.display().to_string(),
        "old": report.get("old_id").cloned().unwrap_or_default(),
        "new": report.get("new_id").cloned().unwrap_or_default(),
        "changed_files": report.get("files").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "affected_entities": report.get("affected_entities").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "tuning_added": report.get("tuning").and_then(|t| t.get("added")).and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "tuning_removed": report.get("tuning").and_then(|t| t.get("removed")).and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "tuning_changed": report.get("tuning").and_then(|t| t.get("changed")).and_then(|x| x.as_object()).map(|m| m.len()).unwrap_or(0),
        "layer_b": report.get("layer_b").cloned().unwrap_or(serde_json::Value::Null),
        "tier0_hits": tier0.as_array().map(|a| a.len()).unwrap_or(0),
    }))
}

/// `update-scan`: snapshot diff -> association attribution -> Tier0 rules.
///
/// Read-only: writes `impact.json` + `changes.patch` under the output dir.
pub(crate) async fn run_update_scan(
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

    let out_dir = out.unwrap_or_else(|| {
        PathBuf::from("output")
            .join("scan")
            .join(format!("{old_id}_{new_id}"))
    });
    std::fs::create_dir_all(&out_dir)?;
    if let Some(state) = crate::update::state::ScanState::load(&out_dir) {
        if state.old_id == old_id && state.new_id == new_id && state.status == "completed" {
            reporter.log("检测到已完成 state.json，跳过重算".to_string());
            reporter.log(format!("输出目录 {}", out_dir.display()));
            return cached_scan_summary(&out_dir);
        }
    }
    let mut state = crate::update::state::ScanState::new(old_id, new_id);
    state.save(&out_dir)?;

    reporter.log(format!(
        "diff: {} <-> {}",
        old_root.display(),
        new_root.display()
    ));
    let diff = crate::update::TreeDiff::diff_trees(&old_root, &new_root)?;
    state.mark_stage("diff", true, &out_dir)?;
    reporter.log(format!("变更文件 {} 个", diff.files.len()));

    reporter.stage("构建关联索引（新树）");
    let atlas = crate::update::build_atlas_from_dir(&new_root)?;
    state.mark_stage("atlas", true, &out_dir)?;

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
        state.mark_stage("layer_b", true, &out_dir)?;
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
        let mut chosen: Vec<&str> = files.clone();
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
        crate::platform::fs::write_json_atomic(&path, &graded)?;
        let drafts = graded
            .iter()
            .filter(|g| g.tier == crate::update::GradeTier::SuggestDraft)
            .count();

        // Formal create-check list: entities the pipeline believes need a wiki
        // page but currently have no page mapping.
        let create_checks: Vec<&crate::update::GradedChange> = graded
            .iter()
            .filter(|g| g.tier == crate::update::GradeTier::CreateCheck)
            .collect();
        let cc_json = out_dir.join("create_check.json");
        crate::platform::fs::write_json_atomic(&cc_json, &create_checks)?;
        let mut cc_md = String::from("# Create-Check 清单\n\n");
        for g in &create_checks {
            cc_md.push_str(&format!(
                "- `{}` {}：{}\n",
                g.prefab, g.field, g.change_text
            ));
        }
        let cc_md_path = out_dir.join("create_check.md");
        std::fs::write(&cc_md_path, cc_md)?;
        reporter.log(format!(
            "Layer B 明细 {} 条（建议 {}，create_check {}）→ {}",
            graded.len(),
            drafts,
            create_checks.len(),
            path.display()
        ));
        reporter.log(format!(
            "Create-Check 清单 {} 条 → {} / {}",
            create_checks.len(),
            cc_json.display(),
            cc_md_path.display()
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
    crate::platform::fs::write_json_atomic(
        &out_dir.join("impact.json"),
        &serde_json::json!({
            "report": report,
            "tier0": tier0,
        }),
    )?;
    let patch = diff.to_patch(&old_root, &new_root)?;
    std::fs::write(out_dir.join("changes.patch"), &patch)?;

    reporter.log(format!("受影响实体 {}", report.affected_entities.len()));
    reporter.log(format!("已写入 {}", out_dir.display()));
    state.complete(&out_dir)?;

    Ok(summary)
}

/// `update-index`: build the association atlas and cache it under
/// `output/atlas/<build>/` (or an explicit `--out` directory).
///
/// Purely local: reads game scripts, writes two JSON artifacts, no wiki I/O.
pub(crate) async fn run_update_index(
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
    crate::platform::fs::write_json_atomic(&index_path, &atlas.index)?;
    let tuning_path = out_dir.join("tuning.json");
    crate::platform::fs::write_json_atomic(&tuning_path, &atlas.tuning)?;

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

/// `symbol-annotate`: build Page→Symbol evidence packs, render prompts, and
/// optionally consume LLM/human verdicts to produce coverage reports.
///
/// Local-only; never writes the wiki.
pub(crate) struct SymbolAnnotateParams {
    pub(crate) root: String,
    pub(crate) corpus: String,
    pub(crate) limit: usize,
    pub(crate) out: Option<String>,
    pub(crate) verdicts: Option<String>,
    pub(crate) llm: bool,
    pub(crate) batch_pages: usize,
    pub(crate) batch_max_chars: usize,
    pub(crate) skip_no_fact_pages: bool,
}

pub(crate) async fn run_symbol_annotate(
    params: &SymbolAnnotateParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let SymbolAnnotateParams {
        root,
        corpus,
        limit,
        out,
        verdicts,
        llm,
        batch_pages,
        batch_max_chars,
        skip_no_fact_pages,
    } = params;
    let out = opt_path(out);
    let verdicts = verdicts.as_deref();
    let (root, corpus) = (root.as_str(), corpus.as_str());
    reporter.stage("构建代码关联索引");
    let atlas = crate::update::build_atlas_from_dir(std::path::Path::new(root))?;

    reporter.stage("加载语料页面视图");
    let view = crate::update::CorpusPageView::load(std::path::Path::new(corpus))?;
    reporter.log(format!(
        "页面 {} 个 / facts {} 页",
        view.pages.len(),
        view.facts.len()
    ));

    reporter.stage("生成高引用 symbol 证据包");
    let packs = crate::update::build_symbol_evidence_packs(&atlas.index, &view, *limit);
    reporter.log(format!("生成 {} 个 symbol 证据包", packs.len()));
    if *llm {
        reporter.log(format!(
            "LLM 分批策略：每批 ≤ {} 页（0 = 单批）",
            batch_pages
        ));
    }

    let out_dir = out.unwrap_or_else(|| {
        PathBuf::from("output").join("symbol-annotate").join(
            atlas
                .build_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        )
    });
    std::fs::create_dir_all(&out_dir)?;

    let packs_path = out_dir.join("symbol_packs.json");
    crate::platform::fs::write_json_atomic(&packs_path, &packs)?;

    let mut prompts = String::from("# Symbol Annotation Prompts\n\n");
    for pack in &packs {
        let symbol = match &pack.symbol {
            crate::update::SymbolRef::File { path } => path.clone(),
            crate::update::SymbolRef::Fn { file, name } => format!("{file}#{name}"),
            crate::update::SymbolRef::Const { file, name } => format!("{file}#{name}"),
            crate::update::SymbolRef::Behaviour { name } => format!("behaviours/{name}"),
        };
        let semantics = format!("{}（代码证据待补充）", symbol);
        prompts.push_str(&crate::update::render_symbol_annotation_prompt(
            pack, &semantics,
        ));
        prompts.push('\n');
    }
    let prompts_path = out_dir.join("symbol_prompts.md");
    std::fs::write(&prompts_path, &prompts)?;

    let mut summary = serde_json::json!({
        "packs": packs.len(),
        "output": out_dir,
        "prompts": prompts_path,
        "packs_file": packs_path,
    });

    let mut llm_responses: Option<Vec<crate::update::SymbolAnnotationResponse>> = None;
    if let Some(vp) = verdicts {
        reporter.stage("读取标注结果并生成覆盖报告");
        let raw = std::fs::read_to_string(vp)?;
        let responses: Vec<crate::update::SymbolAnnotationResponse> = serde_json::from_str(&raw)?;
        llm_responses = Some(responses);
    } else if *llm {
        reporter.stage("LLM 标注");
        match crate::llm::LlmConfig::from_env() {
            None => {
                reporter
                    .log("未配置 LLM__API_KEY，跳过 LLM 标注，仅生成 Prompt/证据包".to_string());
            }
            Some(config) => {
                reporter.log(format!("使用模型 {} / {}", config.model, config.base_url));
                let mut responses = Vec::new();
                let mut batches_total = 0usize;
                let mut batches_failed = 0usize;
                let mut batches_skipped_local = 0usize;
                // Raw LLM responses are kept on disk first so a malformed
                // output can be diagnosed (or repaired) after the fact.
                let raw_dir = out_dir.join("llm_raw");
                std::fs::create_dir_all(&raw_dir)?;
                let system =
                    "你是 DST Huiji Wiki 的代码符号页面影响标注助手。必须严格按用户要求输出 JSON。";
                for pack in &packs {
                    let symbol = crate::update::symbol_display(&pack.symbol);
                    let raw_stem = symbol.replace(['/', '\\', '#', ':'], "_");
                    let semantics = format!("{symbol}（代码证据待补充）");

                    // Bounded per-request work: optionally keep zero-evidence
                    // pages out of the LLM path, then split the rest into
                    // evidence-ranked batches under both caps.
                    let (send_pids, local_verdicts) =
                        crate::update::partition_no_fact_pages(pack, *skip_no_fact_pages);
                    let mut working = pack.clone();
                    working.affected_pageids = send_pids;
                    let batches = crate::update::paginate_affected_pages(
                        &working,
                        *batch_pages,
                        *batch_max_chars,
                    );
                    let total_pages: usize = batches.iter().map(Vec::len).sum();
                    reporter.log(format!(
                        "标注 {symbol}：送审 {total_pages} 页分 {} 批（≤ {} 页 / ≤ {}B 每批）",
                        batches.len(),
                        batch_pages,
                        batch_max_chars,
                    ));
                    let mut sym_verdicts = Vec::new();
                    let mut sym_local = Vec::new();
                    if !local_verdicts.is_empty() {
                        reporter.log(format!(
                            "另有 {} 个零证据页走本地判定（不消耗 LLM）",
                            local_verdicts.len()
                        ));
                        batches_skipped_local += 1;
                        sym_local.extend(local_verdicts);
                    }
                    batches_total += batches.len();

                    for (bi, batch) in batches.iter().enumerate() {
                        let prompt = crate::update::render_symbol_annotation_prompt_for_pages(
                            pack, &semantics, batch,
                        );
                        // One immediate retry per batch; a twice-failed batch
                        // is skipped and accounted for instead of sinking the
                        // whole run.
                        let mut outcome = None;
                        for attempt in 1..=2 {
                            reporter.log(format!(
                                "批次 {}/{}（{} 页）第 {attempt} 次尝试…",
                                bi + 1,
                                batches.len(),
                                batch.len(),
                            ));
                            let raw_file = raw_dir.join(format!("{raw_stem}_b{:02}.json", bi + 1));
                            let result = async {
                                let raw = config
                                    .complete_streaming(system, &prompt, |ev| match ev {
                                        crate::llm::LlmStreamEvent::FirstToken { elapsed_secs } => {
                                            reporter.log(format!(
                                                "首个输出分片已到达（{elapsed_secs}s）"
                                            ));
                                        }
                                        crate::llm::LlmStreamEvent::Tick { chars } => {
                                            if chars == 0 {
                                                reporter.log("仍在等待模型输出…".to_string());
                                            } else {
                                                reporter.log(format!("流式接收中：{chars} 字符"));
                                            }
                                        }
                                        crate::llm::LlmStreamEvent::Done {
                                            chars,
                                            elapsed_secs,
                                        } => {
                                            reporter.log(format!(
                                                "输出完成：{chars} 字符 / {elapsed_secs}s"
                                            ));
                                        }
                                    })
                                    .await?;
                                std::fs::write(&raw_file, &raw).map_err(crate::error::Error::Io)?;
                                crate::update::parse_symbol_annotation_response(&raw).map_err(|e| {
                                    crate::error::Error::Llm(format!(
                                        "输出无法解析为 JSON（原始响应已保存到 {}）：{e}",
                                        raw_file.display()
                                    ))
                                })
                            }
                            .await;
                            match result {
                                Ok(verdicts) => {
                                    outcome = Some(Ok(verdicts));
                                    break;
                                }
                                Err(e) => outcome = Some(Err(e)),
                            }
                        }
                        match outcome.expect("loop ran at least once") {
                            Ok(verdicts) => sym_verdicts.extend(verdicts),
                            Err(e) => {
                                batches_failed += 1;
                                reporter.log(format!(
                                    "批次 {}/{} 两次失败，跳过：{e}",
                                    bi + 1,
                                    batches.len()
                                ));
                            }
                        }
                    }
                    // Guardrail: every requested page must end up answered.
                    let gap_filled = crate::update::fill_missing_requested(
                        &working.affected_pageids,
                        &mut sym_verdicts,
                    );
                    if gap_filled > 0 {
                        reporter.log(format!("对账补齐 {gap_filled} 页批次输出缺失的判定"));
                    }
                    sym_verdicts.append(&mut sym_local);
                    reporter.log(format!(
                        "标注 {} 完成（{} 条 verdict）",
                        symbol,
                        sym_verdicts.len()
                    ));
                    responses.push(crate::update::SymbolAnnotationResponse {
                        symbol,
                        verdicts: sym_verdicts,
                    });
                }
                summary["llm_batches"] = serde_json::json!({
                    "total": batches_total,
                    "failed": batches_failed,
                    "max_pages_per_batch": *batch_pages,
                    "max_input_chars_per_batch": *batch_max_chars,
                    "symbols_with_local_only_pages": batches_skipped_local,
                });
                if batches_failed > 0 {
                    reporter.log(format!(
                        "警告：{batches_failed}/{batches_total} 个批次失败被跳过"
                    ));
                }
                let verdicts_path = out_dir.join("symbol_verdicts.json");
                crate::platform::fs::write_json_atomic(&verdicts_path, &responses)?;
                reporter.log(format!(
                    "LLM 标注完成 {} 个 symbol → {}",
                    responses.len(),
                    verdicts_path.display()
                ));
                summary["llm_verdicts"] = serde_json::json!(verdicts_path);
                llm_responses = Some(responses);
            }
        }
    }

    if let Some(responses) = llm_responses {
        let reports = crate::update::build_coverage_reports(&responses);
        let cov_json = out_dir.join("symbol_coverage.json");
        crate::platform::fs::write_json_atomic(&cov_json, &reports)?;
        let mut md = String::from("# Symbol Coverage 报告\n\n");
        for r in &reports {
            md.push_str(&crate::update::render_coverage_report_md(r));
        }
        let cov_md = out_dir.join("symbol_coverage.md");
        std::fs::write(&cov_md, &md)?;
        reporter.log(format!(
            "覆盖报告 {} 个 symbol → {} / {}",
            reports.len(),
            cov_json.display(),
            cov_md.display()
        ));
        summary["coverage"] = serde_json::json!({
            "symbols": reports.len(),
            "json": cov_json,
            "markdown": cov_md,
        });
    }

    reporter.log(format!("已写入 {}", out_dir.display()));
    Ok(summary)
}
