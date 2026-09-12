//! 统一的 wiki 写入流程。
//!
//! 此前“diff 预览 → 确认 → `decide_write` → `edit_page`（basetimestamp
//! 冲突保护）→ 日志”的尾段在 `output_json_result_with_update`、
//! `output_copyclip_result_with_update`、`skilltree_wiki::maintain_character`
//! 各自实现一遍，`maintain-strings` 又有自己的批量确认版本；很容易出现
//! 某个命令忘带 basetimestamp、dry-run 漏判或确认粒度不一致。
//! [`WikiWriter`] 是该流程的唯一实现：
//!
//! - 单页语义：[`WikiWriter::propose`]（逐页确认）；
//! - 批量语义：调用方自行收集请求后用 [`WikiWriter::decide`] 做一次批量
//!   确认、再用 [`WikiWriter::edit_page`] 逐页写入（maintain-strings）。

use crate::error::Result;
use crate::platform::progress::{decide_write, Reporter, WriteDecision, WriteMode};
use crate::wiki::{EditResult, WikiClient};

pub struct WikiWriter<'a> {
    pub client: &'a WikiClient,
    pub reporter: &'a dyn Reporter,
    pub mode: WriteMode,
}

/// 单页写请求。
pub struct PageEdit<'a> {
    pub title: &'a str,
    pub new_content: &'a str,
    /// 当前页面内容；`None` 表示新建（跳过 diff 预览）。
    pub old_content: Option<&'a str>,
    /// 所基于修订的时间戳（`PageInfo::last_rev_timestamp`），冲突保护。
    pub basetimestamp: Option<&'a str>,
    /// diff 预览保留空白（wikitext 页 true；JSON 数据页用归一 diff，false）。
    pub preserve_whitespace: bool,
    /// 编辑摘要。
    pub summary: &'a str,
}

/// 一页的写决定与结果。
#[derive(Debug, Clone)]
pub struct PageOutcome {
    pub status: String,
    pub added: usize,
    pub removed: usize,
    pub oldrevid: Option<i64>,
    pub newrevid: Option<i64>,
}

impl PageOutcome {
    fn no_changes() -> Self {
        Self {
            status: "no_changes".into(),
            added: 0,
            removed: 0,
            oldrevid: None,
            newrevid: None,
        }
    }

    fn skipped(reason: &'static str) -> Self {
        Self {
            status: reason.to_string(),
            added: 0,
            removed: 0,
            oldrevid: None,
            newrevid: None,
        }
    }
}

impl<'a> WikiWriter<'a> {
    pub fn new(client: &'a WikiClient, reporter: &'a dyn Reporter, mode: WriteMode) -> Self {
        Self {
            client,
            reporter,
            mode,
        }
    }

    /// 确认策略（唯一出口）：只有 Interactive 模式询问 reporter；
    /// AutoConfirm/DryRun 不得在无人值守运行中阻塞 stdin。
    pub fn decide(&self, prompt: &str) -> WriteDecision {
        let confirmed = if self.mode == WriteMode::Interactive {
            self.reporter.confirm(prompt)
        } else {
            false
        };
        decide_write(self.mode, confirmed)
    }

    /// 单页完整流程：diff 预览 → 确认 → 写入。
    ///
    /// `changed` 由调用方比较得出（各页面的比较语义不同：JSON 数据页做
    /// serde round-trip 后比 trim，wikitext 页比精确文本）。
    pub async fn propose(&self, edit: &PageEdit<'_>, changed: bool) -> Result<PageOutcome> {
        if !changed {
            self.reporter.log("未检测到变化。".to_string());
            return Ok(PageOutcome::no_changes());
        }

        let (added, removed) = match edit.old_content {
            Some(old) => {
                let diff = if edit.preserve_whitespace {
                    crate::diff_lines_preserve_whitespace(old, edit.new_content)
                } else {
                    crate::diff_lines(old, edit.new_content)
                };
                let (added, removed) = crate::count_diff_stats(&diff);
                self.reporter.log("--- 检测到变化 ---".to_string());
                self.reporter.diff(edit.title, &diff, added, removed);
                (added, removed)
            }
            None => {
                self.reporter
                    .log(format!("{}：页面不存在，将创建。", edit.title));
                (edit.new_content.lines().count(), 0)
            }
        };

        match self.decide(&format!("更新维基页面 {}？", edit.title)) {
            WriteDecision::Skip(reason) => {
                self.reporter
                    .log(format!("已跳过更新 {}（{}）。", edit.title, reason));
                let mut outcome = PageOutcome::skipped(reason);
                outcome.added = added;
                outcome.removed = removed;
                Ok(outcome)
            }
            WriteDecision::Apply => {
                self.reporter
                    .log(format!("正在更新维基页面: {}", edit.title));
                let result = self.edit_page(edit).await?;
                self.reporter.log(format!(
                    "{}：已写入（oldrev={:?} newrev={:?}）。",
                    edit.title, result.oldrevid, result.newrevid
                ));
                Ok(PageOutcome {
                    status: "updated".into(),
                    added,
                    removed,
                    oldrevid: result.oldrevid,
                    newrevid: result.newrevid,
                })
            }
        }
    }

    /// 带结构化日志 span 的单次编辑。
    ///
    /// 携带 `basetimestamp`（写前拉取的修订时间戳），MediaWiki 以
    /// `editconflict` 拒绝并发覆盖而不是静默覆盖；执行前统一
    /// `ensure_login`。
    pub async fn edit_page(&self, edit: &PageEdit<'_>) -> Result<EditResult> {
        let span = tracing::info_span!("wiki_edit", page = %edit.title);
        async move {
            let edit_result = self
                .client
                .edit_page(
                    edit.title,
                    edit.new_content,
                    Some(edit.summary),
                    false,
                    edit.basetimestamp,
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
}

use tracing::Instrument;

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome_json(o: &PageOutcome, page: &str) -> serde_json::Value {
        serde_json::json!({ "status": o.status, "page": page, "newrevid": o.newrevid })
    }

    #[test]
    fn outcome_json_shape() {
        let o = PageOutcome {
            status: "updated".into(),
            added: 3,
            removed: 1,
            oldrevid: Some(7),
            newrevid: Some(9),
        };
        let v = outcome_json(&o, "模块:X");
        assert_eq!(v["status"], "updated");
        assert_eq!(v["page"], "模块:X");
        assert_eq!(v["newrevid"], 9);
    }
}
