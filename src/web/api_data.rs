//! Data browse / visualization endpoints backed by the in-memory dataset.

use super::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use dst_huiji_wiki::error::Result;
use std::collections::HashMap;
use std::sync::Arc;

type Q = Query<HashMap<String, String>>;

fn page_params(q: &HashMap<String, String>) -> (usize, usize) {
    let page = q.get("page").and_then(|v| v.parse().ok()).unwrap_or(0);
    let page_size = q
        .get("page_size")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 500);
    (page, page_size)
}

fn err_status(e: dst_huiji_wiki::error::Error) -> StatusCode {
    tracing::warn!("api error: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
}

/// GET /api/data/meta — dataset overview + filter options.
pub async fn meta(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let ds = state
        .datasets
        .get_or_load(snapshot)
        .await
        .map_err(err_status)?;
    Ok(Json(serde_json::json!({
        "label": ds.label,
        "snapshot": ds.snapshot,
        "recipe_count": ds.recipes.len(),
        "ingredient_count": ds.ingredient_index.len(),
        "po_total_entries": ds.po_total_entries,
        "po_categories": ds.po_categories,
        "tech_levels": ds.tech_levels,
        "skill_characters": ds.skill_characters,
        "tuning_count": ds.tuning.len(),
    })))
}

/// GET /api/data/recipes
pub async fn recipes(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let ds = state
        .datasets
        .get_or_load(snapshot)
        .await
        .map_err(err_status)?;
    let (page, page_size) = page_params(&q);
    let (total, items) = ds.search_recipes(
        q.get("q").map(|s| s.as_str()).unwrap_or(""),
        q.get("tech").map(|s| s.as_str()),
        q.get("ingredient").map(|s| s.as_str()),
        page,
        page_size,
    );
    Ok(Json(serde_json::json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "items": items,
    })))
}

/// GET /api/data/ingredients — reverse index browse with usage counts.
pub async fn ingredients(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let ds = state
        .datasets
        .get_or_load(snapshot)
        .await
        .map_err(err_status)?;
    let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();

    let mut rows: Vec<serde_json::Value> = ds
        .ingredient_index
        .iter()
        .filter(|(name, _)| ql.is_empty() || name.to_lowercase().contains(&ql))
        .map(|(name, idxs)| serde_json::json!({ "item": name, "used_in": idxs.len() }))
        .collect();
    rows.sort_by_key(|r| -r["used_in"].as_i64().unwrap_or(0));

    let total = rows.len();
    let (page, page_size) = page_params(&q);
    let start = page.saturating_mul(page_size);
    Ok(Json(serde_json::json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "items": rows.into_iter().skip(start).take(page_size).collect::<Vec<_>>(),
    })))
}

/// GET /api/data/po/entries — paginated PO browsing.
pub async fn po_entries(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let ds = state
        .datasets
        .get_or_load(snapshot)
        .await
        .map_err(err_status)?;

    let category = q.get("category").filter(|c| !c.is_empty());
    let translated_filter = match q.get("translated").map(|s| s.as_str()) {
        Some("yes") => Some(true),
        Some("no") => Some(false),
        _ => None,
    };
    let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();

    let filtered: Vec<&dst_huiji_wiki::service::dataset::PoEntryDto> = ds
        .po_entries
        .iter()
        .filter(|e| {
            if let Some(cat) = category {
                let prefix = format!("STRINGS.{}.", cat);
                let in_cat = match &e.msgctxt {
                    Some(c) => c.starts_with(&prefix),
                    None => cat == "NONE",
                };
                if !in_cat {
                    return false;
                }
            }
            match translated_filter {
                Some(true) if e.msgstr.trim().is_empty() => return false,
                Some(false) if !e.msgstr.trim().is_empty() => return false,
                _ => {}
            }
            ql.is_empty()
                || e.msgid.to_lowercase().contains(&ql)
                || e.msgstr.to_lowercase().contains(&ql)
                || e.msgctxt
                    .as_ref()
                    .map(|c| c.to_lowercase().contains(&ql))
                    .unwrap_or(false)
        })
        .collect();

    let total = filtered.len();
    let (page, page_size) = page_params(&q);
    let start = page.saturating_mul(page_size);
    Ok(Json(serde_json::json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "items": filtered.into_iter().skip(start).take(page_size).collect::<Vec<_>>(),
    })))
}

/// GET /api/data/constants — TUNING table browser.
pub async fn constants(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let ds = state
        .datasets
        .get_or_load(snapshot)
        .await
        .map_err(err_status)?;
    let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();

    let filtered: Vec<&dst_huiji_wiki::service::dataset::TuningEntry> = ds
        .tuning
        .iter()
        .filter(|e| {
            ql.is_empty()
                || e.key.to_lowercase().contains(&ql)
                || e.value.to_lowercase().contains(&ql)
        })
        .collect();
    let total = filtered.len();
    let (page, page_size) = page_params(&q);
    let start = page.saturating_mul(page_size);

    Ok(Json(serde_json::json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "items": filtered.into_iter().skip(start).take(page_size).collect::<Vec<_>>(),
    })))
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
) -> Result<serde_json::Value>
where
    F: FnOnce(&str, &str) -> Result<serde_json::Value>,
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
