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
        crate::platform::fs::write_text_atomic(&output_path, &new_json)?;
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
            reporter.warn(format!("获取维基当前数据失败：{}", e));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };

    let historical_content = match page.content {
        Some(c) => c,
        None => {
            reporter.warn(format!("页面 '{}' 无内容", page_title));
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
            reporter.warn(format!("解析维基历史数据失败：{}", e));
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

/// Copyclip 的"是否真的有变化"判定：忽略行尾/行首 ASCII 空白与换行符差异。
///
/// 与归一 diff（`preserve_whitespace = false`）保持一致——否则纯空白差异会
/// 产生一次"diff 为空"的线上编辑。两侧内容仍按原样写入，本函数只决定要不要写。
fn copyclip_content_changed(old: &str, new: &str) -> bool {
    crate::utils::canonical_for_compare(old) != crate::utils::canonical_for_compare(new)
}

pub(super) async fn output_copyclip_result_with_update(
    ctx: &WriteCtx<'_>,
    page_title: &str,
    target_content: &str,
    updated_content: &str,
    output: Option<PathBuf>,
    base_timestamp: Option<String>,
) -> Result<serde_json::Value> {
    if !copyclip_content_changed(target_content, updated_content) {
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
        // Lua 模块块是生成的；归一 diff 避免缩进/行尾空白噪声。
        preserve_whitespace: false,
        summary: "Update via dst-huiji-wiki tool",
    };
    let outcome = writer.propose(&edit, true).await?;
    Ok(serde_json::json!(
        { "status": outcome.status, "page": page_title, "newrevid": outcome.newrevid }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copyclip_change_detection_ignores_line_endings_and_edge_whitespace() {
        assert!(!copyclip_content_changed(
            "local a = 1\nlocal b = 2\n",
            "  local a = 1\r\n\tlocal b = 2  \r\n"
        ));
        // 行内空白与字符串内容仍然算变化。
        assert!(copyclip_content_changed("local a = 1", "local  a = 1"));
        assert!(copyclip_content_changed(
            "local s = \"x\"",
            "local s = \" x\""
        ));
    }
}
