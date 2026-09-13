use super::*;

pub(super) fn eval_number(
    expr: &ast::Expression,
    constants: &BTreeMap<String, f64>,
) -> Option<f64> {
    match expr {
        ast::Expression::Number(n) => n.token().to_string().trim().parse::<f64>().ok(),
        ast::Expression::Var(ast::Var::Name(name)) => {
            constants.get(&name.token().to_string()).copied()
        }
        // `TUNING.SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD` 等点路径常量。
        ast::Expression::Var(ast::Var::Expression(var_expr)) => {
            let text: String = var_expr
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            constants.get(&text).copied()
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
                ast::BinOp::Caret(_) => Some(l.powf(r)),
                _ => None,
            }
        }
        ast::Expression::FunctionCall(call) => eval_math_call(call, constants),
        _ => None,
    }
}

/// Evaluates deterministic `math.*` helpers used in position/constant math.
pub(super) fn eval_math_call(
    call: &ast::FunctionCall,
    constants: &BTreeMap<String, f64>,
) -> Option<f64> {
    let head = call_head(call);
    let args: Vec<f64> = call_args(call)?
        .iter()
        .map(|a| eval_number(a, constants))
        .collect::<Option<Vec<_>>>()?;
    let value = match head.as_str() {
        "math.floor" => args.first()?.floor(),
        "math.ceil" => args.first()?.ceil(),
        "math.abs" => args.first()?.abs(),
        "math.sqrt" => args.first()?.sqrt(),
        "math.max" => args.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        "math.min" => args.iter().cloned().fold(f64::INFINITY, f64::min),
        "math.sin" => args.first()?.sin(),
        "math.cos" => args.first()?.cos(),
        "math.atan2" | "math.atan" => match args.len() {
            2 => args[0].atan2(args[1]),
            1 => args[0].atan(),
            _ => return None,
        },
        _ => return None,
    };
    Some(value)
}

pub(super) fn string_literal(s: &str) -> String {
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

pub(super) fn eval_string(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(s) => Some(string_literal(&s.to_string())),
        _ => None,
    }
}

/// 变量/点路径引用原文（如 `STRINGS.Skilltree.X.Y_TITLE`、`TUNING.X`），
/// 去除内部空白；字符串与其余形态返回 `None`。
pub(super) fn eval_var_path(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::Var(ast::Var::Name(name)) => Some(name.token().to_string()),
        ast::Expression::Var(ast::Var::Expression(var_expr)) => Some(
            var_expr
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect(),
        ),
        _ => None,
    }
}

pub(super) fn field_name(field: &ast::Field) -> Option<String> {
    if let ast::Field::NameKey { key, .. } = field {
        return Some(key.token().to_string());
    }
    None
}

pub(super) fn field_value(field: &ast::Field) -> Option<&ast::Expression> {
    if let ast::Field::NameKey { value, .. } = field {
        return Some(value);
    }
    None
}

pub(super) fn extract_pos(
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

/// Extracts a position from a call argument that is either a bare `{x, y}`
/// table or an extra-data wrapper like `{ pos = {x, y}, connects = {...} }`.
pub(super) fn extract_pos_arg(
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<(f64, f64)> {
    if let Some(pos) = extract_pos(table, constants) {
        return Some(pos);
    }
    for field in table.fields() {
        if let ast::Field::NameKey {
            key,
            value: ast::Expression::TableConstructor(t),
            ..
        } = field
        {
            if key.token().to_string() == "pos" {
                return extract_pos(t, constants);
            }
        }
    }
    None
}

pub(super) fn parse_positions(
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

pub(super) fn parse_skill_defs(
    table: &ast::TableConstructor,
    ctx: &ScanCtx<'_>,
) -> Vec<(String, RawSkillDef)> {
    let mut defs = Vec::new();
    for field in table.fields() {
        if let ast::Field::NameKey { key, value, .. } = field {
            let name = key.token().to_string();
            let def = match value {
                ast::Expression::TableConstructor(t) => raw_def_from_table(t, ctx),
                ast::Expression::FunctionCall(call) => raw_def_from_call(call, ctx),
                _ => None,
            };
            if let Some(def) = def {
                defs.push((name, def));
            }
        }
    }
    defs
}

pub(super) fn raw_def_from_table(
    table: &ast::TableConstructor,
    ctx: &ScanCtx<'_>,
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
                    def.pos = extract_pos(t, ctx.constants);
                }
            }
            "group" => def.group = eval_string(value),
            "icon" => def.icon = eval_string(value),
            "title" => def.title = eval_var_path(value),
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
            "lock_open" => {
                def.lock = true;
                def.lock_open = translate_lock_open_value(value, ctx);
            }
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
            "onactivate" => def.onactivate = true,
            "ondeactivate" => def.ondeactivate = true,
            "defaultfocus" => {
                def.defaultfocus =
                    matches!(value, ast::Expression::Symbol(s) if s.token().to_string() == "true");
            }
            "infographic" => {
                def.infographic =
                    matches!(value, ast::Expression::Symbol(s) if s.token().to_string() == "true");
            }
            "forced_focus" => {
                if let ast::Expression::TableConstructor(t) = value {
                    let mut map = serde_json::Map::new();
                    for f in t.fields() {
                        if let (Some(k), Some(v)) = (field_name(f), field_value(f)) {
                            if let Some(s) = eval_string(v) {
                                map.insert(k, serde_json::Value::String(s));
                            }
                        }
                    }
                    if !map.is_empty() {
                        def.forced_focus = Some(serde_json::Value::Object(map));
                    }
                }
            }
            "button_decorations" => {
                def.button_decorations = true;
                // 静态货架装饰：`button_decorations = CreateShelfDecor(...)`
                // 赋给的局部变量名。
                if let ast::Expression::Var(ast::Var::Name(name)) = value {
                    if let Some(list) = ctx.decorations.get(&name.token().to_string()) {
                        def.decorations = list.clone();
                    }
                }
            }
            _ => {}
        }
    }
    Some(def)
}
