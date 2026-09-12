//! 静态素材与快照 diff 端点（split/objects 资产、skilltree、snapshots、diff）。

use super::super::state::{ktools_out_dir, AppState};
use super::Q;
use super::{err_status, run_diff_generic, sanitize_snapshot};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

pub async fn split_asset(
    Path((dir, name)): Path<(String, String)>,
) -> std::result::Result<axum::response::Response, StatusCode> {
    if !matches!(
        dir.as_str(),
        "skilltree"
            | "skilltree_icons"
            | "global_redux"
            | "inventoryimages"
            | "crafting_menu_icons"
    ) || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = ktools_out_dir()
        .join("current/split")
        .join(&dir)
        .join(&name);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // current 树随 build 更新（同名文件内容可能变化），缓存适度。
    Ok(png_response(bytes, "public, max-age=86400"))
}

/// PNG 响应（带缓存头；`cache_control` 由调用方按内容可变性选择）。
fn png_response(bytes: Vec<u8>, cache_control: &'static str) -> axum::response::Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, cache_control),
        ],
        bytes,
    )
        .into_response()
}

/// GET /static/objects/{hash} — 按内容 sha256 取回 CAS 中的历史版本图片。
pub async fn object_asset(
    Path(hash): Path<String>,
) -> std::result::Result<axum::response::Response, StatusCode> {
    let hash = hash.to_ascii_lowercase();
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let path = ktools_out_dir()
        .join("history/objects")
        .join(&hash[..2])
        .join(&hash);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // 内容寻址：hash 即内容，可永久缓存。
    Ok(png_response(bytes, "public, max-age=31536000, immutable"))
}

/// GET /api/viz/skilltree?character=wilson
pub async fn skilltree(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let character = q
        .get("character")
        .cloned()
        .unwrap_or_else(|| "wilson".to_string());
    // Basic input hardening: this value joins a file path.
    if character.contains("..") || character.contains('/') || character.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }

    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let strings = state
        .skill_strings(snapshot.clone())
        .await
        .map_err(err_status)?;
    let snap = snapshot.clone();
    let result = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::service::dataset::load_skill_tree(snap.as_deref(), &character, &strings)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;

    Ok(Json(result))
}

/// GET /api/snapshots
pub async fn snapshots() -> Json<serde_json::Value> {
    let list = dst_huiji_wiki::DstContext::list_snapshots();
    Json(serde_json::json!({ "snapshots": list }))
}

/// GET /api/diff/recipes?from=&to=
pub async fn diff_recipes(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let from = sanitize_snapshot(q.get("from"))?;
    let to = sanitize_snapshot(q.get("to"))?;
    let key = format!("recipes|{}|{}", from, to);

    let (f_from, f_to) = (from.clone(), to.clone());
    let value = state
        .cached_diff(key, move || {
            run_diff_generic("recipes", from, to, move |a, b| {
                let mut diff = dst_huiji_wiki::service::snapshot_diff::diff_recipes_sources(a, b)?;
                diff.from = f_from;
                diff.to = f_to;
                Ok(serde_json::to_value(&diff)?)
            })
        })
        .await
        .map_err(err_status)?;
    Ok(Json(value))
}

/// GET /api/diff/po?from=&to=
pub async fn diff_po(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let from = sanitize_snapshot(q.get("from"))?;
    let to = sanitize_snapshot(q.get("to"))?;
    let key = format!("po|{}|{}", from, to);

    let (f_from, f_to) = (from.clone(), to.clone());
    let value = state
        .cached_diff(key, move || {
            run_diff_generic("po", from, to, move |a, b| {
                let mut diff = dst_huiji_wiki::service::snapshot_diff::diff_po_sources(a, b)?;
                diff.from = f_from;
                diff.to = f_to;
                Ok(serde_json::to_value(&diff)?)
            })
        })
        .await
        .map_err(err_status)?;
    Ok(Json(value))
}
