//! Parser for `preparedfoods.lua`, `preparedfoods_warly.lua` and
//! `preparednonfoods.lua`.
//!
//! These files all follow the same shape: a top-level local table of recipe
//! entries, a trailing loop that fills defaults (`name = key`, `weight = 1`,
//! `priority = 0`) and a final `return`. We read the table keys/fields
//! directly, applying the same defaults without evaluating Lua.

use super::expr::translate_test_function;
use crate::error::{Error, Result};
use crate::models::CookingRecipe;
use crate::parser::string_literal_text;
use full_moon::ast::{self, Ast};

/// Parse one recipe source file into cooked recipes. `source_name` is stored
/// for audit and diagnostics.
pub fn parse_recipe_file(source: &str, source_name: &str) -> Result<Vec<CookingRecipe>> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let table = find_top_level_table(&ast, &["foods", "items"]).ok_or_else(|| {
        Error::ParseError(format!(
            "cooking recipe file `{}`: no top-level foods/items table found",
            source_name
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
                source_name, name
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

        recipes.push(CookingRecipe {
            name,
            priority,
            weight,
            source: source_name.to_string(),
            test: test_expr.to_json(),
            name_en: None,
            name_zh: None,
            icon: None,
        });
    }

    if recipes.is_empty() {
        return Err(Error::ParseError(format!(
            "cooking recipe file `{}` produced no recipes",
            source_name
        )));
    }
    Ok(recipes)
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

    #[test]
    fn parses_recipe_defaults_and_explicit_priority() {
        let source = r#"
local foods =
{
    wetgoop =
    {
        test = function(cooker, names, tags) return true end,
        priority = -10,
    },
    butterflymuffin =
    {
        test = function(cooker, names, tags) return names.butterflywings end,
        priority = 1,
        weight = 1,
    },
}

for k, v in pairs(foods) do
    v.name = k
    v.weight = v.weight or 1
    v.priority = v.priority or 0
end

return foods
"#;
        let recipes = parse_recipe_file(source, "fixture.lua").unwrap();
        assert_eq!(recipes.len(), 2);
        let wetgoop = recipes.iter().find(|r| r.name == "wetgoop").unwrap();
        assert_eq!(wetgoop.priority, -10.0);
        assert_eq!(wetgoop.weight, 1.0);
        let muffin = recipes
            .iter()
            .find(|r| r.name == "butterflymuffin")
            .unwrap();
        assert_eq!(muffin.priority, 1.0);
        assert_eq!(muffin.test[0], "field");
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

for k, v in pairs(items) do v.name = k; v.weight = v.weight or 1; v.priority = v.priority or 0 end
return items
"#;
        let recipes = parse_recipe_file(source, "preparednonfoods.lua").unwrap();
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
        assert!(parse_recipe_file(no_test, "fixture.lua").is_err());

        let dynamic_priority = r#"
local foods = { x = { test = function() return true end, priority = TUNING.X } }
return foods
"#;
        assert!(parse_recipe_file(dynamic_priority, "fixture.lua").is_err());
    }
}
