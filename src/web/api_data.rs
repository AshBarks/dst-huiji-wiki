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

/// Default `anim-index.json` path; can be overridden with `ANIM_INDEX`.
fn anim_index_path() -> std::path::PathBuf {
    std::env::var("ANIM_INDEX")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("output/anim-index.json"))
}

fn load_anim_index_value() -> std::result::Result<serde_json::Value, StatusCode> {
    let path = anim_index_path();
    let text = std::fs::read_to_string(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    serde_json::from_str(&text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /api/anim/assets/prefabs?q=hound
pub async fn anim_assets_prefabs(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let index = load_anim_index_value()?;
    let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();
    let items: Vec<serde_json::Value> = index
        .get("prefabs")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .filter(|r| {
                    if ql.is_empty() {
                        return true;
                    }
                    let name = r
                        .get("prefab_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let file = r
                        .get("prefab_file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    name.contains(&ql) || file.contains(&ql)
                })
                .take(200)
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "total": items.len(),
        "items": items,
    })))
}

/// GET /api/anim/assets/prefab/{name}
pub async fn anim_assets_prefab(
    Path(name): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let index = load_anim_index_value()?;
    let name_l = name.to_lowercase();
    let items: Vec<serde_json::Value> = index
        .get("prefabs")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .filter(|r| {
                    r.get("prefab_name")
                        .and_then(|v| v.as_str())
                        .map(|n| n.eq_ignore_ascii_case(&name))
                        .unwrap_or(false)
                        || r.get("prefab_file")
                            .and_then(|v| v.as_str())
                            .map(|f| f.to_lowercase().contains(&name_l))
                            .unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    if items.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let anim_files = index
        .get("anim_files")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let build_files = index
        .get("build_files")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut seen = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    for item in &items {
        if let Some(anims) = item.get("anims").and_then(|v| v.as_array()) {
            for a in anims {
                if let Some(path) = a.get("normalized").and_then(|v| v.as_str()) {
                    if seen.insert(path.to_string()) {
                        let content = anim_files
                            .get(path)
                            .and_then(|v| v.get("content").cloned().or_else(|| Some(v.clone())))
                            .or_else(|| build_files.get(path).cloned())
                            .unwrap_or(serde_json::json!({}));
                        files.push(serde_json::json!({ "path": path, "content": content }));
                    }
                }
            }
        }
        if let Some(related) = item.get("related_files").and_then(|v| v.as_array()) {
            for path_val in related {
                if let Some(path) = path_val.as_str() {
                    if seen.insert(path.to_string()) {
                        let content = build_files
                            .get(path)
                            .cloned()
                            .or_else(|| {
                                anim_files.get(path).and_then(|v| {
                                    v.get("content").cloned().or_else(|| Some(v.clone()))
                                })
                            })
                            .unwrap_or(serde_json::json!({}));
                        files.push(serde_json::json!({ "path": path, "content": content }));
                    }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "items": items,
        "files": files,
        "build_files": build_files,
    })))
}

/// GET /api/anim/assets/render?file=...&bank=...&animation=...&format=gif|png
pub async fn anim_assets_render(
    Query(q): Q,
) -> std::result::Result<axum::response::Response, StatusCode> {
    let files: Vec<String> = q
        .get("files")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if files.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bank = q.get("bank").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let animation = q.get("animation").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let format = match q.get("format").map(String::as_str) {
        Some("gif") => dst_huiji_wiki::scripts_sync::anim::preview::RenderFormat::Gif,
        Some("png") => dst_huiji_wiki::scripts_sync::anim::preview::RenderFormat::Png,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let anim_root = anim_root_from_index_or_env()?;
    let disabled_builds = split_csv(q.get("disabled_builds"));
    let hidden_symbols = split_csv(q.get("hidden_symbols"));
    let symbol_builds = q
        .get("symbol_builds")
        .map(|raw| {
            raw.split(',')
                .filter_map(|pair| {
                    let (sym, build) = pair.split_once(':')?;
                    Some((sym.trim().to_lowercase(), build.trim().to_string()))
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();

    let output = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::render_animation(
            &dst_huiji_wiki::scripts_sync::anim::preview::RenderParams {
                anim_root,
                files,
                disabled_builds,
                hidden_symbols,
                symbol_builds,
                bank,
                animation,
                format,
            },
        )
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;

    let mut response = output.bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(output.content_type),
    );
    if let Ok(disposition) =
        header::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", output.filename))
    {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, disposition);
    }
    Ok(response)
}

/// GET /api/anim/assets/info?files=...&bank=...&animation=...
pub async fn anim_assets_info(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let files: Vec<String> = q
        .get("files")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if files.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bank = q.get("bank").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let animation = q.get("animation").cloned().ok_or(StatusCode::BAD_REQUEST)?;

    let anim_root = anim_root_from_index_or_env()?;
    let value = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::animation_info(
            &dst_huiji_wiki::scripts_sync::anim::preview::RenderParams {
                anim_root,
                files,
                disabled_builds: Vec::new(),
                hidden_symbols: Vec::new(),
                symbol_builds: std::collections::HashMap::new(),
                bank,
                animation,
                format: dst_huiji_wiki::scripts_sync::anim::preview::RenderFormat::Gif,
            },
        )
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;
    Ok(Json(value))
}

/// GET /api/anim/assets/preview?files=...&bank=...&animation=...
/// Returns base64 PNG frames for browser-side animation preview.
pub async fn anim_assets_preview(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let files: Vec<String> = q
        .get("files")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if files.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bank = q.get("bank").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let animation = q.get("animation").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let disabled_builds = split_csv(q.get("disabled_builds"));
    let hidden_symbols = split_csv(q.get("hidden_symbols"));
    let symbol_builds = q
        .get("symbol_builds")
        .map(|raw| {
            raw.split(',')
                .filter_map(|pair| {
                    let (sym, build) = pair.split_once(':')?;
                    Some((sym.trim().to_lowercase(), build.trim().to_string()))
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();

    let anim_root = anim_root_from_index_or_env()?;
    let value = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::render_animation_preview_json(
            &dst_huiji_wiki::scripts_sync::anim::preview::RenderParams {
                anim_root,
                files,
                disabled_builds,
                hidden_symbols,
                symbol_builds,
                bank,
                animation,
                format: dst_huiji_wiki::scripts_sync::anim::preview::RenderFormat::Png,
            },
        )
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;
    Ok(Json(value))
}

/// GET /api/anim/assets/find-builds?symbols=a,b,c
/// Search `data/anim` for build files providing the requested symbols.
pub async fn anim_assets_find_builds(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let symbols = split_csv(q.get("symbols"));
    if symbols.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let anim_root = anim_root_from_index_or_env()?;
    let builds = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::find_builds_for_symbols(&anim_root, &symbols)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;

    Ok(Json(serde_json::json!({ "builds": builds })))
}

fn split_csv(v: Option<&String>) -> Vec<String> {
    v.map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn anim_root_from_index_or_env() -> std::result::Result<std::path::PathBuf, StatusCode> {
    if let Ok(index) = load_anim_index_value() {
        if let Some(root) = index
            .get("anim_root")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Ok(std::path::PathBuf::from(root));
        }
    }
    if let Ok(root) = std::env::var("DST__ROOT") {
        return Ok(std::path::PathBuf::from(root).join("data/anim"));
    }
    Err(StatusCode::NOT_FOUND)
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
