//! `create-redirect`：在 wiki 上创建页面重定向（`#REDIRECT [[目标]]`）。
//!
//! 独立的 wiki 写作业：`--dry-run` 只预览生成的 redirect wikitext，不登录、
//! 不写入；交互模式确认后写入；WebUI 按全局 wiki 干跑开关处理。

use crate::error::{Error, Result};
use crate::platform::progress::{decide_write, Reporter, WriteDecision, WriteMode};
use crate::wiki::WikiClient;

/// 生成一条重定向的 wikitext（独立函数便于测试与预览）。
pub(crate) fn redirect_wikitext(target: &str) -> String {
    format!("#REDIRECT [[{}]]", target.trim())
}

/// 创建写作业的入口；返回 report schema v1 内层数据。
pub async fn run_create_redirect(
    from: &str,
    to: &str,
    summary: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let from = from.trim();
    let to = to.trim();
    if from.is_empty() {
        return Err(Error::Config("源页面标题不能为空".to_string()));
    }
    if to.is_empty() {
        return Err(Error::Config("目标页面标题不能为空".to_string()));
    }
    if from == to {
        return Err(Error::Config("源页面与目标页面不能相同".to_string()));
    }

    let text = redirect_wikitext(to);
    reporter.stage("建立页面重定向");
    reporter.log(format!("{from} → {to}"));
    reporter.diff(from, &text, 1, 0);

    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm(&format!("在 {from} 建立指向 {to} 的重定向？"))
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            reporter.log(format!("已跳过写入（{reason}）。"));
            Ok(serde_json::json!({
                "status": reason,
                "from": from,
                "to": to,
                "redirect_text": text,
            }))
        }
        WriteDecision::Apply => {
            reporter.stage("登录维基");
            let client = WikiClient::from_env()?;
            client.login().await?;
            let summary = summary.unwrap_or("创建重定向");
            let result = client.create_redirect(from, to, Some(summary)).await?;
            reporter.log(format!(
                "已建立重定向：{from} → {to}（pageid: {:?}, newrevid: {:?}）",
                result.pageid, result.newrevid
            ));
            Ok(serde_json::json!({
                "status": "created",
                "from": from,
                "to": to,
                "pageid": result.pageid,
                "newrevid": result.newrevid,
                "result": result.result,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::redirect_wikitext;

    #[test]
    fn redirect_wikitext_ignores_surrounding_whitespace() {
        assert_eq!(
            redirect_wikitext("  File:Wendy potion 3.png  "),
            "#REDIRECT [[File:Wendy potion 3.png]]"
        );
    }
}
