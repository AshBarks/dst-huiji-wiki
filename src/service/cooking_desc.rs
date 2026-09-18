//! Render a compiled recipe `test` AST into the wiki-style requirement text
//! (烹饪料理描述表的「属性要求 / 特殊要求」两列)。
//!
//! 规则与 [`crate::service::cooking_eval`] 的求值契约一致：裸 `names`/`tags`
//! 字段为真 ⇔ 聚合值非 nil（非负即 > 0），`(x or 0) + …` 是食材计数求和。
//! 未知 tag、未知食材 key、无法渲染的节点一律报错——游戏更新引入新形态时
//! 必须显式暴露，而不是静默降级导出文案。

use crate::error::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;

/// 食物属性 tag → 中文「X度」。覆盖当前全部食谱 test 引用的 13 个 tag；
/// 新 tag 出现时导出会在 [`tag_zh`] 处显式报错，补表即可。
fn tag_zh(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "meat" => "肉度",
        "veggie" => "蔬菜度",
        "fruit" => "水果度",
        "inedible" => "不可食用度",
        "egg" => "蛋度",
        "fish" => "鱼度",
        "sweetener" => "甜味剂度",
        "monster" => "怪物度",
        "seed" => "种子度",
        "dairy" => "乳制品度",
        "frozen" => "冰度",
        "magic" => "魔法度",
        "fat" => "油脂度",
        _ => return None,
    })
}

fn cmp_zh(op: &str) -> Option<&'static str> {
    Some(match op {
        "gt" => ">",
        "ge" => "≥",
        "lt" => "<",
        "le" => "≤",
        "eq" => "=",
        "ne" => "≠",
        _ => return None,
    })
}

fn flip(op: &str) -> &str {
    match op {
        "gt" => "lt",
        "lt" => "gt",
        "ge" => "le",
        "le" => "ge",
        _ => op,
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Attr,
    Special,
    /// `return true`：两列均不贡献内容。
    Noop,
}

#[derive(Debug, Clone)]
struct Item {
    kind: Kind,
    text: String,
    /// 单字段约束时记 `t:<tag>` / `n:<key>`，用于把裸字段的 `> 0`/`≥ 1`
    /// 吸收进同字段的显式比较式。
    key: Option<String>,
    bare: bool,
}

/// 一个食谱渲染出的两列描述（wiki 无该列内容时为空串）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecipeDesc {
    /// 属性要求：tag 约束合取，`，` 分隔。
    pub attrs: String,
    /// 特殊要求：食材计数（及 cooker 限定）合取，`；` 分隔。
    pub specials: String,
}

/// 渲染一个编译后的食谱 test。`ingredient_names` 需同时以别名 key 与
/// prefab 索引到「特殊要求」列使用的显示名（中文优先）。
pub fn describe_recipe(
    recipe: &str,
    test: &Value,
    ingredient_names: &HashMap<String, String>,
) -> Result<RecipeDesc> {
    let ctx = Ctx {
        recipe,
        names: ingredient_names,
    };
    let mut conjuncts = Vec::new();
    flatten_and(test, &mut conjuncts);
    let items = drop_bare(
        conjuncts
            .iter()
            .map(|node| render_conjunct(&ctx, node, 0))
            .collect::<Result<Vec<_>>>()?,
    );
    let mut attrs = Vec::new();
    let mut specials = Vec::new();
    for item in items {
        match item.kind {
            Kind::Attr => attrs.push(item.text),
            Kind::Special => specials.push(item.text),
            Kind::Noop => {}
        }
    }
    Ok(RecipeDesc {
        attrs: attrs.join("，"),
        specials: specials.join("；"),
    })
}

struct Ctx<'a> {
    recipe: &'a str,
    names: &'a HashMap<String, String>,
}

impl Ctx<'_> {
    fn tag(&self, tag: &str) -> Result<&'static str> {
        tag_zh(tag).ok_or_else(|| {
            Error::ParseError(format!(
                "料理描述：食谱 `{}` 引用未知食材属性 tag `{tag}`，请补充 cooking_desc::tag_zh 对照表",
                self.recipe
            ))
        })
    }

    fn ingredient(&self, key: &str) -> Result<String> {
        self.names.get(key).cloned().ok_or_else(|| {
            Error::ParseError(format!(
                "料理描述：食谱 `{}` 引用未知食材 key `{key}`（检查 PO 名称映射）",
                self.recipe
            ))
        })
    }

    fn unsupported(&self, node: &Value) -> Error {
        Error::ParseError(format!(
            "料理描述：食谱 `{}` 存在无法渲染的表达式节点 {}",
            self.recipe,
            truncate(&node.to_string())
        ))
    }
}

fn truncate(s: &str) -> String {
    let mut out: String = s.chars().take(160).collect();
    if out.chars().count() < s.chars().count() {
        out.push_str("...");
    }
    out
}

fn arr(node: &Value) -> Option<&[Value]> {
    node.as_array().map(|v| v.as_slice())
}

fn op(node: &Value) -> Option<&str> {
    arr(node)?.first().and_then(Value::as_str)
}

fn is_field(node: &Value, base: &str) -> bool {
    arr(node).is_some_and(|a| {
        a.len() == 3
            && a[0].as_str() == Some("field")
            && arr(&a[1]).is_some_and(|b| {
                b.len() == 2 && b[0].as_str() == Some("var") && b[1].as_str() == Some(base)
            })
    })
}

fn field_name(node: &Value) -> &str {
    arr(node)
        .and_then(|a| a.get(2))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn num_value(node: &Value) -> Option<f64> {
    let a = arr(node)?;
    (a.len() == 2 && a[0].as_str()? == "num").then(|| a[1].as_f64())?
}

fn str_value(node: &Value) -> Option<&str> {
    let a = arr(node)?;
    if a.len() == 2 && a[0].as_str()? == "str" {
        a[1].as_str()
    } else {
        None
    }
}

fn is_var(node: &Value, name: &str) -> bool {
    arr(node).is_some_and(|a| {
        a.len() == 2 && a[0].as_str() == Some("var") && a[1].as_str() == Some(name)
    })
}

fn flatten_and<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
    if op(node) == Some("and") {
        let a = arr(node).expect("and op");
        flatten_and(&a[1], out);
        flatten_and(&a[2], out);
    } else {
        out.push(node);
    }
}

fn or_branches<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
    if op(node) == Some("or") {
        let a = arr(node).expect("or op");
        or_branches(&a[1], out);
        or_branches(&a[2], out);
    } else {
        out.push(node);
    }
}

/// `(names.a or 0) + (names.b or 0) + …` → `["a","b"]`；单个裸 names 字段也接受。
fn name_sum(node: &Value) -> Option<Vec<String>> {
    match op(node)? {
        "add" => {
            let a = arr(node).expect("add op");
            let mut l = name_sum(&a[1])?;
            l.extend(name_sum(&a[2])?);
            Some(l)
        }
        "or" => {
            let a = arr(node).expect("or op");
            if a.len() == 3 && is_field(&a[1], "names") && num_value(&a[2]) == Some(0.0) {
                return Some(vec![field_name(&a[1]).to_string()]);
            }
            None
        }
        "field" if is_field(node, "names") => Some(vec![field_name(node).to_string()]),
        _ => None,
    }
}

/// 同一合取内，裸字段的 `> 0`/`≥ 1` 被同字段显式比较式吸收。
fn drop_bare(items: Vec<Item>) -> Vec<Item> {
    let cmp_keys: Vec<String> = items
        .iter()
        .filter(|i| !i.bare)
        .filter_map(|i| i.key.clone())
        .collect();
    items
        .into_iter()
        .filter(|i| !(i.bare && i.key.as_ref().is_some_and(|k| cmp_keys.contains(k))))
        .collect()
}

fn item(kind: Kind, text: String) -> Item {
    Item {
        kind,
        text,
        key: None,
        bare: false,
    }
}

fn keyed(kind: Kind, text: String, key: String, bare: bool) -> Item {
    Item {
        kind,
        text,
        key: Some(key),
        bare,
    }
}

fn render_conjunct(ctx: &Ctx, node: &Value, depth: u8) -> Result<Item> {
    let o = op(node).ok_or_else(|| ctx.unsupported(node))?;
    let a = arr(node).ok_or_else(|| ctx.unsupported(node))?;
    match o {
        "not" => {
            let child = &a[1];
            if is_field(child, "tags") {
                Ok(item(
                    Kind::Attr,
                    format!("{} = 0", ctx.tag(field_name(child))?),
                ))
            } else if is_field(child, "names") {
                Ok(item(
                    Kind::Special,
                    format!("不含 {}", ctx.ingredient(field_name(child))?),
                ))
            } else {
                Err(ctx.unsupported(node))
            }
        }
        "field" if is_field(node, "tags") => Ok(keyed(
            Kind::Attr,
            format!("{} > 0", ctx.tag(field_name(node))?),
            format!("t:{}", field_name(node)),
            true,
        )),
        "field" if is_field(node, "names") => Ok(keyed(
            Kind::Special,
            format!("{} ≥ 1", ctx.ingredient(field_name(node))?),
            format!("n:{}", field_name(node)),
            true,
        )),
        "gt" | "ge" | "lt" | "le" | "eq" | "ne" => render_comparison(ctx, o, a),
        "or" => render_or(ctx, node, depth),
        "and" if depth > 0 => {
            // 仅作为 or 分支出现（顶层 and 链已在入口处摊平）。
            let mut parts = Vec::new();
            flatten_and(node, &mut parts);
            let rendered = drop_bare(
                parts
                    .iter()
                    .map(|p| render_conjunct(ctx, p, depth + 1))
                    .collect::<Result<Vec<_>>>()?,
            );
            let has_attr = rendered.iter().any(|p| p.kind == Kind::Attr);
            let kind = if !has_attr && rendered.iter().any(|p| p.kind == Kind::Special) {
                Kind::Special
            } else {
                Kind::Attr
            };
            Ok(item(
                kind,
                rendered
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join("，"),
            ))
        }
        "bool" if a.len() == 2 && a[1].as_bool() == Some(true) => {
            Ok(item(Kind::Noop, String::new()))
        }
        _ => Err(ctx.unsupported(node)),
    }
}

fn render_comparison(ctx: &Ctx, o: &str, a: &[Value]) -> Result<Item> {
    let node = Value::Array(a.to_vec());
    let (lhs, rhs) = (&a[1], &a[2]);
    let cmp = cmp_zh(o).expect("checked by caller");
    let flip_cmp = cmp_zh(flip(o)).expect("closed table");
    if let Some(v) = num_value(rhs).filter(|_| is_field(lhs, "tags")) {
        return Ok(keyed(
            Kind::Attr,
            format!("{} {} {}", ctx.tag(field_name(lhs))?, cmp, fmt_num(v)),
            format!("t:{}", field_name(lhs)),
            false,
        ));
    }
    if let Some(v) = num_value(lhs).filter(|_| is_field(rhs, "tags")) {
        return Ok(keyed(
            Kind::Attr,
            format!("{} {} {}", fmt_num(v), flip_cmp, ctx.tag(field_name(rhs))?),
            format!("t:{}", field_name(rhs)),
            false,
        ));
    }
    if let Some(sum) = name_sum(lhs).filter(|_| num_value(rhs).is_some()) {
        let v = num_value(rhs).expect("checked");
        let names = ingredient_chain(ctx, &sum)?;
        let key = (sum.len() == 1).then(|| format!("n:{}", sum[0]));
        return Ok(Item {
            kind: Kind::Special,
            text: format!("{names} {cmp} {}", fmt_num(v)),
            key,
            bare: false,
        });
    }
    if let Some(sum) = name_sum(rhs).filter(|_| num_value(lhs).is_some()) {
        let v = num_value(lhs).expect("checked");
        let names = ingredient_chain(ctx, &sum)?;
        return Ok(item(
            Kind::Special,
            format!("{names} {flip_cmp} {}", fmt_num(v)),
        ));
    }
    if is_field(lhs, "names") {
        if let Some(v) = num_value(rhs) {
            return Ok(keyed(
                Kind::Special,
                format!(
                    "{} {} {}",
                    ctx.ingredient(field_name(lhs))?,
                    cmp,
                    fmt_num(v)
                ),
                format!("n:{}", field_name(lhs)),
                false,
            ));
        }
    }
    if o == "eq" && is_var(lhs, "cooker") {
        if let Some(cooker @ ("cookpot" | "portablecookpot")) = str_value(rhs) {
            let zh = if cooker == "portablecookpot" {
                "只能由沃利使用便携烹饪锅烹饪"
            } else {
                "只能由烹饪锅烹饪"
            };
            return Ok(item(Kind::Special, zh.to_string()));
        }
    }
    Err(ctx.unsupported(&node))
}

fn ingredient_chain(ctx: &Ctx, keys: &[String]) -> Result<String> {
    keys.iter()
        .map(|k| ctx.ingredient(k))
        .collect::<Result<Vec<_>>>()
        .map(|v| v.join("/"))
}

/// `or` 节点的整体渲染（对应 wiki 表用 `/` 表达的两类分支：食材变体组 / 属性组）。
fn render_or(ctx: &Ctx, node: &Value, depth: u8) -> Result<Item> {
    let mut branches = Vec::new();
    or_branches(node, &mut branches);

    // (X and X≥2) or (Y and Y≥2) or (X and Y) → 「X/Y ≥ 2（或各含其一）」
    if branches.len() >= 2 {
        if let Some(text) = render_pair_or(ctx, &branches)? {
            return Ok(item(Kind::Special, text));
        }
    }
    if branches.iter().all(|b| is_field(b, "names")) {
        let keys: Vec<String> = branches.iter().map(|b| field_name(b).to_string()).collect();
        return Ok(item(
            Kind::Special,
            format!("{} ≥ 1", ingredient_chain(ctx, &keys)?),
        ));
    }
    let tagish = |node: &Value| -> bool {
        if is_field(node, "tags") {
            return true;
        }
        if op(node) == Some("not") {
            return arr(node).is_some_and(|a| a.len() == 2 && is_field(&a[1], "tags"));
        }
        match (arr(node), op(node)) {
            (Some(x), Some(o)) if x.len() == 3 && matches!(o, "gt" | "ge" | "lt" | "le" | "eq") => {
                is_field(&x[1], "tags")
            }
            _ => false,
        }
    };
    if branches.iter().all(|b| tagish(b)) {
        let texts = branch_texts(ctx, &branches, depth)?;
        return Ok(item(Kind::Attr, texts.join(" / ")));
    }
    let per_branch: Vec<Vec<&Value>> = branches
        .iter()
        .map(|b| {
            let mut parts = Vec::new();
            flatten_and(b, &mut parts);
            parts
        })
        .collect();
    if !per_branch.is_empty()
        && per_branch
            .iter()
            .all(|p| !p.is_empty() && p.iter().all(|x| tagish(x)))
    {
        let mut texts = Vec::new();
        for parts in &per_branch {
            let rendered = drop_bare(
                parts
                    .iter()
                    .map(|p| render_conjunct(ctx, p, depth + 1))
                    .collect::<Result<Vec<_>>>()?,
            );
            texts.push(
                rendered
                    .iter()
                    .map(|i| i.text.as_str())
                    .collect::<Vec<_>>()
                    .join("，"),
            );
        }
        return Ok(item(Kind::Attr, texts.join(" / ")));
    }
    // 结构未能归并：保留分支结构交人工审阅（当前快照无此形态）。
    let texts = branch_texts(ctx, &branches, depth)?;
    Ok(item(Kind::Attr, texts.join(" / ")))
}

fn branch_texts(ctx: &Ctx, branches: &[&Value], depth: u8) -> Result<Vec<String>> {
    branches
        .iter()
        .map(|b| render_conjunct(ctx, b, depth + 1).map(|i| i.text))
        .collect()
}

fn render_pair_or(ctx: &Ctx, branches: &[&Value]) -> Result<Option<String>> {
    let mut pair_keys: Vec<String> = Vec::new();
    let mut pair_n: Option<f64> = None;
    for branch in branches {
        if op(branch) != Some("and") {
            return Ok(None);
        }
        let mut parts = Vec::new();
        flatten_and(branch, &mut parts);
        let mut names_of: Vec<String> = Vec::new();
        for part in &parts {
            if is_field(part, "names") {
                names_of.push(field_name(part).to_string());
                continue;
            }
            let po = op(part);
            let pa = match arr(part) {
                Some(a) if a.len() == 3 => a,
                _ => return Ok(None),
            };
            if matches!(po, Some("gt") | Some("ge"))
                && is_field(&pa[1], "names")
                && num_value(&pa[2]).is_some()
            {
                names_of.push(field_name(&pa[1]).to_string());
                let raw = num_value(&pa[2]).expect("checked");
                let v = if po == Some("gt") { raw + 1.0 } else { raw };
                pair_n = Some(pair_n.map_or(v, |n| n.min(v)));
                continue;
            }
            return Ok(None);
        }
        let mut uniq: Vec<String> = Vec::new();
        for name in names_of {
            if !uniq.contains(&name) {
                uniq.push(name);
            }
        }
        // 同一食材的 裸+裸 分支没有阈值语义，交回通用 names or 链规则。
        if parts.len() == 2 && uniq.len() == 1 && pair_n.is_none() {
            return Ok(None);
        }
        if uniq.is_empty() || uniq.len() > 2 {
            return Ok(None);
        }
        for name in uniq {
            if !pair_keys.contains(&name) {
                pair_keys.push(name);
            }
        }
    }
    let n = match pair_n {
        Some(n) if pair_keys.len() >= 2 => n,
        _ => return Ok(None),
    };
    Ok(Some(format!(
        "{} ≥ {}（或各含其一）",
        ingredient_chain(ctx, &pair_keys)?,
        fmt_num(n)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn names() -> HashMap<String, String> {
        [
            ("froglegs", "蛙腿"),
            ("froglegs_cooked", "熟蛙腿"),
            ("twigs", "树枝"),
            ("butterflywings", "蝴蝶翅膀"),
            ("moonbutterflywings", "月蛾翅膀"),
            ("kelp", "海带叶"),
            ("kelp_cooked", "熟海带叶"),
            ("turkey_legs", "鸟腿"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    fn describe(test: Value) -> RecipeDesc {
        describe_recipe("test_recipe", &test, &names()).unwrap()
    }

    #[test]
    fn tag_conjunction_renders_attrs_only() {
        // 培根煎蛋：tags.meat > 1 and tags.egg > 1 and not tags.veggie
        let desc = describe(json!([
            "and",
            [
                "and",
                ["gt", ["field", ["var", "tags"], "meat"], ["num", 1.0]],
                ["gt", ["field", ["var", "tags"], "egg"], ["num", 1.0]]
            ],
            ["not", ["field", ["var", "tags"], "veggie"]]
        ]));
        assert_eq!(desc.attrs, "肉度 > 1，蛋度 > 1，蔬菜度 = 0");
        assert_eq!(desc.specials, "");
    }

    #[test]
    fn bare_field_absorbed_by_same_key_comparison() {
        // 蝴蝶松饼真实形态：裸 veggie 被 veggie ≥ 0.5 吸收
        let desc = describe(json!([
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
        ]));
        assert_eq!(desc.attrs, "肉度 = 0，蔬菜度 ≥ 0.5");
        assert_eq!(desc.specials, "蝴蝶翅膀/月蛾翅膀 ≥ 1");
    }

    #[test]
    fn bare_field_without_comparison_renders_gt_zero() {
        let desc = describe(json!(["field", ["var", "tags"], "frozen"]));
        assert_eq!(desc.attrs, "冰度 > 0");
        assert_eq!(desc.specials, "");
    }

    #[test]
    fn pair_or_of_raw_and_cooked_variants() {
        // 蓝带鱼排形态：(蛙腿且≥2) or (熟蛙腿且≥2) or (两者各一)
        let desc = describe(json!([
            "and",
            [
                "and",
                [
                    "or",
                    [
                        "or",
                        [
                            "and",
                            ["field", ["var", "names"], "froglegs"],
                            ["ge", ["field", ["var", "names"], "froglegs"], ["num", 2.0]]
                        ],
                        [
                            "and",
                            ["field", ["var", "names"], "froglegs_cooked"],
                            [
                                "ge",
                                ["field", ["var", "names"], "froglegs_cooked"],
                                ["num", 2.0]
                            ]
                        ]
                    ],
                    [
                        "and",
                        ["field", ["var", "names"], "froglegs"],
                        ["field", ["var", "names"], "froglegs_cooked"]
                    ]
                ],
                ["field", ["var", "tags"], "fish"]
            ],
            ["not", ["field", ["var", "tags"], "inedible"]]
        ]));
        assert_eq!(desc.attrs, "鱼度 > 0，不可食用度 = 0");
        assert_eq!(desc.specials, "蛙腿/熟蛙腿 ≥ 2（或各含其一）");
    }

    #[test]
    fn name_sum_comparison_counts_variants_together() {
        // (names.kelp or 0) + (names.kelp_cooked or 0) == 2
        let desc = describe(json!([
            "eq",
            [
                "add",
                ["or", ["field", ["var", "names"], "kelp"], ["num", 0.0]],
                [
                    "or",
                    ["field", ["var", "names"], "kelp_cooked"],
                    ["num", 0.0]
                ]
            ],
            ["num", 2.0]
        ]));
        assert_eq!(desc.attrs, "");
        assert_eq!(desc.specials, "海带叶/熟海带叶 = 2");
    }

    #[test]
    fn numeric_lhs_keeps_flipped_comparison() {
        let desc = describe(json!([
            "gt",
            ["num", 0.0],
            ["field", ["var", "tags"], "inedible"]
        ]));
        assert_eq!(desc.attrs, "0 < 不可食用度");
    }

    #[test]
    fn or_of_tag_branches_joins_with_slash() {
        // 火鸡正餐：(蔬菜度 and ≥0.5) or 水果度 → 分支内裸字段被吸收
        let desc = describe(json!([
            "or",
            [
                "and",
                ["field", ["var", "tags"], "veggie"],
                ["ge", ["field", ["var", "tags"], "veggie"], ["num", 0.5]]
            ],
            ["field", ["var", "tags"], "fruit"]
        ]));
        assert_eq!(desc.attrs, "蔬菜度 ≥ 0.5 / 水果度 > 0");
    }

    #[test]
    fn cooker_restriction_renders_special() {
        let desc = describe(json!(["eq", ["var", "cooker"], ["str", "portablecookpot"]]));
        assert_eq!(desc.specials, "只能由沃利使用便携烹饪锅烹饪");
    }

    #[test]
    fn always_true_test_renders_empty_columns() {
        let desc = describe(json!(["bool", true]));
        assert_eq!(desc, RecipeDesc::default());
    }

    #[test]
    fn unknown_tag_is_a_loud_error() {
        let err = describe_recipe("r", &json!(["field", ["var", "tags"], "spicy"]), &names())
            .unwrap_err();
        assert!(err.to_string().contains("spicy"), "{err}");
    }

    #[test]
    fn unknown_ingredient_is_a_loud_error() {
        let err = describe_recipe(
            "r",
            &json!(["field", ["var", "names"], "mystery_meat"]),
            &names(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("mystery_meat"), "{err}");
    }

    #[test]
    fn unrenderable_node_is_a_loud_error() {
        // tag 求和不是已知形态
        let err = describe_recipe(
            "r",
            &json!([
                "ge",
                [
                    "add",
                    ["field", ["var", "tags"], "meat"],
                    ["field", ["var", "tags"], "fish"]
                ],
                ["num", 2.0]
            ]),
            &names(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("无法渲染"), "{err}");
    }
}
