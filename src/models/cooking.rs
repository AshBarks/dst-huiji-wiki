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

/// Entry-instant effect of a dish on the eater.
///
/// Values come straight from the recipe entry (`health` / `hunger` / `sanity`)
/// with `TUNING.*` resolved; they are not the in-game cookbook numbers (those
/// override the raw value for `jellybean` / `sweettea`) and not the per-character
/// result (absorption and food-memory multipliers are out of scope). Entries
/// whose real numbers are defined elsewhere — `preparednonfoods.lua` products
/// such as `batnosehat` / `dustmeringue` — are exported as zeros.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct CookingStats {
    pub health: f64,
    pub hunger: f64,
    pub sanity: f64,
}

/// Instant temperature change applied when the dish is eaten
/// (`temperature` / `temperatureduration` in the recipe entry).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CookingTemperature {
    /// Temperature delta; `COLD_FOOD_BONUS_TEMP` is negative.
    pub delta: f64,
    /// How long the delta lasts, in seconds.
    pub duration: f64,
    /// `nochill`: the dish does not apply the usual chill effect.
    #[serde(skip_serializing_if = "is_false")]
    pub no_chill: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// One cooker recipe after compilation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CookingRecipe {
    pub name: String,
    pub priority: f64,
    /// Reserved for future weighted draws. Current game files keep this at 1.
    pub weight: f64,
    /// Entry-instant health/hunger/sanity effect.
    pub stats: CookingStats,
    /// Present for the dishes the game marks as hot or cold food.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<CookingTemperature>,
    /// Food type (`FOODTYPE.*` name, e.g. `MEAT` / `GOODIES`). `None` means the
    /// entry is not an edible food at all (`preparednonfoods.lua` products such
    /// as `batnosehat`); recipes that simply omit the field default to
    /// `GENERIC`, mirroring `prefabs/preparedfoods.lua`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foodtype: Option<String>,
    /// Secondary food type (`secondaryfoodtype`), only a few recipes have one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_foodtype: Option<String>,
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
