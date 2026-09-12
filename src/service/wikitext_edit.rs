//! `maintain-wikitext`：用 wikitext 解析器对页面里指定模板做外科手术式
//! 参数批量编辑（设置/删除），逐页 diff 后按 [`WriteMode`] 决定是否写入。
//!
//! 这是 docs/WIKITEXT_PARSER_PLAN.md M3（写侧）的第一个真实用例：
//! 未修改部分逐字节还原，diff 只包含目标参数的变化。
//!
//! `--dry-run` 下只产报告，绝不写维基。

use super::make_ctx;
use crate::error::{Error, Result};
use crate::platform::progress::Reporter;
use crate::platform::progress::{WriteDecision, WriteMode};
use crate::wikitext::Wikicode;
use serde::Serialize;

/// 一次 [`run`] 调用的参数。
#[derive(Debug, Clone, Default)]
pub struct WikitextEditParams {
    /// 页面标题列表。
    pub pages: Vec<String>,
    /// 目标模板名（归一化匹配）。
    pub template: String,
    /// 要设置的参数（`key=value`，首个 `=` 分隔；value 原样写入）。
    pub set: Vec<String>,
    /// 要删除的参数名。
    pub remove: Vec<String>,
    /// 报告/新页面文本的输出目录（可选）。
    pub output: Option<std::path::PathBuf>,
}

/// 单页执行结果。
#[derive(Debug, Clone, Serialize)]
struct PageResult {
    page: String,
    /// ok | no_template | no_changes | dry_run | updated | skipped | fetch_failed
    status: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    newrevid: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub async fn run(
    params: &WikitextEditParams,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    if params.set.is_empty() && params.remove.is_empty() {
        return Err(Error::Config(
            "maintain-wikitext 需要至少一个 --set 或 --remove".to_string(),
        ));
    }
    let ctx = make_ctx(&None)?;
    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    let mut results: Vec<PageResult> = Vec::new();
    let mut new_texts: Vec<(String, String)> = Vec::new();
    for (idx, title) in params.pages.iter().enumerate() {
        reporter.stage(&format!(
            "处理 {title}（{}/{}）",
            idx + 1,
            params.pages.len()
        ));
        let result = process_page(&ctx, params, mode, title, reporter, &mut new_texts).await;
        match &result {
            Ok(r) => {
                reporter.log(format!("{}: {}", title, r.status));
                results.push(r.clone());
            }
            Err(e) => {
                reporter.log(format!("{title}: 出错 {e}"));
                results.push(PageResult {
                    page: title.clone(),
                    status: "error",
                    changed: Vec::new(),
                    newrevid: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let updated = results.iter().filter(|r| r.status == "updated").count();
    let report = serde_json::json!({
        "write_mode": mode.name(),
        "template": params.template,
        "pages_total": params.pages.len(),
        "updated": updated,
        "results": results,
    });
    if let Some(out) = &params.output {
        std::fs::create_dir_all(out)?;
        crate::platform::fs::write_text_atomic(
            out.join("report.json"),
            serde_json::to_string_pretty(&report)?,
        )?;
        for (title, text) in &new_texts {
            let safe = title.replace(['/', ':', '#'], "_");
            crate::platform::fs::write_text_atomic(out.join(format!("{safe}.wikitext")), text)?;
        }
        reporter.log(format!("报告与新文本已写入 {:?}", out));
    }
    Ok(report)
}

async fn process_page(
    ctx: &crate::DstContext,
    params: &WikitextEditParams,
    mode: WriteMode,
    title: &str,
    reporter: &dyn Reporter,
    new_texts: &mut Vec<(String, String)>,
) -> Result<PageResult> {
    let client = ctx.wiki();
    let page = client.get_page(title).await?;
    let Some(original) = page.content else {
        return Ok(PageResult {
            page: title.to_string(),
            status: "fetch_failed",
            changed: Vec::new(),
            newrevid: None,
            error: Some("页面无内容".to_string()),
        });
    };

    let mut code = Wikicode::parse(&original);
    let matched = code.templates_named(&params.template).len();
    if matched == 0 {
        return Ok(PageResult {
            page: title.to_string(),
            status: "no_template",
            changed: Vec::new(),
            newrevid: None,
            error: None,
        });
    }

    let mut changed: Vec<String> = Vec::new();
    for spec in &params.set {
        let Some((key, value)) = spec.split_once('=') else {
            return Err(Error::Config(format!(
                "--set 需要 key=value 形式：{spec:?}"
            )));
        };
        let mut hit = false;
        code.with_templates_named_mut(&params.template, |t| {
            hit |= t.set_param(key, value);
        });
        if hit {
            changed.push(format!("set {key}"));
        }
    }
    for key in &params.remove {
        let mut hit = false;
        code.with_templates_named_mut(&params.template, |t| {
            hit |= t.remove_param(key);
        });
        if hit {
            changed.push(format!("remove {key}"));
        }
    }

    let new_text = code.serialize();
    if changed.is_empty() || new_text == original {
        return Ok(PageResult {
            page: title.to_string(),
            status: "no_changes",
            changed,
            newrevid: None,
            error: None,
        });
    }

    reporter.log(format!(
        "命中模板 {} 处，变更：{}",
        matched,
        changed.join("；")
    ));
    let diff = crate::diff_lines(&original, &new_text);
    let (added, removed) = crate::count_diff_stats(&diff);
    reporter.diff(title, &diff, added, removed);

    let writer = crate::service::wiki_write::WikiWriter::new(client, reporter, mode);
    match writer.decide("更新维基页面？") {
        WriteDecision::Skip(reason) => {
            new_texts.push((title.to_string(), new_text));
            Ok(PageResult {
                page: title.to_string(),
                status: if reason == "dry_run" {
                    "dry_run"
                } else {
                    "skipped"
                },
                changed,
                newrevid: None,
                error: None,
            })
        }
        WriteDecision::Apply => {
            let edit = writer
                .edit_page(&crate::service::wiki_write::PageEdit {
                    title,
                    new_content: &new_text,
                    old_content: Some(&original),
                    basetimestamp: page.last_rev_timestamp.as_deref(),
                    preserve_whitespace: false,
                    summary: "Update via dst-huiji-wiki tool",
                })
                .await?;
            Ok(PageResult {
                page: title.to_string(),
                status: "updated",
                changed,
                newrevid: edit.newrevid,
                error: None,
            })
        }
    }
}
