use super::*;

/// Translates a `lock_open` field value into the wiki's declarative JSON.
///
/// Falls back to `true` ("open") whenever the condition cannot be resolved
/// statically, matching the wiki's handling of external achievements and
/// custom helper functions.
pub(super) fn translate_lock_open_value(
    value: &ast::Expression,
    ctx: &ScanCtx<'_>,
) -> Option<serde_json::Value> {
    if matches!(value, ast::Expression::Symbol(s) if s.token().to_string() == "true") {
        return Some(serde_json::json!(true));
    }
    let cond = match value {
        ast::Expression::Function(body) => {
            translate_lock_body(body.body().block(), ctx, 0, &mut HashSet::new())
        }
        ast::Expression::FunctionCall(call) => translate_lock_call(call, ctx),
        ast::Expression::Var(ast::Var::Name(name)) => {
            // e.g. `lock_open = BasicShadowAllegianceLockFn`
            match ctx.local_fns.get(&name.token().to_string()) {
                Some(block) => translate_lock_body(block, ctx, 0, &mut HashSet::new()),
                None => Cond::Unknown,
            }
        }
        _ => Cond::Unknown,
    };
    Some(cond_to_lock_open_json(cond))
}

/// Translates a call-shaped `lock_open`: game helper functions first, then
/// local helper functions defined in the same file.
pub(super) fn translate_lock_call(call: &ast::FunctionCall, ctx: &ScanCtx<'_>) -> Cond {
    let head = call_head(call);
    match head.as_str() {
        h if h.ends_with("CreateSkillCountLock") => {
            let args = call_args(call).unwrap_or_default();
            let group = args.first().and_then(eval_string);
            let count = args.get(1).and_then(|a| eval_number(a, ctx.constants));
            match (group, count) {
                (Some(g), Some(c)) => Cond::Cmp(
                    "GreaterOrEqThan",
                    Box::new(Cond::CountTags(g)),
                    Box::new(Cond::Number(c)),
                ),
                _ => Cond::Unknown,
            }
        }
        h if h.ends_with("MakeNoShadowLock") => Cond::eq_count_tags("shadow_favor"),
        h if h.ends_with("MakeNoLunarLock") => Cond::eq_count_tags("lunar_favor"),
        // 外部成就类（击败远古织影者 / 天体英雄、 accomplishments）：
        // 本地无法验证，视为已解锁（与维基的处理一致）。
        h if h.ends_with("MakeFuelWeaverLock")
            || h.ends_with("MakeCelestialChampionLock")
            || h.ends_with("CreateAccomplishmentLockFn")
            || h.ends_with("CreateAccomplishmentCountLockFn") =>
        {
            Cond::Unknown
        }
        _ => {
            // Local helper functions (e.g. BasicShadowAllegianceLockFn).
            if let Some(block) = ctx.local_fns.get(head.as_str()) {
                translate_lock_body(block, ctx, 0, &mut HashSet::new())
            } else {
                Cond::Unknown
            }
        }
    }
}

/// Interprets a lock closure body: `if <cond> then return false end` guards
/// are inverted and AND-ed with the unconditional return value.
pub(super) fn translate_lock_body(
    block: &ast::Block,
    ctx: &ScanCtx<'_>,
    depth: usize,
    visiting: &mut HashSet<String>,
) -> Cond {
    if depth > 4 {
        return Cond::Unknown;
    }
    let mut vars: HashMap<String, Cond> = HashMap::new();
    let mut guards: Vec<Cond> = Vec::new();
    let mut positive: Option<Cond> = None;

    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::If(if_stmt) => {
                handle_lock_if_branch(
                    Some(if_stmt.condition()),
                    if_stmt.block(),
                    ctx,
                    &vars,
                    depth,
                    visiting,
                    &mut guards,
                    &mut positive,
                );
                if let Some(else_ifs) = if_stmt.else_if() {
                    for branch in else_ifs {
                        handle_lock_if_branch(
                            Some(branch.condition()),
                            branch.block(),
                            ctx,
                            &vars,
                            depth,
                            visiting,
                            &mut guards,
                            &mut positive,
                        );
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    handle_lock_if_branch(
                        None,
                        else_block,
                        ctx,
                        &vars,
                        depth,
                        visiting,
                        &mut guards,
                        &mut positive,
                    );
                }
            }
            ast::Stmt::LocalAssignment(assignment) => {
                for (name, expr) in assignment
                    .names()
                    .iter()
                    .zip(assignment.expressions().iter())
                {
                    let value = translate_expr(expr, ctx, &vars, depth, visiting);
                    vars.insert(name.token().to_string(), value);
                }
            }
            _ => {}
        }
    }

    // A trailing `return ...` is stored as the block's last statement, not
    // among the regular statements. A trailing `nil`/`false` (the game's
    // "Important to return nil and not false" idiom) just means "closed
    // otherwise" and must not clobber a conditional-true branch.
    if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
        if let Some(first) = ret.returns().iter().next() {
            let tail = translate_expr(first, ctx, &vars, depth, visiting);
            match tail {
                Cond::False => {
                    if positive.is_none() {
                        positive = Some(Cond::False);
                    }
                }
                _ => positive = Some(tail),
            }
        }
    }

    let value = match positive {
        Some(Cond::Unknown) | None => Cond::True,
        Some(other) => other,
    };
    // Unknown guards（无法静态识别的前提）被丢弃，锁按已解锁处理。
    let mut value = value;
    for guard in guards.into_iter().rev() {
        if guard.contains_unknown() {
            continue;
        }
        value = Cond::And(Box::new(guard), Box::new(value));
    }
    value
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_lock_if_branch(
    condition: Option<&ast::Expression>,
    block: &ast::Block,
    ctx: &ScanCtx<'_>,
    vars: &HashMap<String, Cond>,
    depth: usize,
    visiting: &mut HashSet<String>,
    guards: &mut Vec<Cond>,
    positive: &mut Option<Cond>,
) {
    let Some(cond_expr) = condition else {
        // Unconditional else branch: its return is a plain positive value.
        if let Some(ret) = top_level_return(block) {
            if let Some(first) = ret.returns().iter().next() {
                *positive = Some(translate_expr(first, ctx, vars, depth, visiting));
            }
        }
        return;
    };
    if is_readonly_ref(cond_expr) {
        // `if readonly then return "question" end` — local simulation always
        // runs in "answered" mode, so this branch constrains nothing.
        return;
    }
    let cond = translate_expr(cond_expr, ctx, vars, depth, visiting);
    let Some(ret) = top_level_return(block) else {
        return;
    };
    let Some(first) = ret.returns().iter().next() else {
        return;
    };
    match first {
        ast::Expression::Symbol(s) if s.token().to_string() == "false" => {
            guards.push(invert_cond(cond));
        }
        ast::Expression::Symbol(s) if s.token().to_string() == "true" => {
            // Conditional open: opens when cond holds (rare).
            let opened = positive.take().unwrap_or(Cond::False);
            *positive = Some(Cond::Or(Box::new(cond), Box::new(opened)));
        }
        ast::Expression::String(s) if string_literal(&s.to_string()) == "question" => {}
        _ => {}
    }
}

/// Returns the block's trailing `return`, if any. In Lua a `return` is always
/// the last statement of its block, so nested conditional returns are handled
/// by the branch walker instead.
pub(super) fn top_level_return(block: &ast::Block) -> Option<&ast::Return> {
    match block.last_stmt() {
        Some(ast::LastStmt::Return(ret)) => Some(ret),
        _ => None,
    }
}

pub(super) fn is_readonly_ref(expr: &ast::Expression) -> bool {
    matches!(expr, ast::Expression::Var(ast::Var::Name(n)) if n.token().to_string() == "readonly")
}

/// Internal representation of a translated `lock_open` condition.
#[derive(Debug, Clone)]
pub(super) enum Cond {
    True,
    False,
    Number(f64),
    Str(String),
    /// 无法静态识别（外部成就、自定义函数等），最终按"已解锁"处理。
    Unknown,
    CountTags(String),
    CountSkills,
    ActivatedSkill(String),
    /// GreaterThan / GreaterOrEqThan / LessThan / LessOrEqThan / Eq
    Cmp(&'static str, Box<Cond>, Box<Cond>),
    Add(Box<Cond>, Box<Cond>),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
    /// 沃拓克斯天秤倾向：`nice`/`naughty` 为两侧计数表达式，`affinity`
    /// 求值出 `"lunar"`/`"shadow"` 时为对应阵营加成一次，`threshold` 为
    /// 倾斜阈值；整体求值为 `"nice"`/`"naughty"`/`nil`。
    Inclination {
        nice: Box<Cond>,
        naughty: Box<Cond>,
        affinity: Box<Cond>,
        threshold: f64,
    },
}

impl Cond {
    fn eq_count_tags(tag: &str) -> Cond {
        Cond::Cmp(
            "Eq",
            Box::new(Cond::CountTags(tag.to_string())),
            Box::new(Cond::Number(0.0)),
        )
    }

    /// `Eq(count-expr, 0)` for whatever count expression is on the left.
    fn eq_count_tags_str(left: &Cond, value: f64) -> Cond {
        Cond::Cmp("Eq", Box::new(left.clone()), Box::new(Cond::Number(value)))
    }

    pub(super) fn contains_unknown(&self) -> bool {
        match self {
            Cond::Unknown => true,
            Cond::Cmp(_, l, r) | Cond::Add(l, r) | Cond::And(l, r) | Cond::Or(l, r) => {
                l.contains_unknown() || r.contains_unknown()
            }
            Cond::Not(x) => x.contains_unknown(),
            Cond::Inclination {
                nice,
                naughty,
                affinity,
                ..
            } => {
                nice.contains_unknown() || naughty.contains_unknown() || affinity.contains_unknown()
            }
            _ => false,
        }
    }
}

pub(super) fn invert_cond(cond: Cond) -> Cond {
    match cond {
        Cond::Cmp("GreaterThan", l, r)
            if matches!(l.as_ref(), Cond::CountTags(_) | Cond::CountSkills)
                && matches!(r.as_ref(), Cond::Number(n) if *n == 0.0) =>
        {
            Cond::Cmp("Eq", l, r)
        }
        Cond::Cmp("GreaterThan", l, r) => Cond::Cmp("LessOrEqThan", l, r),
        Cond::Cmp("LessOrEqThan", l, r) => Cond::Cmp("GreaterThan", l, r),
        Cond::Cmp("GreaterOrEqThan", l, r) => Cond::Cmp("LessThan", l, r),
        Cond::Cmp("LessThan", l, r) => Cond::Cmp("GreaterOrEqThan", l, r),
        Cond::Cmp("Eq", l, r) => match (l.as_ref(), r.as_ref()) {
            // Count semantics: "tag count == 0 blocks the lock" ⇒ needs >= 1.
            (Cond::CountTags(_) | Cond::CountSkills, Cond::Number(n)) if *n == 0.0 => {
                Cond::Cmp("GreaterOrEqThan", l, Box::new(Cond::Number(1.0)))
            }
            _ => Cond::Not(Box::new(Cond::Cmp("Eq", l, r))),
        },
        Cond::And(l, r) => Cond::Or(Box::new(invert_cond(*l)), Box::new(invert_cond(*r))),
        Cond::Or(l, r) => Cond::And(Box::new(invert_cond(*l)), Box::new(invert_cond(*r))),
        Cond::Not(x) => *x,
        other => Cond::Not(Box::new(other)),
    }
}

pub(super) fn simplify_cond(cond: Cond) -> Cond {
    match cond {
        Cond::And(l, r) => {
            let items = flatten_chain(Cond::And(l, r));
            let items: Vec<Cond> = items.into_iter().map(simplify_cond).collect();
            if items.iter().any(|c| matches!(c, Cond::False)) {
                return Cond::False;
            }
            let items: Vec<Cond> = items
                .into_iter()
                .filter(|c| !matches!(c, Cond::True))
                .collect();
            fold_chain("And", items)
        }
        Cond::Or(l, r) => {
            let items = flatten_chain(Cond::Or(l, r));
            let items: Vec<Cond> = items.into_iter().map(simplify_cond).collect();
            if items.iter().any(|c| matches!(c, Cond::True)) {
                return Cond::True;
            }
            let items: Vec<Cond> = items
                .into_iter()
                .filter(|c| !matches!(c, Cond::False))
                .collect();
            fold_chain("Or", items)
        }
        Cond::Not(x) => match simplify_cond(*x) {
            Cond::True => Cond::False,
            Cond::False => Cond::True,
            x => Cond::Not(Box::new(x)),
        },
        Cond::Cmp(op, l, r) => canonicalize_cmp(op, *l, *r),
        other => other,
    }
}

/// Flattens nested same-op `And`/`Or` chains into a list of operands.
pub(super) fn flatten_chain(cond: Cond) -> Vec<Cond> {
    match cond {
        Cond::And(l, r) => {
            let mut items = flatten_chain(*l);
            items.extend(flatten_chain(*r));
            items
        }
        Cond::Or(l, r) => {
            let mut items = flatten_chain(*l);
            items.extend(flatten_chain(*r));
            items
        }
        other => vec![other],
    }
}

/// Re-folds a flattened chain right-associatively, matching the wiki's
/// hand-written `And(a, And(b, c))` nesting.
pub(super) fn fold_chain(op: &'static str, items: Vec<Cond>) -> Cond {
    let make = |l, r| {
        if op == "And" {
            Cond::And(Box::new(l), Box::new(r))
        } else {
            Cond::Or(Box::new(l), Box::new(r))
        }
    };
    let mut iter = items.into_iter().rev();
    let Some(last) = iter.next() else {
        return Cond::True;
    };
    iter.fold(last, |acc, item| make(item, acc))
}

/// Normalizes count comparisons: `count <= 0` / `count < 1` read better as
/// `Eq(count, 0)`, matching the wiki's encodings.
pub(super) fn canonicalize_cmp(op: &'static str, l: Cond, r: Cond) -> Cond {
    let is_count = matches!(l, Cond::CountTags(_) | Cond::CountSkills);
    if is_count {
        if let Cond::Number(n) = r {
            if op == "LessOrEqThan" && n == 0.0 {
                return Cond::eq_count_tags_str(&l, 0.0);
            }
            if op == "LessThan" && n == 1.0 {
                return Cond::eq_count_tags_str(&l, 0.0);
            }
        }
    }
    Cond::Cmp(op, Box::new(l), Box::new(r))
}

pub(super) fn cond_to_lock_open_json(cond: Cond) -> serde_json::Value {
    let simplified = simplify_cond(cond);
    match &simplified {
        Cond::True | Cond::Unknown => serde_json::json!(true),
        Cond::False => serde_json::json!(false),
        _ => cond_to_json(&simplified),
    }
}

/// Serializes counts/positions as JSON integers when the value is integral,
/// matching the wiki's hand-written JSON (`2` instead of `2.0`).
pub(super) fn number_to_json(n: f64) -> serde_json::Value {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 9.0e15 {
        serde_json::json!(n as i64)
    } else {
        serde_json::json!(n)
    }
}

pub(super) fn cond_to_json(cond: &Cond) -> serde_json::Value {
    match cond {
        Cond::True | Cond::Unknown => serde_json::json!(true),
        Cond::False => serde_json::json!(false),
        Cond::Number(n) => number_to_json(*n),
        Cond::Str(s) => serde_json::json!(s),
        Cond::CountTags(tag) => serde_json::json!({ "CountTags": tag }),
        Cond::CountSkills => serde_json::json!({ "CountSkills": "CountSkills" }),
        Cond::ActivatedSkill(skill) => serde_json::json!({ "ActivatedSkill": skill }),
        Cond::Cmp(op, l, r) => {
            let mut obj = serde_json::Map::new();
            obj.insert(
                (*op).to_string(),
                serde_json::json!({ "left": cond_to_json(l), "right": cond_to_json(r) }),
            );
            serde_json::Value::Object(obj)
        }
        Cond::Add(l, r) => serde_json::json!({
            "Add": { "left": cond_to_json(l), "right": cond_to_json(r) }
        }),
        Cond::And(l, r) => serde_json::json!({
            "And": { "left": cond_to_json(l), "right": cond_to_json(r) }
        }),
        Cond::Or(l, r) => serde_json::json!({
            "Or": { "left": cond_to_json(l), "right": cond_to_json(r) }
        }),
        Cond::Not(x) => serde_json::json!({ "Not": cond_to_json(x) }),
        Cond::Inclination {
            nice,
            naughty,
            affinity,
            threshold,
        } => serde_json::json!({
            "Inclination": {
                "nice": cond_to_json(nice),
                "naughty": cond_to_json(naughty),
                "affinity": cond_to_json(affinity),
                "threshold": number_to_json(*threshold),
            }
        }),
    }
}

pub(super) fn translate_expr(
    expr: &ast::Expression,
    ctx: &ScanCtx<'_>,
    vars: &HashMap<String, Cond>,
    depth: usize,
    visiting: &mut HashSet<String>,
) -> Cond {
    // Pure numeric expressions (locals/arithmetic/math.*) fold to a number.
    if let Some(n) = eval_number(expr, ctx.constants) {
        return Cond::Number(n);
    }
    match expr {
        ast::Expression::Symbol(s) => match s.token().to_string().as_str() {
            "true" => Cond::True,
            "false" => Cond::False,
            "nil" => Cond::False,
            _ => Cond::Unknown,
        },
        ast::Expression::Number(n) => Cond::Number(
            n.token()
                .to_string()
                .trim()
                .parse::<f64>()
                .unwrap_or(f64::NAN),
        ),
        ast::Expression::String(s) => Cond::Str(string_literal(&s.to_string())),
        ast::Expression::Parentheses { expression, .. } => {
            translate_expr(expression, ctx, vars, depth, visiting)
        }
        ast::Expression::Var(var) => translate_var(var, ctx, vars, depth, visiting),
        ast::Expression::FunctionCall(call) => {
            translate_call_expr(call, ctx, vars, depth, visiting)
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            let l = translate_expr(lhs, ctx, vars, depth, visiting);
            let r = translate_expr(rhs, ctx, vars, depth, visiting);
            match binop {
                ast::BinOp::And(_) => Cond::And(Box::new(l), Box::new(r)),
                ast::BinOp::Or(_) => Cond::Or(Box::new(l), Box::new(r)),
                ast::BinOp::GreaterThan(_) => cmp("GreaterThan", l, r),
                ast::BinOp::GreaterThanEqual(_) => cmp("GreaterOrEqThan", l, r),
                ast::BinOp::LessThan(_) => cmp("LessThan", l, r),
                ast::BinOp::LessThanEqual(_) => cmp("LessOrEqThan", l, r),
                ast::BinOp::TwoEqual(_) => cmp("Eq", l, r),
                ast::BinOp::Plus(_) => Cond::Add(Box::new(l), Box::new(r)),
                ast::BinOp::TildeEqual(_) => Cond::Not(Box::new(cmp("Eq", l, r))),
                _ => Cond::Unknown,
            }
        }
        ast::Expression::UnaryOperator {
            unop: ast::UnOp::Not(_),
            expression,
        } => Cond::Not(Box::new(translate_expr(
            expression, ctx, vars, depth, visiting,
        ))),
        _ => Cond::Unknown,
    }
}

pub(super) fn cmp(op: &'static str, l: Cond, r: Cond) -> Cond {
    if l.contains_unknown() || r.contains_unknown() {
        // e.g. `TheGenericKV:GetKV("fuelweaver_killed") == "1"`
        Cond::Unknown
    } else {
        Cond::Cmp(op, Box::new(l), Box::new(r))
    }
}
