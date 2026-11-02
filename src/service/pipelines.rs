//! 游戏更新后的编排作业：`update-local` / `update-data`。
//!
//! 本模块只按固定顺序调度既有作业实现（`scripts_sync` / `images` / `anim` /
//! `maintain_*` / `strings_wiki`），不复制任何业务逻辑；原作业的入口与语义
//! 保持不变，仍可单独运行。步骤顺序的约束见 docs/UPDATE_TASK_GROUPS.md。
//!
//! 失败语义：任一步失败即中止（fail-fast），错误消息带步骤名；已完成步骤
//! 的产物保留（各子作业自身幂等，重跑即可）。

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

fn fail(step: &str, err: Error) -> Error {
    Error::Config(format!("{step} 步骤失败：{err}"))
}

/// `update-local`：`scripts-sync → images-sync → anim-sync`。
pub(super) async fn run_update_local(
    force: bool,
    dry_run: bool,
    skip_wiki_status: bool,
    reporter: &dyn Reporter,
) -> Result<Value> {
    let mut steps = Vec::new();

    reporter.step(1, UPDATE_LOCAL_STEPS.len(), UPDATE_LOCAL_STEPS[0]);
    let result = super::run_scripts_sync(force, dry_run, None, reporter)
        .await
        .map_err(|e| fail(UPDATE_LOCAL_STEPS[0], e))?;
    steps.push(json!({ "step": UPDATE_LOCAL_STEPS[0], "result": result }));

    reporter.step(2, UPDATE_LOCAL_STEPS.len(), UPDATE_LOCAL_STEPS[1]);
    let result = super::run_images_sync(force, dry_run, skip_wiki_status, reporter)
        .await
        .map_err(|e| fail(UPDATE_LOCAL_STEPS[1], e))?;
    steps.push(json!({ "step": UPDATE_LOCAL_STEPS[1], "result": result }));

    reporter.step(3, UPDATE_LOCAL_STEPS.len(), UPDATE_LOCAL_STEPS[2]);
    let result = super::run_anim_sync(force, dry_run, None, None, reporter)
        .await
        .map_err(|e| fail(UPDATE_LOCAL_STEPS[2], e))?;
    steps.push(json!({ "step": UPDATE_LOCAL_STEPS[2], "result": result }));

    Ok(json!({
        "status": if dry_run { "dry_run" } else { "success" },
        "steps": steps,
    }))
}

/// `update-data`：`maintain-copy-clip → maintain-dst-recipes →
/// maintain-item-table → maintain-strings`，四步共用同一 `WriteMode` 与
/// `snapshot`；不暴露各步的 `output`（产物路径语义各步不同，需要时单独跑）。
pub(super) async fn run_update_data(
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<Value> {
    let mut steps = Vec::new();

    reporter.step(1, UPDATE_DATA_STEPS.len(), UPDATE_DATA_STEPS[0]);
    let result =
        super::maintain_jobs::run_maintain_copyclip(None, None, snapshot.clone(), reporter, mode)
            .await
            .map_err(|e| fail(UPDATE_DATA_STEPS[0], e))?;
    steps.push(json!({ "step": UPDATE_DATA_STEPS[0], "result": result }));

    reporter.step(2, UPDATE_DATA_STEPS.len(), UPDATE_DATA_STEPS[1]);
    let result =
        super::maintain_jobs::run_maintain_dst_recipes(None, snapshot.clone(), reporter, mode)
            .await
            .map_err(|e| fail(UPDATE_DATA_STEPS[1], e))?;
    steps.push(json!({ "step": UPDATE_DATA_STEPS[1], "result": result }));

    reporter.step(3, UPDATE_DATA_STEPS.len(), UPDATE_DATA_STEPS[2]);
    let result =
        super::maintain_jobs::run_maintain_item_table(None, snapshot.clone(), reporter, mode)
            .await
            .map_err(|e| fail(UPDATE_DATA_STEPS[2], e))?;
    steps.push(json!({ "step": UPDATE_DATA_STEPS[2], "result": result }));

    reporter.step(4, UPDATE_DATA_STEPS.len(), UPDATE_DATA_STEPS[3]);
    let result = super::strings_wiki::run(
        &super::strings_wiki::StringsWikiParams {
            version: "DST".to_string(),
            snapshot,
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
    .map_err(|e| fail(UPDATE_DATA_STEPS[3], e))?;
    steps.push(json!({ "step": UPDATE_DATA_STEPS[3], "result": result }));

    Ok(json!({
        "status": if matches!(mode, WriteMode::DryRun) { "dry_run" } else { "success" },
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
    }
}
