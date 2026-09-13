//! 合成/科技静态表解析（模板维护 + 制作栏图标命名专用）：
//! - `constants.lua` 的 `TECH = {...}`：常量名 → 科技树键
//! - `tuning.lua` 的 `TUNING.PROTOTYPER_TREES`：原型/制作站 → 科技树键
//! - `recipes_filter.lua` 的 `CRAFTING_FILTERS.<NAME>.recipes`：分类 → 配方名
//! - `recipes_filter.lua` 的 `CRAFTING_FILTER_DEFS`：过滤器 → 图标 stem
//! - `recipes.lua` 的 `PROTOTYPER_DEFS`：prefab → 图标 stem / 制作站过滤键
//!
//! 各表都只取字面量；函数值（如 `FAVORITES.recipes`、CHARACTER 的
//! `image = GetCharacterImage`）整体跳过。
//! 用于 `模块:Constants/CraftingNames` 的制作站别名派生、
//! `maintain-template-check` 的模板覆盖率核对，以及 `images-sync`
//! 图标元数据（`scripts_sync::images::meta::CraftingKeys`）的名称解析。

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

/// `CRAFTING_FILTER_DEFS` 的一个条目（`recipes_filter.lua`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterIconDef {
    /// 过滤器名（`STRINGS.UI.CRAFTING_FILTERS` 键），如 `TOOLS`。
    pub name: String,
    /// 图标 stem（`filter_tool.tex` → `filter_tool`）。
    pub image: String,
}

/// 解析 `recipes_filter.lua` 的
/// `CRAFTING_FILTER_DEFS = { {name = ..., image = ...}, ... }`，按定义序
/// 返回 `name`/`image` 均为字符串字面量的条目（`image` 为函数引用的
/// CHARACTER 等条目不出现）。同一图标可被多个过滤器复用，调用方自行取舍。
pub fn parse_crafting_filter_defs(source: &str) -> Result<Vec<FilterIconDef>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut visitor = FilterDefsVisitor::default();
    visitor.visit_ast(&ast);
    Ok(visitor.out)
}

#[derive(Default)]
struct FilterDefsVisitor {
    out: Vec<FilterIconDef>,
}

impl Visitor for FilterDefsVisitor {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let mut vars = assignment.variables().iter();
        let mut exprs = assignment.expressions().iter();
        let (Some(ast::Var::Name(name)), Some(ast::Expression::TableConstructor(table))) =
            (vars.next(), exprs.next())
        else {
            return;
        };
        if name.token().to_string() != "CRAFTING_FILTER_DEFS" {
            return;
        }
        for field in table.fields() {
            let ast::Field::NoKey(expr) = field else {
                continue;
            };
            let ast::Expression::TableConstructor(entry) = expr else {
                continue;
            };
            let (Some(name), Some(image)) = (
                table_string_field(entry, "name"),
                table_string_field(entry, "image"),
            ) else {
                continue;
            };
            self.out.push(FilterIconDef {
                name,
                image: strip_tex(&image),
            });
        }
    }
}

/// `PROTOTYPER_DEFS` 的一个条目（`recipes.lua`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrototyperIconDef {
    /// prefab 名（表键），如 `carpentry_station`。
    pub prefab: String,
    /// 图标 stem（`station_carpentry.tex` → `station_carpentry`）。
    pub image: String,
    /// `filter_text` 引用的 `STRINGS.UI.CRAFTING_STATION_FILTERS` 键；
    /// 无该字段或形态不符时为 `None`。
    pub filter_key: Option<String>,
}

/// 解析 `recipes.lua` 的 `PROTOTYPER_DEFS = { <prefab> = {...}, ... }`，按
/// 定义序返回 `icon_image` 为字符串字面量的条目；`filter_text` 仅识别
/// `STRINGS.UI.CRAFTING_STATION_FILTERS.<KEY>` 形态。
pub fn parse_prototyper_defs(source: &str) -> Result<Vec<PrototyperIconDef>> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;
    let mut visitor = PrototyperDefsVisitor::default();
    visitor.visit_ast(&ast);
    Ok(visitor.out)
}

#[derive(Default)]
struct PrototyperDefsVisitor {
    out: Vec<PrototyperIconDef>,
}

impl Visitor for PrototyperDefsVisitor {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let mut vars = assignment.variables().iter();
        let mut exprs = assignment.expressions().iter();
        let (Some(ast::Var::Name(name)), Some(ast::Expression::TableConstructor(table))) =
            (vars.next(), exprs.next())
        else {
            return;
        };
        if name.token().to_string() != "PROTOTYPER_DEFS" {
            return;
        }
        for field in table.fields() {
            let ast::Field::NameKey { key, value, .. } = field else {
                continue;
            };
            let ast::Expression::TableConstructor(entry) = value else {
                continue;
            };
            let Some(image) = table_string_field(entry, "icon_image") else {
                continue;
            };
            let filter_key = table_var_field(entry, "filter_text")
                .as_deref()
                .and_then(station_filter_key);
            self.out.push(PrototyperIconDef {
                prefab: key.token().to_string(),
                image: strip_tex(&image),
                filter_key,
            });
        }
    }
}

/// `STRINGS.UI.CRAFTING_STATION_FILTERS.<KEY>` → `KEY`，其余形态返回 `None`。
fn station_filter_key(path: &[String]) -> Option<String> {
    match path {
        [head, ui, table, key]
            if head == "STRINGS" && ui == "UI" && table == "CRAFTING_STATION_FILTERS" =>
        {
            Some(key.clone())
        }
        _ => None,
    }
}

/// 表中字符串字面量字段的值；缺失或非字符串返回 `None`。
fn table_string_field(table: &ast::TableConstructor, key: &str) -> Option<String> {
    table
        .fields()
        .into_iter()
        .filter_map(|field| match field {
            ast::Field::NameKey {
                key: k,
                value: expr,
                ..
            } if k.token().to_string() == key => string_literal(expr),
            _ => None,
        })
        .next()
}

/// 表中变量引用字段的点路径（`STRINGS.UI.X` → `["STRINGS", "UI", "X"]`）。
fn table_var_field(table: &ast::TableConstructor, key: &str) -> Option<Vec<String>> {
    table
        .fields()
        .into_iter()
        .filter_map(|field| match field {
            ast::Field::NameKey {
                key: k,
                value: ast::Expression::Var(ast::Var::Expression(var_expr)),
                ..
            } if k.token().to_string() == key => var_path(var_expr),
            _ => None,
        })
        .next()
}

/// 图标纹理名去 `.tex` 后缀（其余形态原样返回）。
fn strip_tex(image: &str) -> String {
    image.strip_suffix(".tex").unwrap_or(image).to_string()
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

    #[test]
    fn test_parse_crafting_filter_defs() {
        let source = r#"
CRAFTING_FILTER_DEFS =
{
	{name = "FAVORITES",			atlas = GetCraftingMenuAtlas,	image = "filter_favorites.tex",		custom_pos = true},
	{name = "CRAFTING_STATION",		atlas = GetCraftingMenuAtlas,	image = "filter_none.tex",			custom_pos = true},
	{name = "CHARACTER",			atlas = GetCharacterAtlas,		image = GetCharacterImage,			image_size = 80},
	{name = "TOOLS",				atlas = GetCraftingMenuAtlas,	image = "filter_tool.tex",			},
	{name = "EVERYTHING",			atlas = GetCraftingMenuAtlas,	image = "filter_none.tex",			show_hidden = true},
}

CRAFTING_FILTERS = {}
"#;
        let defs = parse_crafting_filter_defs(source).unwrap();
        // CHARACTER 的 image 是函数引用 → 跳过；保留定义序
        assert_eq!(
            defs,
            vec![
                FilterIconDef {
                    name: "FAVORITES".into(),
                    image: "filter_favorites".into()
                },
                FilterIconDef {
                    name: "CRAFTING_STATION".into(),
                    image: "filter_none".into()
                },
                FilterIconDef {
                    name: "TOOLS".into(),
                    image: "filter_tool".into()
                },
                FilterIconDef {
                    name: "EVERYTHING".into(),
                    image: "filter_none".into()
                },
            ]
        );
    }

    #[test]
    fn test_parse_crafting_filter_defs_missing_is_empty() {
        assert!(parse_crafting_filter_defs("local x = 1")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_parse_prototyper_defs() {
        let source = r#"
PROTOTYPER_DEFS =
{
	none						= {icon_atlas = CRAFTING_ICONS_ATLAS, icon_image = "station_none.tex",				is_crafting_station = false},
	researchlab					= {icon_atlas = CRAFTING_ICONS_ATLAS, icon_image = "station_science.tex",			is_crafting_station = false},
	researchlab2				= {icon_atlas = CRAFTING_ICONS_ATLAS, icon_image = "station_science.tex",			is_crafting_station = false},
	researchlab4				= {icon_atlas = CRAFTING_ICONS_ATLAS, icon_image = "station_arcane.tex",			is_crafting_station = false},
	carpentry_station			= {icon_atlas = CRAFTING_ICONS_ATLAS, icon_image = "station_carpentry.tex",			is_crafting_station = true,		action_str = "OPERATE",		filter_text = STRINGS.UI.CRAFTING_STATION_FILTERS.CARPENTRY},
	perdshrine				    = {icon_atlas = CRAFTING_ICONS_ATLAS, icon_image = "station_perd_offering.tex",		is_crafting_station = true,		filter_text = STRINGS.UI.CRAFTING_STATION_FILTERS.YOT_SHRINE_DOFFERING},
}

PROTOTYPER_DEFS.wargshrine = PROTOTYPER_DEFS.perdshrine
"#;
        let defs = parse_prototyper_defs(source).unwrap();
        assert_eq!(
            defs,
            vec![
                PrototyperIconDef {
                    prefab: "none".into(),
                    image: "station_none".into(),
                    filter_key: None
                },
                PrototyperIconDef {
                    prefab: "researchlab".into(),
                    image: "station_science".into(),
                    filter_key: None
                },
                PrototyperIconDef {
                    prefab: "researchlab2".into(),
                    image: "station_science".into(),
                    filter_key: None
                },
                PrototyperIconDef {
                    prefab: "researchlab4".into(),
                    image: "station_arcane".into(),
                    filter_key: None
                },
                PrototyperIconDef {
                    prefab: "carpentry_station".into(),
                    image: "station_carpentry".into(),
                    filter_key: Some("CARPENTRY".into()),
                },
                PrototyperIconDef {
                    prefab: "perdshrine".into(),
                    image: "station_perd_offering".into(),
                    filter_key: Some("YOT_SHRINE_DOFFERING".into()),
                },
            ]
        );
        // 别名赋值（`PROTOTYPER_DEFS.x = PROTOTYPER_DEFS.y`）不重复出现
        assert!(defs.iter().all(|d| d.prefab != "wargshrine"));
    }

    #[test]
    fn test_parse_prototyper_defs_missing_is_empty() {
        assert!(parse_prototyper_defs("local x = 1").unwrap().is_empty());
    }
}
