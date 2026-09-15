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
use dst_huiji_wiki::service::cooking_assets::{names_for, po_name_index, IconResolver};
use std::collections::HashMap;
use std::sync::Arc;

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

    let icons = match (state.icons_index().await, state.icon_meta().await) {
        (Ok(index), Ok(meta)) => IconResolver::from_index(&index, &meta),
        (Ok(index), Err(_)) => IconResolver::from_index(&index, &Default::default()),
        _ => IconResolver::default(),
    };

    for ingredient in &mut data.ingredients {
        let (en, zh) = names_for(&names, &ingredient.prefab);
        ingredient.icon = icons.resolve(&ingredient.prefab, en.as_deref());
        ingredient.name_en = en;
        ingredient.name_zh = zh;
    }

    for recipe in data.recipes.values_mut() {
        let (en, zh) = names_for(&names, &recipe.name);
        recipe.icon = icons.resolve(&recipe.name, en.as_deref());
        recipe.name_en = en;
        recipe.name_zh = zh;
    }
}
