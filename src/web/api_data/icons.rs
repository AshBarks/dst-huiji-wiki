//! 物品图标端点（五态状态、版本清单、标题映射编辑）。

use super::super::state::{ktools_out_dir, AppState};
use super::err_status;
use super::Q;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use dst_huiji_wiki::scripts_sync::images::meta::{self as icon_meta, WikiStatus};
use std::collections::HashMap;
use std::sync::Arc;

/// 五态判定：有效标题 + 上传状态；无标题时区分有英文名/无英文名。
pub(super) fn icon_status(
    title: Option<&str>,
    name_en: Option<&str>,
    exists: Option<bool>,
) -> &'static str {
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
/// `null` 时清除映射（恢复自动标题）；与自动命名一致的标题同样视为未指定，
/// 不落映射表。映射写入 `icon_title_overrides.json` 并立即更新
/// `icon_meta.json`；标题变化时旧上传状态失效，并 best-effort 查询一次
/// 新标题的维基状态。
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
    // 与自动命名一致的手动标题视为未指定：不落映射表（等同清除），
    // 覆盖表只收偏离默认的真例外。
    let name_en = meta.icons.get(&file).and_then(|e| e.name_en.clone());
    let is_default = title.as_deref().is_some_and(|t| {
        icon_meta::title_is_default(&file, name_en.as_deref(), source_id.as_deref(), t)
    });

    let path = icon_meta::overrides_path();
    let mut overrides =
        icon_meta::load_overrides(&path).map_err(|e| internal_error(e.to_string()))?;
    let changed = match (&title, is_default) {
        (Some(_), true) => overrides.remove(&file).is_some(),
        (Some(t), false) => {
            let changed = overrides.get(&file) != Some(t.as_str());
            if changed {
                overrides.insert(&file, t);
            }
            changed
        }
        (None, _) => overrides.remove(&file).is_some(),
    };
    if changed {
        icon_meta::save_overrides(&path, &overrides).map_err(|e| internal_error(e.to_string()))?;
    }

    let entry = meta
        .icons
        .entry(file.clone())
        .or_insert_with(|| icon_meta::IconMetaEntry {
            name_en: None,
            name_zh: None,
            source: source_id.clone(),
            crafting: None,
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

pub(super) fn bad_request(msg: impl Into<String>) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg.into() })),
    )
}

pub(super) fn internal_error(msg: impl Into<String>) -> ApiError {
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
///
/// 仅作为 `icon_meta.json` 完全缺失时的兜底，且只覆盖物品栏图标；制作栏
/// 图标的命名解析在 `images-sync` 元数据构建时完成（CraftingKeys 解析链），
/// 不在此重复实现。
pub(super) async fn inventory_names(state: &AppState) -> HashMap<String, (String, String)> {
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
