//! Cooking simulator data model.
//!
//! The Rust side compiles the DST cooking Lua sources into this serializable
//! structure. The WebUI frontend then interprets the recipe `test` AST against
//! four selected ingredients; no Lua VM is involved on either side.

use serde::Serialize;
use std::collections::BTreeMap;

/// Current JSON contract version consumed by the WebUI interpreter.
pub const COOKING_SCHEMA_VERSION: u32 = 1;

/// One selectable ingredient prefab.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CookingIngredient {
    /// Actual game prefab shown in the ingredient panel.
    pub prefab: String,
    /// Key used by recipe `test` expressions (alias-normalised).
    pub key: String,
    /// Accumulated ingredient tags, e.g. `{"fruit": 0.5}`.
    pub tags: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_zh: Option<String>,
    /// Inventory image file name below `split/inventoryimages/`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// One cooker recipe after compilation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CookingRecipe {
    pub name: String,
    pub priority: f64,
    /// Reserved for future weighted draws. Current game files keep this at 1.
    pub weight: f64,
    /// Source Lua file (for audit / debugging).
    pub source: String,
    /// JSON-encoded expression tree interpreted by the frontend.
    pub test: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_zh: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Complete compiled payload for `/api/data/cooking`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CookingData {
    pub schema_version: u32,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    /// Cooker name -> recipe names, deterministic order.
    pub cookers: BTreeMap<String, Vec<String>>,
    /// Only actual prefabs accepted by the crock pot.
    pub ingredients: Vec<CookingIngredient>,
    /// All known recipes keyed by product name.
    pub recipes: BTreeMap<String, CookingRecipe>,
}
