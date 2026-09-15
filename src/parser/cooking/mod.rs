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

use crate::error::{Error, Result};
use crate::models::{CookingData, CookingRecipe, COOKING_SCHEMA_VERSION};
use std::collections::{BTreeMap, BTreeSet};

/// Raw Lua sources needed to compile the simulator payload.
#[derive(Debug, Clone, Copy)]
pub struct CookingSources<'a> {
    pub cooking: &'a str,
    pub oceanfish: &'a str,
    pub scrapbook_prefabs: &'a str,
    pub preparedfoods: &'a str,
    pub preparedfoods_warly: &'a str,
    pub preparednonfoods: &'a str,
}

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
    let base = recipes::parse_recipe_file(sources.preparedfoods, "preparedfoods.lua")?;
    let nonfoods = recipes::parse_recipe_file(sources.preparednonfoods, "preparednonfoods.lua")?;
    let warly = recipes::parse_recipe_file(sources.preparedfoods_warly, "preparedfoods_warly.lua")?;

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

    const PREPARED: &str = r#"
local foods =
{
    applesauce =
    {
        test = function(cooker, names, tags) return tags.fruit and names.apple end,
        priority = 1,
    },
    wetgoop =
    {
        test = function(cooker, names, tags) return true end,
        priority = -10,
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
    fn rejects_empty_ingredient_selection() {
        let bad = CookingSources {
            cooking: COOKING,
            oceanfish: OCEANFISH,
            scrapbook_prefabs: "local PREFABS = { [\"other\"] = true } return PREFABS",
            preparedfoods: PREPARED,
            preparedfoods_warly: PREPARED_WARLY,
            preparednonfoods: PREPARED_NONFOODS,
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
local foods = { same = { test = function() return true end } }
return foods
"#;
        let warly = r#"
local foods = { same = { test = function() return true end } }
return foods
"#;
        let collision = CookingSources {
            cooking: COOKING,
            oceanfish: OCEANFISH,
            scrapbook_prefabs: SCRAPBOOK,
            preparedfoods: base,
            preparedfoods_warly: warly,
            preparednonfoods: PREPARED_NONFOODS,
        };
        assert!(compile_from_sources(&collision).is_err());

        let weighted = CookingSources {
            preparedfoods: r#"
local foods = { weighted = { test = function() return true end, weight = 2 } }
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
        }
    }
}
