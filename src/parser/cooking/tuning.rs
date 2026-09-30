//! `TUNING.*` constant resolution for the per-dish stat fields.
//!
//! `preparedfoods*.lua` writes `health` / `hunger` / `sanity` (and the optional
//! `temperature` / `temperatureduration` pair) as arithmetic over
//! `TUNING.<NAME>`. Those values only exist once `tuning.lua`'s `Tune()` has
//! run; the file calls `Tune()` without arguments at the very end, so its body
//! is the shipped default value set. We rebuild those defaults from the same
//! source instead of hardcoding numbers, and only allow the constants the stat
//! fields actually use: a constant outside [`ALLOWED_CONSTANTS`] must surface
//! as a parse error, never as a silently wrong dish value.

use super::expr::short_expr;
use crate::error::{Error, Result};
use crate::parser::string_literal_text;
use full_moon::ast::{self, Ast};
use std::collections::BTreeMap;

/// `TUNING.*` names the recipe stat fields may reference. Everything here is
/// resolved from `tuning.lua`; extending the list is a deliberate act after
/// checking where the new value comes from.
pub const ALLOWED_CONSTANTS: &[&str] = &[
    // `health`
    "HEALING_TINY",
    "HEALING_SMALL",
    "HEALING_MEDSMALL",
    "HEALING_MED",
    "HEALING_MEDLARGE",
    "HEALING_LARGE",
    "HEALING_HUGE",
    "HEALING_MOREHUGE",
    "HEALING_SUPERHUGE",
    // `hunger`
    "CALORIES_SMALL",
    "CALORIES_MEDSMALL",
    "CALORIES_MED",
    "CALORIES_LARGE",
    "CALORIES_HUGE",
    "CALORIES_MOREHUGE",
    "CALORIES_SUPERHUGE",
    // `sanity`
    "SANITY_TINY",
    "SANITY_SMALL",
    "SANITY_MED",
    "SANITY_MEDLARGE",
    "SANITY_LARGE",
    "SANITY_HUGE",
    // One-off stat values (`jellybean`, `batnosehat`).
    "JELLYBEAN_TICK_VALUE",
    "HUNGERREGEN_TICK_VALUE",
    // `temperature` / `temperatureduration`
    "HOT_FOOD_BONUS_TEMP",
    "COLD_FOOD_BONUS_TEMP",
    "FOOD_TEMP_BRIEF",
    "FOOD_TEMP_AVERAGE",
    "FOOD_TEMP_LONG",
    "BUFF_FOOD_TEMP_DURATION",
];

/// Guards against a self-referential `local` / `TUNING` chain.
const MAX_REFERENCE_DEPTH: usize = 8;

/// Default `Tune()` values, kept as unevaluated expressions and resolved on
/// demand (the table has thousands of entries; only the stat constants matter).
pub struct TuningTable<'a> {
    locals: BTreeMap<String, &'a ast::Expression>,
    constants: BTreeMap<String, &'a ast::Expression>,
}

impl<'a> TuningTable<'a> {
    /// Read the `Tune()` body: its simple `local x = <expr>` bindings and the
    /// `TUNING = { ... }` table. Values are not evaluated here.
    pub fn parse(ast: &'a Ast) -> Result<Self> {
        let tune = find_tune(ast).ok_or_else(|| {
            Error::ParseError("tuning.lua: no `Tune` function declaration found".to_string())
        })?;

        let mut locals = BTreeMap::new();
        let mut constants = BTreeMap::new();
        let mut found_table = false;
        for stmt in tune.stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assignment) => {
                    for (name, expr) in assignment
                        .names()
                        .iter()
                        .zip(assignment.expressions().iter())
                    {
                        locals.insert(name.token().to_string(), expr);
                    }
                }
                ast::Stmt::Assignment(assignment) => {
                    let mut variables = assignment.variables().iter();
                    let is_tuning = matches!(
                        variables.next(),
                        Some(ast::Var::Name(name)) if name.token().to_string() == "TUNING"
                    ) && variables.next().is_none();
                    if !is_tuning {
                        continue;
                    }
                    let Some(ast::Expression::TableConstructor(table)) =
                        assignment.expressions().iter().next()
                    else {
                        return Err(Error::ParseError(
                            "tuning.lua: `TUNING` is not assigned a table".to_string(),
                        ));
                    };
                    for field in table.fields() {
                        match field {
                            ast::Field::NameKey { key, value, .. } => {
                                constants.insert(key.token().to_string(), value);
                            }
                            ast::Field::ExpressionKey {
                                key: ast::Expression::String(key),
                                value,
                                ..
                            } => {
                                constants
                                    .insert(string_literal_text(&key.token().to_string()), value);
                            }
                            other => {
                                return Err(Error::ParseError(format!(
                                    "tuning.lua: unsupported TUNING field `{}`",
                                    clamp_text(&other.to_string())
                                )))
                            }
                        }
                    }
                    found_table = true;
                }
                _ => {}
            }
        }

        if !found_table {
            return Err(Error::ParseError(
                "tuning.lua: `TUNING = { ... }` not found inside `Tune`".to_string(),
            ));
        }
        Ok(Self { locals, constants })
    }

    /// Resolve a whitelisted `TUNING.<NAME>` value.
    fn constant(&self, name: &str, depth: usize) -> Result<f64> {
        if depth > MAX_REFERENCE_DEPTH {
            return Err(Error::ParseError(format!(
                "tuning.lua: `TUNING.{}` reference chain is too deep (cycle?)",
                name
            )));
        }
        if !ALLOWED_CONSTANTS.contains(&name) {
            return Err(Error::ParseError(format!(
                "tuning.lua: `TUNING.{}` is not an allowed stat constant; check its value source \
                 and add it to ALLOWED_CONSTANTS",
                name
            )));
        }
        let Some(expr) = self.constants.get(name) else {
            return Err(Error::ParseError(format!(
                "tuning.lua: `TUNING.{}` is not defined by the default `Tune()` table",
                name
            )));
        };
        self.eval(expr, depth, true)
    }

    /// Resolve a `Tune()` local (e.g. `calories_per_day`).
    fn local(&self, name: &str, depth: usize) -> Result<f64> {
        if depth > MAX_REFERENCE_DEPTH {
            return Err(Error::ParseError(format!(
                "tuning.lua: local `{}` reference chain is too deep (cycle?)",
                name
            )));
        }
        let Some(expr) = self.locals.get(name) else {
            return Err(Error::ParseError(format!(
                "tuning.lua: local `{}` is not defined in `Tune`",
                name
            )));
        };
        self.eval(expr, depth, true)
    }

    /// Evaluate the supported constant subset. `allow_locals` is false for
    /// recipe-level fields, which may only use `TUNING.<NAME>`.
    fn eval(&self, expr: &ast::Expression, depth: usize, allow_locals: bool) -> Result<f64> {
        match expr {
            ast::Expression::Number(token) => token
                .token()
                .to_string()
                .trim()
                .parse::<f64>()
                .map_err(|_| {
                    Error::ParseError(format!(
                        "tuning.lua: invalid numeric literal `{}`",
                        token.token()
                    ))
                }),
            ast::Expression::Parentheses { expression, .. } => {
                self.eval(expression, depth, allow_locals)
            }
            ast::Expression::UnaryOperator { unop, expression } => {
                let value = self.eval(expression, depth, allow_locals)?;
                match unop {
                    ast::UnOp::Minus(_) => Ok(-value),
                    other => Err(Error::ParseError(format!(
                        "tuning.lua: unsupported unary operator `{}`",
                        other
                    ))),
                }
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let left = self.eval(lhs, depth, allow_locals)?;
                let right = self.eval(rhs, depth, allow_locals)?;
                match binop {
                    ast::BinOp::Plus(_) => Ok(left + right),
                    ast::BinOp::Minus(_) => Ok(left - right),
                    ast::BinOp::Star(_) => Ok(left * right),
                    ast::BinOp::Slash(_) => Ok(left / right),
                    other => Err(Error::ParseError(format!(
                        "tuning.lua: unsupported binary operator `{}`",
                        other
                    ))),
                }
            }
            ast::Expression::Var(ast::Var::Name(name)) => {
                if !allow_locals {
                    return Err(Error::ParseError(format!(
                        "tuning.lua: bare identifier `{}` is not a recipe-level constant",
                        name.token()
                    )));
                }
                self.local(&name.token().to_string(), depth + 1)
            }
            ast::Expression::Var(ast::Var::Expression(var_expr)) => {
                let ast::Prefix::Name(prefix) = var_expr.prefix() else {
                    return Err(Error::ParseError(format!(
                        "tuning.lua: unsupported constant reference `{}`",
                        short_expr(expr)
                    )));
                };
                let mut path = vec![prefix.token().to_string()];
                for suffix in var_expr.suffixes() {
                    match suffix {
                        ast::Suffix::Index(ast::Index::Dot { name, .. }) if path.len() < 2 => {
                            path.push(name.token().to_string());
                        }
                        _ => {
                            return Err(Error::ParseError(format!(
                                "tuning.lua: unsupported constant reference `{}`",
                                short_expr(expr)
                            )))
                        }
                    }
                }
                if path.len() == 2 && path[0] == "TUNING" {
                    self.constant(&path[1], depth + 1)
                } else {
                    Err(Error::ParseError(format!(
                        "tuning.lua: unsupported constant reference `{}`",
                        short_expr(expr)
                    )))
                }
            }
            other => Err(Error::ParseError(format!(
                "tuning.lua: unsupported expression `{}`",
                short_expr(other)
            ))),
        }
    }
}

/// Evaluate a recipe-level numeric field: number literals, parentheses, unary
/// minus, `+ - * /` and whitelisted `TUNING.<NAME>` references. Anything else
/// (local variables, function calls, field lookups) is an error.
pub fn eval_const(expr: &ast::Expression, tuning: &TuningTable<'_>) -> Result<f64> {
    tuning.eval(expr, 0, false)
}

/// Find `function Tune(...)` at the top level and return its body block.
fn find_tune(ast: &Ast) -> Option<&ast::Block> {
    for stmt in ast.nodes().stmts() {
        let ast::Stmt::FunctionDeclaration(declaration) = stmt else {
            continue;
        };
        let name = declaration.name();
        if name.method_name().is_some() {
            continue;
        }
        let mut names = name.names().iter();
        let Some(first) = names.next() else {
            continue;
        };
        if first.token().to_string() == "Tune" && names.next().is_none() {
            return Some(declaration.body().block());
        }
    }
    None
}

/// One-line, length-capped rendering for error messages.
fn clamp_text(text: &str) -> String {
    let mut text = text.replace('\n', " ");
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 80 {
        text.chars().take(77).collect::<String>() + "..."
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped after the real `tuning.lua`: locals computed from other locals,
    /// then the table, then the no-argument `Tune()` call.
    const TUNING_LUA: &str = r#"
TUNING = {}
TUNING_MODIFIERS = {}

function Tune(overrides)
    if overrides == nil then
        overrides = {}
    end

    local seg_time = 30
    local total_day_time = seg_time*16
    local day_segs = 10
    local day_time = seg_time * day_segs
    local calories_per_day = 75
    local perish_warp = 1

    TUNING =
    {
        HEALING_SMALL = 3,
        SANITY_TINY = 5,
        CALORIES_LARGE = calories_per_day/2,
        CALORIES_MEDSMALL = calories_per_day/4,
        HUNGERREGEN_TICK_VALUE = 5 * (calories_per_day*2.5) / (0.5*total_day_time*perish_warp),
        BUFF_FOOD_TEMP_DURATION = day_time,
        NOT_FOR_STATS = 1,
    }
end

Tune()
"#;

    fn table_from(source: &str) -> Ast {
        full_moon::parse(source).expect("fixture parses")
    }

    /// `return <expr>` fixture → the returned expression.
    fn return_expr(ast: &Ast) -> &ast::Expression {
        let Some(ast::LastStmt::Return(ret)) = ast.nodes().last_stmt() else {
            panic!("fixture is not a return statement")
        };
        ret.returns().iter().next().expect("return has a value")
    }

    fn resolve(source: &str, name: &str) -> Result<f64> {
        let ast = table_from(source);
        let table = TuningTable::parse(&ast)?;
        table.constant(name, 0)
    }

    #[test]
    fn resolves_literals_and_computed_locals() {
        assert_eq!(resolve(TUNING_LUA, "HEALING_SMALL").unwrap(), 3.0);
        assert_eq!(resolve(TUNING_LUA, "CALORIES_LARGE").unwrap(), 37.5);
        assert_eq!(resolve(TUNING_LUA, "CALORIES_MEDSMALL").unwrap(), 18.75);
        assert_eq!(
            resolve(TUNING_LUA, "BUFF_FOOD_TEMP_DURATION").unwrap(),
            300.0
        );
        let hungerregen = resolve(TUNING_LUA, "HUNGERREGEN_TICK_VALUE").unwrap();
        assert!((hungerregen - 3.90625).abs() < 1e-9, "{}", hungerregen);
    }

    #[test]
    fn rejects_constants_outside_the_allowlist_and_unknown_names() {
        let err = resolve(TUNING_LUA, "NOT_FOR_STATS").unwrap_err();
        assert!(err.to_string().contains("not an allowed"), "{}", err);

        let ast = table_from(TUNING_LUA);
        let table = TuningTable::parse(&ast).unwrap();
        let err = table.constant("HEALING_TINY", 0).unwrap_err();
        assert!(err.to_string().contains("not defined"), "{}", err);
    }

    #[test]
    fn evaluates_recipe_level_expressions() {
        let ast = table_from(TUNING_LUA);
        let table = TuningTable::parse(&ast).unwrap();
        let cases = [
            ("1", 1.0),
            ("-TUNING.HEALING_SMALL", -3.0),
            ("TUNING.CALORIES_LARGE*2", 75.0),
            ("TUNING.CALORIES_LARGE/2", 18.75),
            ("TUNING.CALORIES_LARGE + TUNING.CALORIES_MEDSMALL", 56.25),
            ("(TUNING.HEALING_SMALL)", 3.0),
        ];
        for (source, expected) in cases {
            let expr_source = format!("return {}", source);
            let expr_ast = table_from(&expr_source);
            let value = eval_const(return_expr(&expr_ast), &table).unwrap();
            assert!(
                (value - expected).abs() < 1e-9,
                "{} -> {} != {}",
                source,
                value,
                expected
            );
        }
    }

    #[test]
    fn rejects_unsupported_recipe_level_shapes() {
        let ast = table_from(TUNING_LUA);
        let table = TuningTable::parse(&ast).unwrap();
        for source in [
            "some_local",
            "TUNING.NOT_FOR_STATS",
            "TUNING",
            "f(1)",
            "TUNING.X.Y",
        ] {
            let expr_source = format!("return {}", source);
            let expr_ast = table_from(&expr_source);
            assert!(
                eval_const(return_expr(&expr_ast), &table).is_err(),
                "`{}` should be rejected",
                source
            );
        }
    }

    #[test]
    fn rejects_missing_tune_function() {
        let ast = table_from("local x = 1\n");
        let Err(err) = TuningTable::parse(&ast) else {
            panic!("expected a parse error")
        };
        assert!(err.to_string().contains("Tune"), "{}", err);
    }
}
