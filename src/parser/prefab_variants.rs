//! 数据驱动 prefab 变体推导（spice / winter ornament 等运行时生成族）。
//!
//! 这些变体名不是字面量，而是由循环/拼接在运行时生成，逐文件 AST 扫描无法
//! 覆盖。这里按已知生成点做确定性展开，输出 `变体 prefab -> 基准 prefab`：
//!
//! - `spicedfoods.lua`：`foods` 表的每个食物 × `SPICES` 表每个香料，
//!   生成 `<food>_<spice>`，重定向到 `<food>`；
//! - `winter_ornaments.lua`：`MakeOrnament(id, base, ...)` 生成
//!   `winter_ornament_<id>`，重定向到 `base`，含 `"plain"..i` 数值循环。

use crate::error::{Error, Result};
use crate::scripts_sync::anim::index::string_literal_text;
use full_moon::ast::{self, Ast};
use std::collections::{BTreeSet, HashMap};

/// 一个由生成器推导出的 prefab 名重定向。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefabVariant {
    pub prefab: String,
    pub base: String,
    /// 来源文件（用于审计/冲突报告）。
    pub origin: String,
}

/// `spicedfoods.lua` 的 `SPICES` 表键转香料后缀（`SPICE_GARLIC` → `spice_garlic`）。
pub fn spice_names_from_source(source: &str) -> Vec<String> {
    let Ok(ast) = full_moon::parse(source) else {
        return Vec::new();
    };
    let Some(table) = find_table_assignment(&ast, "SPICES") else {
        return Vec::new();
    };
    let mut names: Vec<String> = table
        .fields()
        .iter()
        .filter_map(|field| match field {
            ast::Field::NameKey { key, .. } => Some(key.token().to_string().to_lowercase()),
            _ => None,
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// 解析 `preparedfoods.lua` / `preparedfoods_warly.lua`，按 `spice_names`
/// 展开每个食物。
pub fn parse_spiced_foods(
    food_sources: &[(String, String)],
    spice_names: &[String],
) -> Result<Vec<PrefabVariant>> {
    if spice_names.is_empty() {
        return Ok(Vec::new());
    }
    let mut variants = Vec::new();
    for (origin, source) in food_sources {
        let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
        let Some(foods) = find_table_assignment(&ast, "foods") else {
            continue;
        };
        for field in foods.fields() {
            let name = match field {
                ast::Field::NameKey { key, .. } => key.token().to_string(),
                ast::Field::ExpressionKey {
                    key: ast::Expression::String(s),
                    ..
                } => string_literal_text(&s.to_string()),
                _ => continue,
            };
            if name.is_empty() {
                continue;
            }
            for spice in spice_names {
                variants.push(PrefabVariant {
                    prefab: format!("{name}_{spice}"),
                    base: name.clone(),
                    origin: origin.clone(),
                });
            }
        }
    }
    Ok(variants)
}

/// 解析 `winter_ornaments.lua` 的 `MakeOrnament` 调用（含数值循环）。
pub fn parse_winter_ornaments(source: &str) -> Result<Vec<PrefabVariant>> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let mut consts = HashMap::new();
    collect_numeric_consts(ast.nodes(), &mut consts);

    let mut variants = Vec::new();
    walk_block(
        ast.nodes(),
        &consts,
        &HashMap::new(),
        "prefabs/winter_ornaments.lua",
        &mut variants,
    );
    variants.sort_by(|a, b| a.prefab.cmp(&b.prefab));
    variants.dedup_by(|a, b| a.prefab == b.prefab);
    Ok(variants)
}

/// 表键 → `{name}_oversized_waxed` / `{name}_oversized`（`veggies.lua` 的
/// `VEGGIES` 表 + `MakeVeggie` 命名规则；`SetPrefabNameOverride(name.."_oversized")`）。
pub fn parse_oversized_waxed(source: &str, origin: &str) -> Result<Vec<PrefabVariant>> {
    let names = table_keys(source, "VEGGIES")?;
    Ok(names
        .into_iter()
        .map(|name| PrefabVariant {
            prefab: format!("{name}_oversized_waxed"),
            base: format!("{name}_oversized"),
            origin: origin.to_string(),
        })
        .collect())
}

/// `BOBBERS` 表键 → `{name}_projectile` / `{name}_floater`（`oceanfishingbobber.lua`）。
pub fn parse_bobbers(source: &str, origin: &str) -> Result<Vec<PrefabVariant>> {
    let names = table_keys(source, "BOBBERS")?;
    let mut variants = Vec::new();
    for name in names {
        for suffix in ["projectile", "floater"] {
            variants.push(PrefabVariant {
                prefab: format!("{name}_{suffix}"),
                base: name.clone(),
                origin: origin.to_string(),
            });
        }
    }
    Ok(variants)
}

/// `SPIKE_SIZES` 数组值 → `{base}_{size}`（`glass_spike.lua` / `sand_spike.lua`；
/// `SetPrefabNameOverride("glassspike")`）。
pub fn parse_spike_sizes(source: &str, base: &str, origin: &str) -> Result<Vec<PrefabVariant>> {
    let sizes = table_values(source, "SPIKE_SIZES")?;
    Ok(sizes
        .into_iter()
        .map(|size| PrefabVariant {
            prefab: format!("{base}_{size}"),
            base: base.to_string(),
            origin: origin.to_string(),
        })
        .collect())
}

/// 数值循环命名族：`for i = 1, N do local name = "{base}"..tostring(i)` →
/// `{base}1..N`（`goosplash.lua`）。
pub fn parse_indexed_variants(
    source: &str,
    base: &str,
    origin: &str,
) -> Result<Vec<PrefabVariant>> {
    let ast = parse_ast(source)?;
    let mut consts = HashMap::new();
    collect_numeric_consts(ast.nodes(), &mut consts);
    let mut variants = Vec::new();
    walk_indexed(
        ast.nodes(),
        base,
        &consts,
        &HashMap::new(),
        origin,
        &mut variants,
    );
    variants.sort_by(|a, b| a.prefab.cmp(&b.prefab));
    variants.dedup_by(|a, b| a.prefab == b.prefab);
    Ok(variants)
}

/// 字面量注册对：文件内出现的 `Prefab("<name>", ...)` 与给定 base 配对。
pub fn parse_literal_registrations(
    source: &str,
    base: &str,
    names: &[&str],
    origin: &str,
) -> Result<Vec<PrefabVariant>> {
    let ast = parse_ast(source)?;
    let mut registered = BTreeSet::new();
    collect_prefab_registrations(ast.nodes(), &mut registered);
    Ok(names
        .iter()
        .filter(|name| registered.contains(**name))
        .map(|name| PrefabVariant {
            prefab: (*name).to_string(),
            base: base.to_string(),
            origin: origin.to_string(),
        })
        .collect())
}

/// 单 base 文件 + 精确名字列表（脚本无拼接、但主解析器未覆盖的注册样式）。
pub struct LiteralFamily {
    pub file: &'static str,
    pub base: &'static str,
    pub names: &'static [&'static str],
}

/// 字面量族配置（`Prefab("name")` 必须真实存在才产出）。
pub const LITERAL_FAMILIES: &[LiteralFamily] = &[
    LiteralFamily {
        file: "prefabs/statueruins.lua",
        base: "ancient_statue",
        names: &[
            "ruins_statue_head",
            "ruins_statue_head_nogem",
            "ruins_statue_mage",
            "ruins_statue_mage_nogem",
        ],
    },
    LiteralFamily {
        file: "prefabs/statue_marble.lua",
        base: "statue_marble",
        names: &["statue_marble_muse", "statue_marble_pawn"],
    },
    LiteralFamily {
        file: "prefabs/driftwood_trees.lua",
        base: "DRIFTWOOD_TREE",
        names: &["driftwood_tall", "driftwood_small1", "driftwood_small2"],
    },
    LiteralFamily {
        file: "prefabs/deerclops_laser.lua",
        base: "deerclops",
        names: &["deerclops_laser", "deerclops_laserempty"],
    },
    LiteralFamily {
        file: "prefabs/alterguardian_laser.lua",
        base: "deerclops",
        names: &["alterguardian_laser", "alterguardian_laserempty"],
    },
    LiteralFamily {
        file: "prefabs/skeleton.lua",
        base: "skeleton_notplayer",
        names: &["skeleton_notplayer_1", "skeleton_notplayer_2"],
    },
    LiteralFamily {
        file: "prefabs/scrapbook_page.lua",
        base: "scrapbook_page",
        names: &["scrapbook_page_special"],
    },
    LiteralFamily {
        file: "prefabs/worm_boss.lua",
        base: "worm_boss",
        names: &["worm_boss_dirt", "worm_boss_segment"],
    },
    LiteralFamily {
        file: "prefabs/carnivaldecor_figure.lua",
        base: "carnivaldecor_figure",
        names: &["carnivaldecor_figure_season2"],
    },
    LiteralFamily {
        file: "prefabs/redlantern.lua",
        base: "redlantern",
        names: &["yots_redlantern"],
    },
    LiteralFamily {
        file: "prefabs/deer.lua",
        base: "deer_gemmed",
        names: &["deer_blue", "deer_red"],
    },
];

fn parse_ast(source: &str) -> Result<Ast> {
    full_moon::parse(source).map_err(Error::LuaParse)
}

fn walk_indexed(
    block: &ast::Block,
    base: &str,
    consts: &HashMap<String, i64>,
    env: &HashMap<String, i64>,
    origin: &str,
    out: &mut Vec<PrefabVariant>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::NumericFor(for_stmt) => {
                let start = eval_number(for_stmt.start(), consts);
                let end = eval_number(for_stmt.end(), consts);
                let step = for_stmt
                    .step()
                    .and_then(|e| eval_number(e, consts))
                    .unwrap_or(1);
                let (Some(start), Some(end)) = (start, end) else {
                    continue;
                };
                if step == 0 {
                    continue;
                }
                let var = for_stmt.index_variable().to_string().trim().to_string();
                let mut i = start;
                while (step > 0 && i <= end) || (step < 0 && i >= end) {
                    let mut loop_env = env.clone();
                    loop_env.insert(var.clone(), i);
                    collect_indexed_names(for_stmt.block(), base, consts, &loop_env, origin, out);
                    i += step;
                }
            }
            ast::Stmt::If(if_stmt) => {
                walk_indexed(if_stmt.block(), base, consts, env, origin, out);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        walk_indexed(else_if.block(), base, consts, env, origin, out);
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    walk_indexed(else_block, base, consts, env, origin, out);
                }
            }
            ast::Stmt::Do(do_stmt) => {
                walk_indexed(do_stmt.block(), base, consts, env, origin, out);
            }
            ast::Stmt::While(while_stmt) => {
                walk_indexed(while_stmt.block(), base, consts, env, origin, out);
            }
            ast::Stmt::Repeat(repeat_stmt) => {
                walk_indexed(repeat_stmt.block(), base, consts, env, origin, out);
            }
            _ => {}
        }
    }
}

fn collect_indexed_names(
    block: &ast::Block,
    base: &str,
    consts: &HashMap<String, i64>,
    env: &HashMap<String, i64>,
    origin: &str,
    out: &mut Vec<PrefabVariant>,
) {
    for stmt in block.stmts() {
        let ast::Stmt::LocalAssignment(assign) = stmt else {
            continue;
        };
        for (_, expr) in assign.names().iter().zip(assign.expressions().iter()) {
            if let Some(name) = eval_string_expr(expr, consts, env) {
                if name.starts_with(base) && name != base {
                    out.push(PrefabVariant {
                        prefab: name,
                        base: base.to_string(),
                        origin: origin.to_string(),
                    });
                }
            }
        }
    }
}

fn collect_prefab_registrations(block: &ast::Block, out: &mut BTreeSet<String>) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(assign) => {
                for expr in assign.expressions() {
                    collect_registrations_expr(expr, out);
                }
            }
            ast::Stmt::Assignment(assign) => {
                for expr in assign.expressions() {
                    collect_registrations_expr(expr, out);
                }
            }
            ast::Stmt::FunctionCall(call) => collect_registrations_call(call, out),
            ast::Stmt::NumericFor(for_stmt) => {
                collect_prefab_registrations(for_stmt.block(), out);
            }
            ast::Stmt::GenericFor(for_stmt) => {
                collect_prefab_registrations(for_stmt.block(), out);
            }
            ast::Stmt::If(if_stmt) => {
                collect_prefab_registrations(if_stmt.block(), out);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_prefab_registrations(else_if.block(), out);
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    collect_prefab_registrations(else_block, out);
                }
            }
            ast::Stmt::Do(do_stmt) => collect_prefab_registrations(do_stmt.block(), out),
            ast::Stmt::While(while_stmt) => {
                collect_prefab_registrations(while_stmt.block(), out);
            }
            ast::Stmt::Repeat(repeat_stmt) => {
                collect_prefab_registrations(repeat_stmt.block(), out);
            }
            ast::Stmt::LocalFunction(local_fn) => {
                collect_prefab_registrations(local_fn.body().block(), out);
            }
            ast::Stmt::FunctionDeclaration(decl) => {
                collect_prefab_registrations(decl.body().block(), out);
            }
            _ => {}
        }
    }
    if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
        for expr in ret.returns().iter() {
            collect_registrations_expr(expr, out);
        }
    }
}

fn collect_registrations_expr(expr: &ast::Expression, out: &mut BTreeSet<String>) {
    match expr {
        ast::Expression::FunctionCall(call) => collect_registrations_call(call, out),
        ast::Expression::TableConstructor(table) => {
            for field in table.fields() {
                if let Some(value) = field_value(field) {
                    collect_registrations_expr(value, out);
                }
            }
        }
        ast::Expression::Parentheses { expression, .. } => {
            collect_registrations_expr(expression, out);
        }
        ast::Expression::BinaryOperator { lhs, rhs, .. } => {
            collect_registrations_expr(lhs, out);
            collect_registrations_expr(rhs, out);
        }
        ast::Expression::UnaryOperator { expression, .. } => {
            collect_registrations_expr(expression, out);
        }
        _ => {}
    }
}

fn collect_registrations_call(call: &ast::FunctionCall, out: &mut BTreeSet<String>) {
    if let Some(args) = call_args(call) {
        if matches!(call.prefix(), ast::Prefix::Name(name) if name.token().to_string() == "Prefab")
        {
            if let Some(ast::Expression::String(s)) = args.first() {
                out.insert(string_literal_text(&s.to_string()));
            }
        }
        for arg in args {
            collect_registrations_expr(arg, out);
        }
    }
}

fn table_keys(source: &str, var: &str) -> Result<Vec<String>> {
    let ast = parse_ast(source)?;
    let Some(table) = find_table_assignment(&ast, var) else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    for field in table.fields() {
        match field {
            ast::Field::NameKey { key, .. } => names.push(key.token().to_string()),
            ast::Field::ExpressionKey {
                key: ast::Expression::String(s),
                ..
            } => names.push(string_literal_text(&s.to_string())),
            _ => {}
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn table_values(source: &str, var: &str) -> Result<Vec<String>> {
    let ast = parse_ast(source)?;
    let Some(table) = find_table_assignment(&ast, var) else {
        return Ok(Vec::new());
    };
    let mut values = Vec::new();
    for field in table.fields() {
        if let ast::Field::NoKey(ast::Expression::String(s)) = field {
            values.push(string_literal_text(&s.to_string()));
        }
    }
    values.sort();
    values.dedup();
    Ok(values)
}

fn find_table_assignment<'a>(ast: &'a Ast, var: &str) -> Option<&'a ast::TableConstructor> {
    for stmt in ast.nodes().stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(assign) => {
                for (name, expr) in assign.names().iter().zip(assign.expressions().iter()) {
                    if name.token().to_string() == var {
                        if let ast::Expression::TableConstructor(table) = expr {
                            return Some(table);
                        }
                    }
                }
            }
            ast::Stmt::Assignment(assign) => {
                for (target, expr) in assign.variables().iter().zip(assign.expressions().iter()) {
                    if let ast::Var::Name(name) = target {
                        if name.token().to_string() == var {
                            if let ast::Expression::TableConstructor(table) = expr {
                                return Some(table);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_numeric_consts(block: &ast::Block, consts: &mut HashMap<String, i64>) {
    for stmt in block.stmts() {
        if let ast::Stmt::LocalAssignment(assign) = stmt {
            for (name, expr) in assign.names().iter().zip(assign.expressions().iter()) {
                if let Some(value) = eval_number(expr, consts) {
                    consts.insert(name.token().to_string(), value);
                }
            }
        }
    }
}

fn walk_block(
    block: &ast::Block,
    consts: &HashMap<String, i64>,
    env: &HashMap<String, i64>,
    origin: &str,
    out: &mut Vec<PrefabVariant>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(assign) => {
                for expr in assign.expressions() {
                    walk_expr(expr, consts, env, origin, out);
                }
            }
            ast::Stmt::Assignment(assign) => {
                for expr in assign.expressions() {
                    walk_expr(expr, consts, env, origin, out);
                }
            }
            ast::Stmt::FunctionCall(call) => {
                walk_call(call, consts, env, origin, out);
            }
            ast::Stmt::NumericFor(for_stmt) => {
                let start = eval_number(for_stmt.start(), consts);
                let end = eval_number(for_stmt.end(), consts);
                let step = for_stmt
                    .step()
                    .and_then(|e| eval_number(e, consts))
                    .unwrap_or(1);
                let (Some(start), Some(end)) = (start, end) else {
                    continue;
                };
                if step == 0 {
                    continue;
                }
                let var = for_stmt.index_variable().to_string().trim().to_string();
                let mut i = start;
                while (step > 0 && i <= end) || (step < 0 && i >= end) {
                    let mut loop_env = env.clone();
                    loop_env.insert(var.clone(), i);
                    walk_block(for_stmt.block(), consts, &loop_env, origin, out);
                    i += step;
                }
            }
            ast::Stmt::GenericFor(for_stmt) => {
                walk_block(for_stmt.block(), consts, env, origin, out);
            }
            ast::Stmt::If(if_stmt) => {
                walk_block(if_stmt.block(), consts, env, origin, out);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        walk_block(else_if.block(), consts, env, origin, out);
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    walk_block(else_block, consts, env, origin, out);
                }
            }
            ast::Stmt::Do(do_stmt) => walk_block(do_stmt.block(), consts, env, origin, out),
            ast::Stmt::While(while_stmt) => {
                walk_block(while_stmt.block(), consts, env, origin, out);
            }
            ast::Stmt::Repeat(repeat_stmt) => {
                walk_block(repeat_stmt.block(), consts, env, origin, out);
            }
            ast::Stmt::LocalFunction(local_fn) => {
                walk_block(local_fn.body().block(), consts, env, origin, out);
            }
            ast::Stmt::FunctionDeclaration(decl) => {
                walk_block(decl.body().block(), consts, env, origin, out);
            }
            _ => {}
        }
    }
    if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
        for expr in ret.returns().iter() {
            walk_expr(expr, consts, env, origin, out);
        }
    }
}

fn walk_expr(
    expr: &ast::Expression,
    consts: &HashMap<String, i64>,
    env: &HashMap<String, i64>,
    origin: &str,
    out: &mut Vec<PrefabVariant>,
) {
    match expr {
        ast::Expression::FunctionCall(call) => walk_call(call, consts, env, origin, out),
        ast::Expression::TableConstructor(table) => {
            for field in table.fields() {
                if let Some(value) = field_value(field) {
                    walk_expr(value, consts, env, origin, out);
                }
            }
        }
        ast::Expression::Parentheses { expression, .. } => {
            walk_expr(expression, consts, env, origin, out);
        }
        ast::Expression::BinaryOperator { lhs, rhs, .. } => {
            walk_expr(lhs, consts, env, origin, out);
            walk_expr(rhs, consts, env, origin, out);
        }
        ast::Expression::UnaryOperator { expression, .. } => {
            walk_expr(expression, consts, env, origin, out);
        }
        _ => {}
    }
}

fn walk_call(
    call: &ast::FunctionCall,
    consts: &HashMap<String, i64>,
    env: &HashMap<String, i64>,
    origin: &str,
    out: &mut Vec<PrefabVariant>,
) {
    if let Some(args) = call_args(call) {
        if matches!(call.prefix(), ast::Prefix::Name(name) if name.token().to_string() == "MakeOrnament")
        {
            if let (Some(id), Some(base)) = (
                args.first().and_then(|a| eval_string_expr(a, consts, env)),
                args.get(1).and_then(|a| eval_string_expr(a, consts, env)),
            ) {
                out.push(PrefabVariant {
                    prefab: format!("winter_ornament_{id}"),
                    base,
                    origin: origin.to_string(),
                });
            }
        }
        for arg in &args {
            walk_expr(arg, consts, env, origin, out);
        }
    }
}

fn call_args(call: &ast::FunctionCall) -> Option<Vec<&ast::Expression>> {
    for suffix in call.suffixes() {
        if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
            return match args {
                ast::FunctionArgs::Parentheses { arguments, .. } => {
                    Some(arguments.iter().collect())
                }
                _ => None,
            };
        }
    }
    None
}

fn field_value(field: &ast::Field) -> Option<&ast::Expression> {
    match field {
        ast::Field::ExpressionKey { value, .. } => Some(value),
        ast::Field::NameKey { value, .. } => Some(value),
        ast::Field::NoKey(expr) => Some(expr),
        _ => None,
    }
}

fn eval_number(expr: &ast::Expression, consts: &HashMap<String, i64>) -> Option<i64> {
    match expr {
        ast::Expression::Number(n) => n.token().to_string().trim().parse().ok(),
        ast::Expression::Var(ast::Var::Name(name)) => {
            consts.get(&name.token().to_string()).copied()
        }
        ast::Expression::UnaryOperator { unop, expression } => {
            let value = eval_number(expression, consts)?;
            if unop.to_string().trim() == "-" {
                Some(-value)
            } else {
                Some(value)
            }
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            let left = eval_number(lhs, consts)?;
            let right = eval_number(rhs, consts)?;
            match binop.to_string().trim() {
                "+" => Some(left + right),
                "-" => Some(left - right),
                "*" => Some(left * right),
                "/" if right != 0 => Some(left / right),
                _ => None,
            }
        }
        ast::Expression::Parentheses { expression, .. } => eval_number(expression, consts),
        _ => None,
    }
}

fn eval_string_expr(
    expr: &ast::Expression,
    consts: &HashMap<String, i64>,
    env: &HashMap<String, i64>,
) -> Option<String> {
    match expr {
        ast::Expression::String(s) => Some(string_literal_text(&s.to_string())),
        ast::Expression::Number(n) => Some(n.token().to_string().trim().to_string()),
        ast::Expression::Var(ast::Var::Name(name)) => {
            let name = name.token().to_string();
            env.get(&name)
                .map(i64::to_string)
                .or_else(|| consts.get(&name).map(i64::to_string))
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            if binop.to_string().trim() != ".." {
                return None;
            }
            let left = eval_string_expr(lhs, consts, env)?;
            let right = eval_string_expr(rhs, consts, env)?;
            Some(format!("{left}{right}"))
        }
        ast::Expression::Parentheses { expression, .. } => {
            eval_string_expr(expression, consts, env)
        }
        ast::Expression::FunctionCall(call) => {
            if !matches!(call.prefix(), ast::Prefix::Name(name) if name.token().to_string() == "tostring")
            {
                return None;
            }
            call_args(call)?
                .first()
                .and_then(|arg| eval_string_expr(arg, consts, env))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spice_names_from_source() {
        let source = r#"
local SPICES =
{
    SPICE_GARLIC = { oneatenfn = f },
    SPICE_SUGAR  = {},
    SPICE_CHILI  = {},
    SPICE_SALT   = {},
}
"#;
        assert_eq!(
            spice_names_from_source(source),
            vec!["spice_chili", "spice_garlic", "spice_salt", "spice_sugar"]
        );
        assert!(spice_names_from_source("local x = 1").is_empty());
    }

    #[test]
    fn test_parse_spiced_foods() {
        let source = r#"
local foods =
{
    butterflymuffin =
    {
        priority = 1,
    },
    ["quoted_food"] =
    {
        priority = 2,
    },
}

return foods
"#;
        let foods = vec![("preparedfoods.lua".to_string(), source.to_string())];
        let spices = vec!["spice_chili".to_string(), "spice_salt".to_string()];
        let variants = parse_spiced_foods(&foods, &spices).unwrap();
        let names: Vec<&str> = variants.iter().map(|v| v.prefab.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "butterflymuffin_spice_chili",
                "butterflymuffin_spice_salt",
                "quoted_food_spice_chili",
                "quoted_food_spice_salt",
            ]
        );
        assert!(variants.iter().all(|v| v.base.starts_with("butterfly")
            || v.base == "butterflymuffin"
            || v.base == "quoted_food"));
    }

    #[test]
    fn test_parse_winter_ornaments_literals_and_loops() {
        let source = r#"
local NUM_BASIC_ORNAMENT = 3
local NUM_LIGHT_ORNAMENT = 2

local ornament =
{
    MakeOrnament("boss_antlion", "winter_ornamentboss", nil, nil, 0.70),
    MakeOrnament("festivalevents1", "winter_ornamentforge"),
}

for i = 1, NUM_BASIC_ORNAMENT do
    table.insert(ornament, MakeOrnament("plain"..i, "winter_ornament", nil, nil, 0.65))
end
for i = 1, NUM_LIGHT_ORNAMENT do
    table.insert(ornament, MakeOrnament("light"..i, "winter_ornamentlight", nil, nil, 0.70))
end

return unpack(ornament)
"#;
        let variants = parse_winter_ornaments(source).unwrap();
        let pairs: Vec<(&str, &str)> = variants
            .iter()
            .map(|v| (v.prefab.as_str(), v.base.as_str()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("winter_ornament_boss_antlion", "winter_ornamentboss"),
                ("winter_ornament_festivalevents1", "winter_ornamentforge"),
                ("winter_ornament_light1", "winter_ornamentlight"),
                ("winter_ornament_light2", "winter_ornamentlight"),
                ("winter_ornament_plain1", "winter_ornament"),
                ("winter_ornament_plain2", "winter_ornament"),
                ("winter_ornament_plain3", "winter_ornament"),
            ]
        );
    }

    #[test]
    fn test_parse_winter_ornaments_skips_unresolved() {
        let source = r#"
local function MakeOrnament(a, b) end
local ornament = { MakeOrnament(name_var, "winter_ornament") }
return unpack(ornament)
"#;
        assert!(parse_winter_ornaments(source).unwrap().is_empty());
    }

    #[test]
    fn test_parse_oversized_waxed() {
        let source = r#"
VEGGIES =
{
    carrot = MakeVegStats(1),
    ["cave_banana"] = MakeVegStats(2),
    kelp = MakeVegStats(3),
}
"#;
        let variants = parse_oversized_waxed(source, "prefabs/veggies.lua").unwrap();
        let pairs: Vec<(&str, &str)> = variants
            .iter()
            .map(|v| (v.prefab.as_str(), v.base.as_str()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("carrot_oversized_waxed", "carrot_oversized"),
                ("cave_banana_oversized_waxed", "cave_banana_oversized"),
                ("kelp_oversized_waxed", "kelp_oversized"),
            ]
        );
    }

    #[test]
    fn test_parse_bobbers() {
        let source = r#"
local BOBBERS =
{
    ["oceanfishingbobber_twig"] = { make_inv_item = false },
    ["oceanfishingbobber_ball"] = { make_inv_item = true },
}
for name, v in pairs(BOBBERS) do
    table.insert(ret, Prefab(name.."_projectile", fn))
    table.insert(ret, Prefab(name.."_floater", fn))
end
"#;
        let variants = parse_bobbers(source, "prefabs/oceanfishingbobber.lua").unwrap();
        let names: Vec<&str> = variants.iter().map(|v| v.prefab.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "oceanfishingbobber_ball_projectile",
                "oceanfishingbobber_ball_floater",
                "oceanfishingbobber_twig_projectile",
                "oceanfishingbobber_twig_floater",
            ]
        );
        assert!(variants
            .iter()
            .all(|v| v.base.starts_with("oceanfishingbobber_")));
    }

    #[test]
    fn test_parse_spike_sizes() {
        let source = r#"
local SPIKE_SIZES =
{
    "short",
    "med",
    "tall",
}
"#;
        let variants = parse_spike_sizes(source, "glassspike", "prefabs/glass_spike.lua").unwrap();
        let names: Vec<&str> = variants.iter().map(|v| v.prefab.as_str()).collect();
        assert_eq!(
            names,
            vec!["glassspike_med", "glassspike_short", "glassspike_tall"]
        );
        assert!(variants.iter().all(|v| v.base == "glassspike"));
    }

    #[test]
    fn test_parse_indexed_variants() {
        let source = r#"
local NUM_A = 3
local NUM_B = 2

local ret = {}
local prefs = {}
for i = 1, NUM_A * NUM_B do
    local name = "goosplash"..tostring(i)
    table.insert(prefs, name)
    table.insert(ret, MakeSplash(name, i))
end
table.insert(ret, MakeSplash("goosplash", nil, prefs))
return unpack(ret)
"#;
        let variants =
            parse_indexed_variants(source, "goosplash", "prefabs/goosplash.lua").unwrap();
        let names: Vec<&str> = variants.iter().map(|v| v.prefab.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "goosplash1",
                "goosplash2",
                "goosplash3",
                "goosplash4",
                "goosplash5",
                "goosplash6",
            ]
        );
    }

    #[test]
    fn test_parse_literal_registrations() {
        let source = r#"
local assets = {}
return Prefab("ruins_statue_head", fn, assets),
    Prefab("ruins_statue_mage", fn, assets),
    Prefab("other_thing", fn, assets)
"#;
        let variants = parse_literal_registrations(
            source,
            "ancient_statue",
            &["ruins_statue_head", "ruins_statue_mage", "not_registered"],
            "prefabs/statueruins.lua",
        )
        .unwrap();
        let names: Vec<&str> = variants.iter().map(|v| v.prefab.as_str()).collect();
        assert_eq!(names, vec!["ruins_statue_head", "ruins_statue_mage"]);
        assert!(variants.iter().all(|v| v.base == "ancient_statue"));
    }
}
