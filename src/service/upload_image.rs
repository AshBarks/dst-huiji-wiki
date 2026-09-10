//! `upload-image`：上传本地图片到维基。
//!
//! 按素材目录自动补文件描述分类：
//! - `skilltree/` → `[[分类:技能树素材]]`
//! - `skilltree_icons/` → `[[分类:技能树图标]]`
//! - `inventoryimages/` → `[[分类:物品栏图标]]`
//!
//! 同名重传需 `ignore_warnings`（MediaWiki `ignorewarnings=1`）。

use super::{decide_write, WriteDecision, WriteMode};
use crate::error::{Error, Result};
use crate::service::progress::Reporter;
use crate::wiki::WikiClient;
use std::path::Path;

/// 素材目录 → 默认文件描述（分类 wikitext）。
pub fn default_description(path: &Path) -> Option<&'static str> {
    let dir = path.parent()?.file_name()?.to_str()?;
    match dir {
        "skilltree" => Some("[[分类:技能树素材]]"),
        "skilltree_icons" => Some("[[分类:技能树图标]]"),
        "inventoryimages" => Some("[[分类:物品栏图标]]"),
        _ => None,
    }
}

/// 上传一张本地图片，成功返回 `{status, file, result, url}`。
pub async fn run_upload_image(
    path: &Path,
    name: Option<&str>,
    description: Option<&str>,
    comment: Option<&str>,
    ignore_warnings: bool,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let size = std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(Error::Io)?;
    let filename = match name {
        Some(n) => n.to_string(),
        None => path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?
            .to_string(),
    };
    let description = match description {
        Some(d) => Some(d.to_string()),
        None => default_description(path).map(str::to_string),
    };

    reporter.log(format!(
        "待上传 {:?}（{} 字节）→ File:{}",
        path, size, filename
    ));
    match &description {
        Some(d) => reporter.log(format!("描述：{}", d)),
        None => reporter.log("描述：<空>（目录不在常用素材类型内）".to_string()),
    }

    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm(&format!("上传 File:{}？", filename))
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            reporter.log(format!("已跳过上传（{}）。", reason));
            Ok(serde_json::json!({ "status": reason, "file": filename }))
        }
        WriteDecision::Apply => {
            let mut client = WikiClient::from_env()?;
            reporter.log("登录维基".to_string());
            client.login().await?;
            let result = client
                .upload_file(
                    path,
                    Some(&filename),
                    description.as_deref(),
                    comment,
                    ignore_warnings,
                )
                .await?;
            reporter.log(format!(
                "已上传 File:{}（{}）{}",
                result.filename,
                result.result,
                result.url.as_deref().unwrap_or("")
            ));
            Ok(serde_json::json!({
                "status": "uploaded",
                "file": result.filename,
                "result": result.result,
                "url": result.url,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_default_description_by_directory() {
        assert_eq!(
            default_description(&PathBuf::from("/out/split/skilltree/wilson_background.png")),
            Some("[[分类:技能树素材]]")
        );
        assert_eq!(
            default_description(&PathBuf::from("/out/split/skilltree_icons/abigail.png")),
            Some("[[分类:技能树图标]]")
        );
        assert_eq!(
            default_description(&PathBuf::from("/out/split/inventoryimages/axe.png")),
            Some("[[分类:物品栏图标]]")
        );
        assert_eq!(default_description(&PathBuf::from("/tmp/loose.png")), None);
    }
}
