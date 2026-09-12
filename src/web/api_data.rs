//! Data browse / visualization endpoints backed by the in-memory dataset.

use super::state::{ktools_out_dir, AppState};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use dst_huiji_wiki::error::Result;
use dst_huiji_wiki::scripts_sync::images::icons::{
    compare_entries, IconEntry, IconSort, ICON_SOURCES,
};
use dst_huiji_wiki::scripts_sync::images::meta::{self as icon_meta, WikiStatus};
use std::collections::{BTreeMap, HashMap};
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

/// GET /api/data/inventoryicons — 图标网格（文件名/加入历史两种排序，
/// 支持按来源过滤：inventory 物品栏 / crafting 制作栏 / all，按五态过滤：
/// 已上传/未上传/状态未知/无法上传/无英文名）。
pub async fn inventoryicons(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let index = state.icons_index().await.map_err(err_status)?;
    let meta = state.icon_meta().await.map_err(err_status)?;
    let sort = match q.get("sort").map(String::as_str) {
        Some("history") => IconSort::History,
        _ => IconSort::Name,
    };
    let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();
    let status_filter = q.get("status").map(String::as_str).unwrap_or("all");
    let source_filter = q.get("source").map(String::as_str).unwrap_or("all");
    let source_ok = |e: &IconEntry| source_filter == "all" || e.source == source_filter;

    // 元数据缺失时回退到数据集动态解析（旧行为）：翻译 best-effort，数据集
    // （游戏 zip）不可用时仍可浏览图标。
    let fallback = if meta.icons.is_empty() {
        inventory_names(&state).await
    } else {
        HashMap::new()
    };

    struct Resolved {
        name_en: Option<String>,
        name_zh: Option<String>,
        title: Option<String>,
        title_source: Option<icon_meta::TitleSource>,
        uploadable: bool,
        note: Option<String>,
        status: &'static str,
        wiki_uploaded: Option<bool>,
        wiki_title: Option<String>,
    }

    let resolve = |e: &IconEntry| -> Resolved {
        if let Some(m) = meta.icons.get(&e.file) {
            let exists = m.wiki.as_ref().and_then(|w| w.exists);
            return Resolved {
                name_en: m.name_en.clone(),
                name_zh: m.name_zh.clone(),
                title: m.title.clone(),
                title_source: m.title_source,
                uploadable: m.uploadable,
                note: m.note.clone(),
                status: icon_status(m.title.as_deref(), m.name_en.as_deref(), exists),
                wiki_uploaded: exists,
                wiki_title: m.title.as_deref().map(|t| format!("File:{t}")),
            };
        }
        let key = icon_meta::file_stem_key(&e.file);
        let (name_en, name_zh) = match fallback.get(&key) {
            Some((en, zh)) => (Some(en.clone()), (!zh.is_empty()).then(|| zh.clone())),
            None => (None, None),
        };
        let title = name_en.as_deref().and_then(icon_meta::auto_title);
        let status = icon_status(title.as_deref(), name_en.as_deref(), None);
        Resolved {
            name_en,
            name_zh,
            title_source: title.as_ref().map(|_| icon_meta::TitleSource::Auto),
            uploadable: title.is_some(),
            wiki_title: title.as_deref().map(|t| format!("File:{t}")),
            title,
            note: None,
            status,
            wiki_uploaded: None,
        }
    };

    let mut rows: Vec<(&IconEntry, Resolved)> = index
        .entries
        .iter()
        .filter(|e| source_ok(e))
        .map(|e| (e, resolve(e)))
        .filter(|(e, r)| {
            e.file.to_lowercase().contains(&ql)
                || r.name_en
                    .as_deref()
                    .is_some_and(|en| en.to_lowercase().contains(&ql))
                || r.name_zh.as_deref().is_some_and(|zh| zh.contains(&ql))
        })
        .collect();

    // 状态计数在状态过滤前统计（叠加搜索），供筛选项显示数量。
    let mut status_counts = BTreeMap::from([
        ("uploaded", 0usize),
        ("missing", 0usize),
        ("unknown", 0usize),
        ("unuploadable", 0usize),
        ("unnamed", 0usize),
    ]);
    for (_, r) in &rows {
        if let Some(c) = status_counts.get_mut(r.status) {
            *c += 1;
        }
    }

    if status_filter != "all" {
        rows.retain(|(_, r)| r.status == status_filter);
    }
    rows.sort_by(|a, b| compare_entries(a.0, b.0, sort));

    // 历史排序分组汇总：每个 build 组的图标数 / 有名称数 / 维基缺失数。
    #[derive(Default, serde::Serialize)]
    struct BuildSummary {
        total: usize,
        named: usize,
        missing: usize,
    }
    let mut builds: BTreeMap<String, BuildSummary> = BTreeMap::new();
    for e in &index.entries {
        if !source_ok(e) {
            continue;
        }
        let summary = builds.entry(e.first_build.clone()).or_default();
        summary.total += 1;
        if let Some(m) = meta.icons.get(&e.file) {
            if m.name_en.is_some() {
                summary.named += 1;
            }
            if m.uploadable && m.wiki.as_ref().and_then(|w| w.exists) == Some(false) {
                summary.missing += 1;
            }
        }
    }

    // 各来源图标总数（供前端来源切换标签展示）。
    let mut source_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &index.entries {
        *source_counts.entry(e.source.as_str()).or_default() += 1;
    }
    let sources: Vec<serde_json::Value> = ICON_SOURCES
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "label": s.label,
                "total": source_counts.get(s.id).copied().unwrap_or(0),
            })
        })
        .collect();

    let total = rows.len();
    let (page, page_size) = page_params(&q);
    let start = page.saturating_mul(page_size);
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(|(e, r)| {
            serde_json::json!({
                "file": e.file,
                "source": e.source,
                "first_build": e.first_build,
                "first_synced_at": e.first_synced_at,
                "name_en": r.name_en,
                "name_zh": r.name_zh,
                "title": r.title,
                "title_source": r.title_source,
                "uploadable": r.uploadable,
                "note": r.note,
                "status": r.status,
                "wiki_uploaded": r.wiki_uploaded,
                "wiki_title": r.wiki_title,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "total": total,
        "page": page,
        "page_size": page_size,
        "latest_build": index.latest_build,
        "latest_synced_at": index.latest_synced_at,
        "meta_build": meta.build,
        "source": source_filter,
        "sources": sources,
        "status_counts": status_counts,
        "builds": builds,
        "items": items,
    })))
}

/// 五态判定：有效标题 + 上传状态；无标题时区分有英文名/无英文名。
fn icon_status(title: Option<&str>, name_en: Option<&str>, exists: Option<bool>) -> &'static str {
    match (title, exists) {
        (Some(_), Some(true)) => "uploaded",
        (Some(_), Some(false)) => "missing",
        (Some(_), None) => "unknown",
        (None, _) if name_en.is_some() => "unuploadable",
        (None, _) => "unnamed",
    }
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

    let meta = state.icon_meta().await.map_err(err_status)?;
    let (name_en, name_zh, source, title, title_source, uploadable, note, wiki) =
        match meta.icons.get(file) {
            Some(m) => (
                m.name_en.clone(),
                m.name_zh.clone(),
                m.source.clone(),
                m.title.clone(),
                m.title_source,
                m.uploadable,
                m.note.clone(),
                m.wiki.clone(),
            ),
            // 元数据未生成时回退数据集解析；仅在该文件确实不可命名时才有成本。
            None if meta.icons.is_empty() => {
                let names = inventory_names(&state).await;
                let key = icon_meta::file_stem_key(file);
                match names.get(&key) {
                    Some((en, zh)) => {
                        let title = icon_meta::auto_title(en);
                        (
                            Some(en.clone()),
                            (!zh.is_empty()).then(|| zh.clone()),
                            None,
                            title,
                            Some(icon_meta::TitleSource::Auto),
                            true,
                            None,
                            None,
                        )
                    }
                    None => (None, None, None, None, None, false, None, None),
                }
            }
            None => (None, None, None, None, None, false, None, None),
        };
    let status = icon_status(
        title.as_deref(),
        name_en.as_deref(),
        wiki.as_ref().and_then(|w| w.exists),
    );
    let wiki_title = title.as_deref().map(|t| format!("File:{t}"));

    Ok(Json(serde_json::json!({
        "file": file,
        "present": index.contains(file),
        "versions": versions,
        "name_en": name_en,
        "name_zh": name_zh,
        "source": source,
        "title": title,
        "title_source": title_source,
        "uploadable": uploadable,
        "note": note,
        "status": status,
        "wiki_title": wiki_title,
        "wiki": wiki_status_json(wiki.as_ref()),
    })))
}

/// POST /api/data/inventoryicons/title — 设置/清除图标的 wiki 文件名映射。
///
/// 请求体 `{ "file": "x.png", "title": "Pick-Axe.png" }`；`title` 为空或
/// `null` 时清除映射（恢复自动标题）。映射写入 `icon_title_overrides.json`
/// 并立即更新 `icon_meta.json`；标题变化时旧上传状态失效，并 best-effort
/// 查询一次新标题的维基状态。
pub async fn set_inventoryicon_title(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IconTitleEditRequest>,
) -> std::result::Result<Json<serde_json::Value>, ApiError> {
    let file = req.file.trim().to_string();
    if !icon_meta::is_icon_file_name(&file) {
        return Err(bad_request(format!("无效的图标文件名：{file}")));
    }
    let index = state
        .icons_index()
        .await
        .map_err(|e| internal_error(e.to_string()))?;
    if !index.contains(&file) {
        return Err(bad_request(format!("当前图标清单中没有 {file}")));
    }
    let title = match req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => {
            Some(icon_meta::normalize_explicit_title(raw).map_err(|e| bad_request(e.to_string()))?)
        }
        None => None,
    };

    let path = icon_meta::overrides_path();
    let mut overrides =
        icon_meta::load_overrides(&path).map_err(|e| internal_error(e.to_string()))?;
    let changed = match &title {
        Some(t) => overrides.get(&file) != Some(t.as_str()),
        None => overrides.remove(&file).is_some(),
    };
    if let Some(t) = &title {
        overrides.insert(&file, t);
    }
    if changed {
        icon_meta::save_overrides(&path, &overrides).map_err(|e| internal_error(e.to_string()))?;
    }

    let source_id = index
        .entries
        .iter()
        .find(|e| e.file == file)
        .map(|e| e.source.clone());
    let out_dir = ktools_out_dir();
    let mut meta = (*state
        .icon_meta()
        .await
        .map_err(|e| internal_error(e.to_string()))?)
    .clone();
    let entry = meta
        .icons
        .entry(file.clone())
        .or_insert_with(|| icon_meta::IconMetaEntry {
            name_en: None,
            name_zh: None,
            source: source_id.clone(),
            title: None,
            title_source: None,
            uploadable: false,
            note: None,
            wiki: None,
        });
    if entry.source.is_none() {
        entry.source = source_id;
    }
    icon_meta::refresh_entry_title(entry, &file, &overrides);
    // 无英文名且无标题的空条目没有保留价值，直接移除。
    if meta
        .icons
        .get(&file)
        .is_some_and(|e| e.name_en.is_none() && e.title.is_none())
    {
        meta.icons.remove(&file);
    }
    // best-effort 查询新标题状态（凭据缺失/网络失败时保持“状态未知”）。
    if title.is_some() {
        if let Err(e) = dst_huiji_wiki::service::upload_icons::query_wiki_status(
            &mut meta,
            std::slice::from_ref(&file),
        )
        .await
        {
            tracing::warn!("查询 {} 的维基状态失败: {}", file, e);
        }
    }
    icon_meta::save(&out_dir, &meta).map_err(|e| internal_error(e.to_string()))?;

    let entry = meta.icons.get(&file);
    let name_en = entry.and_then(|e| e.name_en.clone());
    let title = entry.and_then(|e| e.title.clone());
    let wiki = entry.and_then(|e| e.wiki.clone());
    let status = icon_status(
        title.as_deref(),
        name_en.as_deref(),
        wiki.as_ref().and_then(|w| w.exists),
    );
    Ok(Json(serde_json::json!({
        "file": file,
        "source": entry.and_then(|e| e.source.clone()),
        "title": title,
        "title_source": entry.and_then(|e| e.title_source),
        "status": status,
        "wiki_title": title.as_deref().map(|t| format!("File:{t}")),
        "wiki": wiki_status_json(wiki.as_ref()),
        "overrides_path": path.display().to_string(),
    })))
}

/// 映射编辑请求体。
#[derive(Debug, serde::Deserialize)]
pub struct IconTitleEditRequest {
    pub file: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// Web UI 的可读错误（`(status, {"error": msg})`）。
type ApiError = (StatusCode, Json<serde_json::Value>);

fn bad_request(msg: impl Into<String>) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg.into() })),
    )
}

fn internal_error(msg: impl Into<String>) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg.into() })),
    )
}

/// `WikiStatus` → API JSON（`None` 表示尚未查询）。
fn wiki_status_json(status: Option<&WikiStatus>) -> Option<serde_json::Value> {
    status.map(|w| {
        serde_json::json!({
            "exists": w.exists,
            "title": w.title,
            "url": w.url,
            "checked_at": w.checked_at,
        })
    })
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

/// ANIM__OUT_DIR，缺省 `output/anim`。
fn anim_out_dir() -> std::path::PathBuf {
    dst_huiji_wiki::platform::config::anim_out_dir()
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

/// Default `anim-remap-index.json` path: sibling of the anim index (or
/// `ANIM_REMAP_INDEX` override).
fn anim_remap_index_path() -> std::path::PathBuf {
    std::env::var("ANIM_REMAP_INDEX")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| anim_index_path().with_file_name("anim-remap-index.json"))
}

/// GET /api/anim/remap-manifests — 列出重映射索引快照。
pub async fn anim_remap_manifests() -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let dir = dst_huiji_wiki::scripts_sync::anim::remap_history::remap_history_dir();
    let labels = dst_huiji_wiki::scripts_sync::anim::remap_history::list_remap_labels(&dir)
        .map_err(err_status)?;
    let manifests = labels
        .iter()
        .filter_map(|label| {
            dst_huiji_wiki::scripts_sync::anim::remap_history::load_remap_snapshot(&dir, label)
                .ok()
                .as_ref()
                .map(dst_huiji_wiki::scripts_sync::anim::remap_history::remap_snapshot_summary)
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "manifests": manifests })))
}

/// GET /api/anim/remap-diff?from=&to= — 对比两份重映射快照。
pub async fn anim_remap_diff(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let from = q.get("from").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let to = q.get("to").cloned().ok_or(StatusCode::BAD_REQUEST)?;
    let dir = dst_huiji_wiki::scripts_sync::anim::remap_history::remap_history_dir();
    let old = dst_huiji_wiki::scripts_sync::anim::remap_history::load_remap_snapshot(&dir, &from)
        .map_err(err_status)?;
    let new = dst_huiji_wiki::scripts_sync::anim::remap_history::load_remap_snapshot(&dir, &to)
        .map_err(err_status)?;
    let diff = dst_huiji_wiki::scripts_sync::anim::remap_history::diff_remap_indexes(&old, &new);
    Ok(Json(
        serde_json::to_value(diff)
            .map_err(dst_huiji_wiki::error::Error::Json)
            .map_err(err_status)?,
    ))
}

/// GET /api/anim/assets/remaps?symbols=a,b,c
///
/// Remap entries from `anim-remap-index.json` (Tier-A static AnimState
/// override calls), optionally filtered to the animation's symbols. Degrades
/// to an empty index when the artifact has not been built.
pub async fn anim_assets_remaps(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let path = anim_remap_index_path();
    let index = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        Err(_) => serde_json::json!({ "symbols": {} }),
    };
    let symbols = index
        .get("symbols")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let filtered = match q.get("symbols") {
        None => symbols,
        Some(raw) => {
            let wanted: std::collections::HashSet<String> = raw
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            symbols
                .into_iter()
                .filter(|(k, _)| wanted.contains(k))
                .collect()
        }
    };
    Ok(Json(serde_json::json!({ "symbols": filtered })))
}

/// GET /api/anim/assets/clothing-overrides
///
/// Tier-B clothing.lua 数据：
/// - 无 `name`：返回衣物摘要列表（可 `q=` 过滤，最多 50 条）；
/// - 有 `name`（可带 `character=`、`skintype=`）：按 skinner.lua 规则解析出
///   anim symbol → 源 symbol 的覆盖表，并给出可直接使用的
///   `symbol_overrides` CSV（`sym:build:src,...`）。
pub async fn anim_assets_clothing_overrides(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let path = anim_remap_index_path();
    let index: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        Err(_) => serde_json::json!({}),
    };
    let clothing = index
        .get("clothing")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    match q.get("name").map(|s| s.trim().to_lowercase()) {
        Some(name) => {
            let raw = clothing.get(&name).ok_or(StatusCode::NOT_FOUND)?;
            let entry: dst_huiji_wiki::parser::clothing_overrides::ClothingEntry =
                serde_json::from_value(raw.clone())
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let character = q
                .get("character")
                .map(|s| s.as_str())
                .filter(|s| !s.trim().is_empty());
            let skintype = q
                .get("skintype")
                .map(|s| s.as_str())
                .filter(|s| !s.trim().is_empty());
            let resolved = entry.resolve_overrides(&name, character, skintype);
            let csv = resolved
                .overrides
                .iter()
                .map(|(sym, src)| format!("{sym}:{}:{src}", resolved.build))
                .collect::<Vec<_>>()
                .join(",");
            Ok(Json(serde_json::json!({
                "name": name,
                "build": resolved.build,
                "overrides": resolved.overrides,
                "symbol_overrides": csv,
            })))
        }
        None => {
            let ql = q.get("q").map(|s| s.to_lowercase()).unwrap_or_default();
            let items: Vec<serde_json::Value> = clothing
                .iter()
                .filter(|(name, _)| ql.is_empty() || name.to_lowercase().contains(&ql))
                .take(50)
                .map(|(name, e)| {
                    serde_json::json!({
                        "name": name,
                        "type": e.get("clothing_type"),
                        "symbols": e.get("symbol_overrides").and_then(|v| v.as_array()).map(Vec::len).unwrap_or(0),
                        "characters": e.get("symbol_overrides_by_character").and_then(|v| v.as_object()).map(|o| o.len()).unwrap_or(0),
                    })
                })
                .collect();
            Ok(Json(serde_json::json!({ "items": items })))
        }
    }
}

/// Default `skin-index.json` path; can be overridden with `SKIN_INDEX`.
fn skin_index_path() -> std::path::PathBuf {
    std::env::var("SKIN_INDEX")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("output/skin-index.json"))
}

/// Load `skin-index.json`; `Ok(None)` when the artifact has not been built.
fn load_skin_index_optional() -> std::result::Result<Option<serde_json::Value>, StatusCode> {
    let path = skin_index_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `base_prefab -> [SkinEntry, ...]` from the skin index, if available.
fn skin_index_prefab_skins(
    index: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    index
        .get("prefab_skins")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default()
}

/// Keep only renderable skins (zip + dyn both paired) of a skin group.
fn renderable_skins(entries: &[serde_json::Value]) -> Vec<serde_json::Value> {
    entries
        .iter()
        .filter(|e| e.get("zip").is_some() && e.get("dyn").is_some())
        .cloned()
        .collect()
}

/// `AnimContent.builds` 非空表示该归档提供 build.bin。
fn content_has_build(content: &serde_json::Value) -> bool {
    content
        .get("builds")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

/// 一个 prefab 记录引用的文件里，实际带 build.bin 的归档数量。
fn prefab_build_file_count(
    record: &serde_json::Value,
    anim_files: &serde_json::Map<String, serde_json::Value>,
    build_files: &serde_json::Map<String, serde_json::Value>,
) -> usize {
    let mut paths: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if let Some(anims) = record.get("anims").and_then(|v| v.as_array()) {
        for a in anims {
            if let Some(p) = a.get("normalized").and_then(|v| v.as_str()) {
                paths.insert(p.to_string());
            }
        }
    }
    if let Some(related) = record.get("related_files").and_then(|v| v.as_array()) {
        for p in related {
            if let Some(p) = p.as_str() {
                paths.insert(p.to_string());
            }
        }
    }
    paths
        .iter()
        .filter(|p| {
            anim_files
                .get(*p)
                .map(|v| content_has_build(v.get("content").unwrap_or(v)))
                .unwrap_or(false)
                || build_files.get(*p).map(content_has_build).unwrap_or(false)
        })
        .count()
}

/// GET /api/anim/assets/prefabs?q=hound
pub async fn anim_assets_prefabs(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let index = load_anim_index_value()?;
    let skin_counts = load_skin_index_optional()?
        .as_ref()
        .map(skin_index_prefab_skins)
        .unwrap_or_default();
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
                .map(|mut r| {
                    if let Some(name) = r.get("prefab_name").and_then(|v| v.as_str()) {
                        if let Some(skins) = skin_counts
                            .get(name)
                            .and_then(|v| v.as_array())
                            .map(|group| renderable_skins(group).len())
                        {
                            r["skin_count"] = serde_json::json!(skins);
                        }
                    }
                    r["build_file_count"] =
                        serde_json::json!(prefab_build_file_count(&r, &anim_files, &build_files));
                    r
                })
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

    // Union of renderable skins across the matched prefab names.
    let skin_skins = load_skin_index_optional()?
        .map(|index| skin_index_prefab_skins(&index))
        .unwrap_or_default();
    let mut skins: Vec<serde_json::Value> = Vec::new();
    let mut skin_seen = std::collections::BTreeSet::new();
    for item in &items {
        if let Some(name) = item.get("prefab_name").and_then(|v| v.as_str()) {
            if let Some(group) = skin_skins.get(name).and_then(|v| v.as_array()) {
                for skin in renderable_skins(group) {
                    if let Some(s) = skin.get("skin").and_then(|v| v.as_str()) {
                        if skin_seen.insert(s.to_string()) {
                            skins.push(skin);
                        }
                    }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "items": items,
        "files": files,
        "build_files": build_files,
        "skins": skins,
    })))
}

/// GET /api/anim/assets/skins?prefab=abigail
/// Renderable skins (zip + dyn paired) for a base prefab from `skin-index.json`.
pub async fn anim_assets_skins(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let prefab = q
        .get("prefab")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let index = load_skin_index_optional()?.ok_or(StatusCode::NOT_FOUND)?;
    let skins = skin_index_prefab_skins(&index)
        .get(prefab)
        .and_then(|v| v.as_array())
        .map(|group| renderable_skins(group))
        .unwrap_or_default();
    Ok(Json(serde_json::json!({ "skins": skins })))
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
    let (skin_zip, skin_dyn) = skin_query_params(&q);
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
    let symbol_overrides = q
        .get("symbol_overrides")
        .map(|raw| dst_huiji_wiki::scripts_sync::anim::preview::parse_symbol_overrides(raw))
        .unwrap_or_default();

    let output = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::render_animation(
            &dst_huiji_wiki::scripts_sync::anim::preview::RenderParams {
                anim_root,
                files,
                disabled_builds,
                hidden_symbols,
                symbol_builds,
                skin_zip,
                skin_dyn,
                symbol_overrides,
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
    let (skin_zip, skin_dyn) = skin_query_params(&q);

    let anim_root = anim_root_from_index_or_env()?;
    let value = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::animation_info(
            &dst_huiji_wiki::scripts_sync::anim::preview::RenderParams {
                anim_root,
                files,
                disabled_builds: Vec::new(),
                hidden_symbols: Vec::new(),
                symbol_builds: std::collections::HashMap::new(),
                skin_zip,
                skin_dyn,
                symbol_overrides: Vec::new(),
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
    let (skin_zip, skin_dyn) = skin_query_params(&q);
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
    let symbol_overrides = q
        .get("symbol_overrides")
        .map(|raw| dst_huiji_wiki::scripts_sync::anim::preview::parse_symbol_overrides(raw))
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
                skin_zip,
                skin_dyn,
                symbol_overrides,
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
/// Search `data/anim` for builds that can serve the requested symbols, by
/// same-name provider or via a Tier-A symbol map entry.
pub async fn anim_assets_find_builds(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let symbols = split_csv(q.get("symbols"));
    if symbols.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let anim_root = anim_root_from_index_or_env()?;
    let builds = tokio::task::spawn_blocking(move || {
        let remaps = load_remap_index();
        dst_huiji_wiki::scripts_sync::anim::preview::find_builds_for_symbols(
            &anim_root,
            &symbols,
            remaps.as_ref(),
        )
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;

    Ok(Json(serde_json::json!({ "builds": builds })))
}

/// Tier-A 符号表（`anim-remap-index.json`）；产物缺失时返回 None。
fn load_remap_index() -> Option<dst_huiji_wiki::parser::anim_override::SymbolRemapIndex> {
    let text = std::fs::read_to_string(anim_remap_index_path()).ok()?;
    let artifact: dst_huiji_wiki::scripts_sync::anim::remap_history::RemapArtifact =
        serde_json::from_str(&text).ok()?;
    Some(dst_huiji_wiki::parser::anim_override::SymbolRemapIndex {
        symbols: artifact.symbols,
    })
}

/// GET /api/anim/assets/files?q=&limit=
/// 手动导入：按相对路径检索 `data/anim` 下的 zip/dyn 归档。
pub async fn anim_assets_files(
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let query = q.get("q").cloned().unwrap_or_default();
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200)
        .min(1000);
    let anim_root = anim_root_from_index_or_env()?;
    let files = tokio::task::spawn_blocking(move || {
        dst_huiji_wiki::scripts_sync::anim::preview::list_archives(&anim_root, &query, limit)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(err_status)?;

    Ok(Json(serde_json::json!({ "files": files })))
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

/// Optional `skin_zip` / `skin_dyn` query parameters for the render endpoints.
/// Empty values are treated as absent; `skin_dyn` may be omitted so the
/// backend auto-pairs the stem `<stem>.dyn` next to the skin zip.
fn skin_query_params(
    q: &std::collections::HashMap<String, String>,
) -> (Option<String>, Option<String>) {
    let trim_opt = |v: Option<&String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    (trim_opt(q.get("skin_zip")), trim_opt(q.get("skin_dyn")))
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
    if let Some(root) = dst_huiji_wiki::platform::config::dst_root_opt() {
        return Ok(root.join("data/anim"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_status_five_states() {
        assert_eq!(
            icon_status(Some("A.png"), Some("A"), Some(true)),
            "uploaded"
        );
        assert_eq!(
            icon_status(Some("A.png"), Some("A"), Some(false)),
            "missing"
        );
        assert_eq!(icon_status(Some("A.png"), Some("A"), None), "unknown");
        assert_eq!(icon_status(None, Some("A"), None), "unuploadable");
        assert_eq!(icon_status(None, None, None), "unnamed");
        // 无英文名但有映射标题 → 按上传状态归类
        assert_eq!(icon_status(Some("Skin.png"), None, Some(false)), "missing");
    }
}
