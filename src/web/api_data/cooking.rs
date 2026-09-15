//! Compiled cooking simulator payload endpoint.
//!
//! `/api/data/cooking` returns the JSON contract produced by
//! [`dst_huiji_wiki::service::cooking`], enriched with PO names and local
//! inventory icon file names so the frontend can stay presentation-only.

use super::super::state::AppState;
use super::{err_status, Q};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use dst_huiji_wiki::models::CookingData;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Image names that differ from the prefab name and cannot be resolved by the
/// PO English-name fallback (the wiki metadata uses slightly different names,
/// e.g. `Roast Onion` vs `Roasted Onion`).
const ICON_EXCEPTIONS: &[(&str, &str)] = &[
    ("onion", "quagmire_onion.png"),
    ("onion_cooked", "quagmire_onion_cooked.png"),
    ("tomato", "quagmire_tomato.png"),
    ("tomato_cooked", "quagmire_tomato_cooked.png"),
];

/// GET /api/data/cooking?snapshot=...
pub async fn cooking(
    State(state): State<Arc<AppState>>,
    Query(q): Q,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let snapshot = q.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let cached = state
        .cooking
        .get_or_load(snapshot.clone())
        .await
        .map_err(err_status)?;
    let mut data = (*cached).clone();

    enrich(&state, &mut data, snapshot).await;

    let value = serde_json::to_value(data).map_err(|e| err_status(e.into()))?;
    Ok(Json(value))
}

async fn enrich(state: &AppState, data: &mut CookingData, snapshot: Option<String>) {
    let names = match state.datasets.get_or_load(snapshot).await {
        Ok(dataset) => po_name_index(&dataset.po_entries),
        Err(e) => {
            tracing::warn!("cooking API: PO names unavailable: {}", e);
            HashMap::new()
        }
    };

    let mut inventory_files: HashSet<String> = HashSet::new();
    let mut icon_by_name: HashMap<String, String> = HashMap::new();
    if let Ok(index) = state.icons_index().await {
        for entry in &index.entries {
            if entry.source == "inventory" {
                inventory_files.insert(entry.file.clone());
            }
        }
    }
    if let Ok(meta) = state.icon_meta().await {
        for (file, entry) in &meta.icons {
            if entry.source.as_deref().unwrap_or("inventory") != "inventory" {
                continue;
            }
            if let Some(name) = &entry.name_en {
                icon_by_name
                    .entry(name.to_lowercase())
                    .or_insert_with(|| file.clone());
            }
        }
    }

    for ingredient in &mut data.ingredients {
        let (en, zh) = names_for(&names, &ingredient.prefab);
        ingredient.icon = resolve_icon(
            &inventory_files,
            &icon_by_name,
            &ingredient.prefab,
            en.as_deref(),
        );
        ingredient.name_en = en;
        ingredient.name_zh = zh;
    }

    for recipe in data.recipes.values_mut() {
        let (en, zh) = names_for(&names, &recipe.name);
        recipe.icon = resolve_icon(&inventory_files, &icon_by_name, &recipe.name, en.as_deref());
        recipe.name_en = en;
        recipe.name_zh = zh;
    }
}

fn names_for(
    index: &HashMap<String, (Option<String>, Option<String>)>,
    prefab: &str,
) -> (Option<String>, Option<String>) {
    index
        .get(&prefab.to_ascii_uppercase())
        .cloned()
        .unwrap_or_default()
}

/// `STRINGS.NAMES.*` PO entry index: upper-snake key -> (en, zh).
fn po_name_index(
    entries: &[dst_huiji_wiki::service::dataset::PoEntryDto],
) -> HashMap<String, (Option<String>, Option<String>)> {
    let mut out = HashMap::new();
    for entry in entries {
        let Some(ctxt) = &entry.msgctxt else {
            continue;
        };
        let Some(key) = ctxt.strip_prefix("STRINGS.NAMES.") else {
            continue;
        };
        if !key.is_empty() {
            let en = (!entry.msgid.trim().is_empty()).then(|| entry.msgid.clone());
            let zh = (!entry.msgstr.trim().is_empty() && entry.msgstr != entry.msgid)
                .then(|| entry.msgstr.trim().to_string());
            out.insert(key.to_ascii_uppercase(), (en, zh));
        }
    }
    out
}

fn resolve_icon(
    inventory_files: &HashSet<String>,
    icon_by_name: &HashMap<String, String>,
    prefab: &str,
    name_en: Option<&str>,
) -> Option<String> {
    let exact = format!("{}.png", prefab);
    let exception = ICON_EXCEPTIONS
        .iter()
        .find_map(|(key, file)| (*key == prefab).then_some((*file).to_string()));
    let exact = inventory_files.contains(&exact).then_some(exact);
    let named = name_en.and_then(|name| icon_by_name.get(&name.to_lowercase()).cloned());
    exception
        .or(exact)
        .or(named)
        .filter(|file| inventory_files.contains(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn po_name_index_uses_upper_snake_key() {
        let entries = vec![dst_huiji_wiki::service::dataset::PoEntryDto {
            msgctxt: Some("STRINGS.NAMES.BUTTERFLYMUFFIN".into()),
            msgid: "Butter Muffin".into(),
            msgstr: "蝴蝶松饼".into(),
        }];
        let index = po_name_index(&entries);
        let (en, zh) = names_for(&index, "butterflymuffin");
        assert_eq!(en.as_deref(), Some("Butter Muffin"));
        assert_eq!(zh.as_deref(), Some("蝴蝶松饼"));
    }

    #[test]
    fn reserved_icon_name_fallback_prefers_known_exception_and_checks_inventory() {
        let inventory = HashSet::from(["quagmire_onion.png".to_string()]);
        let by_name = HashMap::new();
        assert_eq!(
            resolve_icon(&inventory, &by_name, "onion", Some("Onion")).as_deref(),
            Some("quagmire_onion.png")
        );
        assert_eq!(
            resolve_icon(&HashSet::new(), &by_name, "onion", Some("Onion")),
            None
        );
    }
}
