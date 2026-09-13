use super::*;

pub(super) fn translate_var(
    var: &ast::Var,
    ctx: &ScanCtx<'_>,
    vars: &HashMap<String, Cond>,
    depth: usize,
    visiting: &mut HashSet<String>,
) -> Cond {
    match var {
        ast::Var::Name(name) => {
            let name = name.token().to_string();
            match name.as_str() {
                // The lock closure's own truthy parameters.
                "activatedskills" | "prefabname" => Cond::True,
                _ => {
                    if let Some(v) = vars.get(&name) {
                        v.clone()
                    } else if let Some(block) = ctx.local_fns.get(&name) {
                        translate_lock_body(block, ctx, depth + 1, visiting)
                    } else {
                        Cond::Unknown
                    }
                }
            }
        }
        ast::Var::Expression(var_expr) => {
            // activatedskills["skill"] / activatedskills.skill
            let mut suffixes = var_expr.suffixes();
            if let (ast::Prefix::Name(prefix), Some(suffix)) = (var_expr.prefix(), suffixes.next())
            {
                if prefix.token().to_string() == "activatedskills" {
                    let skill = match suffix {
                        ast::Suffix::Index(ast::Index::Brackets {
                            expression: ast::Expression::String(s),
                            ..
                        }) => Some(string_literal(&s.to_string())),
                        ast::Suffix::Index(ast::Index::Dot { name, .. }) => {
                            Some(name.token().to_string())
                        }
                        _ => None,
                    };
                    if let Some(skill) = skill {
                        return Cond::ActivatedSkill(skill);
                    }
                }
            }
            Cond::Unknown
        }
        _ => Cond::Unknown,
    }
}

pub(super) fn translate_call_expr(
    call: &ast::FunctionCall,
    ctx: &ScanCtx<'_>,
    vars: &HashMap<String, Cond>,
    depth: usize,
    visiting: &mut HashSet<String>,
) -> Cond {
    let head = call_head(call);
    let args = call_args(call).unwrap_or_default();
    if head.ends_with("CountTags") {
        if let Some(tag) = args.get(1).and_then(eval_string) {
            return Cond::CountTags(tag);
        }
        return Cond::Unknown;
    }
    if head.ends_with("CountSkills") {
        return Cond::CountSkills;
    }
    // HasTag(prefabname, "T", activatedskills) ≡ "至少一个已激活技能带该
    // 标签" ≡ CountTags(T) >= 1（上一版把该前提丢弃，导致暗影/月亮沃比
    // 这类锁少了冲刺小狗已激活的前提）。
    if head.ends_with("HasTag") {
        if let Some(tag) = args.get(1).and_then(eval_string) {
            return Cond::Cmp(
                "GreaterOrEqThan",
                Box::new(Cond::CountTags(tag)),
                Box::new(Cond::Number(1.0)),
            );
        }
        return Cond::Unknown;
    }
    // 沃拓克斯天秤：`CUSTOM_FUNCTIONS.CalculateInclination(nice, naughty,
    // affinitytype) == "nice"`。把两侧计数、阵营表达式与阈值提取成声明式
    // `Inclination`，前端按同一套规则求值，无需复刻 Lua 闭包。
    if head.ends_with("CalculateInclination") {
        let (Some(nice), Some(naughty), Some(affinity)) = (args.first(), args.get(1), args.get(2))
        else {
            return Cond::Unknown;
        };
        let nice = translate_expr(nice, ctx, vars, depth, visiting);
        let naughty = translate_expr(naughty, ctx, vars, depth, visiting);
        let affinity = translate_expr(affinity, ctx, vars, depth, visiting);
        if nice.contains_unknown() || naughty.contains_unknown() || affinity.contains_unknown() {
            return Cond::Unknown;
        }
        let Some(threshold) = find_inclination_threshold(&head, ctx) else {
            return Cond::Unknown;
        };
        return Cond::Inclination {
            nice: Box::new(nice),
            naughty: Box::new(naughty),
            affinity: Box::new(affinity),
            threshold,
        };
    }
    if let Some(block) = ctx.local_fns.get(head.as_str()) {
        let key = head.clone();
        if visiting.contains(&key) {
            return Cond::Unknown;
        }
        visiting.insert(key.clone());
        let result = translate_lock_body(block, ctx, depth + 1, visiting);
        visiting.remove(&key);
        return result;
    }
    Cond::Unknown
}

/// Scans a known custom-function body for the inclination threshold
/// (`if math.abs(diff) >= TUNING.X then`), resolving the bound through the
/// shared constants map.
pub(super) fn find_inclination_threshold(head: &str, ctx: &ScanCtx<'_>) -> Option<f64> {
    let block = ctx.table_fns.get(head)?;
    for stmt in block.stmts() {
        if let ast::Stmt::If(if_stmt) = stmt {
            if let ast::Expression::BinaryOperator { lhs, binop, rhs } = if_stmt.condition() {
                if matches!(binop, ast::BinOp::GreaterThanEqual(_)) && is_math_abs(lhs) {
                    if let Some(value) = eval_number(rhs, ctx.constants) {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

pub(super) fn is_math_abs(expr: &ast::Expression) -> bool {
    matches!(expr, ast::Expression::FunctionCall(call) if call_head(call) == "math.abs")
}

/// Callee head text of a call, e.g. `SkillTreeFns.CreateSkillCountLock`.
pub(super) fn call_head(call: &ast::FunctionCall) -> String {
    let text = call.to_string();
    text.split(['(', '{'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(super) fn call_args(call: &ast::FunctionCall) -> Option<Vec<ast::Expression>> {
    // Dotted calls (`SkillTreeFns.CountTags(x)`) carry an extra Index suffix
    // before the Call suffix; take the last Call suffix whatever the prefix.
    let call_suffix = call
        .suffixes()
        .filter_map(|s| match s {
            ast::Suffix::Call(c) => Some(c),
            _ => None,
        })
        .last()?;
    match call_suffix {
        ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().cloned().collect())
        }
        ast::Call::MethodCall(method_call) => match method_call.args() {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                Some(arguments.iter().cloned().collect())
            }
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn raw_def_from_call(
    call: &ast::FunctionCall,
    ctx: &ScanCtx<'_>,
) -> Option<RawSkillDef> {
    let head = call_head(call);
    let args = call_args(call).unwrap_or_default();
    let mut def = RawSkillDef {
        lock: true,
        ..Default::default()
    };
    match head.as_str() {
        h if h.ends_with("CreateSkillCountLock") => {
            // (group, count[, extra_data])
            def.group = args.first().and_then(eval_string);
            if let Some(ast::Expression::TableConstructor(t)) = args.get(2) {
                def.pos = extract_pos_arg(t, ctx.constants);
            }
        }
        h if h.ends_with("MakeNoShadowLock")
            || h.ends_with("MakeNoLunarLock")
            || h.ends_with("MakeFuelWeaverLock")
            || h.ends_with("MakeCelestialChampionLock") =>
        {
            // (extra_data[, not_root]) — allegiance locks with a fixed group.
            def.group = Some("allegiance".to_string());
            def.tags.push("allegiance".to_string());
            if let Some(ast::Expression::TableConstructor(t)) = args.first() {
                def.pos = extract_pos_arg(t, ctx.constants);
            }
            if let Some(ast::Expression::Symbol(s)) = args.get(1) {
                def.root = s.token().to_string() != "true";
            }
        }
        h if h.ends_with("CreateAccomplishmentLockFn")
            || h.ends_with("CreateAccomplishmentCountLockFn") =>
        {
            def.group = None;
        }
        _ => {
            // Unknown lock helper (e.g. a mod-specific factory).
            if let Some(ast::Expression::TableConstructor(t)) = args.first() {
                def.pos = extract_pos_arg(t, ctx.constants);
            }
        }
    }
    def.lock_open = Some(cond_to_lock_open_json(translate_lock_call(call, ctx)));
    Some(def)
}

pub(super) fn collect_numeric_locals<'a>(
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

/// Registers `local function name() ... end` bodies (any nesting level) so
/// lock closures can reference shared helper functions.
pub(super) fn collect_local_functions<'a>(
    stmts: impl Iterator<Item = &'a ast::Stmt>,
    local_fns: &mut BTreeMap<String, &'a ast::Block>,
) {
    for stmt in stmts {
        match stmt {
            ast::Stmt::LocalFunction(func) => {
                let name = func.name().token().to_string();
                local_fns.insert(name, func.body().block());
                collect_local_functions(func.body().block().stmts(), local_fns);
            }
            ast::Stmt::FunctionDeclaration(func) => {
                collect_local_functions(func.body().block().stmts(), local_fns);
            }
            ast::Stmt::Do(stmt) => {
                collect_local_functions(stmt.block().stmts(), local_fns);
            }
            ast::Stmt::If(stmt) => {
                collect_local_functions(stmt.block().stmts(), local_fns);
                if let Some(else_ifs) = stmt.else_if() {
                    for branch in else_ifs {
                        collect_local_functions(branch.block().stmts(), local_fns);
                    }
                }
                if let Some(block) = stmt.else_block() {
                    collect_local_functions(block.stmts(), local_fns);
                }
            }
            ast::Stmt::NumericFor(stmt) => {
                collect_local_functions(stmt.block().stmts(), local_fns);
            }
            ast::Stmt::GenericFor(stmt) => {
                collect_local_functions(stmt.block().stmts(), local_fns);
            }
            ast::Stmt::While(stmt) => {
                collect_local_functions(stmt.block().stmts(), local_fns);
            }
            ast::Stmt::Repeat(stmt) => {
                collect_local_functions(stmt.block().stmts(), local_fns);
            }
            _ => {}
        }
    }
}

/// Records `Table.field = function ... end` entries of one table constructor
/// and recurses into the function bodies.
pub(super) fn collect_table_ctor_functions<'a>(
    var: &str,
    table: &'a ast::TableConstructor,
    table_fns: &mut BTreeMap<String, &'a ast::Block>,
) {
    for field in table.fields() {
        if let ast::Field::NameKey {
            key,
            value: ast::Expression::Function(func),
            ..
        } = field
        {
            let key = format!("{}.{}", var, key.token());
            table_fns.insert(key, func.body().block());
            collect_table_functions(func.body().block().stmts(), table_fns);
        }
    }
}

/// Registers functions held in table fields (any nesting level), e.g.
/// `local CUSTOM_FUNCTIONS = { CalculateInclination = function(...) end }`.
/// Keys are dotted (`CUSTOM_FUNCTIONS.CalculateInclination`).
pub(super) fn collect_table_functions<'a>(
    stmts: impl Iterator<Item = &'a ast::Stmt>,
    table_fns: &mut BTreeMap<String, &'a ast::Block>,
) {
    for stmt in stmts {
        match stmt {
            ast::Stmt::LocalAssignment(assignment) => {
                for (name, expr) in assignment
                    .names()
                    .iter()
                    .zip(assignment.expressions().iter())
                {
                    let ast::Expression::TableConstructor(t) = expr else {
                        continue;
                    };
                    collect_table_ctor_functions(&name.token().to_string(), t, table_fns);
                }
            }
            // `local CUSTOM_FUNCTIONS;CUSTOM_FUNCTIONS = { ... }` 是常见的
            // 前向声明 + 普通赋值写法，这里同样收表内函数。
            ast::Stmt::Assignment(assignment) => {
                for (var, expr) in assignment
                    .variables()
                    .iter()
                    .zip(assignment.expressions().iter())
                {
                    let ast::Var::Name(name) = var else {
                        continue;
                    };
                    let ast::Expression::TableConstructor(t) = expr else {
                        continue;
                    };
                    collect_table_ctor_functions(&name.token().to_string(), t, table_fns);
                }
            }
            ast::Stmt::LocalFunction(func) => {
                collect_table_functions(func.body().block().stmts(), table_fns);
            }
            ast::Stmt::FunctionDeclaration(func) => {
                collect_table_functions(func.body().block().stmts(), table_fns);
            }
            ast::Stmt::Do(stmt) => collect_table_functions(stmt.block().stmts(), table_fns),
            ast::Stmt::If(stmt) => {
                collect_table_functions(stmt.block().stmts(), table_fns);
                if let Some(else_ifs) = stmt.else_if() {
                    for branch in else_ifs {
                        collect_table_functions(branch.block().stmts(), table_fns);
                    }
                }
                if let Some(block) = stmt.else_block() {
                    collect_table_functions(block.stmts(), table_fns);
                }
            }
            ast::Stmt::NumericFor(stmt) => collect_table_functions(stmt.block().stmts(), table_fns),
            ast::Stmt::GenericFor(stmt) => collect_table_functions(stmt.block().stmts(), table_fns),
            ast::Stmt::While(stmt) => collect_table_functions(stmt.block().stmts(), table_fns),
            ast::Stmt::Repeat(stmt) => collect_table_functions(stmt.block().stmts(), table_fns),
            _ => {}
        }
    }
}

/// Collects `local X = CreateShelfDecor({ { imagename = ..., x = ..., y = ... } })`
/// static decoration lists (Winona's shelves).
pub(super) fn collect_decorations<'a>(
    stmts: impl Iterator<Item = &'a ast::Stmt>,
    constants: &BTreeMap<String, f64>,
    out: &mut BTreeMap<String, Vec<RawDecoration>>,
) {
    for stmt in stmts {
        match stmt {
            ast::Stmt::LocalAssignment(assignment) => {
                for (name, expr) in assignment
                    .names()
                    .iter()
                    .zip(assignment.expressions().iter())
                {
                    let ast::Expression::FunctionCall(call) = expr else {
                        continue;
                    };
                    if !call_head(call).ends_with("CreateShelfDecor") {
                        continue;
                    }
                    if let Some(list) = parse_shelf_decor_call(call, constants) {
                        out.insert(name.token().to_string(), list);
                    }
                }
            }
            ast::Stmt::LocalFunction(func) => {
                collect_decorations(func.body().block().stmts(), constants, out);
            }
            ast::Stmt::FunctionDeclaration(func) => {
                collect_decorations(func.body().block().stmts(), constants, out);
            }
            ast::Stmt::Do(stmt) => collect_decorations(stmt.block().stmts(), constants, out),
            ast::Stmt::If(stmt) => {
                collect_decorations(stmt.block().stmts(), constants, out);
                if let Some(else_ifs) = stmt.else_if() {
                    for branch in else_ifs {
                        collect_decorations(branch.block().stmts(), constants, out);
                    }
                }
                if let Some(block) = stmt.else_block() {
                    collect_decorations(block.stmts(), constants, out);
                }
            }
            ast::Stmt::NumericFor(stmt) => {
                collect_decorations(stmt.block().stmts(), constants, out)
            }
            ast::Stmt::GenericFor(stmt) => {
                collect_decorations(stmt.block().stmts(), constants, out)
            }
            ast::Stmt::While(stmt) => collect_decorations(stmt.block().stmts(), constants, out),
            ast::Stmt::Repeat(stmt) => collect_decorations(stmt.block().stmts(), constants, out),
            _ => {}
        }
    }
}

/// Parses the first argument of `CreateShelfDecor(...)`:
/// `{ { imagename, width, height, scale, x, y }, ... }`. Applies the
/// helper's `SetPosition(data.x, data.y - 50)` offset so the emitted position
/// is already in widget coordinates.
pub(super) fn parse_shelf_decor_call(
    call: &ast::FunctionCall,
    constants: &BTreeMap<String, f64>,
) -> Option<Vec<RawDecoration>> {
    let arg = call_args(call)?.into_iter().next()?;
    let ast::Expression::TableConstructor(outer) = arg else {
        return None;
    };
    let mut list = Vec::new();
    for field in outer.fields() {
        let ast::Field::NoKey(ast::Expression::TableConstructor(t)) = field else {
            continue;
        };
        let (mut img, mut width, mut height, mut scale, mut x, mut y) =
            (None, None, None, None, None, None);
        for f in t.fields() {
            let (Some(key), Some(value)) = (field_name(f), field_value(f)) else {
                continue;
            };
            match key.as_str() {
                "imagename" => img = eval_string(value),
                "width" => width = eval_number(value, constants),
                "height" => height = eval_number(value, constants),
                "scale" => scale = eval_number(value, constants),
                "x" => x = eval_number(value, constants),
                "y" => y = eval_number(value, constants),
                _ => {}
            }
        }
        let (Some(img), Some(x), Some(y)) = (img, x, y) else {
            continue;
        };
        list.push(RawDecoration {
            img: img.trim_end_matches(".tex").to_string(),
            pos: (x, y - 50.0),
            size: match (width, height) {
                (Some(w), Some(h)) => Some((w, h)),
                _ => None,
            },
            scale,
        });
    }
    Some(list)
}

/// Attempts to interpret a keyed table entry as a skill node (fallback path
/// for unusual files without a recognizable `skills` table).
#[allow(dead_code)]
pub(super) fn try_parse_skill(
    name: &str,
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<SkillNode> {
    let local_fns = BTreeMap::new();
    let table_fns = BTreeMap::new();
    let decorations = BTreeMap::new();
    let ctx = ScanCtx {
        constants,
        local_fns: &local_fns,
        table_fns: &table_fns,
        decorations: &decorations,
    };
    let def = raw_def_from_table(table, &ctx)?;
    let (x, y) = def.pos?;
    let lock = def.is_lock();
    let mut tags = def.tags;
    if lock {
        tags.push("lock".to_string());
    } else if let Some(group) = &def.group {
        tags.push(group.clone());
    }
    Some(SkillNode {
        name: name.to_string(),
        x,
        y,
        group: def.group,
        root: def.root,
        connects: def.connects,
        icon: def.icon,
        title_key: def.title,
        lock,
        locks: def.locks,
        lock_open: def.lock_open,
        tags,
        onactivate: def.onactivate,
        ondeactivate: def.ondeactivate,
        defaultfocus: def.defaultfocus,
        infographic: def.infographic,
        forced_focus: def.forced_focus,
        button_decorations: def.button_decorations,
        decorations: Vec::new(),
    })
}
