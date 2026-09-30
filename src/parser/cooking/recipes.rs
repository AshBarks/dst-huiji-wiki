//! Parser for `preparedfoods.lua`, `preparedfoods_warly.lua` and
//! `preparednonfoods.lua`.
//!
//! These files all follow the same shape: a top-level local table of recipe
//! entries, a trailing loop that fills defaults (`name = key`, `weight = 1`,
//! `priority = 0`) and a final `return`. We read the table keys/fields
//! directly, applying the same defaults without evaluating Lua.
//!
//! Besides the `test` condition every entry carries its entry-instant effect
//! numbers (`health` / `hunger` / `sanity`, optionally `temperature` /
//! `temperatureduration`), resolved through [`super::tuning`].

use super::expr::{short_expr, translate_expr, translate_test_function, Expr};
use super::tuning::{eval_const, TuningTable};
use crate::error::{Error, Result};
use crate::models::{CookingRecipe, CookingStats, CookingTemperature};
use crate::parser::string_literal_text;
use full_moon::ast::{self, Ast};

/// How the entry stat fields of one recipe file are treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsPolicy {
    /// Read `health` / `hunger` / `sanity` (and the temperature pair) from the
    /// entry; all three stats are required.
    Parse,
    /// `preparednonfoods.lua` products reference prefabs defined elsewhere, so
    /// their entry stats are cookbook-only (`batnosehat` is a hat) or differ
    /// from the real prefab (`dustmeringue`): export zeros, no temperature.
    Zero,
}

/// Everything one recipe file needs besides its own source text.
pub struct RecipeFileCtx<'a, 'src> {
    /// File name stored on every recipe for audit and diagnostics.
    pub source_name: &'a str,
    /// Food type applied to entries without an explicit `foodtype`
    /// (`preparedfoods.lua` / `preparedfoods_warly.lua` become real prefabs
    /// whose food type defaults to `GENERIC`, see `prefabs/preparedfoods.lua`;
    /// `preparednonfoods.lua` entries are not turned into prefabs, so
    /// `batnosehat` legitimately has no food type at all).
    pub default_foodtype: Option<&'a str>,
    /// Default `Tune()` values from `tuning.lua`.
    pub tuning: &'a TuningTable<'src>,
    pub stats_policy: StatsPolicy,
}

/// Parse one recipe source file into cooked recipes.
pub fn parse_recipe_file(source: &str, ctx: &RecipeFileCtx<'_, '_>) -> Result<Vec<CookingRecipe>> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let table = find_top_level_table(&ast, &["foods", "items"]).ok_or_else(|| {
        Error::ParseError(format!(
            "cooking recipe file `{}`: no top-level foods/items table found",
            ctx.source_name
        ))
    })?;

    let mut recipes = Vec::new();
    for field in table.fields() {
        let Some((name, entry)) = recipe_entry(field) else {
            continue;
        };
        let Some(test_value) = nested_value(entry, "test") else {
            return Err(Error::ParseError(format!(
                "cooking recipe file `{}`: recipe `{}` has no test function",
                ctx.source_name, name
            )));
        };
        let test_expr = translate_test_function(test_value, &name)?;

        let priority = nested_value(entry, "priority")
            .map(|v| eval_const_f64(v, &name, "priority"))
            .transpose()?
            .unwrap_or(0.0);
        let weight = nested_value(entry, "weight")
            .map(|v| eval_const_f64(v, &name, "weight"))
            .transpose()?
            .unwrap_or(1.0);

        let foodtype = foodtype_field(entry, "foodtype", &name)?
            .or_else(|| ctx.default_foodtype.map(str::to_string));
        let secondary_foodtype = foodtype_field(entry, "secondaryfoodtype", &name)?;

        let (stats, temperature) = stat_fields(entry, &name, ctx)?;

        recipes.push(CookingRecipe {
            name,
            priority,
            weight,
            stats,
            temperature,
            foodtype,
            secondary_foodtype,
            source: ctx.source_name.to_string(),
            test: test_expr.to_json(),
            name_en: None,
            name_zh: None,
            icon: None,
        });
    }

    if recipes.is_empty() {
        return Err(Error::ParseError(format!(
            "cooking recipe file `{}` produced no recipes",
            ctx.source_name
        )));
    }
    Ok(recipes)
}

/// Entry-instant stats plus the optional temperature pair.
fn stat_fields(
    entry: &ast::TableConstructor,
    recipe: &str,
    ctx: &RecipeFileCtx<'_, '_>,
) -> Result<(CookingStats, Option<CookingTemperature>)> {
    if ctx.stats_policy == StatsPolicy::Zero {
        return Ok((CookingStats::default(), None));
    }
    let stats = CookingStats {
        health: required_number(entry, "health", recipe, ctx)?,
        hunger: required_number(entry, "hunger", recipe, ctx)?,
        sanity: required_number(entry, "sanity", recipe, ctx)?,
    };
    Ok((stats, temperature_field(entry, recipe, ctx)?))
}

/// Evaluate a required numeric field, e.g. `health = TUNING.HEALING_MED`.
fn required_number(
    entry: &ast::TableConstructor,
    field: &str,
    recipe: &str,
    ctx: &RecipeFileCtx<'_, '_>,
) -> Result<f64> {
    let Some(expr) = nested_value(entry, field) else {
        return Err(Error::ParseError(format!(
            "cooking recipe `{}`: missing `{}`",
            recipe, field
        )));
    };
    eval_const(expr, ctx.tuning)
        .map_err(|e| Error::ParseError(format!("cooking recipe `{}`: `{}`: {}", recipe, field, e)))
}

/// `temperature` / `temperatureduration` / `nochill`. A dish is either hot or
/// cold food; a delta without a duration (or the reverse) is a shape change and
/// therefore an error.
fn temperature_field(
    entry: &ast::TableConstructor,
    recipe: &str,
    ctx: &RecipeFileCtx<'_, '_>,
) -> Result<Option<CookingTemperature>> {
    let Some(expr) = nested_value(entry, "temperature") else {
        return Ok(None);
    };
    let delta = eval_const(expr, ctx.tuning).map_err(|e| {
        Error::ParseError(format!("cooking recipe `{}`: `temperature`: {}", recipe, e))
    })?;
    let duration = required_number(entry, "temperatureduration", recipe, ctx)?;
    let no_chill = match nested_value(entry, "nochill") {
        None => false,
        Some(expr) => match translate_expr(expr, recipe)? {
            Expr::Bool(value) => value,
            _ => {
                return Err(Error::ParseError(format!(
                    "cooking recipe `{}`: unsupported `nochill` `{}`",
                    recipe,
                    short_expr(expr)
                )))
            }
        },
    };
    Ok(Some(CookingTemperature {
        delta,
        duration,
        no_chill,
    }))
}

/// Extract `(key, table)` for a named recipe entry. Supports both
/// `key = { ... }` and `["key"] = { ... }` table fields.
fn recipe_entry(field: &ast::Field) -> Option<(String, &ast::TableConstructor)> {
    match field {
        ast::Field::NameKey {
            key,
            value: ast::Expression::TableConstructor(table),
            ..
        } => Some((key.token().to_string(), table)),
        ast::Field::ExpressionKey {
            key: ast::Expression::String(key),
            value: ast::Expression::TableConstructor(table),
            ..
        } => Some((string_literal_text(&key.token().to_string()), table)),
        _ => None,
    }
}

fn nested_value<'a>(table: &'a ast::TableConstructor, name: &str) -> Option<&'a ast::Expression> {
    table.fields().iter().find_map(|field| match field {
        ast::Field::NameKey { key, value, .. } if key.token().to_string() == name => Some(value),
        ast::Field::ExpressionKey {
            key: ast::Expression::String(key),
            value,
            ..
        } if string_literal_text(&key.token().to_string()) == name => Some(value),
        _ => None,
    })
}

/// Read a `foodtype` / `secondaryfoodtype` field, which must be a `FOODTYPE.X`
/// constant. Non-constant values (locals, table lookups, function calls) are
/// rejected: guessing one would silently mislabel a dish in the exported
/// bundle, so a game update should surface as a parse error instead.
fn foodtype_field(
    entry: &ast::TableConstructor,
    field: &str,
    recipe: &str,
) -> Result<Option<String>> {
    let Some(expr) = nested_value(entry, field) else {
        return Ok(None);
    };
    let error = || {
        Error::ParseError(format!(
            "cooking recipe `{}`: unsupported {} `{}` (expected FOODTYPE.<NAME>)",
            recipe,
            field,
            short_expr(expr)
        ))
    };

    let ast::Expression::Var(ast::Var::Expression(var_expr)) = expr else {
        return Err(error());
    };
    let ast::Prefix::Name(prefix) = var_expr.prefix() else {
        return Err(error());
    };
    if prefix.token().to_string() != "FOODTYPE" {
        return Err(error());
    }

    let mut name: Option<String> = None;
    for suffix in var_expr.suffixes() {
        match suffix {
            ast::Suffix::Index(ast::Index::Dot { name: key, .. }) if name.is_none() => {
                name = Some(key.token().to_string());
            }
            _ => return Err(error()),
        }
    }
    let Some(name) = name else {
        return Err(error());
    };
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        return Err(error());
    }
    Ok(Some(name))
}

/// Find a top-level `local name = { ... }` table. The recipe files use one
/// table per file; we deliberately only search the top-level statements.
fn find_top_level_table<'a>(ast: &'a Ast, names: &[&str]) -> Option<&'a ast::TableConstructor> {
    for stmt in ast.nodes().stmts() {
        let ast::Stmt::LocalAssignment(assignment) = stmt else {
            continue;
        };
        for (name, expr) in assignment
            .names()
            .iter()
            .zip(assignment.expressions().iter())
        {
            if !names.iter().any(|n| name.token().to_string() == *n) {
                continue;
            }
            if let ast::Expression::TableConstructor(table) = expr {
                return Some(table);
            }
        }
    }
    None
}

/// Evaluate a numeric literal/expression field used by recipe metadata.
/// Current files only contain literals and unary minus; unsupported expressions
/// fail loudly because silently defaulting could change the priority (and thus
/// the selected product).
fn eval_const_f64(expr: &ast::Expression, recipe: &str, field: &str) -> Result<f64> {
    match expr {
        ast::Expression::Number(token) => {
            token
                .token()
                .to_string()
                .trim()
                .parse::<f64>()
                .map_err(|_| {
                    Error::ParseError(format!(
                        "cooking recipe `{}`: invalid numeric {} `{}`",
                        recipe,
                        field,
                        token.token()
                    ))
                })
        }
        ast::Expression::Parentheses { expression, .. } => {
            eval_const_f64(expression, recipe, field)
        }
        ast::Expression::UnaryOperator { unop, expression } => {
            let value = eval_const_f64(expression, recipe, field)?;
            match unop {
                ast::UnOp::Minus(_) => Ok(-value),
                _ => Err(Error::ParseError(format!(
                    "cooking recipe `{}`: unsupported unary operator in {}",
                    recipe, field
                ))),
            }
        }
        _ => Err(Error::ParseError(format!(
            "cooking recipe `{}`: non-constant {} `{}` is not supported",
            recipe,
            field,
            expr.to_string().trim()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped after the real `tuning.lua`: locals computed inside `Tune`, the
    /// table, then the no-argument `Tune()` call.
    const TUNING_LUA: &str = r#"
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
        COLD_FOOD_BONUS_TEMP = -40,
        FOOD_TEMP_BRIEF = 5,
        FOOD_TEMP_AVERAGE = 10,
        BUFF_FOOD_TEMP_DURATION = day_time,
    }
end
Tune()
"#;

    fn parse(
        source: &str,
        default_foodtype: Option<&str>,
        stats_policy: StatsPolicy,
    ) -> Result<Vec<CookingRecipe>> {
        let tuning_ast = full_moon::parse(TUNING_LUA).unwrap();
        let tuning = TuningTable::parse(&tuning_ast).unwrap();
        parse_recipe_file(
            source,
            &RecipeFileCtx {
                source_name: "fixture.lua",
                default_foodtype,
                tuning: &tuning,
                stats_policy,
            },
        )
    }

    #[test]
    fn parses_recipe_defaults_and_explicit_priority() {
        let source = r#"
local foods =
{
    wetgoop =
    {
        test = function(cooker, names, tags) return true end,
        priority = -10,
        health = 0,
        hunger = 0,
        sanity = 0,
    },
    butterflymuffin =
    {
        test = function(cooker, names, tags) return names.butterflywings end,
        priority = 1,
        weight = 1,
        health = TUNING.HEALING_MED,
        hunger = TUNING.CALORIES_LARGE,
        sanity = TUNING.SANITY_TINY,
    },
}

for k, v in pairs(foods) do
    v.name = k
    v.weight = v.weight or 1
    v.priority = v.priority or 0
end

return foods
"#;
        let recipes = parse(source, Some("GENERIC"), StatsPolicy::Parse).unwrap();
        assert_eq!(recipes.len(), 2);
        let wetgoop = recipes.iter().find(|r| r.name == "wetgoop").unwrap();
        assert_eq!(wetgoop.priority, -10.0);
        assert_eq!(wetgoop.weight, 1.0);
        // No explicit `foodtype`: the prepared-food default applies.
        assert_eq!(wetgoop.foodtype.as_deref(), Some("GENERIC"));
        assert_eq!(wetgoop.secondary_foodtype, None);
        assert_eq!(wetgoop.stats, CookingStats::default());
        assert_eq!(wetgoop.temperature, None);
        let muffin = recipes
            .iter()
            .find(|r| r.name == "butterflymuffin")
            .unwrap();
        assert_eq!(muffin.priority, 1.0);
        assert_eq!(muffin.test[0], "field");
        assert_eq!(
            muffin.stats,
            CookingStats {
                health: 20.0,
                hunger: 37.5,
                sanity: 5.0,
            }
        );
    }

    #[test]
    fn parses_negative_and_scaled_stats_with_temperature() {
        let source = r#"
local foods =
{
    taffy =
    {
        test = function(cooker, names, tags) return tags.sweetener and tags.sweetener >= 3 end,
        health = -TUNING.HEALING_SMALL,
        hunger = TUNING.CALORIES_SMALL*2,
        sanity = -TUNING.SANITY_MED,
        temperature = TUNING.HOT_FOOD_BONUS_TEMP,
        temperatureduration = TUNING.FOOD_TEMP_BRIEF,
        nochill = true,
    },
    icecream =
    {
        test = function(cooker, names, tags) return tags.dairy and tags.frozen end,
        health = 0,
        hunger = TUNING.CALORIES_SMALL/2,
        sanity = (TUNING.SANITY_MED),
        temperature = TUNING.COLD_FOOD_BONUS_TEMP,
        temperatureduration = TUNING.FOOD_TEMP_AVERAGE,
    },
}
return foods
"#;
        let recipes = parse(source, Some("GENERIC"), StatsPolicy::Parse).unwrap();
        let find = |name: &str| recipes.iter().find(|r| r.name == name).unwrap();
        assert_eq!(
            find("taffy").stats,
            CookingStats {
                health: -3.0,
                hunger: 25.0,
                sanity: -15.0,
            }
        );
        assert_eq!(
            find("taffy").temperature,
            Some(CookingTemperature {
                delta: 40.0,
                duration: 5.0,
                no_chill: true,
            })
        );
        assert_eq!(
            find("icecream").temperature,
            Some(CookingTemperature {
                delta: -40.0,
                duration: 10.0,
                no_chill: false,
            })
        );
    }

    #[test]
    fn zero_policy_skips_declared_stats() {
        let source = r#"
local items =
{
    batnosehat =
    {
        test = function(cooker, names, tags) return names.batnose and names.kelp end,
        priority = 55,
        health = 0,
        hunger = TUNING.CALORIES_SMALL,
        sanity = 0,
        temperature = TUNING.HOT_FOOD_BONUS_TEMP,
        temperatureduration = TUNING.FOOD_TEMP_BRIEF,
    },
}
return items
"#;
        let recipes = parse(source, None, StatsPolicy::Zero).unwrap();
        assert_eq!(recipes[0].stats, CookingStats::default());
        assert_eq!(recipes[0].temperature, None);
    }

    #[test]
    fn rejects_missing_stats_and_unsupported_stat_shapes() {
        let missing_sanity = r#"
local foods = { x = { test = function() return true end, health = 0, hunger = 0 } }
return foods
"#;
        let err = parse(missing_sanity, None, StatsPolicy::Parse).unwrap_err();
        assert!(err.to_string().contains("missing `sanity`"), "{}", err);

        let local_ref = r#"
local foods = { x = { test = function() return true end, health = some_local, hunger = 0, sanity = 0 } }
return foods
"#;
        assert!(parse(local_ref, None, StatsPolicy::Parse).is_err());

        let delta_without_duration = r#"
local foods = { x = { test = function() return true end, health = 0, hunger = 0, sanity = 0,
    temperature = TUNING.HOT_FOOD_BONUS_TEMP } }
return foods
"#;
        let err = parse(delta_without_duration, None, StatsPolicy::Parse).unwrap_err();
        assert!(err.to_string().contains("temperatureduration"), "{}", err);
    }

    #[test]
    fn parses_explicit_food_types() {
        let source = r#"
local foods =
{
    baconeggs =
    {
        test = function(cooker, names, tags) return tags.egg and tags.meat end,
        priority = 10,
        foodtype = FOODTYPE.MEAT,
    },
    -- 主类型肉，副类型怪物
    monsterlasagna =
    {
        test = function(cooker, names, tags) return tags.monster end,
        priority = 10,
        foodtype = FOODTYPE.MEAT, -- 肉
        secondaryfoodtype = FOODTYPE.MONSTER,
    },
    batnosehat =
    {
        test = function(cooker, names, tags) return names.batnose end,
        priority = 55,
    },
}
return foods
"#;
        let recipes = parse(source, None, StatsPolicy::Zero).unwrap();
        let find = |name: &str| recipes.iter().find(|r| r.name == name).unwrap();
        assert_eq!(find("baconeggs").foodtype.as_deref(), Some("MEAT"));
        assert_eq!(find("baconeggs").secondary_foodtype, None);
        assert_eq!(find("monsterlasagna").foodtype.as_deref(), Some("MEAT"));
        assert_eq!(
            find("monsterlasagna").secondary_foodtype.as_deref(),
            Some("MONSTER")
        );
        // Non-food products keep no type when the file has no default.
        assert_eq!(find("batnosehat").foodtype, None);
    }

    #[test]
    fn parses_nonfood_items_table_and_bracket_keys() {
        let source = r#"
local items =
{
    ["batnosehat"] =
    {
        test = function(cooker, names, tags) return names.batnose and names.kelp end,
        priority = 55,
    },
}
"#;
        let recipes = parse(source, None, StatsPolicy::Zero).unwrap();
        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].name, "batnosehat");
        assert_eq!(recipes[0].priority, 55.0);
    }

    #[test]
    fn rejects_missing_test_or_unsupported_priority() {
        let no_test = r#"
local foods = { x = { priority = 1 } }
return foods
"#;
        assert!(parse(no_test, None, StatsPolicy::Parse).is_err());

        let dynamic_priority = r#"
local foods = { x = { test = function() return true end, priority = TUNING.X } }
return foods
"#;
        assert!(parse(dynamic_priority, None, StatsPolicy::Parse).is_err());
    }

    #[test]
    fn rejects_non_constant_food_type() {
        let dynamic = r#"
local foods = { x = { test = function() return true end, foodtype = get_foodtype() } }
return foods
"#;
        let err = parse(dynamic, None, StatsPolicy::Parse).unwrap_err();
        assert!(err.to_string().contains("foodtype"), "{}", err);

        let indexed = r#"
local foods = { x = { test = function() return true end, foodtype = FOODTYPE[name] } }
return foods
"#;
        assert!(parse(indexed, None, StatsPolicy::Parse).is_err());
    }
}
