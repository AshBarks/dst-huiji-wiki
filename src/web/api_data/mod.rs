//! 数据浏览 API（按域拆分：dataset / icons / assets / anim）。
//!
//! 路由注册（`web/mod.rs`）继续引用 `api_data::X`；本模块 re-export
//! 各子模块的 handler，共享 helper 为 `page_params` / `err_status`。

mod anim;
mod assets;
mod dataset;
mod icons;

pub use anim::*;
pub use assets::*;
pub use dataset::*;
pub use icons::*;

use axum::extract::Query;
use axum::http::StatusCode;
use std::collections::HashMap;

/// 查询参数提取的简写别名。
pub(super) type Q = Query<HashMap<String, String>>;

/// (page, per_page) 分页参数（1-based，默认 1/50，上限 500）。
pub(super) fn page_params(q: &HashMap<String, String>) -> (usize, usize) {
    let page = q.get("page").and_then(|v| v.parse().ok()).unwrap_or(0);
    let page_size = q
        .get("page_size")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 500);
    (page, page_size)
}

/// 统一把领域错误映射为 HTTP 状态码。
pub(super) fn err_status(e: dst_huiji_wiki::error::Error) -> StatusCode {
    tracing::warn!("api error: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
}

fn sanitize_snapshot(v: Option<&String>) -> std::result::Result<String, StatusCode> {
    let s = v.ok_or(StatusCode::BAD_REQUEST)?.clone();
    if s.is_empty() || s.contains("..") || s.contains('/') || s.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(s)
}

fn run_diff_generic<F>(
    kind: &str,
    from: String,
    to: String,
    compute: F,
) -> dst_huiji_wiki::error::Result<serde_json::Value>
where
    F: FnOnce(&str, &str) -> dst_huiji_wiki::error::Result<serde_json::Value>,
{
    let rel = match kind {
        "recipes" => "recipes.lua",
        "po" => "languages/chinese_s.po",
        _ => {
            return Err(dst_huiji_wiki::error::Error::Config(
                "unknown diff kind".into(),
            ))
        }
    };
    let a = dst_huiji_wiki::service::dataset::read_game_file(Some(&from), rel)?;
    let b = dst_huiji_wiki::service::dataset::read_game_file(Some(&to), rel)?;
    compute(&a, &b)
}
