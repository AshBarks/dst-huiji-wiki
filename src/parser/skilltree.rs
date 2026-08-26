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

    let mut nodes = Vec::new();
    for stmt in ast.nodes().stmts() {
        visit_stmt(stmt, &constants, &mut nodes);
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

/// Attempts to interpret a keyed table entry as a skill node.
fn try_parse_skill(
    name: &str,
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<SkillNode> {
    let mut pos: Option<(f64, f64)> = None;
    let mut group: Option<String> = None;
    let mut root = false;
    let mut connects = Vec::new();

    for field in table.fields() {
        let key = field_name(field)?;
        let value = field_value(field)?;
        match key.as_str() {
            "pos" => {
                let table = match value {
                    ast::Expression::TableConstructor(t) => t,
                    _ => return None,
                };
                let nums: Vec<f64> = table
                    .fields()
                    .iter()
                    .filter_map(|f| match f {
                        ast::Field::NoKey(expr) => eval_number(expr, constants),
                        _ => None,
                    })
                    .collect();
                if nums.len() < 2 {
                    return None;
                }
                pos = Some((nums[0], nums[1]));
            }
            "group" => group = eval_string(value),
            "root" => {
                root =
                    matches!(value, ast::Expression::Symbol(s) if s.token().to_string() == "true");
            }
            "connects" => {
                if let ast::Expression::TableConstructor(t) = value {
                    for f in t.fields() {
                        if let ast::Field::NoKey(expr) = f {
                            if let Some(s) = eval_string(expr) {
                                connects.push(s);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let (x, y) = pos?;
    Some(SkillNode {
        name: name.to_string(),
        x,
        y,
        group,
        root,
        connects,
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
}
