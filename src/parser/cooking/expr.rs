//! Recipe `test` expression subset and its JSON AST encoding.
//!
//! DST crock-pot recipe tests are pure expressions over the `cooker`, `names`
//! and `tags` parameters. Across all shipped snapshots they only use logical
//! operators, comparisons, numeric addition and field access, so a small
//! interpreter-friendly AST is enough. Unsupported constructs are rejected
//! loudly rather than being skipped: dropping one recipe would silently change
//! the simulation result.

use crate::error::{Error, Result};
use crate::parser::string_literal_text;
use full_moon::ast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl UnaryOp {
    fn json_name(self) -> &'static str {
        match self {
            UnaryOp::Neg => "neg",
            UnaryOp::Not => "not",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    And,
    Or,
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
    Add,
    Sub,
    Mul,
    Div,
}

impl BinaryOp {
    fn json_name(self) -> &'static str {
        match self {
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::Gt => "gt",
            BinaryOp::Ge => "ge",
            BinaryOp::Lt => "lt",
            BinaryOp::Le => "le",
            BinaryOp::Eq => "eq",
            BinaryOp::Ne => "ne",
            BinaryOp::Add => "add",
            BinaryOp::Sub => "sub",
            BinaryOp::Mul => "mul",
            BinaryOp::Div => "div",
        }
    }
}

/// A Lua value in the supported expression subset.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Nil,
    Bool(bool),
    Number(f64),
    Str(String),
    Var(String),
    Field(Box<Expr>, String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Serialise to the compact JSON AST consumed by `cooking_eval.js`.
    ///
    /// Leaves: `["nil"]`, `["bool",true]`, `["num",0.5]`, `["str","x"]`,
    /// `["var","names"]`.
    /// Operators are arrays with the opcode first, e.g.
    /// `["field",["var","names"],"berries"]`, `["and",a,b]`.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Expr::Nil => serde_json::json!(["nil"]),
            Expr::Bool(v) => serde_json::json!(["bool", v]),
            Expr::Number(v) => serde_json::json!(["num", v]),
            Expr::Str(v) => serde_json::json!(["str", v]),
            Expr::Var(v) => serde_json::json!(["var", v]),
            Expr::Field(base, name) => {
                serde_json::json!(["field", base.to_json(), name])
            }
            Expr::Unary(op, value) => serde_json::json!([op.json_name(), value.to_json()]),
            Expr::Binary(op, lhs, rhs) => {
                serde_json::json!([op.json_name(), lhs.to_json(), rhs.to_json()])
            }
        }
    }
}

/// Translate the body of a `test = function(cooker, names, tags) ... end`
/// expression. Only a single trailing `return <expr>` is supported.
pub fn translate_test_function(value: &ast::Expression, recipe: &str) -> Result<Expr> {
    let ast::Expression::Function(func) = value else {
        return Err(unsupported(
            recipe,
            format!("recipe test is not a function: {}", short_expr(value)),
        ));
    };
    let block = func.body().block();
    if block.stmts().next().is_some() {
        return Err(unsupported(
            recipe,
            "recipe test has statements before its return".to_string(),
        ));
    }
    let Some(ast::LastStmt::Return(ret)) = block.last_stmt() else {
        return Err(unsupported(
            recipe,
            "recipe test has no final return expression".to_string(),
        ));
    };
    let Some(expr) = ret.returns().iter().next() else {
        return Err(unsupported(
            recipe,
            "recipe test returns no value".to_string(),
        ));
    };
    translate_expr(expr, recipe)
}

/// Translate a plain Lua expression from the supported subset.
pub fn translate_expr(expr: &ast::Expression, context: &str) -> Result<Expr> {
    match expr {
        ast::Expression::Number(token) => {
            let raw = token.token().to_string();
            let value = raw.trim().parse::<f64>().map_err(|_| {
                unsupported(context, format!("unsupported number literal `{}`", raw))
            })?;
            Ok(Expr::Number(value))
        }
        ast::Expression::String(token) => {
            Ok(Expr::Str(string_literal_text(&token.token().to_string())))
        }
        ast::Expression::Symbol(symbol) => match symbol.token().to_string().as_str() {
            "true" => Ok(Expr::Bool(true)),
            "false" => Ok(Expr::Bool(false)),
            "nil" => Ok(Expr::Nil),
            other => Err(unsupported(
                context,
                format!("unsupported symbol `{}`", other),
            )),
        },
        ast::Expression::Parentheses { expression, .. } => translate_expr(expression, context),
        ast::Expression::Var(ast::Var::Name(name)) => Ok(Expr::Var(name.token().to_string())),
        ast::Expression::Var(ast::Var::Expression(var_expr)) => {
            var_expr_to_field(var_expr, context)
        }
        ast::Expression::UnaryOperator { unop, expression } => {
            let inner = translate_expr(expression, context)?;
            match unop {
                ast::UnOp::Not(_) => Ok(Expr::Unary(UnaryOp::Not, Box::new(inner))),
                ast::UnOp::Minus(_) => Ok(Expr::Unary(UnaryOp::Neg, Box::new(inner))),
                _ => Err(unsupported(
                    context,
                    format!("unsupported unary operator `{}`", unop),
                )),
            }
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            let left = translate_expr(lhs, context)?;
            let right = translate_expr(rhs, context)?;
            let op = match binop {
                ast::BinOp::And(_) => BinaryOp::And,
                ast::BinOp::Or(_) => BinaryOp::Or,
                ast::BinOp::GreaterThan(_) => BinaryOp::Gt,
                ast::BinOp::GreaterThanEqual(_) => BinaryOp::Ge,
                ast::BinOp::LessThan(_) => BinaryOp::Lt,
                ast::BinOp::LessThanEqual(_) => BinaryOp::Le,
                ast::BinOp::TwoEqual(_) => BinaryOp::Eq,
                ast::BinOp::TildeEqual(_) => BinaryOp::Ne,
                ast::BinOp::Plus(_) => BinaryOp::Add,
                ast::BinOp::Minus(_) => BinaryOp::Sub,
                ast::BinOp::Star(_) => BinaryOp::Mul,
                ast::BinOp::Slash(_) => BinaryOp::Div,
                _ => {
                    return Err(unsupported(
                        context,
                        format!("unsupported binary operator `{}`", binop),
                    ))
                }
            };
            Ok(Expr::Binary(op, Box::new(left), Box::new(right)))
        }
        other => Err(unsupported(
            context,
            format!("unsupported expression `{}`", short_expr(other)),
        )),
    }
}

/// Turn `names.berries` / `names["berries"]` / nested accesses into nested
/// `Expr::Field`. Other VarExpression shapes are rejected.
fn var_expr_to_field(var_expr: &ast::VarExpression, context: &str) -> Result<Expr> {
    let ast::Prefix::Name(name) = var_expr.prefix() else {
        return Err(unsupported(
            context,
            format!("unsupported variable expression `{}`", var_expr),
        ));
    };
    let mut expr = Expr::Var(name.token().to_string());
    for suffix in var_expr.suffixes() {
        match suffix {
            ast::Suffix::Index(ast::Index::Dot { name, .. }) => {
                expr = Expr::Field(Box::new(expr), name.token().to_string());
            }
            ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
                let key = match expression {
                    ast::Expression::String(token) => {
                        string_literal_text(&token.token().to_string())
                    }
                    ast::Expression::Number(token) => token.token().to_string(),
                    _ => {
                        return Err(unsupported(
                            context,
                            format!("unsupported dynamic index in `{}`", var_expr),
                        ))
                    }
                };
                expr = Expr::Field(Box::new(expr), key);
            }
            _ => {
                return Err(unsupported(
                    context,
                    format!("unsupported suffix in `{}`", var_expr),
                ))
            }
        }
    }
    Ok(expr)
}

fn short_expr(expr: &ast::Expression) -> String {
    let mut text = expr.to_string().replace('\n', " ");
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 120 {
        text.chars().take(117).collect::<String>() + "..."
    } else {
        text
    }
}

fn unsupported(context: &str, detail: String) -> Error {
    Error::ParseError(format!("cooking recipe `{}`: {}", context, detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(source: &str) -> Expr {
        let ast = full_moon::parse(source).unwrap();
        for stmt in ast.nodes().stmts() {
            if let ast::Stmt::LocalAssignment(assign) = stmt {
                if let Some(ast::Expression::Function(func)) = assign.expressions().iter().next() {
                    let block = func.body().block();
                    if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
                        return translate_expr(ret.returns().iter().next().unwrap(), "test")
                            .unwrap();
                    }
                }
            }
        }
        panic!("fixture did not contain a local function");
    }

    #[test]
    fn translates_real_recipe_shape() {
        let expr = translate(
            "local f = function(cooker, names, tags) \
             return (names.butterflywings or names.moonbutterflywings) \
             and not tags.meat and tags.veggie >= 0.5 end",
        );
        assert_eq!(
            expr.to_json(),
            serde_json::json!([
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
            ])
        );
    }

    #[test]
    fn translates_bracket_field_and_arithmetic() {
        let expr = translate(
            "local f = function(cooker, names, tags) \
             return ((names[\"cave_banana\"] or 0) + (names.cave_banana_cooked or 0) >= 2) \
             and names.twigs end",
        );
        let json = expr.to_json();
        assert_eq!(json[0], "and");
        assert_eq!(
            json[1][0], "ge",
            "expected arithmetic/comparison subtree: {json}"
        );
    }

    #[test]
    fn rejects_function_calls() {
        let ast =
            full_moon::parse("local f = function(cooker, names, tags) return helper(names) end")
                .unwrap();
        let mut found = false;
        for stmt in ast.nodes().stmts() {
            if let ast::Stmt::LocalAssignment(assign) = stmt {
                if let Some(value) = assign.expressions().iter().next() {
                    assert!(translate_test_function(value, "helper_recipe").is_err());
                    found = true;
                }
            }
        }
        assert!(found);
    }
}
