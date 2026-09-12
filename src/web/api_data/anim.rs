//! 动画历史/皮肤预览端点（manifest、diff、资产渲染、skin-index）。

use super::err_status;
use super::Q;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;

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
