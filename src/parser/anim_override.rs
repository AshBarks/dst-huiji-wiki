//! AnimState 符号重映射提取（Tier A：常量字符串参数的直接调用）。
//!
//! 数据文件层面（build.bin / anim.bin）只有同名 symbol 哈希匹配，间接的
//! “改名重映射”只存在于引擎运行时 API、由 Lua 驱动
//! （docs/ANIM_SKIN_PREVIEW_PLAN.md §8.1）。本模块静态提取其中最可信的
//! 一层——receiver 以 `AnimState` 结尾、参数为字符串字面量的调用：
//!
//! ```lua
//! inst.AnimState:OverrideSymbol("swap_hat", "hat_beehive", "swap_hat")
//! owner.AnimState:OverrideSkinSymbol("torso_pelvis", base_skin, "torso")
//! inst.AnimState:ClearOverrideSymbol("swap_hat")
//! ```
//!
//! 变量 / 表达式参数属于动态层（skinner.lua 式的运行时逻辑），此处静默
//! 跳过，留待 Tier B/C 管线。[`SymbolRemapIndex`] 将提取结果按 lowercase
//! symbol 聚合去重，字段名与 `dst-anim-tool` 的 `SymbolOverrideMap`
//! （render.rs）对齐，可直接喂给 `prepare_animation_frames_with_overrides`。

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

/// 提取可信度。当前只有静态常量调用；为未来 Tier B/C 预留。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// 三个参数均为字符串字面量的直接调用。
    Static,
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
    /// `(build, src_symbol, api)` 的多条调用合并、prefab 并集。
    pub fn from_calls(calls: &[SymbolOverrideCall]) -> Self {
        type Group = BTreeMap<(String, String, OverrideApi), BTreeSet<String>>;
        let mut groups: BTreeMap<String, Group> = BTreeMap::new();
        for call in calls {
            let (Some(build), Some(src_symbol)) =
                (call.build.as_deref(), call.src_symbol.as_deref())
            else {
                continue;
            };
            let key = (build.to_lowercase(), src_symbol.to_lowercase(), call.api);
            let prefabs = groups
                .entry(call.symbol.to_lowercase())
                .or_default()
                .entry(key)
                .or_default();
            if let Some(prefab) = &call.prefab {
                prefabs.insert(prefab.clone());
            }
        }
        let symbols = groups
            .into_iter()
            .map(|(symbol, entries)| {
                let entries = entries
                    .into_iter()
                    .map(|((build, src_symbol, api), prefabs)| SymbolRemapEntry {
                        build,
                        src_symbol,
                        api,
                        confidence: Confidence::Static,
                        prefabs: prefabs.into_iter().collect(),
                    })
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
    };
    visitor.visit_ast(&ast);
    Ok(visitor.calls)
}

struct OverrideCallVisitor {
    prefab: Option<String>,
    calls: Vec<SymbolOverrideCall>,
}

impl Visitor for OverrideCallVisitor {
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
        if !receiver_is_anim_state(&suffixes[..suffixes.len() - 1]) {
            return;
        }
        let ast::FunctionArgs::Parentheses { arguments, .. } = method.args() else {
            return;
        };
        let args: Vec<&ast::Expression> = arguments.iter().collect();
        let parsed = match api {
            OverrideApi::ClearOverrideSymbol => {
                (args.len() == 1).then(|| Some((extract_string_expr(args[0])?, None, None)))
            }
            OverrideApi::OverrideSymbol | OverrideApi::OverrideSkinSymbol => (args.len() == 3)
                .then(|| {
                    Some((
                        extract_string_expr(args[0])?,
                        Some(extract_string_expr(args[1])?),
                        Some(extract_string_expr(args[2])?),
                    ))
                }),
        };
        let Some((symbol, build, src_symbol)) = parsed.flatten() else {
            return;
        };
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
        });
    }
}

/// The suffixes before the method call must resolve to `.AnimState` /
/// `["AnimState"]` (`inst.AnimState:OverrideSymbol(...)`,
/// `inst["AnimState"]:OverrideSymbol(...)`, chains like
/// `a.b.AnimState:...` included by only inspecting the last index segment).
fn receiver_is_anim_state(leading: &[&ast::Suffix]) -> bool {
    match leading.last() {
        Some(ast::Suffix::Index(ast::Index::Dot { name, .. })) => {
            name.token().to_string() == "AnimState"
        }
        Some(ast::Suffix::Index(ast::Index::Brackets { expression, .. })) => {
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
            },
            SymbolOverrideCall {
                api: OverrideApi::OverrideSymbol,
                symbol: "SWAP_HAT".to_string(),
                build: Some("hat_beehive".to_string()),
                src_symbol: Some("swap_hat".to_string()),
                prefab: Some("beebox".to_string()),
                start_byte: 0,
                end_byte: 1,
            },
            SymbolOverrideCall {
                api: OverrideApi::OverrideSymbol,
                symbol: "swap_hat".to_string(),
                build: Some("hat_top".to_string()),
                src_symbol: Some("swap_hat".to_string()),
                prefab: Some("tophat".to_string()),
                start_byte: 0,
                end_byte: 1,
            },
            SymbolOverrideCall {
                api: OverrideApi::ClearOverrideSymbol,
                symbol: "swap_hat".to_string(),
                build: None,
                src_symbol: None,
                prefab: Some("beehivehat".to_string()),
                start_byte: 0,
                end_byte: 1,
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
