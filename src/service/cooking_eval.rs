//! Runtime evaluator for the compact recipe `test` AST produced by the Lua
//! compiler.
//!
//! The frontend uses the same JSON contract; this implementation is the
//! authoritative server-side evaluator used by the game package exporter and
//! (later) by the standalone game backend.

use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub enum EvalValue {
    Nil,
    Bool(bool),
    Number(f64),
    Str(String),
}

impl EvalValue {
    pub fn truthy(&self) -> bool {
        !matches!(self, EvalValue::Nil | EvalValue::Bool(false))
    }
}

/// Evaluation environment for one four-ingredient combination.
#[derive(Debug, Clone)]
pub struct EvalEnv {
    pub names: HashMap<String, f64>,
    pub tags: HashMap<String, f64>,
    pub cooker: String,
}

impl EvalEnv {
    pub fn new(cooker: impl Into<String>) -> Self {
        Self {
            names: HashMap::new(),
            tags: HashMap::new(),
            cooker: cooker.into(),
        }
    }

    pub fn add_name(&mut self, key: &str) {
        *self.names.entry(key.to_string()).or_insert(0.0) += 1.0;
    }

    pub fn add_tag(&mut self, key: &str, value: f64) {
        *self.tags.entry(key.to_string()).or_insert(0.0) += value;
    }
}

#[derive(Debug, PartialEq)]
pub enum EvalError {
    Malformed(String),
    Unsupported(String),
    Type(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Malformed(m) => write!(f, "malformed cooking AST: {}", m),
            EvalError::Unsupported(m) => write!(f, "unsupported cooking AST: {}", m),
            EvalError::Type(m) => write!(f, "cooking AST type error: {}", m),
        }
    }
}

impl std::error::Error for EvalError {}

/// Evaluate one expression AST node.
pub fn eval(expr: &Value, env: &EvalEnv) -> Result<EvalValue, EvalError> {
    let arr = expr
        .as_array()
        .ok_or_else(|| EvalError::Malformed(format!("expected array, got {}", expr)))?;
    let op = arr
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| EvalError::Malformed("missing opcode".into()))?;

    match op {
        "nil" => Ok(EvalValue::Nil),
        "bool" => Ok(EvalValue::Bool(
            arr.get(1)
                .and_then(Value::as_bool)
                .ok_or_else(|| EvalError::Malformed("bool opcode without boolean".into()))?,
        )),
        "num" => Ok(EvalValue::Number(
            arr.get(1)
                .and_then(Value::as_f64)
                .ok_or_else(|| EvalError::Malformed("num opcode without number".into()))?,
        )),
        "str" => Ok(EvalValue::Str(
            arr.get(1)
                .and_then(Value::as_str)
                .ok_or_else(|| EvalError::Malformed("str opcode without string".into()))?
                .to_string(),
        )),
        "var" => match arr.get(1).and_then(Value::as_str) {
            Some("cooker") => Ok(EvalValue::Str(env.cooker.clone())),
            Some("names") | Some("tags") => Err(EvalError::Unsupported(
                "bare names/tags table cannot be a value".into(),
            )),
            Some(other) => Err(EvalError::Unsupported(format!(
                "unknown variable `{}`",
                other
            ))),
            None => Err(EvalError::Malformed("var opcode without name".into())),
        },
        "field" => eval_field(arr, env),
        "not" => Ok(EvalValue::Bool(!arg_truthy(arr, env, 1, "not")?)),
        "neg" => Ok(EvalValue::Number(-arg_number(arr, env, 1, "neg")?)),
        "and" => {
            let lhs = arg_value(arr, env, 1, "and")?;
            if lhs.truthy() {
                arg_value(arr, env, 2, "and")
            } else {
                Ok(lhs)
            }
        }
        "or" => {
            let lhs = arg_value(arr, env, 1, "or")?;
            if lhs.truthy() {
                Ok(lhs)
            } else {
                arg_value(arr, env, 2, "or")
            }
        }
        "add" => Ok(EvalValue::Number(
            arg_number(arr, env, 1, "add")? + arg_number(arr, env, 2, "add")?,
        )),
        "sub" => Ok(EvalValue::Number(
            arg_number(arr, env, 1, "sub")? - arg_number(arr, env, 2, "sub")?,
        )),
        "mul" => Ok(EvalValue::Number(
            arg_number(arr, env, 1, "mul")? * arg_number(arr, env, 2, "mul")?,
        )),
        "div" => Ok(EvalValue::Number(
            arg_number(arr, env, 1, "div")? / arg_number(arr, env, 2, "div")?,
        )),
        "eq" => Ok(EvalValue::Bool(values_equal(
            arg_value(arr, env, 1, "eq")?,
            arg_value(arr, env, 2, "eq")?,
        ))),
        "ne" => Ok(EvalValue::Bool(!values_equal(
            arg_value(arr, env, 1, "ne")?,
            arg_value(arr, env, 2, "ne")?,
        ))),
        "gt" => Ok(EvalValue::Bool(
            arg_number(arr, env, 1, "gt")? > arg_number(arr, env, 2, "gt")?,
        )),
        "ge" => Ok(EvalValue::Bool(
            arg_number(arr, env, 1, "ge")? >= arg_number(arr, env, 2, "ge")?,
        )),
        "lt" => Ok(EvalValue::Bool(
            arg_number(arr, env, 1, "lt")? < arg_number(arr, env, 2, "lt")?,
        )),
        "le" => Ok(EvalValue::Bool(
            arg_number(arr, env, 1, "le")? <= arg_number(arr, env, 2, "le")?,
        )),
        other => Err(EvalError::Unsupported(other.to_string())),
    }
}

/// Convenience: evaluate a recipe test and return Lua truthiness.
pub fn eval_truthy(expr: &Value, env: &EvalEnv) -> Result<bool, EvalError> {
    Ok(eval(expr, env)?.truthy())
}

/// Collect `names.*` / `tags.*` field references for candidate-pool building.
pub fn collect_field_refs(expr: &Value, names: &mut BTreeSet<String>, tags: &mut BTreeSet<String>) {
    let Some(arr) = expr.as_array() else {
        return;
    };
    let Some(op) = arr.first().and_then(Value::as_str) else {
        return;
    };
    match op {
        "field" => {
            let Some(base) = arr.get(1).and_then(Value::as_array) else {
                return;
            };
            let base_is_var = base.first().and_then(Value::as_str) == Some("var");
            if base_is_var {
                match base.get(1).and_then(Value::as_str) {
                    Some("names") => {
                        if let Some(name) = arr.get(2).and_then(Value::as_str) {
                            names.insert(name.to_string());
                        }
                    }
                    Some("tags") => {
                        if let Some(tag) = arr.get(2).and_then(Value::as_str) {
                            tags.insert(tag.to_string());
                        }
                    }
                    _ => {}
                }
            } else {
                collect_field_refs(&base_value(arr, 1), names, tags);
            }
        }
        "not" | "neg" => collect_field_refs(&base_value(arr, 1), names, tags),
        "and" | "or" | "eq" | "ne" | "gt" | "ge" | "lt" | "le" | "add" | "sub" | "mul" | "div" => {
            collect_field_refs(&base_value(arr, 1), names, tags);
            collect_field_refs(&base_value(arr, 2), names, tags);
        }
        _ => {}
    }
}

fn base_value(arr: &[Value], index: usize) -> Value {
    arr.get(index).cloned().unwrap_or(Value::Null)
}

fn eval_field(arr: &[Value], env: &EvalEnv) -> Result<EvalValue, EvalError> {
    let base = arr
        .get(1)
        .and_then(Value::as_array)
        .ok_or_else(|| EvalError::Malformed("field without base".into()))?;
    let base_op = base.first().and_then(Value::as_str);
    let base_name = base.get(1).and_then(Value::as_str);
    let name = arr
        .get(2)
        .and_then(Value::as_str)
        .ok_or_else(|| EvalError::Malformed("field without name".into()))?;

    match (base_op, base_name) {
        (Some("var"), Some("names")) => Ok(env
            .names
            .get(name)
            .copied()
            .map(EvalValue::Number)
            .unwrap_or(EvalValue::Nil)),
        (Some("var"), Some("tags")) => Ok(env
            .tags
            .get(name)
            .copied()
            .map(EvalValue::Number)
            .unwrap_or(EvalValue::Nil)),
        _ => Err(EvalError::Unsupported(format!(
            "field base must be names/tags, got {:?}",
            base
        ))),
    }
}

fn arg_value(arr: &[Value], env: &EvalEnv, index: usize, op: &str) -> Result<EvalValue, EvalError> {
    let expr = arr
        .get(index)
        .ok_or_else(|| EvalError::Malformed(format!("{} missing operand {}", op, index)))?;
    eval(expr, env)
}

fn arg_number(arr: &[Value], env: &EvalEnv, index: usize, op: &str) -> Result<f64, EvalError> {
    match arg_value(arr, env, index, op)? {
        EvalValue::Number(n) => Ok(n),
        other => Err(EvalError::Type(format!(
            "{} operand {} must be number, got {:?}",
            op, index, other
        ))),
    }
}

fn arg_truthy(arr: &[Value], env: &EvalEnv, index: usize, op: &str) -> Result<bool, EvalError> {
    Ok(arg_value(arr, env, index, op)?.truthy())
}

fn values_equal(a: EvalValue, b: EvalValue) -> bool {
    match (a, b) {
        (EvalValue::Nil, EvalValue::Nil) => true,
        (EvalValue::Nil, _) | (_, EvalValue::Nil) => false,
        (EvalValue::Bool(x), EvalValue::Bool(y)) => x == y,
        (EvalValue::Number(x), EvalValue::Number(y)) => x == y,
        (EvalValue::Str(x), EvalValue::Str(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env() -> EvalEnv {
        let mut env = EvalEnv::new("portablecookpot");
        env.add_name("butterflywings");
        env.add_name("berries");
        env.add_tag("veggie", 0.5);
        env.add_tag("fruit", 1.0);
        env
    }

    #[test]
    fn evaluates_real_recipe_shape() {
        let ast = json!([
            "and",
            [
                "and",
                [
                    "or",
                    ["field", ["var", "names"], "butterflywings"],
                    ["field", ["var", "names"], "moonbutterflywings"]
                ],
                ["not", ["field", ["var", "tags"], "meat"]]
            ],
            ["ge", ["field", ["var", "tags"], "veggie"], ["num", 0.5]]
        ]);
        assert!(eval_truthy(&ast, &env()).unwrap());
    }

    #[test]
    fn missing_field_is_nil_and_zero_is_truthy() {
        let ast = json!(["or", ["field", ["var", "names"], "missing"], ["num", 0]]);
        let value = eval(&ast, &env()).unwrap();
        assert_eq!(value, EvalValue::Number(0.0));
        assert!(value.truthy());
    }

    #[test]
    fn comparisons_and_arithmetic() {
        let ast = json!([
            "ge",
            [
                "add",
                ["or", ["field", ["var", "names"], "missing"], ["num", 0]],
                ["field", ["var", "names"], "berries"]
            ],
            ["num", 1]
        ]);
        assert!(eval_truthy(&ast, &env()).unwrap());
    }

    #[test]
    fn cooker_variable_is_available() {
        let ast = json!(["eq", ["var", "cooker"], ["str", "portablecookpot"]]);
        assert!(eval_truthy(&ast, &env()).unwrap());
    }

    #[test]
    fn collect_refs_walks_nested_fields() {
        let ast = json!([
            "and",
            ["field", ["var", "names"], "twigs"],
            ["ge", ["field", ["var", "tags"], "inedible"], ["num", 1]]
        ]);
        let mut names = BTreeSet::new();
        let mut tags = BTreeSet::new();
        collect_field_refs(&ast, &mut names, &mut tags);
        assert!(names.contains("twigs"));
        assert!(tags.contains("inedible"));
    }
}
