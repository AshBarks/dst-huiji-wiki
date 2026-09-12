//! 数据集浏览端点（meta / recipes / ingredients / po / constants）。

use super::super::state::AppState;
use super::icons::{icon_status, inventory_names};
use super::Q;
use super::{err_status, page_params};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use dst_huiji_wiki::scripts_sync::images::icons::{
    compare_entries, IconEntry, IconSort, ICON_SOURCES,
};
use dst_huiji_wiki::scripts_sync::images::meta as icon_meta;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

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
