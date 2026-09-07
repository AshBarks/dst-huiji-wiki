//! AnimState 符号重映射提取（Tier A/B'：常量与可追踪变量的直接调用）。
//!
//! 数据文件层面（build.bin / anim.bin）只有同名 symbol 哈希匹配，间接的
//! “改名重映射”只存在于引擎运行时 API、由 Lua 驱动
//! （docs/ANIM_SKIN_PREVIEW_PLAN.md §8.1）。本模块静态提取其中最可信的
//! 两层：
//!
//! - Tier A（confidence=static）：三个参数均为字符串字面量的直接调用；
//! - Tier C 前哨（confidence=resolved）：参数是字符串常量变量、或字面量/
//!   常量的 `..` 拼接——通过轻量变量追踪解析，调用位置、receiver 以
//!   `AnimState` 结尾（`local as = inst.AnimState` 别名也算）。
//!
//! ```lua
//! inst.AnimState:OverrideSymbol("swap_hat", "hat_beehive", "swap_hat")
//! local skin = "hat_beehive"
//! inst.AnimState:OverrideSymbol("swap_hat", skin, "swap_hat")   -- resolved
//! local as = owner.AnimState
//! as:OverrideSkinSymbol("torso_pelvis", skin, "torso")          -- resolved
//! inst.AnimState:ClearOverrideSymbol("swap_hat")
//! ```
//!
//! 循环变量（`for _, sym in pairs(t) do OverrideSymbol(sym, build, sym) end`）
//! 无法静态枚举 symbol，仍被跳过，留给未来的深度追踪管线。提取结果通过
//! [`SymbolRemapIndex`] 按 lowercase symbol 聚合去重，字段名与
//! `dst-anim-tool` 的 `SymbolOverrideMap`（render.rs）对齐，可直接喂给
//! `prepare_animation_frames_with_overrides`。

use crate::error::{Error, Result};
use full_moon::ast;
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// 产生符号重映射的引擎 API。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum OverrideApi {
    /// `AnimState:OverrideSymbol(sym, build, src_sym)`
    OverrideSymbol,
    /// `AnimState:OverrideSkinSymbol(sym, build, src_sym)`
    OverrideSkinSymbol,
    /// `AnimState:ClearOverrideSymbol(sym)`（负规则，不产生重映射条目）
    ClearOverrideSymbol,
}

/// 提取可信度。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// 三个参数均为字符串字面量的直接调用。
    #[default]
    Static,
    /// 参数含经变量追踪解析的字符串常量 / `..` 拼接（Tier C 前哨）。
    Resolved,
}

/// 一次提取到的重映射调用（原始记录，未聚合）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolOverrideCall {
    pub api: OverrideApi,
    /// 被覆盖的动画 symbol（原样大小写）。
    pub symbol: String,
    /// 覆盖来源 build（Clear 调用为 None）。
    pub build: Option<String>,
    /// 覆盖来源 symbol（Clear 调用为 None）。
    pub src_symbol: Option<String>,
    /// 提取时的上下文标签（通常为 prefab / 文件名）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefab: Option<String>,
    pub confidence: Confidence,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// 一条聚合后的重映射规则。名字统一 lowercase（与 `anim.bin` 元素的
/// `symbol_lower` 及 `dst-anim-tool` 渲染查找一致）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolRemapEntry {
    pub build: String,
    pub src_symbol: String,
    pub api: OverrideApi,
    pub confidence: Confidence,
    /// 观察到该规则的来源上下文（排序去重）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prefabs: Vec<String>,
}

/// 按 lowercase anim symbol 聚合的重映射索引。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SymbolRemapIndex {
    pub symbols: BTreeMap<String, Vec<SymbolRemapEntry>>,
}

impl SymbolRemapIndex {
    /// 从原始调用记录聚合：Clear 调用不产生条目；相同
    /// `(build, src_symbol, api)` 的多条调用合并、prefab 并集、置信度取
    /// 更高（static 优先）。
    pub fn from_calls(calls: &[SymbolOverrideCall]) -> Self {
        type Group = BTreeMap<(String, String, OverrideApi), (Confidence, BTreeSet<String>)>;
        let mut groups: BTreeMap<String, Group> = BTreeMap::new();
        for call in calls {
            let (Some(build), Some(src_symbol)) =
                (call.build.as_deref(), call.src_symbol.as_deref())
            else {
                continue;
            };
            let key = (build.to_lowercase(), src_symbol.to_lowercase(), call.api);
            let group = groups.entry(call.symbol.to_lowercase()).or_default();
            match group.entry(key) {
                std::collections::btree_map::Entry::Vacant(v) => {
                    let mut prefabs = BTreeSet::new();
                    if let Some(prefab) = &call.prefab {
                        prefabs.insert(prefab.clone());
                    }
                    v.insert((call.confidence, prefabs));
                }
                std::collections::btree_map::Entry::Occupied(mut o) => {
                    let slot = o.get_mut();
                    slot.0 = slot.0.min(call.confidence);
                    if let Some(prefab) = &call.prefab {
                        slot.1.insert(prefab.clone());
                    }
                }
            }
        }
        let symbols = groups
            .into_iter()
            .map(|(symbol, entries)| {
                let entries = entries
                    .into_iter()
                    .map(
                        |((build, src_symbol, api), (confidence, prefabs))| SymbolRemapEntry {
                            build,
                            src_symbol,
                            api,
                            confidence,
                            prefabs: prefabs.into_iter().collect(),
                        },
                    )
                    .collect();
                (symbol, entries)
            })
            .collect();
        Self { symbols }
    }

    /// Case-insensitive lookup by anim symbol.
    pub fn get(&self, symbol: &str) -> Option<&[SymbolRemapEntry]> {
        self.symbols.get(&symbol.to_lowercase()).map(Vec::as_slice)
    }

    /// Number of symbols with at least one remap entry.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// 机器可读的索引 JSON（anim-remap-index.json 产物格式）。
    pub fn to_json_string_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(Error::Json)
    }
}

/// Parse all static AnimState symbol override calls from a Lua source.
pub fn parse_anim_overrides(source: &str) -> Result<Vec<SymbolOverrideCall>> {
    parse_anim_overrides_in(source, None)
}

/// Same as [`parse_anim_overrides`], tagging each call with a context label
/// (usually the prefab / file name).
pub fn parse_anim_overrides_in(
    source: &str,
    prefab: Option<&str>,
) -> Result<Vec<SymbolOverrideCall>> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let mut visitor = OverrideCallVisitor {
        prefab: prefab.map(str::to_string),
        calls: Vec::new(),
        consts: BTreeMap::new(),
        anim_state_aliases: BTreeSet::new(),
    };
    visitor.visit_ast(&ast);
    Ok(visitor.calls)
}

struct OverrideCallVisitor {
    prefab: Option<String>,
    calls: Vec<SymbolOverrideCall>,
    /// 局部字符串常量（`local x = "s"` / `x = "s"`），文件级近似、按出现顺序
    /// 覆盖（后写优先），无块作用域区分。
    consts: BTreeMap<String, String>,
    /// 绑定到 `X.AnimState` 的局部变量（`local as = inst.AnimState`）。
    anim_state_aliases: BTreeSet<String>,
}

impl Visitor for OverrideCallVisitor {
    fn visit_stmt(&mut self, stmt: &ast::Stmt) {
        let (names, exprs): (Vec<String>, Vec<&ast::Expression>) = match stmt {
            ast::Stmt::LocalAssignment(assign) => (
                assign
                    .names()
                    .iter()
                    .map(|n| n.token().to_string())
                    .collect(),
                assign.expressions().iter().collect(),
            ),
            ast::Stmt::Assignment(assign) => (
                assign
                    .variables()
                    .iter()
                    .filter_map(|v| match v {
                        ast::Var::Name(name) => Some(name.token().to_string()),
                        _ => None,
                    })
                    .collect(),
                assign.expressions().iter().collect(),
            ),
            _ => return,
        };
        for (name, expr) in names.into_iter().zip(exprs) {
            if let Some((value, _)) = self.resolve_arg(expr) {
                self.consts.insert(name, value);
            } else if is_anim_state_binding(expr) {
                self.anim_state_aliases.insert(name);
            }
        }
    }

    fn visit_function_call(&mut self, call: &ast::FunctionCall) {
        let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::MethodCall(method))) = suffixes.last().copied()
        else {
            return;
        };
        let api = match method.name().token().to_string().as_str() {
            "OverrideSymbol" => OverrideApi::OverrideSymbol,
            "OverrideSkinSymbol" => OverrideApi::OverrideSkinSymbol,
            "ClearOverrideSymbol" => OverrideApi::ClearOverrideSymbol,
            _ => return,
        };
        if !self.receiver_is_anim_state(call.prefix(), &suffixes[..suffixes.len() - 1]) {
            return;
        }
        let ast::FunctionArgs::Parentheses { arguments, .. } = method.args() else {
            return;
        };
        let args: Vec<&ast::Expression> = arguments.iter().collect();
        let needed = match api {
            OverrideApi::ClearOverrideSymbol => 1,
            OverrideApi::OverrideSymbol | OverrideApi::OverrideSkinSymbol => 3,
        };
        if args.len() != needed {
            return;
        }
        let mut any_tracked = false;
        let mut resolved = Vec::with_capacity(args.len());
        for arg in &args {
            let Some((value, tracked)) = self.resolve_arg(arg) else {
                return;
            };
            any_tracked |= tracked;
            resolved.push(value);
        }
        let parsed = match api {
            OverrideApi::ClearOverrideSymbol => (resolved.remove(0), None, None),
            _ => {
                let build = resolved.remove(1);
                let src = resolved.remove(1);
                (resolved.remove(0), Some(build), Some(src))
            }
        };
        let (symbol, build, src_symbol) = parsed;
        if symbol.is_empty() {
            return;
        }
        let Some((start, end)) = call.range() else {
            return;
        };
        self.calls.push(SymbolOverrideCall {
            api,
            symbol,
            build,
            src_symbol,
            prefab: self.prefab.clone(),
            start_byte: start.bytes(),
            end_byte: end.bytes(),
            confidence: if any_tracked {
                Confidence::Resolved
            } else {
                Confidence::Static
            },
        });
    }
}

impl OverrideCallVisitor {
    fn receiver_is_anim_state(&self, prefix: &ast::Prefix, leading: &[&ast::Suffix]) -> bool {
        match leading.last() {
            Some(suffix) => index_is_anim_state(suffix),
            // 无索引段：`as:OverrideSymbol(...)`，receiver 是别名变量。
            None => {
                matches!(prefix, ast::Prefix::Name(name) if self.anim_state_aliases.contains(&name.token().to_string()))
            }
        }
    }

    /// 解析一个参数为字符串值；`tracked` 表示值来自变量追踪（常量变量、
    /// 含变量的 `..` 拼接折叠或 `or` 回退）。`a or b` 取能解析的一侧
    /// （预览场景下的回退值近似，游戏语义为 nil 时取 b）。
    fn resolve_arg(&self, expr: &ast::Expression) -> Option<(String, bool)> {
        match expr {
            ast::Expression::String(token) => {
                Some((extract_string_literal(&token.to_string()), false))
            }
            ast::Expression::Parentheses { expression, .. } => self.resolve_arg(expression),
            ast::Expression::Var(ast::Var::Name(name)) => {
                let name = name.token().to_string();
                self.consts.get(&name).map(|v| (v.clone(), true))
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op = binop.to_string().trim().to_string();
                if op == ".." {
                    let (l, lt) = self.resolve_arg(lhs)?;
                    let (r, rt) = self.resolve_arg(rhs)?;
                    Some((format!("{l}{r}"), lt || rt))
                } else if op == "or" {
                    // or 折叠本身是回退值近似，结果一律标记 tracked。
                    self.resolve_arg(lhs)
                        .or_else(|| self.resolve_arg(rhs))
                        .map(|(v, _)| (v, true))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// `x.AnimState` / `x["AnimState"]` / 链式索引结尾为 AnimState 的表达式
/// （用于识别 `local as = inst.AnimState` 绑定）。
fn is_anim_state_binding(expr: &ast::Expression) -> bool {
    match expr {
        ast::Expression::Var(ast::Var::Expression(vex)) => {
            vex.suffixes().last().is_some_and(index_is_anim_state)
        }
        _ => false,
    }
}

fn index_is_anim_state(suffix: &ast::Suffix) -> bool {
    match suffix {
        ast::Suffix::Index(ast::Index::Dot { name, .. }) => name.token().to_string() == "AnimState",
        ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
            extract_string_expr(expression).as_deref() == Some("AnimState")
        }
        _ => false,
    }
}

fn extract_string_expr(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(token) => Some(extract_string_literal(&token.to_string())),
        _ => None,
    }
}

fn extract_string_literal(raw: &str) -> String {
    let s = raw.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        unescape(&s[1..s.len() - 1])
    } else if let Some(inner) = s.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
        inner.to_string()
    } else {
        s.to_string()
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_basic_override_symbol() {
        let source = r#"
local function onequip(inst)
    inst.AnimState:OverrideSymbol("swap_hat", "hat_beehive", "swap_hat")
end
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.api, OverrideApi::OverrideSymbol);
        assert_eq!(call.symbol, "swap_hat");
        assert_eq!(call.build.as_deref(), Some("hat_beehive"));
        assert_eq!(call.src_symbol.as_deref(), Some("swap_hat"));
        assert_eq!(call.prefab, None);
        assert!(call.start_byte < call.end_byte);
        assert!(source[call.start_byte..call.end_byte].contains("OverrideSymbol"));
    }

    #[test]
    fn extracts_skin_and_clear_calls() {
        let source = r#"
inst.AnimState:OverrideSkinSymbol("torso_pelvis", "wilson_ice", "torso")
inst.AnimState:ClearOverrideSymbol("swap_hat")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].api, OverrideApi::OverrideSkinSymbol);
        assert_eq!(calls[0].symbol, "torso_pelvis");
        assert_eq!(calls[0].build.as_deref(), Some("wilson_ice"));
        assert_eq!(calls[0].src_symbol.as_deref(), Some("torso"));
        assert_eq!(calls[1].api, OverrideApi::ClearOverrideSymbol);
        assert_eq!(calls[1].symbol, "swap_hat");
        assert_eq!(calls[1].build, None);
        assert_eq!(calls[1].src_symbol, None);
    }

    #[test]
    fn accepts_various_receivers() {
        let source = r#"
self.AnimState:OverrideSymbol("a", "build_a", "a")
owner.AnimState:OverrideSymbol("b", "build_b", "b")
inst["AnimState"]:OverrideSymbol("c", "build_c", "c")
ents.something.AnimState:OverrideSymbol("d", "build_d", "d")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        let symbols: Vec<&str> = calls.iter().map(|c| c.symbol.as_str()).collect();
        assert_eq!(symbols, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn ignores_non_anim_state_receiver() {
        let source = r#"
foo:OverrideSymbol("a", "build_a", "a")
inst.AnimStateOverride("x", "y", "z")
AnimState:OverrideSymbol("q", "r", "s")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn ignores_dynamic_args_and_wrong_arity() {
        let source = r#"
inst.AnimState:OverrideSymbol(sym, "wonkey", sym)
inst.AnimState:OverrideSymbol("swap_hat", build, "swap_hat")
inst.AnimState:OverrideSymbol("swap_hat", "hat_beehive")
inst.AnimState:ClearOverrideSymbol()
inst.AnimState:ClearOverrideSymbol("a", "b", "c")
local v = "swap_hat"
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn ignores_other_anim_state_methods() {
        let source = r#"
inst.AnimState:SetBuild("wilson")
inst.AnimState:Hide("swap_hat")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn attaches_prefab_context() {
        let source = r#"inst.AnimState:OverrideSymbol("swap_object", "swap_axe", "swap_object")"#;
        let calls = parse_anim_overrides_in(source, Some("axe")).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].prefab.as_deref(), Some("axe"));
    }

    #[test]
    fn finds_calls_nested_in_functions_and_tables() {
        let source = r#"
local assets = {}
local function fn()
    if TheWorld.ismastersim then
        for i = 1, 3 do
            inst.AnimState:OverrideSymbol("swap_hat", "hat_top" .. i, "swap_hat")
        end
        inst.AnimState:OverrideSymbol("swap_face", "hat_top", "swap_face")
    end
end
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].symbol, "swap_face");
    }

    #[test]
    fn handles_string_variants() {
        let source = r#"
inst.AnimState:OverrideSymbol('swap_hat', "hat_beehive", 'swap_hat')
inst.AnimState:OverrideSymbol([[swap_body]], "armor_grass", [[swap_body]])
inst.AnimState:OverrideSymbol("esc\\ape", "build", "sym")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].symbol, "swap_hat");
        assert_eq!(calls[1].symbol, "swap_body");
        assert_eq!(calls[2].symbol, "esc\\ape");
    }

    #[test]
    fn index_groups_and_dedupes() {
        let calls = vec![
            SymbolOverrideCall {
                api: OverrideApi::OverrideSymbol,
                symbol: "swap_hat".to_string(),
                build: Some("hat_beehive".to_string()),
                src_symbol: Some("swap_hat".to_string()),
                prefab: Some("beehivehat".to_string()),
                start_byte: 0,
                end_byte: 1,
                confidence: Confidence::Static,
            },
            SymbolOverrideCall {
                api: OverrideApi::OverrideSymbol,
                symbol: "SWAP_HAT".to_string(),
                build: Some("hat_beehive".to_string()),
                src_symbol: Some("swap_hat".to_string()),
                prefab: Some("beebox".to_string()),
                start_byte: 0,
                end_byte: 1,
                confidence: Confidence::Static,
            },
            SymbolOverrideCall {
                api: OverrideApi::OverrideSymbol,
                symbol: "swap_hat".to_string(),
                build: Some("hat_top".to_string()),
                src_symbol: Some("swap_hat".to_string()),
                prefab: Some("tophat".to_string()),
                start_byte: 0,
                end_byte: 1,
                confidence: Confidence::Static,
            },
            SymbolOverrideCall {
                api: OverrideApi::ClearOverrideSymbol,
                symbol: "swap_hat".to_string(),
                build: None,
                src_symbol: None,
                prefab: Some("beehivehat".to_string()),
                start_byte: 0,
                end_byte: 1,
                confidence: Confidence::Static,
            },
        ];
        let index = SymbolRemapIndex::from_calls(&calls);
        assert_eq!(index.len(), 1);
        let entries = index.get("swap_hat").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].build, "hat_beehive");
        assert_eq!(entries[0].prefabs, vec!["beebox", "beehivehat"]);
        assert_eq!(entries[0].confidence, Confidence::Static);
        assert_eq!(entries[1].build, "hat_top");
        assert!(index.get("swap_face").is_none());
    }

    #[test]
    fn index_json_shape() {
        let source = r#"inst.AnimState:OverrideSymbol("swap_hat", "hat_beehive", "swap_hat")"#;
        let calls = parse_anim_overrides_in(source, Some("beehivehat")).unwrap();
        let index = SymbolRemapIndex::from_calls(&calls);
        let value: serde_json::Value =
            serde_json::from_str(&index.to_json_string_pretty().unwrap()).unwrap();
        let entry = &value["symbols"]["swap_hat"][0];
        assert_eq!(entry["build"], "hat_beehive");
        assert_eq!(entry["src_symbol"], "swap_hat");
        assert_eq!(entry["api"], "OverrideSymbol");
        assert_eq!(entry["confidence"], "static");
        assert_eq!(entry["prefabs"][0], "beehivehat");
    }

    #[test]
    fn tracks_local_string_constants() {
        let source = r#"
local build = "hat_beehive"
local function onequip(inst)
    inst.AnimState:OverrideSymbol("swap_hat", build, "swap_hat")
end
build = "hat_top"
inst.AnimState:OverrideSymbol("swap_hat", build, "swap_hat")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].build.as_deref(), Some("hat_beehive"));
        assert_eq!(calls[0].confidence, Confidence::Resolved);
        // 后写优先：重新赋值后取新值。
        assert_eq!(calls[1].build.as_deref(), Some("hat_top"));
        assert_eq!(calls[1].confidence, Confidence::Resolved);
    }

    #[test]
    fn tracks_anim_state_alias_receiver() {
        let source = r#"
local as = inst.AnimState
as:OverrideSymbol("swap_hat", "hat_beehive", "swap_hat")
owner.AnimState:OverrideSymbol("a", "b", "c")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].symbol, "swap_hat");
        assert_eq!(calls[0].confidence, Confidence::Static);
    }

    #[test]
    fn folds_string_concat_args() {
        let source = r#"
local pre = "swap"
inst.AnimState:OverrideSymbol(pre .. "_hat", "hat_" .. "beehive", pre .. "_hat")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].symbol, "swap_hat");
        assert_eq!(calls[0].build.as_deref(), Some("hat_beehive"));
        assert_eq!(calls[0].src_symbol.as_deref(), Some("swap_hat"));
        assert_eq!(calls[0].confidence, Confidence::Resolved);
    }

    #[test]
    fn folds_or_fallback_args_and_consts() {
        let source = r#"
local build = data.build or "hermitcrab_tea"
inst.AnimState:OverrideSymbol("tea_bottle", build, "tea_bottle")
inst.AnimState:OverrideSymbol("swap_object", "swap_axe", swap_symbol or "swap_axe")
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].build.as_deref(), Some("hermitcrab_tea"));
        assert_eq!(calls[0].confidence, Confidence::Resolved);
        assert_eq!(calls[1].src_symbol.as_deref(), Some("swap_axe"));
        assert_eq!(calls[1].confidence, Confidence::Resolved);
    }

    #[test]
    fn skips_unresolvable_loop_variables() {
        let source = r#"
for _, sym in pairs(symbols) do
    inst.AnimState:OverrideSymbol(sym, "wonkey", sym)
end
inst.AnimState:OverrideSymbol(get_symbol(), "wonkey", get_symbol())
"#;
        let calls = parse_anim_overrides(source).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn merges_confidence_preferring_static() {
        let mk = |build: &str, confidence: Confidence| SymbolOverrideCall {
            api: OverrideApi::OverrideSymbol,
            symbol: "swap_hat".to_string(),
            build: Some(build.to_string()),
            src_symbol: Some("swap_hat".to_string()),
            prefab: Some("p".to_string()),
            start_byte: 0,
            end_byte: 1,
            confidence,
        };
        let index = SymbolRemapIndex::from_calls(&[
            mk("hat_a", Confidence::Resolved),
            mk("hat_a", Confidence::Static),
        ]);
        assert_eq!(
            index.get("swap_hat").unwrap()[0].confidence,
            Confidence::Static
        );
    }

    #[test]
    fn empty_source_yields_empty_index() {
        let calls = parse_anim_overrides("").unwrap();
        assert!(calls.is_empty());
        assert!(SymbolRemapIndex::from_calls(&calls).is_empty());
    }

    #[test]
    fn parse_error_propagates() {
        assert!(parse_anim_overrides("inst.AnimState:OverrideSymbol(").is_err());
    }
}
