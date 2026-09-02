use crate::error::Result;
use full_moon::ast;
use std::collections::BTreeMap;

/// A single skill node extracted from a `skilltree_<character>.lua` file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillNode {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub group: Option<String>,
    pub root: bool,
    pub connects: Vec<String>,
    pub icon: Option<String>,
    pub lock: bool,
    pub locks: Vec<String>,
    pub lock_open: Option<serde_json::Value>,
    pub tags: Vec<String>,
}

/// A full character skill tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillTree {
    pub character: String,
    pub nodes: Vec<SkillNode>,
}

impl SkillTree {
    /// Returns the distinct group names in first-seen order.
    pub fn groups(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for node in &self.nodes {
            if let Some(g) = &node.group {
                if !seen.contains(g) {
                    seen.push(g.clone());
                }
            }
        }
        seen
    }
}

#[derive(Debug, Default, Clone)]
struct RawSkillDef {
    pos: Option<(f64, f64)>,
    group: Option<String>,
    root: bool,
    connects: Vec<String>,
    icon: Option<String>,
    lock: bool,
    locks: Vec<String>,
    lock_open: Option<serde_json::Value>,
    tags: Vec<String>,
}

/// Parses a `skilltree_<character>.lua` source into a structured tree.
///
/// The game files define skills as keyed table entries containing a `pos`
/// field with explicit canvas coordinates, plus optional `connects`, `group`
/// and `root` fields. Coordinate values may reference numeric locals or use
/// simple arithmetic (e.g. `{ ALCHEMY_X, 176 - 54 - 38 }`), so a small
/// constant folder is included.
pub fn parse_skill_tree(source: &str, character: &str) -> Result<SkillTree> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;

    let mut constants = BTreeMap::new();
    collect_numeric_locals(ast.nodes().stmts(), &mut constants);

    let mut positions: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let mut skill_defs: BTreeMap<String, RawSkillDef> = BTreeMap::new();
    collect_table_locals(
        ast.nodes().stmts(),
        &constants,
        &mut positions,
        &mut skill_defs,
    );

    let mut nodes = Vec::new();
    if !skill_defs.is_empty() {
        for (name, def) in skill_defs {
            let position = def.pos.or_else(|| positions.get(&name).copied());
            let Some((x, y)) = position else {
                continue;
            };
            nodes.push(SkillNode {
                name: name.clone(),
                x,
                y,
                group: def.group,
                root: def.root,
                connects: def.connects,
                icon: if def.lock {
                    None
                } else {
                    def.icon.or_else(|| Some(name.clone()))
                },
                lock: def.lock,
                locks: def.locks,
                lock_open: def.lock_open,
                tags: def.tags,
            });
        }
    } else {
        // Fallback for unusual layouts: keep the previous generic table scan.
        for stmt in ast.nodes().stmts() {
            visit_stmt(stmt, &constants, &mut nodes);
        }
    }

    Ok(SkillTree {
        character: character.to_string(),
        nodes,
    })
}

fn eval_number(expr: &ast::Expression, constants: &BTreeMap<String, f64>) -> Option<f64> {
    match expr {
        ast::Expression::Number(n) => n.token().to_string().trim().parse::<f64>().ok(),
        ast::Expression::Var(ast::Var::Name(name)) => {
            constants.get(&name.token().to_string()).copied()
        }
        ast::Expression::Parentheses { expression, .. } => eval_number(expression, constants),
        ast::Expression::UnaryOperator { unop, expression } => {
            if matches!(unop, ast::UnOp::Minus(_)) {
                eval_number(expression, constants).map(|v| -v)
            } else {
                None
            }
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            let l = eval_number(lhs, constants)?;
            let r = eval_number(rhs, constants)?;
            match binop {
                ast::BinOp::Plus(_) => Some(l + r),
                ast::BinOp::Minus(_) => Some(l - r),
                ast::BinOp::Star(_) => Some(l * r),
                ast::BinOp::Slash(_) => Some(l / r),
                _ => None,
            }
        }
        _ => None,
    }
}

fn string_literal(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else if s.starts_with("[[") && s.ends_with("]]") && s.len() >= 4 {
        s[2..s.len() - 2].trim().to_string()
    } else {
        s.to_string()
    }
}

fn eval_string(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(s) => Some(string_literal(&s.to_string())),
        _ => None,
    }
}

fn field_name(field: &ast::Field) -> Option<String> {
    if let ast::Field::NameKey { key, .. } = field {
        return Some(key.token().to_string());
    }
    None
}

fn field_value(field: &ast::Field) -> Option<&ast::Expression> {
    if let ast::Field::NameKey { value, .. } = field {
        return Some(value);
    }
    None
}

fn extract_pos(
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<(f64, f64)> {
    let mut x = None;
    let mut y = None;
    let mut nums = Vec::new();
    for field in table.fields() {
        match field {
            ast::Field::NoKey(expr) => {
                if let Some(v) = eval_number(expr, constants) {
                    nums.push(v);
                }
            }
            ast::Field::NameKey { key, value, .. } => {
                let key_str = key.token().to_string();
                if key_str == "x" {
                    x = eval_number(value, constants);
                } else if key_str == "y" {
                    y = eval_number(value, constants);
                }
            }
            _ => {}
        }
    }
    if let (Some(x), Some(y)) = (x, y) {
        return Some((x, y));
    }
    if nums.len() >= 2 {
        Some((nums[0], nums[1]))
    } else {
        None
    }
}

fn parse_positions(
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> BTreeMap<String, (f64, f64)> {
    let mut map = BTreeMap::new();
    for field in table.fields() {
        if let ast::Field::NameKey {
            key,
            value: ast::Expression::TableConstructor(t),
            ..
        } = field
        {
            if let Some(pos) = extract_pos(t, constants) {
                map.insert(key.token().to_string(), pos);
            }
        }
    }
    map
}

fn parse_skill_defs(
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> BTreeMap<String, RawSkillDef> {
    let mut map = BTreeMap::new();
    for field in table.fields() {
        if let ast::Field::NameKey { key, value, .. } = field {
            let name = key.token().to_string();
            let def = match value {
                ast::Expression::TableConstructor(t) => raw_def_from_table(&name, t, constants),
                ast::Expression::FunctionCall(call) => raw_def_from_call(&name, call, constants),
                _ => None,
            };
            if let Some(def) = def {
                map.insert(name, def);
            }
        }
    }
    map
}

fn raw_def_from_table(
    _name: &str,
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<RawSkillDef> {
    let mut def = RawSkillDef::default();
    for field in table.fields() {
        let Some(key) = field_name(field) else {
            continue;
        };
        let Some(value) = field_value(field) else {
            continue;
        };
        match key.as_str() {
            "pos" => {
                if let ast::Expression::TableConstructor(t) = value {
                    def.pos = extract_pos(t, constants);
                }
            }
            "group" => def.group = eval_string(value),
            "icon" => def.icon = eval_string(value),
            "root" => {
                def.root =
                    matches!(value, ast::Expression::Symbol(s) if s.token().to_string() == "true");
            }
            "connects" => {
                if let ast::Expression::TableConstructor(t) = value {
                    for f in t.fields() {
                        if let ast::Field::NoKey(expr) = f {
                            if let Some(s) = eval_string(expr) {
                                def.connects.push(s);
                            }
                        }
                    }
                }
            }
            "lock_open" => def.lock = true,
            "tags" => {
                if let ast::Expression::TableConstructor(t) = value {
                    for f in t.fields() {
                        if let ast::Field::NoKey(expr) = f {
                            if let Some(s) = eval_string(expr) {
                                def.tags.push(s);
                            }
                        }
                    }
                }
            }
            "locks" => {
                if let ast::Expression::TableConstructor(t) = value {
                    for f in t.fields() {
                        if let ast::Field::NoKey(expr) = f {
                            if let Some(s) = eval_string(expr) {
                                def.locks.push(s);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Some(def)
}

fn raw_def_from_call(
    _name: &str,
    call: &ast::FunctionCall,
    constants: &BTreeMap<String, f64>,
) -> Option<RawSkillDef> {
    let mut def = RawSkillDef {
        lock: true,
        ..Default::default()
    };
    let mut group: Option<String> = None;
    let mut count: Option<f64> = None;
    if let Some(args) = call_args(call) {
        for arg in &args {
            match arg {
                ast::Expression::String(s) => {
                    if def.group.is_none() {
                        let value = string_literal(&s.to_string());
                        def.group = Some(value.clone());
                        group = Some(value);
                    }
                }
                ast::Expression::TableConstructor(t) if def.pos.is_none() => {
                    def.pos = extract_pos(t, constants);
                }
                ast::Expression::Number(n) => {
                    count = n.token().to_string().trim().parse::<f64>().ok();
                }
                _ => {}
            }
        }
    }

    let call_text = call.to_string();
    let head = call_text
        .split(['(', '{'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    def.lock_open = if head.ends_with("CreateSkillCountLock") {
        match (group, count) {
            (Some(g), Some(c)) => Some(serde_json::json!({
                "GreaterOrEqThan": {
                    "left": { "CountTags": g },
                    "right": c
                }
            })),
            _ => None,
        }
    } else if head.ends_with("MakeNoShadowLock") {
        Some(serde_json::json!({
            "Eq": {
                "left": { "CountTags": "shadow_favor" },
                "right": 0
            }
        }))
    } else if head.ends_with("MakeNoLunarLock") {
        Some(serde_json::json!({
            "Eq": {
                "left": { "CountTags": "lunar_favor" },
                "right": 0
            }
        }))
    } else if head.ends_with("MakeFuelWeaverLock") || head.ends_with("MakeCelestialChampionLock") {
        // 外部成就类 lock 在本地技能树中默认视为已解锁。
        Some(serde_json::json!(true))
    } else {
        None
    };

    Some(def)
}

fn call_args(call: &ast::FunctionCall) -> Option<Vec<ast::Expression>> {
    let suffixes: Vec<_> = call.suffixes().collect();
    if suffixes.len() != 1 {
        return None;
    }
    match &suffixes[0] {
        ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        })) => Some(arguments.iter().cloned().collect()),
        ast::Suffix::Call(ast::Call::MethodCall(method_call)) => match method_call.args() {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                Some(arguments.iter().cloned().collect())
            }
            _ => None,
        },
        _ => None,
    }
}

fn collect_table_locals<'a>(
    stmts: impl Iterator<Item = &'a ast::Stmt>,
    constants: &BTreeMap<String, f64>,
    positions: &mut BTreeMap<String, (f64, f64)>,
    skill_defs: &mut BTreeMap<String, RawSkillDef>,
) {
    for stmt in stmts {
        if let ast::Stmt::LocalAssignment(assignment) = stmt {
            for (name, expr) in assignment
                .names()
                .iter()
                .zip(assignment.expressions().iter())
            {
                let name_str = name.token().to_string();
                if name_str == "POSITIONS" {
                    if let ast::Expression::TableConstructor(t) = expr {
                        *positions = parse_positions(t, constants);
                    }
                } else if name_str == "skills" {
                    if let ast::Expression::TableConstructor(t) = expr {
                        *skill_defs = parse_skill_defs(t, constants);
                    }
                }
            }
        }

        match stmt {
            ast::Stmt::Do(stmt) => {
                collect_table_locals_in_block(stmt.block(), constants, positions, skill_defs);
            }
            ast::Stmt::While(stmt) => {
                collect_table_locals_in_block(stmt.block(), constants, positions, skill_defs);
            }
            ast::Stmt::Repeat(stmt) => {
                collect_table_locals_in_block(stmt.block(), constants, positions, skill_defs);
            }
            ast::Stmt::If(stmt) => {
                collect_table_locals_in_block(stmt.block(), constants, positions, skill_defs);
                if let Some(else_ifs) = stmt.else_if() {
                    for branch in else_ifs {
                        collect_table_locals_in_block(
                            branch.block(),
                            constants,
                            positions,
                            skill_defs,
                        );
                    }
                }
                if let Some(block) = stmt.else_block() {
                    collect_table_locals_in_block(block, constants, positions, skill_defs);
                }
            }
            ast::Stmt::NumericFor(stmt) => {
                collect_table_locals_in_block(stmt.block(), constants, positions, skill_defs);
            }
            ast::Stmt::GenericFor(stmt) => {
                collect_table_locals_in_block(stmt.block(), constants, positions, skill_defs);
            }
            ast::Stmt::LocalFunction(func) => {
                collect_table_locals_in_block(
                    func.body().block(),
                    constants,
                    positions,
                    skill_defs,
                );
            }
            ast::Stmt::FunctionDeclaration(func) => {
                collect_table_locals_in_block(
                    func.body().block(),
                    constants,
                    positions,
                    skill_defs,
                );
            }
            _ => {}
        }
    }
}

fn collect_table_locals_in_block(
    block: &ast::Block,
    constants: &BTreeMap<String, f64>,
    positions: &mut BTreeMap<String, (f64, f64)>,
    skill_defs: &mut BTreeMap<String, RawSkillDef>,
) {
    collect_table_locals(block.stmts(), constants, positions, skill_defs);
}

/// Attempts to interpret a keyed table entry as a skill node.
fn try_parse_skill(
    name: &str,
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<SkillNode> {
    let def = raw_def_from_table(name, table, constants)?;
    let (x, y) = def.pos?;
    Some(SkillNode {
        name: name.to_string(),
        x,
        y,
        group: def.group,
        root: def.root,
        connects: def.connects,
        icon: if def.lock {
            None
        } else {
            def.icon.or_else(|| Some(name.to_string()))
        },
        lock: def.lock,
        locks: def.locks,
        lock_open: def.lock_open,
        tags: def.tags,
    })
}

fn inspect_table(
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
    nodes: &mut Vec<SkillNode>,
) {
    // Recurse into nested tables first (skills tables may be wrapped).
    for field in table.fields() {
        if let Some(value) = field_value(field) {
            visit_expression(value, constants, nodes);
        }
        if let ast::Field::NoKey(expr) = field {
            visit_expression(expr, constants, nodes);
        }
    }

    for field in table.fields() {
        if let ast::Field::NameKey { key, value, .. } = field {
            let node = match value {
                ast::Expression::TableConstructor(inner) => {
                    let key_str = key.token().to_string();
                    try_parse_skill(&key_str, inner, constants)
                }
                _ => None,
            };
            if let Some(node) = node {
                nodes.push(node);
            }
        }
    }
}

fn visit_expression(
    expr: &ast::Expression,
    constants: &BTreeMap<String, f64>,
    nodes: &mut Vec<SkillNode>,
) {
    match expr {
        ast::Expression::TableConstructor(table) => inspect_table(table, constants, nodes),
        ast::Expression::Parentheses { expression, .. } => {
            visit_expression(expression, constants, nodes)
        }
        ast::Expression::BinaryOperator { lhs, rhs, .. } => {
            visit_expression(lhs, constants, nodes);
            visit_expression(rhs, constants, nodes);
        }
        ast::Expression::UnaryOperator { expression, .. } => {
            visit_expression(expression, constants, nodes)
        }
        _ => {}
    }
}

fn visit_block(block: &ast::Block, constants: &BTreeMap<String, f64>, nodes: &mut Vec<SkillNode>) {
    for stmt in block.stmts() {
        visit_stmt(stmt, constants, nodes);
    }
    if let Some(ast::LastStmt::Return(return_stmt)) = block.last_stmt() {
        for expr in return_stmt.returns() {
            visit_expression(expr, constants, nodes);
        }
    }
}

fn visit_stmt(stmt: &ast::Stmt, constants: &BTreeMap<String, f64>, nodes: &mut Vec<SkillNode>) {
    match stmt {
        ast::Stmt::Assignment(assignment) => {
            for expr in assignment.expressions() {
                visit_expression(expr, constants, nodes);
            }
        }
        ast::Stmt::LocalAssignment(assignment) => {
            for expr in assignment.expressions() {
                visit_expression(expr, constants, nodes);
            }
        }
        ast::Stmt::FunctionCall(call) => {
            for suffix in call.suffixes() {
                if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                    match args {
                        ast::FunctionArgs::Parentheses { arguments, .. } => {
                            for arg in arguments.iter() {
                                visit_expression(arg, constants, nodes);
                            }
                        }
                        ast::FunctionArgs::TableConstructor(table) => {
                            inspect_table(table, constants, nodes);
                        }
                        _ => {}
                    }
                }
            }
        }
        ast::Stmt::Do(stmt) => visit_block(stmt.block(), constants, nodes),
        ast::Stmt::While(stmt) => visit_block(stmt.block(), constants, nodes),
        ast::Stmt::Repeat(stmt) => visit_block(stmt.block(), constants, nodes),
        ast::Stmt::If(stmt) => {
            if let Some(branches) = stmt.else_if() {
                for branch in branches {
                    visit_block(branch.block(), constants, nodes);
                }
            }
            if let Some(block) = stmt.else_block() {
                visit_block(block, constants, nodes);
            }
        }
        ast::Stmt::NumericFor(stmt) => visit_block(stmt.block(), constants, nodes),
        ast::Stmt::GenericFor(stmt) => visit_block(stmt.block(), constants, nodes),
        ast::Stmt::LocalFunction(func) => visit_block(func.body().block(), constants, nodes),
        ast::Stmt::FunctionDeclaration(func) => visit_block(func.body().block(), constants, nodes),
        _ => {}
    }
}

fn collect_numeric_locals<'a>(
    stmts: impl Iterator<Item = &'a ast::Stmt>,
    constants: &mut BTreeMap<String, f64>,
) {
    for stmt in stmts {
        if let ast::Stmt::LocalAssignment(assignment) = stmt {
            for (name, expr) in assignment
                .names()
                .iter()
                .zip(assignment.expressions().iter())
            {
                if let Some(value) = eval_number(expr, constants) {
                    constants.insert(name.token().to_string(), value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
local TORCH_X = -190
local ALCHEMY_X = -58

local ORDERS =
{
    {"torch",           { TORCH_X   , 176 + 30 }},
    {"alchemy",         { ALCHEMY_X , 176 + 30 }},
}

local function BuildSkillsData(SkillTreeFns)
    local skills =
    {
        wilson_alchemy_1 = {
            title = STRINGS.SKILLTREE.WILSON.WILSON_ALCHEMY_1_TITLE,
            desc = STRINGS.SKILLTREE.WILSON.WILSON_ALCHEMY_1_DESC,
            icon = "wilson_alchemy_1",
            pos = {ALCHEMY_X, 176},
            group = "alchemy",
            tags = {"alchemy"},
            root = true,
            connects = {
                "wilson_alchemy_2",
                "wilson_alchemy_3",
            },
        },
        wilson_alchemy_2 = {
            icon = "wilson_alchemy_gem_1",
            pos = {ALCHEMY_X, 176-54},
            group = "alchemy",
            connects = {
                "wilson_alchemy_5",
            },
        },
        wilson_torch_1 = {
            icon = "wilson_torch",
            pos = {TORCH_X, 100},
            group = "torch",
        },
    }

    return SkillTreeFns.CreateSkillTree(skills)
end
"#;

    #[test]
    fn test_parse_skill_tree_basic() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        assert_eq!(tree.character, "wilson");
        assert_eq!(tree.nodes.len(), 3);

        let alchemy1 = tree
            .nodes
            .iter()
            .find(|n| n.name == "wilson_alchemy_1")
            .unwrap();
        assert_eq!(alchemy1.x, -58.0);
        assert_eq!(alchemy1.y, 176.0);
        assert_eq!(alchemy1.icon.as_deref(), Some("wilson_alchemy_1"));
        assert!(alchemy1.root);
        assert_eq!(
            alchemy1.connects,
            vec![
                "wilson_alchemy_2".to_string(),
                "wilson_alchemy_3".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_skill_tree_arithmetic_and_negatives() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();

        let alchemy2 = tree
            .nodes
            .iter()
            .find(|n| n.name == "wilson_alchemy_2")
            .unwrap();
        assert_eq!(alchemy2.y, 122.0); // 176 - 54
        assert_eq!(alchemy2.icon.as_deref(), Some("wilson_alchemy_gem_1"));

        let torch = tree
            .nodes
            .iter()
            .find(|n| n.name == "wilson_torch_1")
            .unwrap();
        assert_eq!(torch.x, -190.0); // negative local constant
        assert!(!torch.root);
        assert!(torch.connects.is_empty());
    }

    #[test]
    fn test_groups_in_order() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        assert_eq!(
            tree.groups(),
            vec!["alchemy".to_string(), "torch".to_string()]
        );
    }

    #[test]
    fn test_no_pos_entries_skipped() {
        // The ORDERS table entries are NoKey tables and must not be picked up.
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        assert!(tree.nodes.iter().all(|n| n.name.starts_with("wilson_")));
    }

    #[test]
    fn test_parse_positions_table_and_lock_helpers() {
        let source = r#"
local POSITIONS = {
    skill_one = { 10, 20 },
    lock_one = { x = 30, y = 40 },
}

local function BuildSkillsData(SkillTreeFns)
    local function MakeLock()
        return { lock_open = function() return true end }
    end

    local skills = {
        skill_one = { group = "g" },
        lock_one = MakeLock(),
    }

    return SkillTreeFns.CreateSkillTree(skills)
end
"#;
        let tree = parse_skill_tree(source, "test").unwrap();
        assert_eq!(tree.nodes.len(), 2);

        let skill = tree.nodes.iter().find(|n| n.name == "skill_one").unwrap();
        assert_eq!((skill.x, skill.y), (10.0, 20.0));
        assert!(!skill.lock);
        assert_eq!(skill.icon.as_deref(), Some("skill_one"));

        let lock = tree.nodes.iter().find(|n| n.name == "lock_one").unwrap();
        assert_eq!((lock.x, lock.y), (30.0, 40.0));
        assert!(lock.lock);
        assert!(lock.icon.is_none());
    }
}
