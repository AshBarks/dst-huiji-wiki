//! TuningTable: scalar leaf extraction from the game's `tuning.lua`.
//!
//! Real structure of tuning.lua: a global `Tune(overrides)` function that
//! folds numeric locals (`seg_time`, `total_day_time`, ...) and finally
//! assigns one giant table `TUNING = { KEY = expr, ... }`. Values reference
//! those locals through arithmetic. World-option overrides (`tuning_override`)
//! are explicitly out of scope; anything non-scalar is recorded as skipped.
//!
//! `build_tuning_leaves` additionally walks nested sub-tables and returns
//! numeric leaves keyed by dotted path (`SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD`),
//! which parsers use to resolve `TUNING.*` references in game scripts.

use std::collections::BTreeMap;

use full_moon::ast;
use serde::Serialize;

use crate::Result;

/// A resolved tuning leaf value.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum TuningVal {
    Num(f64),
    Str(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedEntry {
    pub key: Option<String>,
    pub reason: String,
}

/// Key -> scalar value map extracted from tuning.lua.
#[derive(Debug, Clone, Serialize, Default)]
pub struct TuningTable {
    pub values: BTreeMap<String, TuningVal>,
    pub skipped: Vec<SkippedEntry>,
}

impl TuningTable {
    pub fn get(&self, key: &str) -> Option<&TuningVal> {
        self.values.get(key)
    }

    /// Resolve a dotted `TUNING.KEY` reference.
    pub fn resolve(&self, dotted: &str) -> Option<&TuningVal> {
        self.values.get(dotted.strip_prefix("TUNING.")?)
    }
}

/// Numeric expression evaluator over folded locals.
fn eval_num(expr: &ast::Expression, locals: &BTreeMap<String, f64>) -> Option<f64> {
    match expr {
        ast::Expression::Number(n) => n.to_string().trim().parse::<f64>().ok(),
        ast::Expression::Var(ast::Var::Name(name)) => {
            locals.get(&name.token().to_string()).copied()
        }
        ast::Expression::UnaryOperator { unop, expression } => {
            let v = eval_num(expression, locals)?;
            if unop.to_string().trim() == "-" {
                Some(-v)
            } else {
                None
            }
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            let l = eval_num(lhs, locals)?;
            let r = eval_num(rhs, locals)?;
            match binop.to_string().trim() {
                "+" => Some(l + r),
                "-" => Some(l - r),
                "*" => Some(l * r),
                "/" => Some(l / r),
                "^" => Some(l.powf(r)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Locates the body of `function Tune(overrides)`.
fn find_tune_block(ast: &ast::Ast) -> Option<&ast::Block> {
    ast.nodes().stmts().find_map(|stmt| match stmt {
        ast::Stmt::FunctionDeclaration(decl) if decl.name().to_string() == "Tune" => {
            Some(decl.body().block())
        }
        _ => None,
    })
}

/// Folds numeric locals to a fixpoint (forward references through arithmetic
/// are rare but the fixpoint is cheap).
fn fold_tune_locals(tune_block: &ast::Block) -> BTreeMap<String, f64> {
    let mut locals: BTreeMap<String, f64> = BTreeMap::new();
    for _ in 0..8 {
        let mut changed = false;
        for stmt in tune_block.stmts() {
            if let ast::Stmt::LocalAssignment(assign) = stmt {
                for (name, expr) in assign.names().iter().zip(assign.expressions().iter()) {
                    let var = name.token().to_string();
                    if locals.contains_key(&var) {
                        continue;
                    }
                    if let Some(v) = eval_num(expr, &locals) {
                        locals.insert(var, v);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    locals
}

/// Locates `TUNING = { ... }` inside the `Tune` body.
fn find_tuning_table(tune_block: &ast::Block) -> Option<&ast::TableConstructor> {
    tune_block.stmts().find_map(|stmt| match stmt {
        ast::Stmt::Assignment(assign) => {
            let names: Vec<_> = assign.variables().iter().collect();
            let exprs: Vec<_> = assign.expressions().iter().collect();
            names
                .iter()
                .zip(exprs.iter())
                .find(|(v, _)| matches!(v, ast::Var::Name(n) if n.token().to_string() == "TUNING"))
                .and_then(|(_, e)| match e {
                    ast::Expression::TableConstructor(t) => Some(t),
                    _ => None,
                })
        }
        _ => None,
    })
}

/// Recursively flattens numeric leaves of a `TUNING` sub-table into dotted
/// paths (`SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD`). Strings and non-foldable
/// expressions are ignored.
fn flatten_numeric_leaves(
    table: &ast::TableConstructor,
    prefix: &str,
    locals: &BTreeMap<String, f64>,
    out: &mut BTreeMap<String, f64>,
) {
    for field in table.fields() {
        let ast::Field::NameKey { key, value, .. } = field else {
            continue;
        };
        let key_text = key.token().to_string();
        let path = if prefix.is_empty() {
            key_text
        } else {
            format!("{prefix}.{key_text}")
        };
        match value {
            ast::Expression::TableConstructor(t) => {
                flatten_numeric_leaves(t, &path, locals, out);
            }
            _ => {
                if let Some(v) = eval_num(value, locals) {
                    out.insert(path, v);
                }
            }
        }
    }
}

/// Numeric leaves of the game's `TUNING` table **including nested tables**,
/// keyed by dotted path without the `TUNING.` prefix (e.g.
/// `SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD`). Lets parsers resolve `TUNING.*`
/// references found in game scripts.
pub fn build_tuning_leaves(source: &str) -> Result<BTreeMap<String, f64>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let Some(tune_block) = find_tune_block(&ast) else {
        return Ok(BTreeMap::new());
    };
    let locals = fold_tune_locals(tune_block);
    let Some(table_ctor) = find_tuning_table(tune_block) else {
        return Ok(BTreeMap::new());
    };
    let mut out = BTreeMap::new();
    flatten_numeric_leaves(table_ctor, "", &locals, &mut out);
    Ok(out)
}

/// Builds the tuning table from tuning.lua source text.
pub fn build_tuning(source: &str) -> Result<TuningTable> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;

    let mut table = TuningTable::default();
    let Some(tune_block) = find_tune_block(&ast) else {
        table.skipped.push(SkippedEntry {
            key: None,
            reason: "function Tune not found".to_string(),
        });
        return Ok(table);
    };

    let locals = fold_tune_locals(tune_block);
    let big_table = find_tuning_table(tune_block);

    let Some(table_ctor) = big_table else {
        table.skipped.push(SkippedEntry {
            key: None,
            reason: "TUNING table constructor not found".to_string(),
        });
        return Ok(table);
    };

    for field in table_ctor.fields() {
        if let ast::Field::NameKey { key, value, .. } = field {
            let key_text = key.token().to_string();
            match value {
                ast::Expression::String(s) => {
                    let raw = s.to_string();
                    let inner = raw.trim().trim_matches('"').trim_matches('\'');
                    table
                        .values
                        .insert(key_text, TuningVal::Str(inner.to_string()));
                }
                _ => match eval_num(value, &locals) {
                    Some(v) => {
                        table.values.insert(key_text, TuningVal::Num(v));
                    }
                    None => table.skipped.push(SkippedEntry {
                        key: Some(key_text),
                        reason: format!("non-scalar or unresolved expression: {}", value),
                    }),
                },
            }
        }
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI_TUNING: &str = r#"
TUNING = {}

function Tune(overrides)
    local seg_time = 30
    local total_day_time = seg_time*16

    local multiplayer_goldentool_modifier = 1

    TUNING =
    {
        SEG_TIME = seg_time,
        TOTAL_DAY_TIME = total_day_time,

        GOLDENTOOLFACTOR = 4*multiplayer_goldentool_modifier,

        HOUND_HEALTH = 150,
        HOUND_DAMAGE = 20,
        HOUND_SPEED = 10,

        NEGATIVE_EXAMPLE = -2.5,

        GROWTIME = {base=0.75*day_time, random=0.25*day_time},

        DEPENDS_ON_OVERRIDE = overrides.some_value or 30,

        A_STRING = "hello",
    }
end

Tune()
"#;

    #[test]
    fn scalars_and_folded_locals_resolve() {
        let t = build_tuning(MINI_TUNING).unwrap();
        assert_eq!(t.get("SEG_TIME"), Some(&TuningVal::Num(30.0)));
        assert_eq!(t.get("TOTAL_DAY_TIME"), Some(&TuningVal::Num(480.0)));
        assert_eq!(t.get("GOLDENTOOLFACTOR"), Some(&TuningVal::Num(4.0)));
        assert_eq!(t.get("HOUND_DAMAGE"), Some(&TuningVal::Num(20.0)));
        assert_eq!(t.get("NEGATIVE_EXAMPLE"), Some(&TuningVal::Num(-2.5)));
        assert_eq!(
            t.get("A_STRING"),
            Some(&TuningVal::Str("hello".to_string()))
        );
    }

    #[test]
    fn dotted_resolve_works() {
        let t = build_tuning(MINI_TUNING).unwrap();
        assert_eq!(
            t.resolve("TUNING.HOUND_DAMAGE"),
            Some(&TuningVal::Num(20.0))
        );
        assert!(t.resolve("TUNING.NOPE").is_none());
        assert!(t.resolve("NO_PREFIX").is_none());
    }

    #[test]
    fn nonscalar_and_override_values_are_skipped_with_reasons() {
        let t = build_tuning(MINI_TUNING).unwrap();
        assert!(!t.values.contains_key("GROWTIME"));
        assert!(!t.values.contains_key("DEPENDS_ON_OVERRIDE"));
        let reasons: Vec<&str> = t.skipped.iter().filter_map(|s| s.key.as_deref()).collect();
        assert!(reasons.contains(&"GROWTIME"));
        assert!(reasons.contains(&"DEPENDS_ON_OVERRIDE"));
        assert!(t
            .skipped
            .iter()
            .any(|s| s.key.as_deref() == Some("GROWTIME") && s.reason.contains("non-scalar")));
    }

    #[test]
    fn missing_tune_function_is_recorded_not_fatal() {
        let t = build_tuning("local x = 1\n").unwrap();
        assert!(t.values.is_empty());
        assert_eq!(t.skipped.len(), 1);
        assert!(t.skipped[0].reason.contains("Tune"));
    }

    #[test]
    fn nested_numeric_leaves_resolve_with_dotted_paths() {
        let leaves = build_tuning_leaves(MINI_TUNING).unwrap();
        assert_eq!(leaves.get("HOUND_DAMAGE"), Some(&20.0));
        assert_eq!(leaves.get("SEG_TIME"), Some(&30.0));
        // 嵌套表里引用了未定义局部量（day_time）的叶子无法求值。
        assert_eq!(leaves.get("GROWTIME.base"), None);
        // 字符串/不可求值表达式不会出现在结果里。
        assert!(!leaves.contains_key("A_STRING"));
        assert!(!leaves.contains_key("DEPENDS_ON_OVERRIDE"));
    }

    #[test]
    fn nested_leaves_walk_skill_subtables() {
        let src = r#"
TUNING = {}

function Tune(overrides)
    local seg_time = 30
    TUNING =
    {
        SKILLS =
        {
            WORTOX =
            {
                TIPPED_BALANCE_THRESHOLD = 3,
                NICE_SANITY_MULT = 1 + 1,
            },
        },
        HOUND_DAMAGE = 20,
    }
end

Tune()
"#;
        let leaves = build_tuning_leaves(src).unwrap();
        assert_eq!(
            leaves.get("SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD"),
            Some(&3.0)
        );
        assert_eq!(leaves.get("SKILLS.WORTOX.NICE_SANITY_MULT"), Some(&2.0));
        assert_eq!(leaves.get("HOUND_DAMAGE"), Some(&20.0));
        assert!(!leaves.contains_key("SKILLS"));
    }

    #[test]
    fn nested_leaves_tolerate_missing_tune() {
        let leaves = build_tuning_leaves("local x = 1\n").unwrap();
        assert!(leaves.is_empty());
    }
}
