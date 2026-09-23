//! clothing.lua 数据表提取（Tier B：数据驱动的 symbol 改名映射）。
//!
//! `clothing.lua`（脚本根目录，自动生成）定义 `CLOTHING = { name = {...} }`，
//! 字段语义对应 `components/skinner.lua` 的运行时逻辑：
//!
//! - `symbol_overrides`：该衣物 build 覆盖的动画 symbol 列表（同名取图）；
//! - `symbol_overrides_by_character`：每角色改名表
//!   `{ default = { leg = "leg_wilson", }, walter = { leg = "leg_walter", }, }`，
//!   运行时取 `by_character[prefab] or by_character["default"]`；
//! - `symbol_overrides_<variant>`（skinny/mighty/stage2/stage3/stage4/
//!   powerup/old）：按形态（Wolfgang/Wormwood/Wurt/Wanda）生效的改名表，
//!   只重命名已覆盖 symbol，不扩大覆盖集合；
//! - `build_name_override`：build 名缺省为衣物名（`GetBuildForItem`，
//!   skinsutils.lua:131）。
//!
//! [`ClothingEntry::resolve_overrides`] 复刻 skinner.lua:199-238 的合成规则：
//! symbols = symbol_overrides ∪ keys(by_character 选中的表)；src = 形态表
//! [sym] or sym，再被 by_character 表[sym] or src 覆盖。

use super::string_literal_text;
use crate::error::{Error, Result};
use full_moon::ast;
use full_moon::tokenizer::TokenReference;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
/// 一条 `CLOTHING[name]` 定义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct ClothingEntry {
    /// body / hand / legs / feet / ...
    pub clothing_type: Option<String>,
    /// 缺省 build 名为衣物名本身（skinsutils GetBuildForItem）。
    pub build_name_override: Option<String>,
    /// 覆盖的动画 symbol 列表（同名取图）。
    pub symbol_overrides: Vec<String>,
    /// character（或 `default`）→ anim symbol → 源 symbol。
    pub symbol_overrides_by_character: BTreeMap<String, BTreeMap<String, String>>,
    /// 形态改名表：`symbol_overrides_skinny` → `skinny` 等后缀 → sym → src。
    pub skintype_overrides: BTreeMap<String, BTreeMap<String, String>>,
    /// 隐藏的 symbol。
    pub symbol_hides: Vec<String>,
    /// 请求回退到 base build 的 symbol。
    pub base_fallbacks: Vec<String>,
}

/// 一件衣物按角色/形态解析后的覆盖规则。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedClothingOverride {
    /// 该衣物提供符号的 build（`GetBuildForItem(name)`）。
    pub build: String,
    /// anim symbol → 源 symbol（小写）。
    pub overrides: BTreeMap<String, String>,
}

impl ClothingEntry {
    /// skinner.lua 合成规则。`character` 缺省回退 `default` 表；
    /// `skintype` 是 `symbol_overrides_<variant>` 的后缀（如 `mighty`）。
    pub fn resolve_overrides(
        &self,
        name: &str,
        character: Option<&str>,
        skintype: Option<&str>,
    ) -> ResolvedClothingOverride {
        let alt = match character {
            Some(c) => self
                .symbol_overrides_by_character
                .get(&c.to_lowercase())
                .or_else(|| self.symbol_overrides_by_character.get("default")),
            None => self.symbol_overrides_by_character.get("default"),
        };
        let base_src = skintype.and_then(|v| self.skintype_overrides.get(v));

        let mut overrides = BTreeMap::new();
        let mut symbols: BTreeSet<String> = self
            .symbol_overrides
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
        if let Some(alt) = alt {
            symbols.extend(alt.keys().map(|s| s.to_lowercase()));
        }
        for sym in symbols {
            let mut src = sym.clone();
            if let Some(base_src) = base_src {
                if let Some(s) = base_src.get(&sym) {
                    src = s.to_lowercase();
                }
            }
            if let Some(s) = alt.and_then(|a| a.get(&sym)) {
                src = s.to_lowercase();
            }
            overrides.insert(sym, src);
        }
        let build = self
            .build_name_override
            .clone()
            .unwrap_or_else(|| name.to_string());
        ResolvedClothingOverride { build, overrides }
    }
}

/// 解析 `CLOTHING = { name = {...}, ... }` 数据表。
pub fn parse_clothing_overrides(source: &str) -> Result<BTreeMap<String, ClothingEntry>> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let mut out = BTreeMap::new();
    for stmt in ast.nodes().stmts() {
        // `CLOTHING = {...}`（也可能是 `local CLOTHING = {...}`）。
        let (var_name, expr) = match stmt {
            ast::Stmt::Assignment(assign) => {
                let Some(ast::Var::Name(name)) = assign.variables().iter().next() else {
                    continue;
                };
                (name.token().to_string(), assign.expressions().iter().next())
            }
            ast::Stmt::LocalAssignment(assign) => {
                let Some(name) = assign.names().iter().next() else {
                    continue;
                };
                (name.token().to_string(), assign.expressions().iter().next())
            }
            _ => continue,
        };
        if var_name != "CLOTHING" {
            continue;
        }
        let Some(Some(ast::Expression::TableConstructor(table))) = expr.map(Some) else {
            continue;
        };
        for field in table.fields() {
            let ast::Field::NameKey { key, value, .. } = field else {
                continue;
            };
            let Some(name) = field_key_name(key) else {
                continue;
            };
            let Some(entry) = table_expr(value) else {
                continue;
            };
            out.insert(name.to_lowercase(), parse_entry(entry));
        }
    }
    Ok(out)
}

fn parse_entry(table: &ast::TableConstructor) -> ClothingEntry {
    let mut entry = ClothingEntry {
        clothing_type: None,
        build_name_override: None,
        symbol_overrides: Vec::new(),
        symbol_overrides_by_character: BTreeMap::new(),
        skintype_overrides: BTreeMap::new(),
        symbol_hides: Vec::new(),
        base_fallbacks: Vec::new(),
    };
    for field in table.fields() {
        let ast::Field::NameKey { key, value, .. } = field else {
            continue;
        };
        let Some(key) = field_key_name(key) else {
            continue;
        };
        match key.as_str() {
            "type" => entry.clothing_type = string_expr(value),
            "build_name_override" => entry.build_name_override = string_expr(value),
            "symbol_overrides" | "symbol_hides" | "base_fallbacks" => {
                if let Some(table) = table_expr(value) {
                    let list: Vec<String> = table
                        .fields()
                        .iter()
                        .filter_map(|f| match f {
                            ast::Field::NoKey(expr) => string_expr(expr),
                            _ => None,
                        })
                        .collect();
                    match key.as_str() {
                        "symbol_overrides" => entry.symbol_overrides = list,
                        "symbol_hides" => entry.symbol_hides = list,
                        _ => entry.base_fallbacks = list,
                    }
                }
            }
            "symbol_overrides_by_character" => {
                if let Some(chars) = table_expr(value) {
                    for field in chars.fields() {
                        let ast::Field::NameKey { key, value, .. } = field else {
                            continue;
                        };
                        let Some(character) = field_key_name(key) else {
                            continue;
                        };
                        let Some(map) = table_expr(value) else {
                            continue;
                        };
                        entry
                            .symbol_overrides_by_character
                            .insert(character.to_lowercase(), parse_symbol_map(map));
                    }
                }
            }
            other => {
                if let Some(variant) = other.strip_prefix("symbol_overrides_") {
                    if let Some(map) = table_expr(value) {
                        entry
                            .skintype_overrides
                            .insert(variant.to_lowercase(), parse_symbol_map(map));
                    }
                }
            }
        }
    }
    entry
}

/// `{ sym = "src", ... }` 改名表；数组项视为 `sym = sym`（identity）。
fn parse_symbol_map(table: &ast::TableConstructor) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for field in table.fields() {
        match field {
            ast::Field::NameKey { key, value, .. } => {
                if let Some(sym) = field_key_name(key) {
                    let src = string_expr(value).unwrap_or_else(|| sym.to_lowercase());
                    out.insert(sym.to_lowercase(), src.to_lowercase());
                }
            }
            ast::Field::NoKey(expr) => {
                if let Some(sym) = string_expr(expr) {
                    out.insert(sym.to_lowercase(), sym.to_lowercase());
                }
            }
            _ => {}
        }
    }
    out
}

fn field_key_name(key: &TokenReference) -> Option<String> {
    let name = key.token().to_string();
    (!name.is_empty()).then_some(name)
}

fn table_expr(expr: &ast::Expression) -> Option<&ast::TableConstructor> {
    match expr {
        ast::Expression::TableConstructor(t) => Some(t),
        ast::Expression::Parentheses { expression, .. } => table_expr(expression),
        _ => None,
    }
}

fn string_expr(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(token) => Some(string_literal_text(&token.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
CLOTHING =
{
	body_onepiece3_beach =
	{
		type = "body",
		skin_tags = { "SEASIDE", "CLOTHING_BODY", },
		symbol_overrides = { "arm_upper", "leg", "torso", "torso_pelvis", },
		symbol_overrides_by_character = { default = { leg = "leg_wilson", }, walter = { leg = "leg_walter", }, },
		symbol_hides = { "skirt", },
		torso_tuck = "untucked",
	},
	body_catcoon_costume =
	{
		type = "body",
		symbol_overrides = { "torso", "arm_upper", },
		build_name_override = "body_catcoon_costume",
		symbol_overrides_mighty = { arm_upper = "arm_upper_skin", torso = "torso_skin", },
	},
	hand_willow_gladiator =
	{
		type = "hand",
		symbol_overrides = { "hand", },
		base_fallbacks = { "arm_upper", },
	},
}
"#;

    #[test]
    fn parses_entries() {
        let index = parse_clothing_overrides(SAMPLE).unwrap();
        assert_eq!(index.len(), 3);
        let e = &index["body_onepiece3_beach"];
        assert_eq!(e.clothing_type.as_deref(), Some("body"));
        assert_eq!(
            e.symbol_overrides,
            vec!["arm_upper", "leg", "torso", "torso_pelvis"]
        );
        assert_eq!(e.symbol_overrides_by_character.len(), 2);
        assert_eq!(
            e.symbol_overrides_by_character["walter"]["leg"],
            "leg_walter"
        );
        assert_eq!(e.symbol_hides, vec!["skirt"]);
        assert_eq!(e.build_name_override, None);
    }

    #[test]
    fn parses_skintype_and_build_override() {
        let index = parse_clothing_overrides(SAMPLE).unwrap();
        let e = &index["body_catcoon_costume"];
        assert_eq!(
            e.build_name_override.as_deref(),
            Some("body_catcoon_costume")
        );
        assert_eq!(
            e.skintype_overrides["mighty"]["arm_upper"],
            "arm_upper_skin"
        );
    }

    #[test]
    fn resolves_identity_overrides() {
        let index = parse_clothing_overrides(SAMPLE).unwrap();
        let r = index["body_onepiece3_beach"].resolve_overrides(
            "body_onepiece3_beach",
            Some("walter"),
            None,
        );
        assert_eq!(r.build, "body_onepiece3_beach");
        assert_eq!(r.overrides["leg"], "leg_walter");
        assert_eq!(r.overrides["torso"], "torso");
        assert_eq!(r.overrides["torso_pelvis"], "torso_pelvis");
    }

    #[test]
    fn resolves_default_character_fallback() {
        let index = parse_clothing_overrides(SAMPLE).unwrap();
        let r = index["body_onepiece3_beach"].resolve_overrides(
            "body_onepiece3_beach",
            Some("wx78"),
            None,
        );
        assert_eq!(r.overrides["leg"], "leg_wilson");
    }

    #[test]
    fn resolves_skintype_rename_only() {
        let index = parse_clothing_overrides(SAMPLE).unwrap();
        let e = &index["body_catcoon_costume"];
        let r = e.resolve_overrides("body_catcoon_costume", Some("wolfgang"), Some("mighty"));
        assert_eq!(r.build, "body_catcoon_costume");
        // 形态表只改名，不扩大覆盖集合。
        assert_eq!(r.overrides["arm_upper"], "arm_upper_skin");
        assert_eq!(r.overrides["torso"], "torso_skin");
        assert_eq!(r.overrides.get("torso_pelvis"), None);
    }

    #[test]
    fn resolves_without_character() {
        let index = parse_clothing_overrides(SAMPLE).unwrap();
        let r = index["body_onepiece3_beach"].resolve_overrides("body_onepiece3_beach", None, None);
        // 无角色时回退 default 表（skinner 的 `or by_character["default"]`）。
        assert_eq!(r.overrides["leg"], "leg_wilson");
        assert_eq!(r.overrides["arm_upper"], "arm_upper");
    }

    #[test]
    fn ignores_other_tables() {
        let source = r#"
CLOTHING = {}
SOMETHING_ELSE = { a = { type = "body" } }
local OTHER = { b = {} }
"#;
        let index = parse_clothing_overrides(source).unwrap();
        assert!(index.is_empty());
    }

    #[test]
    fn parse_error_propagates() {
        assert!(parse_clothing_overrides("CLOTHING = {").is_err());
    }
}
