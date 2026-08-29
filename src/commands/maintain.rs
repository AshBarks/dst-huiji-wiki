//! Thin CLI wrappers around the shared service layer.

use super::Commands;
use dst_huiji_wiki::service::{
    execute_job_with_mode, ConfirmMode, JobKind, StdoutReporter, WriteMode,
};
use dst_huiji_wiki::Result;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run(args: Commands) -> Result<()> {
    match args {
        Commands::ParsePo {
            input,
            output,
            category,
        } => {
            execute(
                JobKind::ParsePo {
                    input: path_to_string(&input)?,
                    output: opt_path_to_string(&output)?,
                    category,
                },
                WriteMode::Interactive,
                None,
            )
            .await?;
        }
        Commands::MapNames {
            input,
            output,
            compare,
            merge,
            version,
        } => {
            execute(
                JobKind::MapNames {
                    input: path_to_string(&input)?,
                    output: opt_path_to_string(&output)?,
                    compare: opt_path_to_string(&compare)?,
                    merge,
                    version,
                },
                WriteMode::Interactive,
                None,
            )
            .await?;
        }
        Commands::MapRecipes {
            input,
            output,
            compare,
            merge,
            po_file,
            version,
        } => {
            execute(
                JobKind::MapRecipes {
                    input: path_to_string(&input)?,
                    output: opt_path_to_string(&output)?,
                    compare: opt_path_to_string(&compare)?,
                    merge,
                    po_file: opt_path_to_string(&po_file)?,
                    version,
                },
                WriteMode::Interactive,
                None,
            )
            .await?;
        }
        Commands::MaintainItemTable {
            output,
            yes,
            dry_run,
            report_json,
        } => {
            execute(
                JobKind::MaintainItemTable {
                    output: opt_path_to_string(&output)?,
                    snapshot: None,
                },
                write_mode(yes, dry_run),
                report_json,
            )
            .await?;
        }
        Commands::MaintainDSTRecipes {
            output,
            yes,
            dry_run,
            report_json,
        } => {
            execute(
                JobKind::MaintainDstRecipes {
                    output: opt_path_to_string(&output)?,
                    snapshot: None,
                },
                write_mode(yes, dry_run),
                report_json,
            )
            .await?;
        }
        Commands::MaintainCopyClip {
            r#type,
            output,
            yes,
            dry_run,
            report_json,
        } => {
            execute(
                JobKind::MaintainCopyClip {
                    r#type,
                    output: opt_path_to_string(&output)?,
                    snapshot: None,
                },
                write_mode(yes, dry_run),
                report_json,
            )
            .await?;
        }
        Commands::PrefabOverrides { input, output } => {
            execute(
                JobKind::PrefabOverrides {
                    input: path_to_string(&input)?,
                    output: opt_path_to_string(&output)?,
                },
                WriteMode::Interactive,
                None,
            )
            .await?;
        }
        Commands::CorpusFetch { full, dir, dry_run } => {
            // Corpus jobs never write the wiki; DryRun only suppresses local
            // disk writes (enumerate + reconcile + report).
            let mode = if dry_run {
                WriteMode::DryRun
            } else {
                WriteMode::AutoConfirm
            };
            execute(
                JobKind::CorpusSync {
                    full,
                    dir: opt_path_to_string(&dir)?,
                },
                mode,
                None,
            )
            .await?;
        }
        Commands::UpdateScan {
            old,
            new,
            out,
            corpus,
            annotate,
        } => {
            execute(
                JobKind::UpdateScan {
                    old,
                    new,
                    out: opt_path_to_string(&out)?,
                    corpus: opt_path_to_string(&corpus)?,
                    annotate: opt_path_to_string(&annotate)?,
                },
                // Read-only pipeline: no wiki writes ever.
                WriteMode::AutoConfirm,
                None,
            )
            .await?;
        }
        Commands::UpdateIndex { root, out } => {
            execute(
                JobKind::UpdateIndex {
                    root: path_to_string(&root)?,
                    out: opt_path_to_string(&out)?,
                },
                // Local-only job: never touches the wiki.
                WriteMode::AutoConfirm,
                None,
            )
            .await?;
        }
        Commands::CorpusIndex { dir, join, dry_run } => {
            let mode = if dry_run {
                WriteMode::DryRun
            } else {
                WriteMode::AutoConfirm
            };
            execute(
                JobKind::CorpusIndex {
                    dir: opt_path_to_string(&dir)?,
                    join: opt_path_to_string(&join)?,
                },
                mode,
                None,
            )
            .await?;
        }
        Commands::SymbolAnnotate {
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
            execute(
                JobKind::SymbolAnnotate {
                    root: path_to_string(&root)?,
                    corpus: path_to_string(&corpus)?,
                    limit,
                    out: opt_path_to_string(&out)?,
                    verdicts: opt_path_to_string(&verdicts)?,
                    llm,
                    batch_pages,
                    batch_max_chars,
                    skip_no_fact_pages,
                },
                // Local-only job: never touches the wiki.
                WriteMode::AutoConfirm,
                None,
            )
            .await?;
        }
        Commands::KnowledgeScanSymbols {
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
        } => {
            execute(
                JobKind::KnowledgeScanSymbols {
                    root: path_to_string(&root)?,
                    category,
                    knowledge_dir: path_to_string(&knowledge_dir)?,
                    corpus: opt_path_to_string(&corpus)?,
                    sample_pages,
                    limit,
                    force,
                    concurrency,
                    pass2_names,
                    pick_names,
                    refresh_auto,
                },
                // Local-only job: never touches the wiki.
                WriteMode::AutoConfirm,
                None,
            )
            .await?;
        }
        Commands::PageAssist {
            knowledge_dir,
            page,
            json,
        } => {
            execute(
                JobKind::PageAssist {
                    knowledge_dir: path_to_string(&knowledge_dir)?,
                    page: page.clone(),
                    json,
                },
                // Local-only job: never touches the wiki.
                WriteMode::AutoConfirm,
                None,
            )
            .await?;
        }
        Commands::KnowledgeScanWiki {
            root,
            knowledge_dir,
            corpus,
            audit,
            audit_symbols,
            audit_max_pages,
            audit_batch_pages,
            audit_batch_max_chars,
            report,
        } => {
            execute(
                JobKind::KnowledgeScanWiki {
                    root: path_to_string(&root)?,
                    knowledge_dir: path_to_string(&knowledge_dir)?,
                    corpus: path_to_string(&corpus)?,
                    audit,
                    audit_symbols,
                    audit_max_pages,
                    audit_batch_pages,
                    audit_batch_max_chars,
                    report,
                },
                // Local-only job: never touches the wiki.
                WriteMode::AutoConfirm,
                None,
            )
            .await?;
        }
        Commands::Serve { host, port } => {
            crate::web::serve(host, port).await?;
        }
    }

    Ok(())
}

/// Resolves the CLI `--yes`/`--dry-run` flags into a service [`WriteMode`].
/// (The two flags are already mutually exclusive at the clap level.)
fn write_mode(yes: bool, dry_run: bool) -> WriteMode {
    if dry_run {
        WriteMode::DryRun
    } else if yes {
        WriteMode::AutoConfirm
    } else {
        WriteMode::Interactive
    }
}

async fn execute(kind: JobKind, mode: WriteMode, report_json: Option<PathBuf>) -> Result<()> {
    // AutoConfirm/DryRun never consult the reporter (decide_write
    // short-circuits), so stdin prompting stays Interactive-only.
    let reporter = StdoutReporter {
        confirm: ConfirmMode::Interactive,
    };

    let result = execute_job_with_mode(&kind, &reporter, mode).await?;

    if let Some(path) = report_json {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let report = serde_json::json!({
            "generated_at_ms": now_ms,
            "job": kind.name(),
            "params": serde_json::to_value(&kind).unwrap_or_default(),
            "write_mode": mode.name(),
            "result": result,
        });
        let text = serde_json::to_string_pretty(&report)?;
        std::fs::write(&path, text)?;
        println!("报告已写入 {:?}", path);
    }

    Ok(())
}

fn path_to_string(p: &PathBuf) -> Result<String> {
    p.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| dst_huiji_wiki::Error::InvalidPath(format!("{:?}", p)))
}

fn opt_path_to_string(p: &Option<PathBuf>) -> Result<Option<String>> {
    match p {
        Some(path) => Ok(Some(path_to_string(path)?)),
        None => Ok(None),
    }
}
