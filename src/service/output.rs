//! wiki 写入与产物输出的共享 helper（写站流程见 [`super::wiki_write`]）。

use super::wiki_write::{PageEdit, WikiWriter};
use crate::error::Result;
use crate::mapping::WikiDataConverter;
use crate::platform::progress::{Reporter, WriteMode};
use crate::wiki::WikiClient;
use std::path::PathBuf;

pub(super) fn finish_json_output(
    wiki_data: &crate::mapping::WikiJsonData,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    if let Some(output_path) = output {
        let json = WikiDataConverter::to_json_string(wiki_data)?;
        crate::platform::fs::write_text_atomic(&output_path, &json)?;
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
pub(super) struct WriteCtx<'a> {
    pub(super) client: &'a WikiClient,
    pub(super) reporter: &'a dyn Reporter,
    pub(super) mode: WriteMode,
}

pub(super) async fn output_json_result_with_update(
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

    let writer = WikiWriter::new(client, reporter, mode);
    let edit = PageEdit {
        title: page_title,
        new_content: &new_json,
        old_content: Some(&historical_json),
        basetimestamp: page.last_rev_timestamp.as_deref(),
        preserve_whitespace: false,
        summary: "Update via dst-huiji-wiki tool",
    };
    let outcome = writer
        .propose(&edit, new_json.trim() != historical_json.trim())
        .await?;
    Ok(serde_json::json!(
        { "status": outcome.status, "page": page_title, "newrevid": outcome.newrevid }))
}

pub(super) async fn output_copyclip_result_with_update(
    ctx: &WriteCtx<'_>,
    page_title: &str,
    target_content: &str,
    updated_content: &str,
    output: Option<PathBuf>,
    base_timestamp: Option<String>,
) -> Result<serde_json::Value> {
    if target_content == updated_content {
        ctx.reporter.log("未检测到变化。".to_string());
        return Ok(serde_json::json!({ "status": "no_changes" }));
    }

    if let Some(output_path) = output {
        crate::platform::fs::write_text_atomic(&output_path, updated_content)?;
        ctx.reporter
            .log(format!("已写入更新内容到 {:?}", output_path));
    }

    let writer = WikiWriter::new(ctx.client, ctx.reporter, ctx.mode);
    let edit = PageEdit {
        title: page_title,
        new_content: updated_content,
        old_content: Some(target_content),
        basetimestamp: base_timestamp.as_deref(),
        preserve_whitespace: true,
        summary: "Update via dst-huiji-wiki tool",
    };
    let outcome = writer.propose(&edit, true).await?;
    Ok(serde_json::json!(
        { "status": outcome.status, "page": page_title, "newrevid": outcome.newrevid }))
}
