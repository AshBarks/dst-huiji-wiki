//! 合成/科技静态表解析（模板维护专用）：
//! - `constants.lua` 的 `TECH = {...}`：常量名 → 科技树键
//! - `tuning.lua` 的 `TUNING.PROTOTYPER_TREES`：原型/制作站 → 科技树键
//! - `recipes_filter.lua` 的 `CRAFTING_FILTERS.<NAME>.recipes`：分类 → 配方名
//!
//! 三张表都只取字面量；函数值（如 `FAVORITES.recipes`）整体跳过。
//! 用于 `模块:Constants/CraftingNames` 的制作站别名派生与
//! `maintain-template-check` 的模板覆盖率核对。

use crate::Result;
use full_moon::ast;
use full_moon::visitors::Visitor;
use std::collections::BTreeMap;

/// 解析 `constants.lua` 的 `TECH = {...}`，返回常量名 → 科技树键列表
/// （保持源码顺序；`NONE = TechTree.Create()` 返回空列表）。
pub fn parse_tech_constants(source: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut visitor = TechVisitor::default();
    visitor.visit_ast(&ast);
    Ok(visitor.out)
}

#[derive(Default)]
struct TechVisitor {
    out: BTreeMap<String, Vec<String>>,
}

impl Visitor for TechVisitor {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let mut vars = assignment.variables().iter();
        let mut exprs = assignment.expressions().iter();
        let (Some(ast::Var::Name(name)), Some(ast::Expression::TableConstructor(table))) =
            (vars.next(), exprs.next())
        else {
            return;
        };
        if name.token().to_string() != "TECH" {
            return;
        }
        for field in table.fields() {
            if let ast::Field::NameKey { key, value, .. } = field {
                self.out
                    .insert(key.token().to_string(), tree_keys_of_expr(value));
            }
        }
    }
}

/// 解析 `tuning.lua` 的 `TUNING = { ... PROTOTYPER_TREES = {...} ... }`
/// （赋值位于 `Tune()` 函数体内，靠 Visitor 递归命中），返回
/// 原型/制作站名 → 科技树键列表。
pub fn parse_prototyper_trees(source: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut visitor = PrototyperTreesVisitor::default();
    visitor.visit_ast(&ast);
    Ok(visitor.out)
}

#[derive(Default)]
struct PrototyperTreesVisitor {
    out: BTreeMap<String, Vec<String>>,
}

impl Visitor for PrototyperTreesVisitor {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let mut vars = assignment.variables().iter();
        let mut exprs = assignment.expressions().iter();
        let (Some(ast::Var::Name(name)), Some(ast::Expression::TableConstructor(table))) =
            (vars.next(), exprs.next())
        else {
            return;
        };
        if name.token().to_string() != "TUNING" {
            return;
        }
        for field in table.fields() {
            let ast::Field::NameKey { key, value, .. } = field else {
                continue;
            };
            if key.token().to_string() != "PROTOTYPER_TREES" {
                continue;
            }
            let ast::Expression::TableConstructor(trees) = value else {
                continue;
            };
            for entry in trees.fields() {
                let ast::Field::NameKey { key, value, .. } = entry else {
                    continue;
                };
                self.out
                    .insert(key.token().to_string(), tree_keys_of_expr(value));
            }
        }
    }
}

/// 解析 `recipes_filter.lua` 的 `CRAFTING_FILTERS.<NAME>.recipes = {...}`，
/// 返回分类名 → 配方名列表（函数值分类不出现）。
pub fn parse_crafting_filter_lists(source: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut visitor = CraftingFiltersVisitor::default();
    visitor.visit_ast(&ast);
    Ok(visitor.out)
}

#[derive(Default)]
struct CraftingFiltersVisitor {
    out: BTreeMap<String, Vec<String>>,
}

impl Visitor for CraftingFiltersVisitor {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let mut vars = assignment.variables().iter();
        let mut exprs = assignment.expressions().iter();
        let (Some(ast::Var::Expression(var_expr)), Some(ast::Expression::TableConstructor(table))) =
            (vars.next(), exprs.next())
        else {
            return;
        };
        let Some(path) = var_path(var_expr) else {
            return;
        };
        if path.len() != 3 || path[0] != "CRAFTING_FILTERS" || path[2] != "recipes" {
            return;
        }
        let recipes = table
            .fields()
            .into_iter()
            .filter_map(|field| match field {
                ast::Field::NoKey(expr) => string_literal(expr),
                _ => None,
            })
            .collect();
        self.out.insert(path[1].clone(), recipes);
    }
}

/// `TECH` / `TechTree.Create({...})` 的值 → 科技树键列表。
/// 仅接受 `{ KEY = 数字 }` 形态，其余（函数调用、表达式）返回空。
fn tree_keys_of_expr(expr: &ast::Expression) -> Vec<String> {
    match expr {
        ast::Expression::TableConstructor(table) => tree_keys_of_table(table),
        ast::Expression::FunctionCall(call) => call_table_arg(call)
            .map(tree_keys_of_table)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn call_table_arg(call: &ast::FunctionCall) -> Option<&ast::TableConstructor> {
    match call.suffixes().last()? {
        ast::Suffix::Call(ast::Call::AnonymousCall(args)) => match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                arguments.iter().find_map(|arg| match arg {
                    ast::Expression::TableConstructor(table) => Some(table),
                    _ => None,
                })
            }
            ast::FunctionArgs::TableConstructor(table) => Some(table),
            _ => None,
        },
        _ => None,
    }
}

fn tree_keys_of_table(table: &ast::TableConstructor) -> Vec<String> {
    table
        .fields()
        .into_iter()
        .filter_map(|field| match field {
            ast::Field::NameKey {
                key,
                value: ast::Expression::Number(_),
                ..
            } => Some(key.token().to_string()),
            _ => None,
        })
        .collect()
}

/// `a.b.c` / `a["b"].c` 形态的点路径；含其它后缀时返回 `None`。
fn var_path(var_expr: &ast::VarExpression) -> Option<Vec<String>> {
    let mut out = match var_expr.prefix() {
        ast::Prefix::Name(name) => vec![name.token().to_string()],
        _ => return None,
    };
    for suffix in var_expr.suffixes() {
        match suffix {
            ast::Suffix::Index(ast::Index::Dot { name, .. }) => {
                out.push(name.token().to_string());
            }
            ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
                out.push(string_literal(expression)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

fn string_literal(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(s) => Some(strip_quotes(&s.to_string())),
        _ => None,
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tech_constants() {
        let source = r#"
TECH =
{
    NONE = TechTree.Create(),

    SCIENCE_ONE = { SCIENCE = 1 },
    LOST = { MAGIC = 10, SCIENCE = 10, ANCIENT = 10 },

    CARNIVAL_GOLFPROPS_ONE = { CARNIVAL_GOLFPROPS = 1 },
}
"#;
        let map = parse_tech_constants(source).unwrap();
        assert_eq!(map["NONE"], Vec::<String>::new());
        assert_eq!(map["SCIENCE_ONE"], vec!["SCIENCE"]);
        assert_eq!(map["LOST"], vec!["MAGIC", "SCIENCE", "ANCIENT"]);
        assert_eq!(map["CARNIVAL_GOLFPROPS_ONE"], vec!["CARNIVAL_GOLFPROPS"]);
    }

    #[test]
    fn test_parse_tech_constants_missing_is_empty() {
        let map = parse_tech_constants("local x = 1").unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_parse_prototyper_trees_inside_function() {
        let source = r#"
TUNING = {}

function Tune(overrides)
    local x = 1
    TUNING =
    {
        SOME_VALUE = 1,
        PROTOTYPER_TREES =
        {
            CARPENTRY_STATION = TechTree.Create({
                CARPENTRY = 2,
            }),
            CARNIVALGAME_GOLFGAME = TechTree.Create({
                CARNIVAL_GOLFPROPS = 1,
            }),
        },
    }
end
"#;
        let map = parse_prototyper_trees(source).unwrap();
        assert_eq!(map["CARPENTRY_STATION"], vec!["CARPENTRY"]);
        assert_eq!(map["CARNIVALGAME_GOLFGAME"], vec!["CARNIVAL_GOLFPROPS"]);
        assert!(!map.contains_key("SOME_VALUE"));
    }

    #[test]
    fn test_parse_prototyper_trees_missing_is_empty() {
        let map = parse_prototyper_trees("TUNING = { GLOBAL = 1 }").unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_parse_crafting_filter_lists() {
        let source = r#"
CRAFTING_FILTER_DEFS =
{
    {name = "TOOLS", image = "filter_tool.tex"},
}

CRAFTING_FILTERS = {}
for i, v in ipairs(CRAFTING_FILTER_DEFS) do
    CRAFTING_FILTERS[v.name] = v
end

CRAFTING_FILTERS.CHARACTER.recipes =
{
    "lighter",
    "bernie_inactive",
}

CRAFTING_FILTERS.TOOLS.recipes =
{
    "axe",
    "pickaxe",
}

CRAFTING_FILTERS.FAVORITES.recipes = function() return {} end
"#;
        let map = parse_crafting_filter_lists(source).unwrap();
        assert_eq!(map["CHARACTER"], vec!["lighter", "bernie_inactive"]);
        assert_eq!(map["TOOLS"], vec!["axe", "pickaxe"]);
        assert!(!map.contains_key("FAVORITES"));
    }

    #[test]
    fn test_parse_crafting_filter_lists_empty_table() {
        let source = "CRAFTING_FILTERS.MODS.recipes = {}";
        let map = parse_crafting_filter_lists(source).unwrap();
        assert_eq!(map["MODS"], Vec::<String>::new());
    }
}
