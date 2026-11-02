//! 游戏更新后的编排作业：`update-local` / `update-data` / `update-assets` /
//! `update-check`。
//!
//! 本模块只按固定顺序调度既有作业实现（`scripts_sync` / `images` / `anim` /
//! `upload_icons` / `skilltree_wiki` / `maintain_*` / `strings_wiki` /
//! `template_check` / `prefab_overrides::audit`），不复制任何业务逻辑；原作业
//! 的入口与语义保持不变，仍可单独运行。步骤顺序的约束见
//! docs/UPDATE_TASK_GROUPS.md。
//!
//! 失败语义：除 `update-check` 外任一步失败即中止（fail-fast），错误消息带
//! 步骤名；已完成步骤的产物保留（各子作业自身幂等，重跑即可）。
//! `update-check` 是只读核对，按 continue-on-error 执行，报告的 `status`
//! 区分 `success` / `partial` / `failed`。

use crate::error::{Error, Result};
use crate::platform::progress::{Reporter, WriteMode};
use serde_json::{json, Value};

/// `update-local` 的固定步骤（顺序即约束：scripts 必须最先，images 依赖
/// 新的 live 树取 PO 名称；anim 与二者无依赖，放最后统一收尾版本）。
pub(super) const UPDATE_LOCAL_STEPS: &[&str] = &["scripts-sync", "images-sync", "anim-sync"];

/// `update-data` 的固定步骤（copyclip 必须早于 dst-recipes：新制作站键要
/// 先写进 CraftingNames，否则 wiki 端 `模块:DSTRecipe` 会 assert）。
pub(super) const UPDATE_DATA_STEPS: &[&str] = &[
    "maintain-copy-clip",
    "maintain-dst-recipes",
    "maintain-item-table",
    "maintain-strings",
];

/// `update-assets`：图标先上传，技能树的 `icon_url` 才能在站内回填。
pub(super) const UPDATE_ASSETS_STEPS: &[&str] = &["upload-icons", "skilltree-wiki"];

/// `update-check`：只读核对，两步互不依赖（一项失败不影响另一项）。
pub(super) const UPDATE_CHECK_STEPS: &[&str] =
    &["maintain-template-check", "prefab-overrides-audit"];

/// Resolves `--from` to a step index. `None` starts at the beginning; an
/// unknown name is rejected with the valid step list so CLI/WebUI typos fail
/// before any work is done.
pub(super) fn step_start(steps: &[&str], from: Option<&str>) -> Result<usize> {
    match from {
        None => Ok(0),
        Some(name) => steps.iter().position(|s| *s == name).ok_or_else(|| {
            Error::Config(format!(
                "未知的起始步骤 `{name}`，有效值: {}",
                steps.join(" | ")
            ))
        }),
    }
}

fn fail(step: &str, err: Error) -> Error {
    Error::Config(format!("{step} 步骤失败：{err}"))
}

/// `update-local`：`scripts-sync → images-sync → anim-sync`。
pub(super) async fn run_update_local(
    force: bool,
    dry_run: bool,
    skip_wiki_status: bool,
    from: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<Value> {
    let start = step_start(UPDATE_LOCAL_STEPS, from)?;
    let mut steps = Vec::new();

    for (i, name) in UPDATE_LOCAL_STEPS.iter().enumerate() {
        if i < start {
            continue;
        }
        reporter.step(i + 1, UPDATE_LOCAL_STEPS.len(), name);
        let result = match *name {
            "scripts-sync" => super::run_scripts_sync(force, dry_run, None, reporter).await,
            "images-sync" => {
                super::run_images_sync(force, dry_run, skip_wiki_status, reporter).await
            }
            "anim-sync" => super::run_anim_sync(force, dry_run, None, None, reporter).await,
            other => unreachable!("unhandled update-local step: {other}"),
        }
        .map_err(|e| fail(name, e))?;
        steps.push(json!({ "step": name, "result": result }));
    }

    Ok(json!({
        "status": if dry_run { "dry_run" } else { "success" },
        "skipped": &UPDATE_LOCAL_STEPS[..start],
        "steps": steps,
    }))
}

/// `update-data`：`maintain-copy-clip → maintain-dst-recipes →
/// maintain-item-table → maintain-strings`，各步共用同一 `WriteMode` 与
/// `snapshot`；不暴露各步的 `output`（产物路径语义各步不同，需要时单独跑）。
pub(super) async fn run_update_data(
    snapshot: Option<String>,
    from: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<Value> {
    let start = step_start(UPDATE_DATA_STEPS, from)?;
    let mut steps = Vec::new();

    for (i, name) in UPDATE_DATA_STEPS.iter().enumerate() {
        if i < start {
            continue;
        }
        reporter.step(i + 1, UPDATE_DATA_STEPS.len(), name);
        let result = match *name {
            "maintain-copy-clip" => {
                super::maintain_jobs::run_maintain_copyclip(
                    None,
                    None,
                    snapshot.clone(),
                    reporter,
                    mode,
                )
                .await
            }
            "maintain-dst-recipes" => {
                super::maintain_jobs::run_maintain_dst_recipes(
                    None,
                    snapshot.clone(),
                    reporter,
                    mode,
                )
                .await
            }
            "maintain-item-table" => {
                super::maintain_jobs::run_maintain_item_table(
                    None,
                    snapshot.clone(),
                    reporter,
                    mode,
                )
                .await
            }
            "maintain-strings" => {
                super::strings_wiki::run(
                    &super::strings_wiki::StringsWikiParams {
                        version: "DST".to_string(),
                        snapshot: snapshot.clone(),
                        output: None,
                        offline: false,
                        bucket_count: super::strings_wiki::DEFAULT_BUCKETS,
                        rebalance: false,
                        limit: 0,
                    },
                    reporter,
                    mode,
                )
                .await
            }
            other => unreachable!("unhandled update-data step: {other}"),
        }
        .map_err(|e| fail(name, e))?;
        steps.push(json!({ "step": name, "result": result }));
    }

    Ok(json!({
        "status": if matches!(mode, WriteMode::DryRun) { "dry_run" } else { "success" },
        "skipped": &UPDATE_DATA_STEPS[..start],
        "steps": steps,
    }))
}

/// `update-assets`：`upload-icons → skilltree-wiki`。上传全部来源（缺省
/// `only_missing`），随后按站内图片回填技能树 `icon_url`。两步都持有
/// `wiki` 锁，实际串行。
pub(super) async fn run_update_assets(
    from: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<Value> {
    let start = step_start(UPDATE_ASSETS_STEPS, from)?;
    let mut steps = Vec::new();

    for (i, name) in UPDATE_ASSETS_STEPS.iter().enumerate() {
        if i < start {
            continue;
        }
        reporter.step(i + 1, UPDATE_ASSETS_STEPS.len(), name);
        let result = match *name {
            "upload-icons" => {
                super::upload_icons::run_upload_icons(
                    &super::upload_icons::UploadIconsParams {
                        build: None,
                        source: None,
                        file: None,
                        title: None,
                        only_missing: true,
                        ignore_warnings: false,
                        comment: None,
                    },
                    reporter,
                    mode,
                )
                .await
            }
            "skilltree-wiki" => {
                super::skilltree_wiki::run_skilltree_wiki(None, None, None, reporter, mode).await
            }
            other => unreachable!("unhandled update-assets step: {other}"),
        }
        .map_err(|e| fail(name, e))?;
        steps.push(json!({ "step": name, "result": result }));
    }

    Ok(json!({
        "status": if matches!(mode, WriteMode::DryRun) { "dry_run" } else { "success" },
        "skipped": &UPDATE_ASSETS_STEPS[..start],
        "steps": steps,
    }))
}

/// `update-check`：`maintain-template-check` + `prefab-overrides-audit`，
/// 只读。按 continue-on-error 执行：一项失败不影响另一项，作业本身返回
/// `Ok`，`status` 为 `partial` / `failed` 并在 `steps[].error` 给出原因。
pub(super) async fn run_update_check(
    snapshot: Option<String>,
    skip_icon_status: bool,
    from: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<Value> {
    let start = step_start(UPDATE_CHECK_STEPS, from)?;
    let mut steps = Vec::new();
    let mut failed = 0usize;

    for (i, name) in UPDATE_CHECK_STEPS.iter().enumerate() {
        if i < start {
            continue;
        }
        reporter.step(i + 1, UPDATE_CHECK_STEPS.len(), name);
        let result = match *name {
            "maintain-template-check" => {
                super::template_check::run(
                    &super::template_check::TemplateCheckParams {
                        output: None,
                        snapshot: snapshot.clone(),
                        skip_icon_status,
                    },
                    reporter,
                )
                .await
            }
            "prefab-overrides-audit" => {
                super::prefab_overrides::audit::run_prefab_overrides_audit(
                    None, None, None, reporter,
                )
                .await
            }
            other => unreachable!("unhandled update-check step: {other}"),
        };
        match result {
            Ok(value) => steps.push(json!({ "step": name, "status": "ok", "result": value })),
            Err(e) => {
                failed += 1;
                reporter.error(format!("{name} 步骤失败：{e}"));
                steps.push(json!({ "step": name, "status": "failed", "error": e.to_string() }));
            }
        }
    }

    let ran = steps.len();
    let status = if failed == 0 {
        "success"
    } else if failed == ran {
        "failed"
    } else {
        "partial"
    };
    Ok(json!({
        "status": status,
        "skipped": &UPDATE_CHECK_STEPS[..start],
        "steps": steps,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 顺序是硬约束（见 docs/UPDATE_TASK_GROUPS.md §4）：改动步骤顺序必须
    /// 同时改这里，避免"以为是重构把 copyclip 挪到 recipes 后面"。
    #[test]
    fn step_order_is_pinned() {
        assert_eq!(
            UPDATE_LOCAL_STEPS,
            &["scripts-sync", "images-sync", "anim-sync"]
        );
        assert_eq!(
            UPDATE_DATA_STEPS,
            &[
                "maintain-copy-clip",
                "maintain-dst-recipes",
                "maintain-item-table",
                "maintain-strings",
            ]
        );
        assert_eq!(UPDATE_ASSETS_STEPS, &["upload-icons", "skilltree-wiki"]);
        assert_eq!(
            UPDATE_CHECK_STEPS,
            &["maintain-template-check", "prefab-overrides-audit"]
        );
    }

    #[test]
    fn from_resolves_to_step_index_and_rejects_typos() {
        assert_eq!(step_start(UPDATE_LOCAL_STEPS, None).unwrap(), 0);
        assert_eq!(
            step_start(UPDATE_LOCAL_STEPS, Some("images-sync")).unwrap(),
            1
        );
        assert_eq!(
            step_start(UPDATE_DATA_STEPS, Some("maintain-strings")).unwrap(),
            3
        );
        let err = step_start(UPDATE_ASSETS_STEPS, Some("upload-icon")).unwrap_err();
        assert!(err.to_string().contains("upload-icons"), "错误应列出有效值");
    }
}
