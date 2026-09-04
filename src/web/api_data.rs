//! Data browse / visualization endpoints backed by the in-memory dataset.

use super::state::{ktools_out_dir, AppState};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use dst_huiji_wiki::error::Result;
use dst_huiji_wiki::scripts_sync::images::icons::{compare_entries, IconEntry, IconSort};
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

/// GET /api/data/inventoryicons — 物品图标网格（文件名/加入历史两种排序）。
pub async fn inventoryicons(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let index = state.icons_index().await.map_err(err_status)?;
    let sort = match q.get("sort").map(String::as_str) {
        Some("history") => IconSort::History,
        _ => IconSort::Name,
    };
    let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();

    // 翻译为 best-effort：数据集（游戏 zip）不可用时仍可浏览图标。
    let names = inventory_names(&state).await;

    struct Row<'a> {
        entry: &'a IconEntry,
        name_en: Option<String>,
        name_zh: Option<String>,
    }
    // 文件名 stem 大写 → STRINGS.NAMES 键（abigail_flower.png → ABIGAIL_FLOWER）。
    let lookup = |e: &IconEntry| {
        names.get(
            e.file
                .strip_suffix(".png")
                .unwrap_or(&e.file)
                .to_ascii_uppercase()
                .as_str(),
        )
    };
    let mut rows: Vec<Row> = index
        .entries
        .iter()
        .filter(|e| {
            if ql.is_empty() {
                return true;
            }
            e.file.contains(&ql)
                || lookup(e)
                    .is_some_and(|(en, zh)| en.to_lowercase().contains(&ql) || zh.contains(&ql))
        })
        .map(|e| {
            let (name_en, name_zh) = match lookup(e) {
                Some((en, zh)) => (Some(en.clone()), (!zh.is_empty()).then(|| zh.clone())),
                None => (None, None),
            };
            Row {
                entry: e,
                name_en,
                name_zh,
            }
        })
        .collect();
    rows.sort_by(|a, b| compare_entries(a.entry, b.entry, sort));

    let total = rows.len();
    let (page, page_size) = page_params(&q);
    let start = page.saturating_mul(page_size);
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(|r| {
            serde_json::json!({
                "file": r.entry.file,
                "first_build": r.entry.first_build,
                "first_synced_at": r.entry.first_synced_at,
                "name_en": r.name_en,
                "name_zh": r.name_zh,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "latest_build": index.latest_build,
        "latest_synced_at": index.latest_synced_at,
        "items": items,
    })))
}

/// GET /api/data/inventoryicons/versions?file=xxx.png — 图标历代版本（自新向旧）。
pub async fn inventoryicon_versions(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let file = q.get("file").map(String::as_str).unwrap_or_default();
    if !file.ends_with(".png") || file.contains('/') || file.contains('\\') || file.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let index = state.icons_index().await.map_err(err_status)?;
    let Some(versions) = index.version_history(file) else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(Json(serde_json::json!({
        "file": file,
        "present": index.contains(file),
        "versions": versions,
    })))
}

/// 文件名 stem（大写）→ (英文 msgid, 中文 msgstr)，取自 STRINGS.NAMES.* 条目。
async fn inventory_names(state: &AppState) -> HashMap<String, (String, String)> {
    match state.datasets.get_or_load(None).await {
        Ok(ds) => ds
            .po_entries
            .iter()
            .filter_map(|e| {
                let key = e.msgctxt.as_ref()?.strip_prefix("STRINGS.NAMES.")?;
                Some((
                    key.to_ascii_uppercase(),
                    (e.msgid.clone(), e.msgstr.trim().to_string()),
                ))
            })
            .collect(),
        Err(e) => {
            tracing::warn!("数据集加载失败，物品图标将不含名称翻译: {}", e);
            HashMap::new()
        }
    }
}

/// GET /static/split/{dir}/{name}
///
/// Serves PNG assets extracted by `images-sync` from the current split tree.
/// Only the fixed skilltree-related / inventoryimages directories are allowed.
pub async fn split_asset(
    Path((dir, name)): Path<(String, String)>,
) -> std::result::Result<axum::response::Response, StatusCode> {
    if !matches!(
        dir.as_str(),
        "skilltree" | "skilltree_icons" | "global_redux" | "inventoryimages"
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

/// ANIM__OUT_DIR，缺省 `output/anim`。
fn anim_out_dir() -> std::path::PathBuf {
    std::env::var("ANIM__OUT_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("output/anim"))
}

/// GET /api/anim/manifests — 列出动画历史 manifest。
pub async fn anim_manifests() -> Json<serde_json::Value> {
    let store = dst_huiji_wiki::scripts_sync::anim::history::ManifestStore::new(
        anim_out_dir().join("history/manifests"),
    );
    let labels = store.list_labels().unwrap_or_default();
    Json(serde_json::json!({ "manifests": labels }))
}

/// GET /api/anim/diff?from=<label>&to=<label>&zip=<rel>
pub async fn anim_diff(Query(q): Q) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let from = sanitize_anim_label(q.get("from"))?;
    let to = sanitize_anim_label(q.get("to"))?;
    let only = q.get("zip").filter(|s| !s.is_empty()).map(String::as_str);
    let only = only.map(sanitize_anim_rel).transpose()?;

    let out = anim_out_dir();
    let old =
        dst_huiji_wiki::scripts_sync::anim::load_history_files(&out, &from).map_err(err_status)?;
    let new =
        dst_huiji_wiki::scripts_sync::anim::load_history_files(&out, &to).map_err(err_status)?;
    let diff = dst_huiji_wiki::scripts_sync::anim::diff::diff_directories(
        &from,
        &to,
        &old,
        &new,
        only.as_deref(),
        false,
    )
    .map_err(err_status)?;
    Ok(Json(
        serde_json::to_value(diff).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    ))
}

fn sanitize_anim_label(v: Option<&String>) -> std::result::Result<String, StatusCode> {
    let s = v.ok_or(StatusCode::BAD_REQUEST)?.clone();
    if s.is_empty() || s.contains("..") || s.contains('/') || s.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(s)
}

fn sanitize_anim_rel(v: &str) -> std::result::Result<String, StatusCode> {
    if v.is_empty() || v.contains("..") || v.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(v.to_string())
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
