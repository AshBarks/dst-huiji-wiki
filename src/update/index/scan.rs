//! Pass 1: per-file AST collection.
//!
//! Walks one Lua file with full_moon and collects everything Pass 2 needs:
//! require bindings, local constants, function definitions/ranges, `Prefab`
//! registrations, association-relevant call sites, behaviour-constructor
//! exports and `State{}`/`EventHandler()` counts. Dynamic forms are kept as
//! classified [`ArgExpr`]s — nothing is dropped silently.

use std::collections::HashSet;

use full_moon::ast;
use full_moon::node::Node;

use super::symbols::{
    ArgExpr, AssocCall, CallKind, ConstVal, ExportedCtor, FileKey, FileScan, FnDef, FnRefUse,
    PrefabReg, Role,
};

/// Extracts the plain text of a string literal expression (quotes stripped).
fn string_literal_text(raw: &str) -> String {
    let t = raw.trim();
    for q in ['"', '\''] {
        if let Some(inner) = t.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner.to_string();
        }
        if let Some(inner) = t.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
            return inner.trim().to_string();
        }
    }
    t.to_string()
}

fn classify_str_arg(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::String(s) => Some(string_literal_text(&s.to_string())),
        _ => None,
    }
}

pub(crate) fn line_of(source: &str, byte: usize) -> u32 {
    source.as_bytes()[..byte.min(source.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count() as u32
        + 1
}

/// Walks one source file and produces its [`FileScan`].
pub struct Scanner<'s> {
    source: &'s str,
    role: Role,
    out: FileScan,
    scope: Vec<String>,
    /// Names currently known to reference the entity under construction.
    inst_aliases: HashSet<String>,
}

impl<'s> Scanner<'s> {
    pub fn scan(
        path: FileKey,
        role: Role,
        source: &'s str,
    ) -> Result<FileScan, Vec<full_moon::Error>> {
        let ast = full_moon::parse(source)?;
        let mut scanner = Self {
            source,
            role,
            out: FileScan::new(path, role),
            scope: Vec::new(),
            inst_aliases: HashSet::new(),
        };
        let nodes = ast.nodes();
        for stmt in nodes.stmts() {
            scanner.walk_stmt(stmt);
        }
        if let Some(ast::LastStmt::Return(ret)) = nodes.last_stmt() {
            scanner.handle_return(ret);
        }
        scanner.out.parse_ok = true;
        Ok(scanner.out)
    }

    fn line(&self, node: &impl Node) -> u32 {
        node.start_position()
            .map(|p| line_of(self.source, p.bytes()))
            .unwrap_or(0)
    }

    // ---------------------------------------------------------------- blocks

    fn walk_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            self.walk_stmt(stmt);
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            self.handle_return(ret);
        }
    }

    fn handle_return(&mut self, ret: &ast::Return) {
        for expr in ret.returns() {
            if self.try_record_prefab_reg_from_expr(expr) {
                continue;
            }
            self.scan_expr(expr);
        }
    }

    fn walk_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::LocalAssignment(assign) => {
                let names: Vec<_> = assign.names().iter().collect();
                let exprs: Vec<_> = assign.expressions().iter().collect();
                for (idx, name) in names.iter().enumerate() {
                    let var = name.token().to_string();
                    let Some(expr) = exprs.get(idx) else { continue };
                    self.try_record_require(&var, expr);
                    self.try_record_const(&var, expr);
                    self.try_record_dep_table(&var, expr);
                    // Alias of the constructed entity: either an alias of a
                    // known one (`local me = inst`) or the conventional
                    // construction binding (`local inst = CreateEntity()`).
                    let is_alias_of_alias = matches!(
                        expr,
                        ast::Expression::Var(ast::Var::Name(base))
                            if self.inst_aliases.contains(&base.token().to_string())
                    );
                    if is_alias_of_alias || var == "inst" {
                        self.inst_aliases.insert(var);
                    }
                    self.scan_expr(expr);
                }
            }
            ast::Stmt::LocalFunction(local_fn) => {
                let name = local_fn.name().token().to_string();
                self.record_fn_def(
                    &name,
                    local_fn.body(),
                    true,
                    local_fn.start_position(),
                    local_fn.end_position(),
                );
                let saved = self.enter_named_fn(&name, local_fn.body());
                self.walk_block(local_fn.body().block());
                self.exit_named_fn(saved);
            }
            ast::Stmt::FunctionDeclaration(func_decl) => {
                let name = func_decl.name().to_string();
                self.record_fn_def(
                    &name,
                    func_decl.body(),
                    false,
                    func_decl.start_position(),
                    func_decl.end_position(),
                );
                let saved = self.enter_named_fn(&name, func_decl.body());
                self.walk_block(func_decl.body().block());
                self.exit_named_fn(saved);
            }
            ast::Stmt::Assignment(assign) => {
                self.try_record_export(assign);
                for var in assign.variables().iter() {
                    if let ast::Var::Expression(vex) = var {
                        self.scan_var_expression(vex);
                    }
                }
                for expr in assign.expressions().iter() {
                    self.scan_expr(expr);
                }
            }
            ast::Stmt::FunctionCall(call) => self.handle_call(call),
            ast::Stmt::If(if_stmt) => {
                self.scan_expr(if_stmt.condition());
                self.walk_block(if_stmt.block());
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.scan_expr(else_if.condition());
                        self.walk_block(else_if.block());
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.walk_block(else_block);
                }
            }
            ast::Stmt::While(while_stmt) => {
                self.scan_expr(while_stmt.condition());
                self.walk_block(while_stmt.block());
            }
            ast::Stmt::Repeat(repeat_stmt) => {
                self.walk_block(repeat_stmt.block());
                self.scan_expr(repeat_stmt.until());
            }
            ast::Stmt::NumericFor(for_stmt) => {
                self.scan_expr(for_stmt.start());
                self.scan_expr(for_stmt.end());
                if let Some(step) = for_stmt.step() {
                    self.scan_expr(step);
                }
                self.walk_block(for_stmt.block());
            }
            ast::Stmt::GenericFor(for_stmt) => {
                for expr in for_stmt.expressions() {
                    self.scan_expr(expr);
                }
                self.walk_block(for_stmt.block());
            }
            ast::Stmt::Do(do_stmt) => self.walk_block(do_stmt.block()),
            _ => {}
        }
    }

    // ------------------------------------------------------- special records

    fn try_record_require(&mut self, var: &str, expr: &ast::Expression) -> bool {
        let ast::Expression::FunctionCall(call) = expr else {
            return false;
        };
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return false;
        };
        if prefix.token().to_string() != "require" {
            return false;
        }
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(args))) = call.suffixes().next() else {
            return false;
        };
        let ast::FunctionArgs::Parentheses { arguments, .. } = args else {
            return false;
        };
        if let Some(ast::Expression::String(s)) = arguments.iter().next() {
            let path = string_literal_text(&s.to_string());
            self.out.requires.push((var.to_string(), path));
            return true;
        }
        false
    }

    fn try_record_const(&mut self, var: &str, expr: &ast::Expression) -> bool {
        let val = match expr {
            ast::Expression::Number(n) => ConstVal::Num(n.to_string()),
            ast::Expression::String(s) => ConstVal::Str(string_literal_text(&s.to_string())),
            _ => return false,
        };
        self.out.consts.insert(var.to_string(), val);
        true
    }

    fn try_record_dep_table(&mut self, var: &str, expr: &ast::Expression) -> bool {
        let ast::Expression::TableConstructor(table) = expr else {
            return false;
        };
        let mut items = Vec::new();
        for field in table.fields() {
            if let ast::Field::NoKey(ast::Expression::String(s)) = field {
                items.push(string_literal_text(&s.to_string()));
            }
        }
        if items.is_empty() {
            return false;
        }
        self.out.dep_tables.insert(var.to_string(), items);
        true
    }

    fn try_record_export(&mut self, assign: &ast::Assignment) {
        let vars: Vec<_> = assign.variables().iter().collect();
        let exprs: Vec<_> = assign.expressions().iter().collect();
        for (var, expr) in vars.iter().zip(exprs.iter()) {
            let ast::Var::Name(name) = var else { continue };
            let ast::Expression::FunctionCall(call) = expr else {
                continue;
            };
            let ast::Prefix::Name(prefix) = call.prefix() else {
                continue;
            };
            if prefix.token().to_string() != "Class" {
                continue;
            }
            let Some(ast::Suffix::Call(ast::Call::AnonymousCall(args))) = call.suffixes().next()
            else {
                continue;
            };
            let ast::FunctionArgs::Parentheses { arguments, .. } = args else {
                continue;
            };
            for arg in arguments.iter() {
                if let ast::Expression::Function(func) = arg {
                    let params: Vec<String> = func
                        .body()
                        .parameters()
                        .iter()
                        .map(|p| p.to_string())
                        .collect();
                    self.out.exports.push(ExportedCtor {
                        name: name.token().to_string(),
                        params,
                    });
                }
            }
        }
    }

    fn try_record_prefab_reg_from_expr(&mut self, expr: &ast::Expression) -> bool {
        let ast::Expression::FunctionCall(call) = expr else {
            return false;
        };
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return false;
        };
        if prefix.token().to_string() != "Prefab" {
            return false;
        }
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(args))) = call.suffixes().next() else {
            return false;
        };
        let ast::FunctionArgs::Parentheses { arguments, .. } = args else {
            return false;
        };
        let argv: Vec<_> = arguments.iter().collect();
        let name = argv.first().and_then(|e| classify_str_arg(e));
        let fn_ref = argv.get(1).and_then(|e| match e {
            ast::Expression::Var(ast::Var::Name(n)) => Some(n.token().to_string()),
            _ => None,
        });
        let deps_var = argv.get(3).and_then(|e| match e {
            ast::Expression::Var(ast::Var::Name(n)) => Some(n.token().to_string()),
            _ => None,
        });
        self.out.prefab_regs.push(PrefabReg {
            name,
            fn_ref,
            deps_var,
            line: self.line(call),
        });
        true
    }

    fn record_fn_def(
        &mut self,
        name: &str,
        body: &ast::FunctionBody,
        is_local: bool,
        start: Option<full_moon::tokenizer::Position>,
        end: Option<full_moon::tokenizer::Position>,
    ) {
        let line_of_byte = |b: usize| line_of(self.source, b);
        self.out.fns.push(FnDef {
            name: name.to_string(),
            params: body.parameters().iter().map(|p| p.to_string()).collect(),
            start_byte: start.map(|p| p.bytes()).unwrap_or(0),
            end_byte: end.map(|p| p.bytes()).unwrap_or(0),
            start_line: start.map(|p| line_of_byte(p.bytes())).unwrap_or(0),
            end_line: end.map(|p| line_of_byte(p.bytes())).unwrap_or(0),
            is_local,
        });
    }

    // ------------------------------------------------------------ call sites

    fn handle_call(&mut self, call: &ast::FunctionCall) {
        // Count stategraph building blocks wherever they appear.
        if let ast::Prefix::Name(prefix) = call.prefix() {
            match prefix.token().to_string().as_str() {
                "State" => {
                    self.out.state_count += 1;
                }
                "EventHandler" => {
                    self.out.event_handler_count += 1;
                }
                _ => {}
            }
        }

        let line = self.line(call);
        let suffixes: Vec<_> = call.suffixes().collect();

        // Plain call: `name(...)`. Also captures the bare `require "mod"`
        // form used across DST brain/stategraph files.
        if let (ast::Prefix::Name(name), Some(first_suffix)) = (call.prefix(), suffixes.first()) {
            if name.token().to_string() == "require" {
                let path = match first_suffix {
                    ast::Suffix::Call(ast::Call::AnonymousCall(
                        ast::FunctionArgs::Parentheses { arguments, .. },
                    )) => arguments.iter().next().and_then(classify_str_arg),
                    ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::String(s))) => {
                        Some(string_literal_text(&s.to_string()))
                    }
                    _ => None,
                };
                if let Some(path) = path {
                    self.out.requires.push((String::new(), path));
                }
                return;
            }
            if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = first_suffix {
                let parsed = self.classify_args(args);
                let kind = self.classify_plain_call(&name.token().to_string());
                self.out.calls.push(AssocCall {
                    kind,
                    args: parsed,
                    scope: self.scope.clone(),
                    receiver_is_inst: true,
                    line,
                });
                self.descend_args(args);
                return;
            }
        }

        // Method call somewhere in the suffix chain. The receiver path
        // `inst.components.combat` parses as Prefix::Name plus Dot indexes.
        let method_call = suffixes.iter().find_map(|s| match s {
            ast::Suffix::Call(ast::Call::MethodCall(mc)) => Some(mc),
            _ => None,
        });
        if let Some(mc) = method_call {
            let mut parts: Vec<String> = Vec::new();
            if let ast::Prefix::Name(n) = call.prefix() {
                parts.push(n.token().to_string());
            }
            for suffix in &suffixes {
                match suffix {
                    ast::Suffix::Index(ast::Index::Dot { name, .. }) => {
                        parts.push(name.token().to_string());
                    }
                    ast::Suffix::Call(_) => break,
                    _ => {}
                }
            }
            let mname = mc.name().to_string();
            let receiver_is_inst = parts.first().is_some_and(|r| self.inst_aliases.contains(r));
            let kind = match mname.as_str() {
                "AddComponent" | "SetStateGraph" | "SetBrain" if parts.len() == 1 => {
                    match mname.as_str() {
                        "AddComponent" => CallKind::AddComponent,
                        "SetStateGraph" => CallKind::SetStateGraph,
                        _ => CallKind::SetBrain,
                    }
                }
                _ if parts.len() >= 3 && parts[1] == "components" => CallKind::ComponentMethod {
                    component: parts[2].clone(),
                    method: mname.clone(),
                },
                _ => {
                    // Unrelated method call; still descend into its args.
                    self.descend_method_args(mc);
                    return;
                }
            };
            self.out.calls.push(AssocCall {
                kind,
                args: self.classify_args(mc.args()),
                scope: self.scope.clone(),
                receiver_is_inst,
                line,
            });
            self.descend_method_args(mc);

            // Descend into dot-indexed expressions (no-op for names) and any
            // bracketed indexes that may hide calls.
            if let ast::Prefix::Expression(e) = call.prefix() {
                self.scan_expr(e);
            }
            for suffix in &suffixes {
                if let ast::Suffix::Index(ast::Index::Brackets { expression, .. }) = suffix {
                    self.scan_expr(expression);
                }
            }
            return;
        }

        // No recognizable call shape: still walk everything for nested fns.
        if let ast::Prefix::Expression(e) = call.prefix() {
            self.scan_expr(e);
        }
        for suffix in &suffixes {
            match suffix {
                ast::Suffix::Call(ast::Call::AnonymousCall(args)) => self.descend_args(args),
                ast::Suffix::Call(ast::Call::MethodCall(mc)) => self.descend_method_args(mc),
                ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
                    self.scan_expr(expression)
                }
                _ => {}
            }
        }
    }

    fn classify_plain_call(&self, callee: &str) -> CallKind {
        if callee.starts_with("Make") {
            return CallKind::Helper {
                name: callee.to_string(),
            };
        }
        match self.role {
            Role::Brain => CallKind::CtorCall {
                name: callee.to_string(),
            },
            _ => CallKind::LocalFnCall {
                callee: callee.to_string(),
            },
        }
    }

    // ------------------------------------------------------------ expressions

    fn classify_args(&self, args: &ast::FunctionArgs) -> Vec<ArgExpr> {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                arguments.iter().map(|e| self.classify_expr(e)).collect()
            }
            ast::FunctionArgs::String(s) => vec![ArgExpr::Str(string_literal_text(&s.to_string()))],
            _ => vec![ArgExpr::Other],
        }
    }

    fn classify_expr(&self, expr: &ast::Expression) -> ArgExpr {
        match expr {
            ast::Expression::String(s) => ArgExpr::Str(string_literal_text(&s.to_string())),
            ast::Expression::Number(n) => ArgExpr::Num(n.to_string().trim().to_string()),
            ast::Expression::Var(ast::Var::Name(n)) => ArgExpr::Ident(n.token().to_string()),
            ast::Expression::Var(ast::Var::Expression(vex)) => {
                ArgExpr::Field(vex.to_string().trim().to_string())
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op = binop.to_string().trim().to_string();
                let l = Box::new(self.classify_expr(lhs));
                let r = Box::new(self.classify_expr(rhs));
                if op == "or" {
                    ArgExpr::Or(l, r)
                } else if op == "and" {
                    ArgExpr::And(l, r)
                } else {
                    ArgExpr::Other
                }
            }
            ast::Expression::Function(_) => ArgExpr::FnRef,
            _ => ArgExpr::Other,
        }
    }

    fn descend_args(&mut self, args: &ast::FunctionArgs) {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                for e in arguments.iter() {
                    self.scan_expr(e);
                }
            }
            ast::FunctionArgs::TableConstructor(table) => self.walk_table(table),
            _ => {}
        }
    }

    fn descend_method_args(&mut self, mc: &ast::MethodCall) {
        self.descend_args(mc.args());
    }

    fn scan_expr(&mut self, expr: &ast::Expression) {
        match expr {
            ast::Expression::FunctionCall(call) => self.handle_call(call),
            ast::Expression::Function(func) => {
                let saved = self.enter_anon_fn(func.body());
                self.walk_block(func.body().block());
                self.inst_aliases = saved;
            }
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.scan_expr(lhs);
                self.scan_expr(rhs);
            }
            ast::Expression::UnaryOperator { expression, .. } => self.scan_expr(expression),
            ast::Expression::Parentheses { expression, .. } => self.scan_expr(expression),
            ast::Expression::TableConstructor(table) => self.walk_table(table),
            ast::Expression::Var(ast::Var::Name(name)) => {
                let name = name.token().to_string();
                if self.out.fn_def(&name).is_some() {
                    self.out.fn_refs.push(FnRefUse {
                        name,
                        scope: self.scope.clone(),
                        line: self.line(expr),
                    });
                }
            }
            ast::Expression::Var(ast::Var::Expression(vex)) => self.scan_var_expression(vex),
            _ => {}
        }
    }

    fn scan_var_expression(&mut self, vex: &ast::VarExpression) {
        for suffix in vex.suffixes() {
            match suffix {
                ast::Suffix::Call(ast::Call::AnonymousCall(args)) => self.descend_args(args),
                ast::Suffix::Call(ast::Call::MethodCall(mc)) => self.descend_method_args(mc),
                ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
                    self.scan_expr(expression)
                }
                _ => {}
            }
        }
    }

    fn walk_table(&mut self, table: &ast::TableConstructor) {
        for field in table.fields() {
            match field {
                ast::Field::NoKey(expression) => self.scan_expr(expression),
                ast::Field::NameKey { value, .. } => self.scan_expr(value),
                ast::Field::ExpressionKey { key, value, .. } => {
                    self.scan_expr(key);
                    self.scan_expr(value);
                }
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------ fn scoping

    fn enter_named_fn(&mut self, name: &str, body: &ast::FunctionBody) -> HashSet<String> {
        self.scope.push(name.to_string());
        let saved = self.inst_aliases.clone();
        self.apply_param_aliasing(body.parameters());
        saved
    }

    fn exit_named_fn(&mut self, saved: HashSet<String>) {
        self.inst_aliases = saved;
        self.scope.pop();
    }

    fn enter_anon_fn(&mut self, body: &ast::FunctionBody) -> HashSet<String> {
        let saved = self.inst_aliases.clone();
        self.apply_param_aliasing(body.parameters());
        saved
    }

    fn apply_param_aliasing(&mut self, parameters: &ast::punctuated::Punctuated<ast::Parameter>) {
        let names: Vec<String> = parameters.iter().map(|p| p.to_string()).collect();
        if names.iter().any(|p| p == "inst") {
            self.inst_aliases.clear();
            self.inst_aliases.insert("inst".to_string());
        }
    }
}
