use crate::error::Result;
use full_moon::ast;
use std::collections::{BTreeMap, HashMap, HashSet};

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
    /// Presence markers for fields the wiki renderer only needs as flags.
    pub onactivate: bool,
    pub ondeactivate: bool,
    pub defaultfocus: bool,
    pub infographic: bool,
    pub forced_focus: Option<serde_json::Value>,
    pub button_decorations: bool,
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
    onactivate: bool,
    ondeactivate: bool,
    defaultfocus: bool,
    infographic: bool,
    forced_focus: Option<serde_json::Value>,
    button_decorations: bool,
}

impl RawSkillDef {
    fn is_lock(&self) -> bool {
        self.lock
    }
}

/// A skill-like local table captured during the scan (e.g. `skills`,
/// `sisturn_skills`, `potion_skills`), kept in declaration order.
struct SkillTable {
    defs: Vec<(String, RawSkillDef)>,
}

/// Shared resolution context for one parsed file.
struct ScanCtx<'a> {
    constants: &'a BTreeMap<String, f64>,
    local_fns: &'a BTreeMap<String, &'a ast::Block>,
}

/// Parses a `skilltree_<character>.lua` source into a structured tree.
///
/// The game files define skills as keyed table entries (usually a local
/// `skills` table, sometimes split into per-theme subsets merged through a
/// `finalize_skill_group`-style helper, as in wendy). Coordinates may live in
/// a separate `POSITIONS` table or inline in each def, and may reference
/// numeric locals or simple arithmetic (`math.floor` included), so a small
/// constant folder is included.
///
/// `lock_open` closures are translated into the declarative JSON condition
/// language used by the wiki (`CountTags` / `CountSkills` / comparisons /
/// `And`/`Or`/`Not`). Conditions that cannot be resolved statically (external
/// achievements, custom functions) fall back to "open", mirroring the wiki's
/// own handling; the condition text is documented in the skill description.
pub fn parse_skill_tree(source: &str, character: &str) -> Result<SkillTree> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;

    let mut constants = BTreeMap::new();
    collect_numeric_locals(ast.nodes().stmts(), &mut constants);

    let mut local_fns: BTreeMap<String, &ast::Block> = BTreeMap::new();
    collect_local_functions(ast.nodes().stmts(), &mut local_fns);

    let ctx = ScanCtx {
        constants: &constants,
        local_fns: &local_fns,
    };

    let mut scan = ScanState::default();
    let mut positions: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    scan_block(ast.nodes(), &ctx, &mut scan, &mut positions);

    let defs = scan.finalize();

    let mut nodes = Vec::new();
    for (name, def) in defs {
        let position = def.pos.or_else(|| positions.get(&name).copied());
        let Some((x, y)) = position else {
            continue;
        };
        let lock = def.is_lock();
        // Mirror the per-file post-processing found at the bottom of every
        // game skilltree file: locks carry a "lock" tag, regular skills carry
        // their group as a tag and default their icon to the skill name.
        let mut tags = def.tags;
        if lock {
            if !tags.iter().any(|t| t == "lock") {
                tags.push("lock".to_string());
            }
        } else if let Some(group) = &def.group {
            if !tags.iter().any(|t| t == group) {
                tags.push(group.clone());
            }
        }
        let icon = if lock {
            None
        } else {
            def.icon.or_else(|| Some(name.clone()))
        };
        nodes.push(SkillNode {
            name,
            x,
            y,
            group: def.group,
            root: def.root,
            connects: def.connects,
            icon,
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
        });
    }

    Ok(SkillTree {
        character: character.to_string(),
        nodes,
    })
}

#[derive(Default)]
struct ScanState {
    /// Skill-like local tables in declaration order; re-declaration of the
    /// same variable replaces the previous entry in place.
    tables: Vec<SkillTable>,
    table_slots: HashMap<String, usize>,
    /// Defs merged through `finalize_skill_group`-style calls, in call order.
    final_defs: Vec<(String, RawSkillDef)>,
    final_index: HashMap<String, usize>,
}

impl ScanState {
    fn record_table(&mut self, var: &str, defs: Vec<(String, RawSkillDef)>) {
        match self.table_slots.get(var) {
            Some(&idx) => self.tables[idx].defs = defs,
            None => {
                self.table_slots.insert(var.to_string(), self.tables.len());
                self.tables.push(SkillTable { defs });
            }
        }
    }

    /// Merges `tables[var]` into `final_defs`, tagging every entry with
    /// `group` (mirroring the wendy-style finalize helpers).
    fn merge_with_group(&mut self, var: &str, group: &str) {
        let Some(idx) = self.table_slots.get(var).copied() else {
            return;
        };
        for (name, mut def) in std::mem::take(&mut self.tables[idx].defs) {
            def.group = Some(group.to_string());
            if !def.tags.iter().any(|t| t == group) {
                def.tags.push(group.to_string());
            }
            self.upsert_final(name, def);
        }
    }

    fn upsert_final(&mut self, name: String, def: RawSkillDef) {
        match self.final_index.get(&name) {
            Some(&idx) => self.final_defs[idx].1 = def,
            None => {
                self.final_index.insert(name.clone(), self.final_defs.len());
                self.final_defs.push((name, def));
            }
        }
    }

    /// Applies the merge calls and falls back to the plain union of all
    /// captured tables when the file never calls a finalize helper.
    fn finalize(self) -> Vec<(String, RawSkillDef)> {
        let mut this = self;
        if this.final_defs.is_empty() {
            for idx in 0..this.tables.len() {
                for (name, def) in std::mem::take(&mut this.tables[idx].defs) {
                    this.upsert_final(name, def);
                }
            }
        }
        this.final_defs
    }
}

fn scan_block(
    block: &ast::Block,
    ctx: &ScanCtx<'_>,
    scan: &mut ScanState,
    positions: &mut BTreeMap<String, (f64, f64)>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(assignment) => {
                for (name, expr) in assignment
                    .names()
                    .iter()
                    .zip(assignment.expressions().iter())
                {
                    let var = name.token().to_string();
                    if let ast::Expression::TableConstructor(t) = expr {
                        if var == "POSITIONS" {
                            *positions = parse_positions(t, ctx.constants);
                        } else if looks_like_skill_table(t) {
                            let defs = parse_skill_defs(t, ctx);
                            scan.record_table(&var, defs);
                        }
                    }
                }
            }
            ast::Stmt::LocalFunction(func) => {
                // Local helpers may contain nested skill tables; the function
                // itself is already registered by the pre-pass.
                scan_block(func.body().block(), ctx, scan, positions);
            }
            ast::Stmt::FunctionDeclaration(func) => {
                scan_block(func.body().block(), ctx, scan, positions);
            }
            ast::Stmt::FunctionCall(call) => {
                // finalize_skill_group(<subset_table>, "<group>") and similar
                // merge helpers: first argument is a known skill table, the
                // second the group name.
                let Some(args) = call_args(call) else {
                    continue;
                };
                if args.len() != 2 {
                    continue;
                }
                let table_var = match &args[0] {
                    ast::Expression::Var(ast::Var::Name(n)) => n.token().to_string(),
                    _ => continue,
                };
                let group = match &args[1] {
                    ast::Expression::String(s) => string_literal(&s.to_string()),
                    _ => continue,
                };
                if scan.table_slots.contains_key(&table_var) {
                    scan.merge_with_group(&table_var, &group);
                }
            }
            ast::Stmt::Do(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::While(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::Repeat(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::If(stmt) => {
                scan_block(stmt.block(), ctx, scan, positions);
                if let Some(else_ifs) = stmt.else_if() {
                    for branch in else_ifs {
                        scan_block(branch.block(), ctx, scan, positions);
                    }
                }
                if let Some(block) = stmt.else_block() {
                    scan_block(block, ctx, scan, positions);
                }
            }
            ast::Stmt::NumericFor(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            ast::Stmt::GenericFor(stmt) => scan_block(stmt.block(), ctx, scan, positions),
            _ => {}
        }
    }
}

/// Heuristic: a local table whose entries look like skill definitions
/// (named table values carrying skill-ish keys).
fn looks_like_skill_table(table: &ast::TableConstructor) -> bool {
    let mut table_entries = 0;
    let mut has_skill_key = false;
    for field in table.fields() {
        if let ast::Field::NameKey {
            key: _,
            value: ast::Expression::TableConstructor(t),
            ..
        } = field
        {
            table_entries += 1;
            if !has_skill_key {
                for f in t.fields() {
                    if let Some(k) = field_name(f) {
                        if matches!(
                            k.as_str(),
                            "pos"
                                | "connects"
                                | "locks"
                                | "lock_open"
                                | "tags"
                                | "root"
                                | "icon"
                                | "group"
                                | "defaultfocus"
                                | "infographic"
                                | "onactivate"
                                | "ondeactivate"
                        ) {
                            has_skill_key = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    table_entries >= 1 && has_skill_key
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
                ast::BinOp::Caret(_) => Some(l.powf(r)),
                _ => None,
            }
        }
        ast::Expression::FunctionCall(call) => eval_math_call(call, constants),
        _ => None,
    }
}

/// Evaluates deterministic `math.*` helpers used in position/constant math.
fn eval_math_call(call: &ast::FunctionCall, constants: &BTreeMap<String, f64>) -> Option<f64> {
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

/// Extracts a position from a call argument that is either a bare `{x, y}`
/// table or an extra-data wrapper like `{ pos = {x, y}, connects = {...} }`.
fn extract_pos_arg(
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

fn raw_def_from_table(table: &ast::TableConstructor, ctx: &ScanCtx<'_>) -> Option<RawSkillDef> {
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
            "button_decorations" => def.button_decorations = true,
            _ => {}
        }
    }
    Some(def)
}

/// Translates a `lock_open` field value into the wiki's declarative JSON.
///
/// Falls back to `true` ("open") whenever the condition cannot be resolved
/// statically, matching the wiki's handling of external achievements and
/// custom helper functions.
fn translate_lock_open_value(
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
fn translate_lock_call(call: &ast::FunctionCall, ctx: &ScanCtx<'_>) -> Cond {
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
fn translate_lock_body(
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
fn handle_lock_if_branch(
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
fn top_level_return(block: &ast::Block) -> Option<&ast::Return> {
    match block.last_stmt() {
        Some(ast::LastStmt::Return(ret)) => Some(ret),
        _ => None,
    }
}

fn is_readonly_ref(expr: &ast::Expression) -> bool {
    matches!(expr, ast::Expression::Var(ast::Var::Name(n)) if n.token().to_string() == "readonly")
}

/// Internal representation of a translated `lock_open` condition.
#[derive(Debug, Clone)]
enum Cond {
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

    fn contains_unknown(&self) -> bool {
        match self {
            Cond::Unknown => true,
            Cond::Cmp(_, l, r) | Cond::Add(l, r) | Cond::And(l, r) | Cond::Or(l, r) => {
                l.contains_unknown() || r.contains_unknown()
            }
            Cond::Not(x) => x.contains_unknown(),
            _ => false,
        }
    }
}

fn invert_cond(cond: Cond) -> Cond {
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

fn simplify_cond(cond: Cond) -> Cond {
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
fn flatten_chain(cond: Cond) -> Vec<Cond> {
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
fn fold_chain(op: &'static str, items: Vec<Cond>) -> Cond {
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
fn canonicalize_cmp(op: &'static str, l: Cond, r: Cond) -> Cond {
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

fn cond_to_lock_open_json(cond: Cond) -> serde_json::Value {
    let simplified = simplify_cond(cond);
    match &simplified {
        Cond::True | Cond::Unknown => serde_json::json!(true),
        Cond::False => serde_json::json!(false),
        _ => cond_to_json(&simplified),
    }
}

/// Serializes counts/positions as JSON integers when the value is integral,
/// matching the wiki's hand-written JSON (`2` instead of `2.0`).
fn number_to_json(n: f64) -> serde_json::Value {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 9.0e15 {
        serde_json::json!(n as i64)
    } else {
        serde_json::json!(n)
    }
}

fn cond_to_json(cond: &Cond) -> serde_json::Value {
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
    }
}

fn translate_expr(
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
        ast::Expression::FunctionCall(call) => translate_call_expr(call, ctx, depth, visiting),
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

fn cmp(op: &'static str, l: Cond, r: Cond) -> Cond {
    if l.contains_unknown() || r.contains_unknown() {
        // e.g. `TheGenericKV:GetKV("fuelweaver_killed") == "1"`
        Cond::Unknown
    } else {
        Cond::Cmp(op, Box::new(l), Box::new(r))
    }
}

fn translate_var(
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

fn translate_call_expr(
    call: &ast::FunctionCall,
    ctx: &ScanCtx<'_>,
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

/// Callee head text of a call, e.g. `SkillTreeFns.CreateSkillCountLock`.
fn call_head(call: &ast::FunctionCall) -> String {
    let text = call.to_string();
    text.split(['(', '{'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn call_args(call: &ast::FunctionCall) -> Option<Vec<ast::Expression>> {
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

fn raw_def_from_call(call: &ast::FunctionCall, ctx: &ScanCtx<'_>) -> Option<RawSkillDef> {
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

/// Registers `local function name() ... end` bodies (any nesting level) so
/// lock closures can reference shared helper functions.
fn collect_local_functions<'a>(
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

/// Attempts to interpret a keyed table entry as a skill node (fallback path
/// for unusual files without a recognizable `skills` table).
#[allow(dead_code)]
fn try_parse_skill(
    name: &str,
    table: &ast::TableConstructor,
    constants: &BTreeMap<String, f64>,
) -> Option<SkillNode> {
    let local_fns = BTreeMap::new();
    let ctx = ScanCtx {
        constants,
        local_fns: &local_fns,
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
        icon: if lock { None } else { def.icon },
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
    })
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
            tags = {"torch"},
            root = true,
        },
        wilson_torch_lock = {
            group = "torch",
            root = true,
            lock_open = function(prefabname, activatedskills, readonly)
                return SkillTreeFns.CountTags(prefabname, "torch1", activatedskills) > 2
            end,
            connects = {
                "wilson_torch_7",
            },
        },
        wilson_torch_7 = {
            icon = "wilson_torch_throw",
            pos = {TORCH_X,58-38},
            group = "torch",
            tags = {"torch"},
            locks = {"wilson_torch_lock"},
        },
    }

    for name, data in pairs(skills) do
        local uppercase_name = string.upper(name)
        data.pos = data.pos
        data.desc = data.desc or STRINGS.SKILLTREE.WILSON[uppercase_name.."_DESC"]
        if not data.lock_open then
            data.title = data.title or STRINGS.SKILLTREE.WILSON[uppercase_name.."_TITLE"]
            data.icon = data.icon or name
        end
    end

    return {
        SKILLS = skills,
        ORDERS = ORDERS,
    }
end

return BuildSkillsData
"#;

    #[test]
    fn test_parse_sample_tree() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        assert!(tree.nodes.iter().any(|n| n.name == "wilson_alchemy_1"));
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
            vec!["wilson_alchemy_2", "wilson_alchemy_3"]
        );

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
        assert_eq!(torch.x, -190.0);
        assert_eq!(torch.y, 100.0);
    }

    #[test]
    fn test_parse_sample_groups() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        let groups = tree.groups();
        assert!(groups.contains(&"alchemy".to_string()));
        assert!(groups.contains(&"torch".to_string()));
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
    }

    #[test]
    fn test_group_tag_and_lock_tag_post_processing() {
        let source = r#"
local skills = {
    a_one = { pos = {0, 0}, group = "g", tags = {"custom"} },
    b_lock = { pos = {1, 1}, group = "g", lock_open = function() return true end },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let a = tree.nodes.iter().find(|n| n.name == "a_one").unwrap();
        assert_eq!(a.tags, vec!["custom", "g"]);
        let b = tree.nodes.iter().find(|n| n.name == "b_lock").unwrap();
        assert_eq!(b.tags, vec!["lock"]);
        assert!(b.icon.is_none());
    }

    #[test]
    fn test_inline_count_tags_lock_translation() {
        let source = r#"
local skills = {
    torch_lock = {
        pos = {0, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountTags(prefabname, "torch1", activatedskills) > 2
        end,
    },
    bernie_lock = {
        pos = {1, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            local bernie_skills = SkillTreeFns.CountTags(prefabname, "bernie4", activatedskills)
            return bernie_skills >= 4
        end,
    },
    skills_lock = {
        pos = {2, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountSkills(prefabname, activatedskills) >= 12
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let torch = tree.nodes.iter().find(|n| n.name == "torch_lock").unwrap();
        assert_eq!(
            torch.lock_open,
            Some(serde_json::json!({
                "GreaterThan": { "left": { "CountTags": "torch1" }, "right": 2 }
            }))
        );
        let bernie = tree.nodes.iter().find(|n| n.name == "bernie_lock").unwrap();
        assert_eq!(
            bernie.lock_open,
            Some(serde_json::json!({
                "GreaterOrEqThan": { "left": { "CountTags": "bernie4" }, "right": 4 }
            }))
        );
        let skills_lock = tree.nodes.iter().find(|n| n.name == "skills_lock").unwrap();
        assert_eq!(
            skills_lock.lock_open,
            Some(serde_json::json!({
                "GreaterOrEqThan": { "left": { "CountSkills": "CountSkills" }, "right": 12 }
            }))
        );
    }

    #[test]
    fn test_external_lock_defaults_open() {
        let source = r#"
local function CreateAccomplishmentLockFn(key)
    return
        function(prefabname, activatedskills, readonly)
            return readonly and "question" or TheGenericKV:GetKV(key) == "1"
        end
end

local skills = {
    song_lock = {
        pos = {0, 0},
        lock_open = CreateAccomplishmentLockFn("wathgrithr_horn_played"),
    },
    affinity_lock = {
        pos = {1, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            if readonly then
                return "question"
            end
            return TheGenericKV:GetKV("fuelweaver_killed") == "1"
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        for name in ["song_lock", "affinity_lock"] {
            let node = tree.nodes.iter().find(|n| n.name == name).unwrap();
            assert_eq!(node.lock_open, Some(serde_json::json!(true)), "{}", name);
        }
    }

    #[test]
    fn test_compound_lock_translation() {
        let source = r#"
local skills = {
    beaver_lock = {
        pos = {0, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return
                SkillTreeFns.CountTags(prefabname, "beaver", activatedskills) >= 3 and
                SkillTreeFns.CountTags(prefabname, "moose_epic", activatedskills) == 0 and
                SkillTreeFns.CountTags(prefabname, "goose_epic", activatedskills) == 0
        end,
    },
    portable_lock = {
        pos = {1, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return activatedskills and activatedskills["winona_portable_structures"] and SkillTreeFns.CountTags(prefabname, "lowshelf", activatedskills) > 2
        end,
    },
    shelf_lock = {
        pos = {2, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountTags(prefabname, "lowshelf", activatedskills) + SkillTreeFns.CountTags(prefabname, "midshelf", activatedskills) > 5
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let beaver = tree.nodes.iter().find(|n| n.name == "beaver_lock").unwrap();
        assert_eq!(
            beaver.lock_open,
            Some(serde_json::json!({
                "And": {
                    "left": { "GreaterOrEqThan": { "left": { "CountTags": "beaver" }, "right": 3 } },
                    "right": {
                        "And": {
                            "left": { "Eq": { "left": { "CountTags": "moose_epic" }, "right": 0 } },
                            "right": { "Eq": { "left": { "CountTags": "goose_epic" }, "right": 0 } }
                        }
                    }
                }
            }))
        );
        let portable = tree
            .nodes
            .iter()
            .find(|n| n.name == "portable_lock")
            .unwrap();
        assert_eq!(
            portable.lock_open,
            Some(serde_json::json!({
                "And": {
                    "left": { "ActivatedSkill": "winona_portable_structures" },
                    "right": { "GreaterThan": { "left": { "CountTags": "lowshelf" }, "right": 2 } }
                }
            }))
        );
        let shelf = tree.nodes.iter().find(|n| n.name == "shelf_lock").unwrap();
        assert_eq!(
            shelf.lock_open,
            Some(serde_json::json!({
                "GreaterThan": {
                    "left": {
                        "Add": {
                            "left": { "CountTags": "lowshelf" },
                            "right": { "CountTags": "midshelf" }
                        }
                    },
                    "right": 5
                }
            }))
        );
    }

    #[test]
    fn test_local_fn_resolution_and_guards() {
        let source = r#"
local function BasicShadowAllegianceLockFn(prefabname, activatedskills, readonly)
    if SkillTreeFns.CountTags(prefabname, "lunar_favor", activatedskills) > 0 then
        return false
    end
    if readonly then
        return "question"
    end
    return TheGenericKV:GetKV("fuelweaver_killed") == "1"
end

local skills = {
    woby_shadow_lock = {
        pos = {0, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            if not SkillTreeFns.HasTag(prefabname, "woby_dash", activatedskills) then
                return false
            end
            return BasicShadowAllegianceLockFn(prefabname, activatedskills, readonly)
        end,
    },
    direct_ref_lock = {
        pos = {1, 0},
        lock_open = BasicShadowAllegianceLockFn,
    },
    guarded_count_lock = {
        pos = {2, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            local maxbodies = SkillTreeFns.CountTags(prefabname, "wx78_maxbody", activatedskills)
            if maxbodies == 0 then
                return false
            end
            local shadow_skills = SkillTreeFns.CountTags(prefabname, "shadow_favor", activatedskills)
            if shadow_skills > 0 then
                return false
            end
            if readonly then
                return "question"
            end
            return TheGenericKV:GetKV("celestialchampion_killed") == "1"
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        // woby_shadow_lock：HasTag(woby_dash) 前提 + BasicShadow… 的
        // CountTags(lunar_favor)==0，两者 AND。
        let woby_expected = serde_json::json!({
            "And": {
                "left": { "GreaterOrEqThan": { "left": { "CountTags": "woby_dash" }, "right": 1 } },
                "right": { "Eq": { "left": { "CountTags": "lunar_favor" }, "right": 0 } }
            }
        });
        let node = tree
            .nodes
            .iter()
            .find(|n| n.name == "woby_shadow_lock")
            .unwrap();
        assert_eq!(node.lock_open, Some(woby_expected));
        // direct_ref_lock 没有 HasTag 守卫，只剩 allegiance 条件。
        let expected = serde_json::json!({
            "Eq": { "left": { "CountTags": "lunar_favor" }, "right": 0 }
        });
        let node = tree
            .nodes
            .iter()
            .find(|n| n.name == "direct_ref_lock")
            .unwrap();
        assert_eq!(node.lock_open, Some(expected));
        let guarded = tree
            .nodes
            .iter()
            .find(|n| n.name == "guarded_count_lock")
            .unwrap();
        assert_eq!(
            guarded.lock_open,
            Some(serde_json::json!({
                "And": {
                    "left": { "GreaterOrEqThan": { "left": { "CountTags": "wx78_maxbody" }, "right": 1 } },
                    "right": { "Eq": { "left": { "CountTags": "shadow_favor" }, "right": 0 } }
                }
            }))
        );
    }

    #[test]
    fn test_wendy_style_subset_tables() {
        let source = r#"
local function BuildSkillsData(SkillTreeFns)
    local skills = {}
    local function finalize_skill_group(skill_subset, group_name)
        for skill_name, skill_data in pairs(skill_subset) do
            skills[skill_name] = skill_data
        end
    end

    local sisturn_skills =
    {
        wendy_sisturn_1 = {
            pos = {103,173},
            tags = {"sisturn"},
            root = true,
            connects = { "wendy_sisturn_2" },
            defaultfocus = true,
        },
        wendy_sisturn_2 = {
            pos = {144,156},
            tags = {"sisturn"},
            onactivate = function(inst) end,
        },
    }

    finalize_skill_group(sisturn_skills, "sisturn_upgrades")

    local potion_skills =
    {
        wendy_potion_1 = { pos = {10, 10}, tags = {"potion"} },
    }
    finalize_skill_group(potion_skills, "potion_upgrades")

    return { SKILLS = skills }
end
"#;
        let tree = parse_skill_tree(source, "wendy").unwrap();
        assert_eq!(tree.nodes.len(), 3);
        let sisturn = tree
            .nodes
            .iter()
            .find(|n| n.name == "wendy_sisturn_1")
            .unwrap();
        assert_eq!(sisturn.group.as_deref(), Some("sisturn_upgrades"));
        assert!(sisturn.tags.contains(&"sisturn_upgrades".to_string()));
        assert!(sisturn.defaultfocus);
        let sisturn2 = tree
            .nodes
            .iter()
            .find(|n| n.name == "wendy_sisturn_2")
            .unwrap();
        assert!(sisturn2.onactivate);
        let potion = tree
            .nodes
            .iter()
            .find(|n| n.name == "wendy_potion_1")
            .unwrap();
        assert_eq!(potion.group.as_deref(), Some("potion_upgrades"));
    }

    #[test]
    fn test_presence_fields() {
        let source = r#"
local skills = {
    inf = {
        pos = {0, 0},
        infographic = true,
        forced_focus = { left = "a", right = "b" },
        button_decorations = { init = function() end },
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let inf = tree.nodes.iter().find(|n| n.name == "inf").unwrap();
        assert!(inf.infographic);
        assert!(inf.button_decorations);
        assert_eq!(
            inf.forced_focus,
            Some(serde_json::json!({ "left": "a", "right": "b" }))
        );
    }
}
