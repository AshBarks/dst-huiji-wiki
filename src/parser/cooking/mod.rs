//! Cook-pot data compiler.
//!
//! This module turns the shipped DST Lua data into a compact, serialisable
//! JSON structure for the WebUI cooking simulator. Recipe conditions are
//! compiled to small expression ASTs instead of being evaluated in Rust: the
//! frontend performs the actual candidate selection, so the same payload can
//! later be reused by a static wiki gadget.

pub mod expr;
pub mod ingredients;
pub mod recipes;
pub mod tuning;

use crate::error::{Error, Result};
use crate::models::{CookingData, CookingRecipe, COOKING_SCHEMA_VERSION};
use recipes::{RecipeFileCtx, StatsPolicy};
use std::collections::{BTreeMap, BTreeSet};
use tuning::TuningTable;

/// Raw Lua sources needed to compile the simulator payload.
#[derive(Debug, Clone, Copy)]
pub struct CookingSources<'a> {
    pub cooking: &'a str,
    pub oceanfish: &'a str,
    pub scrapbook_prefabs: &'a str,
    pub preparedfoods: &'a str,
    pub preparedfoods_warly: &'a str,
    pub preparednonfoods: &'a str,
    /// `tuning.lua`: source of the `TUNING.*` values used by the stat fields.
    pub tuning: &'a str,
}

/// Food type used by `prefabs/preparedfoods.lua` when a recipe omits
/// `foodtype` (`data.foodtype or FOODTYPE.GENERIC`). Only `wetgoop` relies on
/// it in current game data.
const DEFAULT_PREPARED_FOODTYPE: &str = "GENERIC";

/// Compile the six source files into the WebUI JSON contract.
pub fn compile_from_sources(sources: &CookingSources<'_>) -> Result<CookingData> {
    // --- ingredients ---
    let parsed = ingredients::parse_cooking_lua(sources.cooking)?;
    let mut values = parsed.values;
    for (prefab, value) in ingredients::parse_oceanfish(sources.oceanfish)? {
        values.insert(prefab, value);
    }
    let mut actual_prefabs = ingredients::parse_scrapbook_prefabs(sources.scrapbook_prefabs)?;
    actual_prefabs.extend(
        ingredients::EXTRA_REAL_PREFABS
            .iter()
            .map(|name| (*name).to_string()),
    );
    let selectable = ingredients::selectable_ingredients(&values, &parsed.aliases, &actual_prefabs);
    if selectable.is_empty() {
        return Err(Error::ParseError(
            "cooking compiler: no actual cooking-ingredient prefabs selected".to_string(),
        ));
    }

    // --- recipes ---
    // `preparedfoods*` entries are turned into prefabs by `prefabs/preparedfoods.lua`,
    // which defaults a missing food type to `GENERIC`; `preparednonfoods` entries
    // reference prefabs defined elsewhere, so they keep no default and their
    // entry stats are not the eaten values (see `StatsPolicy::Zero`).
    let tuning_ast = full_moon::parse(sources.tuning).map_err(Error::LuaParse)?;
    let tuning = TuningTable::parse(&tuning_ast)?;
    let base = recipes::parse_recipe_file(
        sources.preparedfoods,
        &RecipeFileCtx {
            source_name: "preparedfoods.lua",
            default_foodtype: Some(DEFAULT_PREPARED_FOODTYPE),
            tuning: &tuning,
            stats_policy: StatsPolicy::Parse,
        },
    )?;
    let nonfoods = recipes::parse_recipe_file(
        sources.preparednonfoods,
        &RecipeFileCtx {
            source_name: "preparednonfoods.lua",
            default_foodtype: None,
            tuning: &tuning,
            stats_policy: StatsPolicy::Zero,
        },
    )?;
    let warly = recipes::parse_recipe_file(
        sources.preparedfoods_warly,
        &RecipeFileCtx {
            source_name: "preparedfoods_warly.lua",
            default_foodtype: Some(DEFAULT_PREPARED_FOODTYPE),
            tuning: &tuning,
            stats_policy: StatsPolicy::Parse,
        },
    )?;

    let mut recipes: BTreeMap<String, CookingRecipe> = BTreeMap::new();
    let mut cookpot_names: BTreeSet<String> = BTreeSet::new();
    let mut portable_names: BTreeSet<String> = BTreeSet::new();

    for recipe in base.into_iter().chain(nonfoods) {
        cookpot_names.insert(recipe.name.clone());
        portable_names.insert(recipe.name.clone());
        recipes.insert(recipe.name.clone(), recipe);
    }
    for recipe in warly {
        if cookpot_names.contains(&recipe.name) {
            // The shared payload cannot represent a name whose base-cookpot
            // test differs from its portable-cookpot test. Fail loudly instead
            // of silently picking one implementation. No such collision exists
            // in current DST data.
            return Err(Error::ParseError(format!(
                "cooking compiler: Warly recipe `{}` collides with a base recipe; \
                 per-cooker recipe variants are not supported",
                recipe.name
            )));
        }
        portable_names.insert(recipe.name.clone());
        recipes.insert(recipe.name.clone(), recipe);
    }

    if let Some((name, _)) = recipes.iter().find(|(_, recipe)| recipe.weight != 1.0) {
        return Err(Error::ParseError(format!(
            "cooking compiler: recipe `{}` has weight != 1; weighted draws are not \
             implemented in the current frontend",
            name
        )));
    }

    let mut cookers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    cookers.insert("cookpot".to_string(), cookpot_names.into_iter().collect());
    cookers.insert(
        "portablecookpot".to_string(),
        portable_names.into_iter().collect(),
    );

    if recipes.is_empty() {
        return Err(Error::ParseError(
            "cooking compiler: no recipes parsed".to_string(),
        ));
    }

    Ok(CookingData {
        schema_version: COOKING_SCHEMA_VERSION,
        label: String::new(),
        snapshot: None,
        cookers,
        ingredients: selectable,
        recipes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const COOKING: &str = r#"
local fruits = {"apple"}
AddIngredientValues(fruits, {fruit=1}, true)
AddIngredientValues({"mushroom"}, {veggie=1}, false)
"#;

    const OCEANFISH: &str = r#"
COOKER_SMALL = { meat = .5, fish = .5 }
local FISH_DEFS =
{
    oceanfish_small_1 = {
        prefab = "oceanfish_small_1",
        cooker_ingredient_value = COOKER_SMALL,
    },
}
return { fish = FISH_DEFS }
"#;

    const SCRAPBOOK: &str = r#"
local PREFABS =
{
    ["apple"] = true,
    ["apple_cooked"] = true,
    ["mushroom"] = true,
    ["oceanfish_small_1_inv"] = true,
}
return PREFABS
"#;

    /// Shaped after the real `tuning.lua`: locals computed inside `Tune`, the
    /// table, then the no-argument `Tune()` call.
    const TUNING: &str = r#"
function Tune(overrides)
    local seg_time = 30
    local day_time = seg_time * 10
    local calories_per_day = 75
    TUNING =
    {
        HEALING_SMALL = 3,
        HEALING_MED = 20,
        CALORIES_SMALL = calories_per_day/6,
        CALORIES_LARGE = calories_per_day/2,
        SANITY_TINY = 5,
        SANITY_MED = 15,
        HOT_FOOD_BONUS_TEMP = 40,
        FOOD_TEMP_BRIEF = 5,
    }
end
Tune()
"#;

    const PREPARED: &str = r#"
local foods =
{
    applesauce =
    {
        test = function(cooker, names, tags) return tags.fruit and names.apple end,
        priority = 1,
        foodtype = FOODTYPE.VEGGIE,
        health = TUNING.HEALING_MED,
        hunger = TUNING.CALORIES_LARGE,
        sanity = TUNING.SANITY_TINY,
    },
    wetgoop =
    {
        test = function(cooker, names, tags) return true end,
        priority = -10,
        health = 0,
        hunger = 0,
        sanity = 0,
        temperature = TUNING.HOT_FOOD_BONUS_TEMP,
        temperatureduration = TUNING.FOOD_TEMP_BRIEF,
    },
}
for k, v in pairs(foods) do v.name = k; v.weight = v.weight or 1; v.priority = v.priority or 0 end
return foods
"#;

    const PREPARED_WARLY: &str = r#"
local foods =
{
    fishermans_delight =
    {
        test = function(cooker, names, tags) return tags.fish and tags.fish >= 1 end,
        priority = 30,
        foodtype = FOODTYPE.MEAT,
        secondaryfoodtype = FOODTYPE.MONSTER,
        health = -TUNING.HEALING_SMALL,
        hunger = TUNING.CALORIES_SMALL,
        sanity = TUNING.SANITY_MED,
    },
}
for k, v in pairs(foods) do v.name = k; v.weight = v.weight or 1; v.priority = v.priority or 0 end
return foods
"#;

    const PREPARED_NONFOODS: &str = r#"
local items =
{
    fruit_hat =
    {
        test = function(cooker, names, tags) return names.apple end,
        priority = 55,
    },
}
for k, v in pairs(items) do v.name = k; v.weight = v.weight or 1; v.priority = v.priority or 0 end
return items
"#;

    fn sources<'a>() -> CookingSources<'a> {
        CookingSources {
            cooking: COOKING,
            oceanfish: OCEANFISH,
            scrapbook_prefabs: SCRAPBOOK,
            preparedfoods: PREPARED,
            preparedfoods_warly: PREPARED_WARLY,
            preparednonfoods: PREPARED_NONFOODS,
            tuning: TUNING,
        }
    }

    #[test]
    fn compiles_ingredients_and_cooker_membership() {
        let data = compile_from_sources(&sources()).unwrap();
        assert_eq!(data.schema_version, COOKING_SCHEMA_VERSION);
        let prefabs: Vec<&str> = data.ingredients.iter().map(|i| i.prefab.as_str()).collect();
        assert_eq!(
            prefabs,
            vec!["apple", "apple_cooked", "mushroom", "oceanfish_small_1_inv"]
        );
        assert!(data.recipes["applesauce"]
            .test
            .to_string()
            .contains("field"));
        assert!(data.cookers["cookpot"].contains(&"wetgoop".to_string()));
        assert!(data.cookers["cookpot"].contains(&"fruit_hat".to_string()));
        assert!(!data.cookers["cookpot"].contains(&"fishermans_delight".to_string()));
        assert!(data.cookers["portablecookpot"].contains(&"fishermans_delight".to_string()));
    }

    #[test]
    fn compiles_food_types_with_prepared_default() {
        let data = compile_from_sources(&sources()).unwrap();
        // Explicit `foodtype` wins.
        assert_eq!(
            data.recipes["applesauce"].foodtype.as_deref(),
            Some("VEGGIE")
        );
        // Omitted in `preparedfoods.lua`: the prefab default applies.
        assert_eq!(data.recipes["wetgoop"].foodtype.as_deref(), Some("GENERIC"));
        assert_eq!(data.recipes["wetgoop"].secondary_foodtype, None);
        // Secondary types survive the compile.
        assert_eq!(
            data.recipes["fishermans_delight"].foodtype.as_deref(),
            Some("MEAT")
        );
        assert_eq!(
            data.recipes["fishermans_delight"]
                .secondary_foodtype
                .as_deref(),
            Some("MONSTER")
        );
        // `preparednonfoods.lua` entries get no default at all.
        assert_eq!(data.recipes["fruit_hat"].foodtype, None);
    }

    #[test]
    fn compiles_stats_and_temperature() {
        let data = compile_from_sources(&sources()).unwrap();
        assert_eq!(
            data.recipes["applesauce"].stats,
            crate::models::CookingStats {
                health: 20.0,
                hunger: 37.5,
                sanity: 5.0,
            }
        );
        assert_eq!(data.recipes["applesauce"].temperature, None);
        assert_eq!(
            data.recipes["wetgoop"].temperature,
            Some(crate::models::CookingTemperature {
                delta: 40.0,
                duration: 5.0,
                no_chill: false,
            })
        );
        assert_eq!(
            data.recipes["fishermans_delight"].stats,
            crate::models::CookingStats {
                health: -3.0,
                hunger: 12.5,
                sanity: 15.0,
            }
        );
        // `preparednonfoods.lua` entries declare no stats here; even if they
        // did (see `recipes::tests`), the zero policy would drop them.
        assert_eq!(
            data.recipes["fruit_hat"].stats,
            crate::models::CookingStats::default()
        );
    }

    #[test]
    fn rejects_empty_ingredient_selection() {
        let bad = CookingSources {
            cooking: COOKING,
            oceanfish: OCEANFISH,
            scrapbook_prefabs: "local PREFABS = { [\"other\"] = true } return PREFABS",
            preparedfoods: PREPARED,
            preparedfoods_warly: PREPARED_WARLY,
            preparednonfoods: PREPARED_NONFOODS,
            tuning: TUNING,
        };
        assert!(compile_from_sources(&bad).is_err());
    }
    /// Integration smoke test against the developer's local DST install.
    /// Skips when `.env` is absent (CI) so the parser fixture tests stay
    /// hermetic. When present it protects the expected current data size
    /// (81 recipes, 146 selectable prefabs) and catches source-shape drift.
    #[test]
    fn compiles_current_dst_data_when_configured() {
        let Some(dst_root) = dst_root_from_env_or_dotenv() else {
            return;
        };
        let source = match crate::platform::game_source::GameSource::new(dst_root, None) {
            Ok(source) => source,
            Err(_) => return,
        };
        let read = |rel: &str| source.read(rel).expect(rel);
        let data = compile_from_sources(&CookingSources {
            cooking: &read("cooking.lua"),
            oceanfish: &read("prefabs/oceanfishdef.lua"),
            scrapbook_prefabs: &read("scrapbook_prefabs.lua"),
            preparedfoods: &read("preparedfoods.lua"),
            preparedfoods_warly: &read("preparedfoods_warly.lua"),
            preparednonfoods: &read("preparednonfoods.lua"),
            tuning: &read("tuning.lua"),
        })
        .unwrap();

        assert_eq!(data.recipes.len(), 81, "recipe count changed");
        assert_eq!(
            data.ingredients.len(),
            146,
            "actual cooking-ingredient whitelist changed"
        );
        assert!(data.ingredients.iter().any(|i| i.prefab == "berries"));
        assert!(data
            .ingredients
            .iter()
            .any(|i| i.prefab == "cookedsmallmeat"));
        assert!(!data.ingredients.iter().any(|i| i.prefab == "honey_cooked"));
        assert!(data.cookers["cookpot"].contains(&"wetgoop".to_string()));
        assert!(data.cookers["portablecookpot"].contains(&"wetgoop".to_string()));
        assert!(data.cookers["portablecookpot"].contains(&"nightmarepie".to_string()));

        // Food types: every dish except the non-food `batnosehat` has one.
        assert_eq!(data.recipes["baconeggs"].foodtype.as_deref(), Some("MEAT"));
        assert_eq!(data.recipes["taffy"].foodtype.as_deref(), Some("GOODIES"));
        // Omitted in game data -> `GENERIC` default.
        assert_eq!(data.recipes["wetgoop"].foodtype.as_deref(), Some("GENERIC"));
        // `batnosehat` is a hat (`noprefab`), not an edible food.
        assert_eq!(data.recipes["batnosehat"].foodtype, None);
        // Current game data has exactly three dual-type dishes.
        let mut dual: Vec<(&str, &str, &str)> = data
            .recipes
            .values()
            .filter_map(|r| {
                Some((
                    r.name.as_str(),
                    r.foodtype.as_deref()?,
                    r.secondary_foodtype.as_deref()?,
                ))
            })
            .collect();
        dual.sort_unstable();
        assert_eq!(
            dual,
            vec![
                ("beefalofeed", "ROUGHAGE", "WOOD"),
                ("monsterlasagna", "MEAT", "MONSTER"),
                ("monstertartare", "MEAT", "MONSTER"),
            ]
        );
        let typed = data
            .recipes
            .values()
            .filter(|r| r.foodtype.is_some())
            .count();
        assert_eq!(typed, 80, "recipes with a food type changed");

        // Entry-instant stats: every recipe carries the three numbers, resolved
        // from `tuning.lua` (see `docs/COOKING_SIMULATOR.md`).
        assert_eq!(
            data.recipes["wetgoop"].stats,
            crate::models::CookingStats {
                health: 0.0,
                hunger: 0.0,
                sanity: 0.0,
            }
        );
        assert_eq!(
            data.recipes["meatballs"].stats,
            crate::models::CookingStats {
                health: 3.0,
                hunger: 62.5,
                sanity: 5.0,
            }
        );
        assert_eq!(
            data.recipes["butterflymuffin"].stats,
            crate::models::CookingStats {
                health: 20.0,
                hunger: 37.5,
                sanity: 5.0,
            }
        );
        // Negative health and a scaled hunger value.
        assert_eq!(
            data.recipes["taffy"].stats,
            crate::models::CookingStats {
                health: -3.0,
                hunger: 25.0,
                sanity: 15.0,
            }
        );
        // `preparednonfoods.lua` products are exported as zero-effect even
        // though their entries declare numbers (cookbook-only / defined in the
        // product's own prefab).
        for name in ["batnosehat", "dustmeringue"] {
            assert_eq!(
                data.recipes[name].stats,
                crate::models::CookingStats::default(),
                "{} should be zeroed",
                name
            );
            assert_eq!(data.recipes[name].temperature, None, "{}", name);
        }
        // Hot/cold food: delta plus duration, both from `tuning.lua`.
        assert_eq!(
            data.recipes["watermelonicle"].temperature,
            Some(crate::models::CookingTemperature {
                delta: -40.0,
                duration: 10.0,
                no_chill: false,
            })
        );
        assert_eq!(
            data.recipes["dragonchilisalad"].temperature,
            Some(crate::models::CookingTemperature {
                delta: 40.0,
                duration: 300.0,
                no_chill: true,
            })
        );
        let warm_or_cold = data
            .recipes
            .values()
            .filter(|r| r.temperature.is_some())
            .count();
        assert_eq!(warm_or_cold, 19, "hot/cold dish count changed");
    }

    fn dst_root_from_env_or_dotenv() -> Option<String> {
        if let Ok(value) = std::env::var("DST__ROOT") {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
        let content = std::fs::read_to_string(".env").ok()?;
        content.lines().find_map(|line| {
            line.trim()
                .strip_prefix("DST__ROOT=")
                .map(|v| v.trim().trim_matches('"').to_string())
                .filter(|v| !v.is_empty())
        })
    }
    #[test]
    fn rejects_warly_name_collision_and_non_unit_weight() {
        let base = r#"
local foods = { same = { test = function() return true end, health = 0, hunger = 0, sanity = 0 } }
return foods
"#;
        let warly = r#"
local foods = { same = { test = function() return true end, health = 0, hunger = 0, sanity = 0 } }
return foods
"#;
        let collision = CookingSources {
            cooking: COOKING,
            oceanfish: OCEANFISH,
            scrapbook_prefabs: SCRAPBOOK,
            preparedfoods: base,
            preparedfoods_warly: warly,
            preparednonfoods: PREPARED_NONFOODS,
            tuning: TUNING,
        };
        assert!(compile_from_sources(&collision).is_err());

        let weighted = CookingSources {
            preparedfoods: r#"
local foods = { weighted = { test = function() return true end, weight = 2, health = 0, hunger = 0, sanity = 0 } }
return foods
"#,
            preparedfoods_warly: PREPARED_WARLY,
            ..collision_inputs()
        };
        assert!(compile_from_sources(&weighted).is_err());
    }

    fn collision_inputs<'a>() -> CookingSources<'a> {
        CookingSources {
            cooking: COOKING,
            oceanfish: OCEANFISH,
            scrapbook_prefabs: SCRAPBOOK,
            preparedfoods: PREPARED,
            preparedfoods_warly: PREPARED_WARLY,
            preparednonfoods: PREPARED_NONFOODS,
            tuning: TUNING,
        }
    }
}
