//! Lua 源码扫描器：从 prefabs 源码中提取 Asset/Prefab 声明与模板绑定
//! （自 index.rs 拆分；类型定义见 [`super`]）。

use super::*;
pub(crate) struct Scanner<'s> {
    source: &'s str,
    /// Local table variable name -> Asset entries collected from a literal
    /// `local x = { Asset(...), ... }` and `table.insert(x, Asset(...))`.
    asset_tables: BTreeMap<String, Vec<AssetEntry>>,
    /// Local string constants used to resolve simple variable arguments.
    consts: BTreeMap<String, String>,
    /// Asset-returning factory functions (`makeassetlist`, etc.).
    asset_factories: BTreeMap<String, AssetFactory>,
    /// Prefab-returning factory functions (`MakeAxe`, `item`, `pillar`, etc.).
    prefab_factories: BTreeMap<String, PrefabFactory>,
    /// Parsed `Prefab(...)` records and expanded factory records.
    prefabs: Vec<PrefabAnimRecord>,
    /// Count of PKGREF `.dyn` references we intentionally skip.
    skipped_pkgref_dyn: usize,
}

impl<'s> Scanner<'s> {
    pub(crate) fn new(source: &'s str) -> Self {
        Self {
            source,
            asset_tables: BTreeMap::new(),
            consts: BTreeMap::new(),
            asset_factories: BTreeMap::new(),
            prefab_factories: BTreeMap::new(),
            prefabs: Vec::new(),
            skipped_pkgref_dyn: 0,
        }
    }

    pub(crate) fn scan(mut self, prefab_file: String) -> ScannedFile {
        let ast = match full_moon::parse(self.source) {
            Ok(ast) => ast,
            Err(e) => {
                // A parse failure should not abort the whole index; report it
                // as an unresolved prefab so the file is visible in stats.
                return ScannedFile {
                    prefabs: vec![PrefabAnimRecord {
                        prefab_file,
                        prefab_name: None,
                        asset_var: None,
                        anims: Vec::new(),
                        related_files: Vec::new(),
                        unresolved: vec![UnresolvedRef {
                            kind: "PARSE_ERROR".to_string(),
                            raw: format!("{:?}", e),
                            line: 0,
                        }],
                        content: AnimContent::default(),
                    }],
                    skipped_pkgref_dyn: 0,
                };
            }
        };
        self.collect_consts_and_factories(ast.nodes());
        self.walk_block(ast.nodes());
        for record in &mut self.prefabs {
            record.prefab_file = prefab_file.clone();
        }
        ScannedFile {
            prefabs: self.prefabs,
            skipped_pkgref_dyn: self.skipped_pkgref_dyn,
        }
    }

    fn collect_consts_and_factories(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assign) => {
                    let names: Vec<_> = assign.names().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        let var = name.token().to_string();
                        if let ast::Expression::String(s) = expr {
                            self.consts
                                .insert(var.clone(), string_literal_text(&s.to_string()));
                        }
                        if let ast::Expression::Function(func) = expr {
                            self.register_factory(&var, func.body());
                            self.collect_consts_and_factories(func.body().block());
                        }
                    }
                }
                ast::Stmt::Assignment(assign) => {
                    let vars: Vec<_> = assign.variables().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (var, expr) in vars.iter().zip(exprs.iter()) {
                        if let ast::Var::Name(name) = var {
                            if let ast::Expression::Function(func) = expr {
                                let var_name = name.token().to_string();
                                self.register_factory(&var_name, func.body());
                                self.collect_consts_and_factories(func.body().block());
                            }
                        }
                    }
                }
                ast::Stmt::LocalFunction(local_fn) => {
                    let name = local_fn.name().to_string();
                    self.register_factory(&name, local_fn.body());
                    self.collect_consts_and_factories(local_fn.body().block());
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    let name = func_decl.name().to_string();
                    self.register_factory(&name, func_decl.body());
                    self.collect_consts_and_factories(func_decl.body().block());
                }
                ast::Stmt::If(if_stmt) => {
                    self.collect_consts_and_factories(if_stmt.block());
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.collect_consts_and_factories(else_if.block());
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.collect_consts_and_factories(else_block);
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.collect_consts_and_factories(while_stmt.block())
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.collect_consts_and_factories(repeat_stmt.block())
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.collect_consts_and_factories(for_stmt.block())
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    self.collect_consts_and_factories(for_stmt.block())
                }
                ast::Stmt::Do(do_stmt) => self.collect_consts_and_factories(do_stmt.block()),
                _ => {}
            }
        }
    }

    fn register_factory(&mut self, name: &str, body: &ast::FunctionBody) {
        let params: Vec<String> = body.parameters().iter().map(|p| p.to_string()).collect();

        if let Some(asset_factory) = self.collect_asset_factory(name, body, &params) {
            self.asset_factories.insert(name.to_string(), asset_factory);
        }
        if let Some(prefab_factory) = self.collect_prefab_factory(name, body, &params) {
            self.prefab_factories
                .insert(name.to_string(), prefab_factory);
        }
    }

    fn collect_asset_factory(
        &self,
        _name: &str,
        body: &ast::FunctionBody,
        params: &[String],
    ) -> Option<AssetFactory> {
        let mut entries = Vec::new();
        if let Some(ast::LastStmt::Return(ret)) = body.block().last_stmt() {
            for expr in ret.returns() {
                if let ast::Expression::TableConstructor(table) = expr {
                    let mut found = false;
                    for field in table.fields() {
                        let fexpr = match field {
                            ast::Field::NoKey(expression) => expression,
                            ast::Field::NameKey { value, .. } => value,
                            ast::Field::ExpressionKey { value, .. } => value,
                            _ => continue,
                        };
                        if let ast::Expression::FunctionCall(call) = fexpr {
                            if let Some(entry) = self.parse_template_asset_call(call, params) {
                                entries.push(entry);
                                found = true;
                            }
                        }
                    }
                    if found {
                        return Some(AssetFactory {
                            params: params.to_vec(),
                            entries,
                        });
                    }
                }
            }
        }
        None
    }

    fn collect_prefab_factory(
        &self,
        _name: &str,
        body: &ast::FunctionBody,
        params: &[String],
    ) -> Option<PrefabFactory> {
        let mut calls = Vec::new();
        self.collect_prefab_calls_in_block(body.block(), &mut calls);
        if calls.is_empty() {
            return None;
        }
        let mut templates = Vec::new();
        for call in calls {
            if let Some(template) = self.parse_prefab_template(call, params) {
                templates.push(template);
            }
        }
        if templates.is_empty() {
            None
        } else {
            Some(PrefabFactory {
                params: params.to_vec(),
                templates,
            })
        }
    }

    fn collect_prefab_calls_in_block<'a>(
        &self,
        block: &'a ast::Block,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call) => {
                    if is_named_call(call, "Prefab") {
                        out.push(call);
                    }
                    self.collect_prefab_calls_in_args(call, out);
                }
                ast::Stmt::LocalAssignment(assign) => {
                    for expr in assign.expressions().iter() {
                        self.collect_prefab_calls_in_expr(expr, out);
                    }
                }
                ast::Stmt::Assignment(assign) => {
                    for expr in assign.expressions().iter() {
                        self.collect_prefab_calls_in_expr(expr, out);
                    }
                }
                ast::Stmt::LocalFunction(_) => {}
                ast::Stmt::FunctionDeclaration(_) => {}
                ast::Stmt::If(if_stmt) => {
                    self.collect_prefab_calls_in_block(if_stmt.block(), out);
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.collect_prefab_calls_in_block(else_if.block(), out);
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.collect_prefab_calls_in_block(else_block, out);
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.collect_prefab_calls_in_block(while_stmt.block(), out);
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.collect_prefab_calls_in_block(repeat_stmt.block(), out);
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.collect_prefab_calls_in_block(for_stmt.block(), out);
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    self.collect_prefab_calls_in_block(for_stmt.block(), out);
                }
                ast::Stmt::Do(do_stmt) => {
                    self.collect_prefab_calls_in_block(do_stmt.block(), out);
                }
                _ => {}
            }
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns() {
                self.collect_prefab_calls_in_expr(expr, out);
            }
        }
    }

    fn collect_prefab_calls_in_args<'a>(
        &self,
        call: &'a ast::FunctionCall,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        for suffix in call.suffixes() {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                self.collect_prefab_calls_in_args_expr(args, out);
            }
        }
    }

    fn collect_prefab_calls_in_args_expr<'a>(
        &self,
        args: &'a ast::FunctionArgs,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                for expr in arguments.iter() {
                    self.collect_prefab_calls_in_expr(expr, out);
                }
            }
            ast::FunctionArgs::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => {
                            self.collect_prefab_calls_in_expr(expression, out)
                        }
                        ast::Field::NameKey { value, .. } => {
                            self.collect_prefab_calls_in_expr(value, out)
                        }
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.collect_prefab_calls_in_expr(key, out);
                            self.collect_prefab_calls_in_expr(value, out);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_prefab_calls_in_expr<'a>(
        &self,
        expr: &'a ast::Expression,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        match expr {
            ast::Expression::FunctionCall(call) => {
                if is_named_call(call, "Prefab") {
                    out.push(call);
                }
                self.collect_prefab_calls_in_args(call, out);
            }
            ast::Expression::Function(_) => {}
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.collect_prefab_calls_in_expr(lhs, out);
                self.collect_prefab_calls_in_expr(rhs, out);
            }
            ast::Expression::UnaryOperator { expression, .. } => {
                self.collect_prefab_calls_in_expr(expression, out);
            }
            ast::Expression::Parentheses { expression, .. } => {
                self.collect_prefab_calls_in_expr(expression, out);
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => {
                            self.collect_prefab_calls_in_expr(expression, out)
                        }
                        ast::Field::NameKey { value, .. } => {
                            self.collect_prefab_calls_in_expr(value, out)
                        }
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.collect_prefab_calls_in_expr(key, out);
                            self.collect_prefab_calls_in_expr(value, out);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn parse_prefab_template(
        &self,
        call: &ast::FunctionCall,
        params: &[String],
    ) -> Option<PrefabTemplate> {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "Prefab" {
            return None;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return None;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 3 {
            return None;
        }
        let name = classify_template_name(args[0], params);
        let assets = self.classify_template_assets(args[2], params);
        Some(PrefabTemplate {
            name,
            assets,
            line: self.line(args[2]),
        })
    }

    fn classify_template_assets(
        &self,
        expr: &ast::Expression,
        params: &[String],
    ) -> TemplateAssets {
        match expr {
            ast::Expression::Var(ast::Var::Name(name)) => {
                let var = name.token().to_string();
                if params.iter().any(|p| p == &var) {
                    TemplateAssets::Param(var)
                } else {
                    TemplateAssets::Other(var)
                }
            }
            ast::Expression::FunctionCall(call) => {
                if let ast::Prefix::Name(prefix) = call.prefix() {
                    let factory_name = prefix.token().to_string();
                    if self.asset_factories.contains_key(&factory_name) {
                        let args = self.classify_template_args(call, params);
                        return TemplateAssets::FactoryCall {
                            name: factory_name,
                            args,
                        };
                    }
                }
                TemplateAssets::Other(expr.to_string().trim().to_string())
            }
            ast::Expression::TableConstructor(table) => {
                let entries = self.parse_template_asset_table(table, params);
                TemplateAssets::Inline(entries)
            }
            other => TemplateAssets::Other(other.to_string().trim().to_string()),
        }
    }

    fn classify_template_args(
        &self,
        call: &ast::FunctionCall,
        params: &[String],
    ) -> Vec<TemplateArg> {
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return Vec::new();
        };
        arguments
            .iter()
            .map(|expr| classify_template_arg(expr, params))
            .collect()
    }

    fn parse_template_asset_table(
        &self,
        table: &ast::TableConstructor,
        params: &[String],
    ) -> Vec<TemplateAssetEntry> {
        let mut entries = Vec::new();
        for field in table.fields() {
            let expr = match field {
                ast::Field::NoKey(expression) => expression,
                ast::Field::NameKey { value, .. } => value,
                ast::Field::ExpressionKey { value, .. } => value,
                _ => continue,
            };
            if let ast::Expression::FunctionCall(call) = expr {
                if let Some(entry) = self.parse_template_asset_call(call, params) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    fn parse_template_asset_call(
        &self,
        call: &ast::FunctionCall,
        params: &[String],
    ) -> Option<TemplateAssetEntry> {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "Asset" {
            return None;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        })) = suffixes.first()?
        else {
            return None;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 2 {
            return None;
        }
        let kind = match &args[0] {
            ast::Expression::String(s) => string_literal_text(&s.to_string()),
            _ => return None,
        };
        if kind != "ANIM" && kind != "DYNAMIC_ANIM" {
            return None;
        }
        let path_template = parse_path_template(args[1], params);
        Some(TemplateAssetEntry {
            kind,
            raw: args[1].to_string().trim().to_string(),
            line: self.line(call),
            path_template,
        })
    }

    fn walk_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assign) => {
                    let names: Vec<_> = assign.names().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        let var = name.token().to_string();
                        if let ast::Expression::TableConstructor(table) = expr {
                            let mut entries = self.parse_asset_table(table);
                            if let Some(existing) = self.asset_tables.get_mut(&var) {
                                existing.append(&mut entries);
                            } else {
                                self.asset_tables.insert(var.clone(), entries);
                            }
                        }
                        let is_prefab_factory_fn = matches!(expr, ast::Expression::Function(_))
                            && self.prefab_factories.contains_key(&var);
                        if !is_prefab_factory_fn {
                            self.scan_expr_for_prefabs(expr);
                        }
                    }
                }
                ast::Stmt::Assignment(assign) => {
                    let vars: Vec<_> = assign.variables().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (var, expr) in vars.iter().zip(exprs.iter()) {
                        let name = match var {
                            ast::Var::Name(name) => name.token().to_string(),
                            _ => continue,
                        };
                        if let ast::Expression::TableConstructor(table) = expr {
                            let mut entries = self.parse_asset_table(table);
                            if let Some(existing) = self.asset_tables.get_mut(&name) {
                                existing.append(&mut entries);
                            } else {
                                self.asset_tables.insert(name.clone(), entries);
                            }
                        }
                        let is_prefab_factory_fn = matches!(expr, ast::Expression::Function(_))
                            && self.prefab_factories.contains_key(&name);
                        if !is_prefab_factory_fn {
                            self.scan_expr_for_prefabs(expr);
                        }
                    }
                }
                ast::Stmt::FunctionCall(call) => {
                    self.try_collect_table_insert(call);
                    self.scan_call_for_prefabs(call);
                }
                ast::Stmt::LocalFunction(local_fn) => {
                    let name = local_fn.name().to_string();
                    if !self.prefab_factories.contains_key(&name) {
                        self.walk_block(local_fn.body().block());
                    }
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    let name = func_decl.name().to_string();
                    if !self.prefab_factories.contains_key(&name) {
                        self.walk_block(func_decl.body().block());
                    }
                }
                ast::Stmt::If(if_stmt) => {
                    self.walk_block(if_stmt.block());
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.walk_block(else_if.block());
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.walk_block(else_block);
                    }
                }
                ast::Stmt::While(while_stmt) => self.walk_block(while_stmt.block()),
                ast::Stmt::Repeat(repeat_stmt) => self.walk_block(repeat_stmt.block()),
                ast::Stmt::NumericFor(for_stmt) => self.walk_block(for_stmt.block()),
                ast::Stmt::GenericFor(for_stmt) => self.walk_block(for_stmt.block()),
                ast::Stmt::Do(do_stmt) => self.walk_block(do_stmt.block()),
                _ => {}
            }
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns() {
                self.scan_expr_for_prefabs(expr);
            }
        }
    }

    fn parse_asset_table(&mut self, table: &ast::TableConstructor) -> Vec<AssetEntry> {
        let mut entries = Vec::new();
        for field in table.fields() {
            let expr = match field {
                ast::Field::NoKey(expression) => expression,
                ast::Field::NameKey { value, .. } => value,
                ast::Field::ExpressionKey { value, .. } => value,
                _ => continue,
            };
            if let ast::Expression::FunctionCall(call) = expr {
                if let Some(entry) = self.parse_asset_call(call) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    fn try_collect_table_insert(&mut self, call: &ast::FunctionCall) {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return;
        };
        if prefix.token().to_string() != "table" {
            return;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        // Find `table.insert(...)`.
        let has_insert = suffixes.iter().any(|s| {
            matches!(
                s,
                ast::Suffix::Index(ast::Index::Dot { name, .. })
                    if name.token().to_string() == "insert"
            )
        });
        if !has_insert {
            return;
        }
        for suffix in &suffixes {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
                arguments,
                ..
            })) = suffix
            {
                let args: Vec<_> = arguments.iter().collect();
                if args.len() < 2 {
                    continue;
                }
                let var = match &args[0] {
                    ast::Expression::Var(ast::Var::Name(name)) => name.token().to_string(),
                    _ => continue,
                };
                if let ast::Expression::FunctionCall(inner) = &args[1] {
                    if let Some(entry) = self.parse_asset_call(inner) {
                        self.asset_tables.entry(var).or_default().push(entry);
                    }
                }
            }
        }
    }

    fn parse_asset_call(&mut self, call: &ast::FunctionCall) -> Option<AssetEntry> {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "Asset" {
            return None;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        })) = suffixes.first()?
        else {
            return None;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 2 {
            return None;
        }
        let kind = match &args[0] {
            ast::Expression::String(s) => string_literal_text(&s.to_string()),
            _ => return None,
        };
        let line = self.line(call);
        if kind == "PKGREF" {
            // Skin `.dyn` references are intentionally out of scope for now,
            // but `.zip` build/package references are useful for rendering.
            if let ast::Expression::String(s) = &args[1] {
                let path = string_literal_text(&s.to_string());
                if path.ends_with(".dyn") {
                    self.skipped_pkgref_dyn += 1;
                    return None;
                }
                return Some(AssetEntry {
                    kind,
                    raw_path: path.clone(),
                    line,
                    normalized: Some(normalize_asset_path("PKGREF", &path)),
                });
            }
            return None;
        }
        if kind != "ANIM" && kind != "DYNAMIC_ANIM" {
            return None;
        }
        let normalized = match &args[1] {
            ast::Expression::String(s) => Some(normalize_asset_path(
                &kind,
                &string_literal_text(&s.to_string()),
            )),
            _ => None,
        };
        let raw = match &args[1] {
            ast::Expression::String(s) => string_literal_text(&s.to_string()),
            _ => args[1].to_string().trim().to_string(),
        };
        Some(AssetEntry {
            kind,
            raw_path: raw,
            line,
            normalized,
        })
    }

    fn scan_call_for_prefabs(&mut self, call: &ast::FunctionCall) {
        // Expand known prefab-returning factory calls (MakeAxe, item, pillar, ...).
        if let ast::Prefix::Name(prefix) = call.prefix() {
            let name = prefix.token().to_string();
            if self.prefab_factories.contains_key(&name) {
                self.instantiate_prefab_factory(&name, call);
                return;
            }
        }
        if self.try_parse_prefab_call(call, None) {
            return;
        }
        // Recurse into arguments for nested Prefab calls / table constructors.
        for suffix in call.suffixes() {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                self.scan_args_for_prefabs(args);
            }
        }
    }

    fn instantiate_prefab_factory(&mut self, factory_name: &str, call: &ast::FunctionCall) {
        let Some(factory) = self.prefab_factories.get(factory_name).cloned() else {
            return;
        };
        let resolved_args = self.resolve_call_args(call);
        let mut bindings = BTreeMap::new();
        for (i, param) in factory.params.iter().enumerate() {
            if let Some(arg) = resolved_args.get(i) {
                bindings.insert(param.clone(), arg.clone());
            }
        }

        for template in &factory.templates {
            let prefab_name = self.resolve_template_name(&template.name, &bindings);
            let mut anims = Vec::new();
            let mut related_files = Vec::new();
            let mut unresolved = Vec::new();
            let mut asset_var = None;

            match &template.assets {
                TemplateAssets::Param(param) => {
                    if let Some(arg) = bindings.get(param) {
                        self.push_resolved_arg_as_assets(
                            arg,
                            &mut anims,
                            &mut related_files,
                            &mut unresolved,
                            &mut asset_var,
                        );
                    } else {
                        unresolved.push(UnresolvedRef {
                            kind: "ASSET_TABLE".to_string(),
                            raw: param.clone(),
                            line: template.line,
                        });
                    }
                }
                TemplateAssets::FactoryCall {
                    name: factory_name,
                    args,
                } => {
                    let mut resolved_factory_args = Vec::new();
                    for arg in args {
                        resolved_factory_args.push(self.resolve_template_arg(arg, &bindings));
                    }
                    let (entries, unrs) =
                        self.eval_asset_factory_resolved(factory_name, &resolved_factory_args);
                    anims.extend(entries.into_iter().map(|e| AnimRef {
                        kind: e.kind,
                        path: e.raw_path,
                        normalized: e.normalized.unwrap_or_default(),
                        exists: false,
                    }));
                    unresolved.extend(unrs);
                }
                TemplateAssets::Inline(entries) => {
                    let (concrete, unrs) = self.resolve_template_entries(entries, &bindings);
                    for e in concrete {
                        self.push_asset_entry(&e, &mut anims, &mut related_files, &mut unresolved);
                    }
                    unresolved.extend(unrs);
                }
                TemplateAssets::Other(raw) => {
                    unresolved.push(UnresolvedRef {
                        kind: "ASSET_TABLE".to_string(),
                        raw: raw.clone(),
                        line: template.line,
                    });
                }
            }

            self.prefabs.push(PrefabAnimRecord {
                prefab_file: String::new(),
                prefab_name,
                asset_var,
                anims,
                related_files,
                unresolved,
                content: AnimContent::default(),
            });
        }
    }

    fn resolve_call_args(&mut self, call: &ast::FunctionCall) -> Vec<ResolvedArg> {
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return Vec::new();
        };
        arguments
            .iter()
            .map(|expr| self.resolve_call_arg(expr))
            .collect()
    }

    fn resolve_call_arg(&mut self, expr: &ast::Expression) -> ResolvedArg {
        match expr {
            ast::Expression::String(s) => ResolvedArg::Str(string_literal_text(&s.to_string())),
            ast::Expression::Var(ast::Var::Name(name)) => {
                ResolvedArg::Var(name.token().to_string())
            }
            ast::Expression::TableConstructor(table) => {
                ResolvedArg::Table(self.parse_asset_table(table))
            }
            other => ResolvedArg::Other(other.to_string().trim().to_string()),
        }
    }

    fn resolve_template_name(
        &self,
        template: &TemplateName,
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> Option<String> {
        match template {
            TemplateName::Lit(s) => Some(s.clone()),
            TemplateName::Param(p) => bindings.get(p).and_then(|arg| self.resolve_arg_string(arg)),
            TemplateName::Other => None,
        }
    }

    fn resolve_template_arg(
        &self,
        arg: &TemplateArg,
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> ResolvedArg {
        match arg {
            TemplateArg::Lit(s) => ResolvedArg::Str(s.clone()),
            TemplateArg::Param(p) => bindings
                .get(p)
                .cloned()
                .unwrap_or(ResolvedArg::Other(p.clone())),
            TemplateArg::Other(raw) => ResolvedArg::Other(raw.clone()),
        }
    }

    fn resolve_arg_string(&self, arg: &ResolvedArg) -> Option<String> {
        match arg {
            ResolvedArg::Str(s) => Some(s.clone()),
            ResolvedArg::Var(v) => self.consts.get(v).cloned(),
            _ => None,
        }
    }

    fn push_resolved_arg_as_assets(
        &mut self,
        arg: &ResolvedArg,
        anims: &mut Vec<AnimRef>,
        related_files: &mut Vec<String>,
        unresolved: &mut Vec<UnresolvedRef>,
        asset_var: &mut Option<String>,
    ) {
        match arg {
            ResolvedArg::Var(v) => {
                *asset_var = Some(v.clone());
                if let Some(entries) = self.asset_tables.get(v) {
                    for entry in entries {
                        self.push_asset_entry(entry, anims, related_files, unresolved);
                    }
                } else {
                    unresolved.push(UnresolvedRef {
                        kind: "ASSET_TABLE".to_string(),
                        raw: v.clone(),
                        line: 0,
                    });
                }
            }
            ResolvedArg::Table(entries) => {
                for entry in entries {
                    self.push_asset_entry(entry, anims, related_files, unresolved);
                }
            }
            ResolvedArg::Str(s) => {
                unresolved.push(UnresolvedRef {
                    kind: "ASSET_TABLE".to_string(),
                    raw: s.clone(),
                    line: 0,
                });
            }
            ResolvedArg::Other(raw) => {
                unresolved.push(UnresolvedRef {
                    kind: "ASSET_TABLE".to_string(),
                    raw: raw.clone(),
                    line: 0,
                });
            }
        }
    }

    fn eval_asset_factory_call(
        &mut self,
        call: &ast::FunctionCall,
    ) -> (Vec<AssetEntry>, Vec<UnresolvedRef>) {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return (Vec::new(), Vec::new());
        };
        let factory_name = prefix.token().to_string();
        let args = self.resolve_call_args(call);
        self.eval_asset_factory_resolved(&factory_name, &args)
    }

    fn eval_asset_factory_resolved(
        &mut self,
        factory_name: &str,
        args: &[ResolvedArg],
    ) -> (Vec<AssetEntry>, Vec<UnresolvedRef>) {
        let Some(factory) = self.asset_factories.get(factory_name).cloned() else {
            return (
                Vec::new(),
                vec![UnresolvedRef {
                    kind: "ASSET_FACTORY".to_string(),
                    raw: factory_name.to_string(),
                    line: 0,
                }],
            );
        };
        let mut bindings = BTreeMap::new();
        for (i, param) in factory.params.iter().enumerate() {
            if let Some(arg) = args.get(i) {
                bindings.insert(param.clone(), arg.clone());
            }
        }
        self.resolve_template_entries(&factory.entries, &bindings)
    }

    fn resolve_template_entries(
        &mut self,
        entries: &[TemplateAssetEntry],
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> (Vec<AssetEntry>, Vec<UnresolvedRef>) {
        let mut anims = Vec::new();
        let mut unresolved = Vec::new();
        for entry in entries {
            let Some(path_template) = &entry.path_template else {
                unresolved.push(UnresolvedRef {
                    kind: entry.kind.clone(),
                    raw: entry.raw.clone(),
                    line: entry.line,
                });
                continue;
            };
            let Some(raw_path) = self.eval_path_template(path_template, bindings) else {
                unresolved.push(UnresolvedRef {
                    kind: entry.kind.clone(),
                    raw: entry.raw.clone(),
                    line: entry.line,
                });
                continue;
            };
            anims.push(AssetEntry {
                kind: entry.kind.clone(),
                raw_path: raw_path.clone(),
                line: entry.line,
                normalized: Some(normalize_asset_path(&entry.kind, &raw_path)),
            });
        }
        (anims, unresolved)
    }

    fn eval_path_template(
        &self,
        template: &PathTemplate,
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> Option<String> {
        match template {
            PathTemplate::Literal(s) => Some(s.clone()),
            PathTemplate::Concat(parts) => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        PathPart::Lit(s) => out.push_str(s),
                        PathPart::Param(p) => {
                            let arg = bindings.get(p)?;
                            out.push_str(&self.resolve_arg_string(arg)?);
                        }
                    }
                }
                Some(out)
            }
        }
    }

    fn scan_expr_for_prefabs(&mut self, expr: &ast::Expression) {
        match expr {
            ast::Expression::FunctionCall(call) => self.scan_call_for_prefabs(call),
            ast::Expression::Function(func) => self.walk_block(func.body().block()),
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.scan_expr_for_prefabs(lhs);
                self.scan_expr_for_prefabs(rhs);
            }
            ast::Expression::UnaryOperator { expression, .. } => {
                self.scan_expr_for_prefabs(expression);
            }
            ast::Expression::Parentheses { expression, .. } => {
                self.scan_expr_for_prefabs(expression);
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => self.scan_expr_for_prefabs(expression),
                        ast::Field::NameKey { value, .. } => self.scan_expr_for_prefabs(value),
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.scan_expr_for_prefabs(key);
                            self.scan_expr_for_prefabs(value);
                        }
                        _ => {}
                    }
                }
            }
            ast::Expression::Var(ast::Var::Expression(vex)) => {
                for suffix in vex.suffixes() {
                    if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                        self.scan_args_for_prefabs(args);
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_args_for_prefabs(&mut self, args: &ast::FunctionArgs) {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                for expr in arguments.iter() {
                    self.scan_expr_for_prefabs(expr);
                }
            }
            ast::FunctionArgs::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => self.scan_expr_for_prefabs(expression),
                        ast::Field::NameKey { value, .. } => self.scan_expr_for_prefabs(value),
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.scan_expr_for_prefabs(key);
                            self.scan_expr_for_prefabs(value);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn try_parse_prefab_call(
        &mut self,
        call: &ast::FunctionCall,
        fallback_file: Option<&str>,
    ) -> bool {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return false;
        };
        if prefix.token().to_string() != "Prefab" {
            return false;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return false;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 3 {
            return false;
        }

        let prefab_name = match &args[0] {
            ast::Expression::String(s) => Some(string_literal_text(&s.to_string())),
            _ => None,
        };

        let mut anims = Vec::new();
        let mut related_files = Vec::new();
        let mut unresolved = Vec::new();
        let mut asset_var = None;

        match &args[2] {
            ast::Expression::Var(ast::Var::Name(name)) => {
                let var = name.token().to_string();
                asset_var = Some(var.clone());
                if let Some(entries) = self.asset_tables.get(&var) {
                    for entry in entries {
                        self.push_asset_entry(
                            entry,
                            &mut anims,
                            &mut related_files,
                            &mut unresolved,
                        );
                    }
                } else {
                    unresolved.push(UnresolvedRef {
                        kind: "ASSET_TABLE".to_string(),
                        raw: var,
                        line: self.line(args[2]),
                    });
                }
            }
            ast::Expression::TableConstructor(table) => {
                for entry in self.parse_asset_table(table) {
                    self.push_asset_entry(&entry, &mut anims, &mut related_files, &mut unresolved);
                }
            }
            ast::Expression::FunctionCall(call) => {
                let (entries, unrs) = self.eval_asset_factory_call(call);
                for entry in entries {
                    self.push_asset_entry(&entry, &mut anims, &mut related_files, &mut unresolved);
                }
                unresolved.extend(unrs);
            }
            other => {
                unresolved.push(UnresolvedRef {
                    kind: "ASSET_TABLE".to_string(),
                    raw: other.to_string().trim().to_string(),
                    line: self.line(args[2]),
                });
            }
        }

        self.prefabs.push(PrefabAnimRecord {
            prefab_file: fallback_file.unwrap_or_default().to_string(),
            prefab_name,
            asset_var,
            anims,
            related_files,
            unresolved,
            content: AnimContent::default(),
        });
        true
    }

    fn push_asset_entry(
        &self,
        entry: &AssetEntry,
        anims: &mut Vec<AnimRef>,
        related_files: &mut Vec<String>,
        unresolved: &mut Vec<UnresolvedRef>,
    ) {
        match &entry.normalized {
            Some(normalized) if entry.kind == "ANIM" || entry.kind == "DYNAMIC_ANIM" => {
                anims.push(AnimRef {
                    kind: entry.kind.clone(),
                    path: entry.raw_path.clone(),
                    normalized: normalized.clone(),
                    exists: false, // filled later by run_index
                });
            }
            Some(normalized) if entry.kind == "PKGREF" => {
                related_files.push(normalized.clone());
            }
            Some(_) | None => unresolved.push(UnresolvedRef {
                kind: entry.kind.clone(),
                raw: entry.raw_path.clone(),
                line: entry.line,
            }),
        }
    }

    fn line(&self, node: &impl Node) -> u32 {
        node.start_position()
            .map(|p| line_of(self.source, p.bytes()))
            .unwrap_or(0)
    }
}

pub(crate) struct ScannedFile {
    pub(crate) prefabs: Vec<PrefabAnimRecord>,
    pub(crate) skipped_pkgref_dyn: usize,
}

pub(crate) fn line_of(source: &str, byte: usize) -> u32 {
    source.as_bytes()[..byte.min(source.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count() as u32
        + 1
}

/// Extract the text of a Lua string literal (`"x"`, `'x'`, `[[x]]`).
pub(crate) fn string_literal_text(raw: &str) -> String {
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

pub(crate) fn normalize_asset_path(kind: &str, raw: &str) -> String {
    let mut p = raw.trim().to_string();
    if let Some(stripped) = p.strip_prefix("anim/") {
        p = stripped.to_string();
    } else if let Some(stripped) = p.strip_prefix("/anim/") {
        p = stripped.to_string();
    }
    // DYNAMIC_ANIM uses `anim/dynamic/xxx.zip`; ANIM usually uses root zips,
    // but if it somehow contains dynamic/ we preserve that as a valid anim-relative path.
    let _ = kind;
    p
}

pub(crate) fn load_anim_content(anim_root: &Path, normalized: &str) -> Result<AnimContent> {
    let bytes = std::fs::read(anim_root.join(normalized))?;
    let data = super::super::archive::parse_archive_bytes(&bytes)?;
    let mut content = AnimContent::default();
    if let Some(anim) = &data.anim {
        for bank in &anim.banks {
            content.banks.push(bank.name.clone());
            for animation in &bank.animations {
                content.animations.push(animation.name.clone());
            }
        }
    }
    if let Some(build) = &data.build {
        content.builds.push(build.name.clone());
        content.atlases.extend(build.atlases.iter().cloned());
        content
            .symbols
            .extend(build.symbols.iter().map(|s| s.name.clone()));
    }
    sort_dedup_content(&mut content);
    Ok(content)
}

pub(crate) fn merge_content(target: &mut AnimContent, src: &AnimContent) {
    target.banks.extend(src.banks.iter().cloned());
    target.animations.extend(src.animations.iter().cloned());
    target.builds.extend(src.builds.iter().cloned());
    target.symbols.extend(src.symbols.iter().cloned());
    target.atlases.extend(src.atlases.iter().cloned());
}

pub(crate) fn sort_dedup_content(content: &mut AnimContent) {
    for v in [
        &mut content.banks,
        &mut content.animations,
        &mut content.builds,
        &mut content.symbols,
        &mut content.atlases,
    ] {
        v.sort();
        v.dedup();
    }
}

fn is_named_call(call: &ast::FunctionCall, name: &str) -> bool {
    matches!(&call.prefix(), ast::Prefix::Name(prefix) if prefix.token().to_string() == name)
}

fn classify_template_name(expr: &ast::Expression, params: &[String]) -> TemplateName {
    match expr {
        ast::Expression::String(s) => TemplateName::Lit(string_literal_text(&s.to_string())),
        ast::Expression::Var(ast::Var::Name(name)) => {
            let var = name.token().to_string();
            if params.iter().any(|p| p == &var) {
                TemplateName::Param(var)
            } else {
                TemplateName::Other
            }
        }
        other => {
            let _ = other;
            TemplateName::Other
        }
    }
}

fn classify_template_arg(expr: &ast::Expression, params: &[String]) -> TemplateArg {
    match expr {
        ast::Expression::String(s) => TemplateArg::Lit(string_literal_text(&s.to_string())),
        ast::Expression::Var(ast::Var::Name(name)) => {
            let var = name.token().to_string();
            if params.iter().any(|p| p == &var) {
                TemplateArg::Param(var)
            } else {
                TemplateArg::Other(var)
            }
        }
        other => TemplateArg::Other(other.to_string().trim().to_string()),
    }
}

fn parse_path_template(expr: &ast::Expression, params: &[String]) -> Option<PathTemplate> {
    match expr {
        ast::Expression::String(s) => {
            Some(PathTemplate::Literal(string_literal_text(&s.to_string())))
        }
        ast::Expression::Parentheses { expression, .. } => parse_path_template(expression, params),
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            if binop.to_string().trim() != ".." {
                return None;
            }
            let mut parts = Vec::new();
            if !flatten_path_parts(lhs, params, &mut parts) {
                return None;
            }
            if !flatten_path_parts(rhs, params, &mut parts) {
                return None;
            }
            Some(PathTemplate::Concat(parts))
        }
        _ => None,
    }
}

fn flatten_path_parts(expr: &ast::Expression, params: &[String], out: &mut Vec<PathPart>) -> bool {
    match expr {
        ast::Expression::String(s) => {
            out.push(PathPart::Lit(string_literal_text(&s.to_string())));
            true
        }
        ast::Expression::Var(ast::Var::Name(name)) => {
            let var = name.token().to_string();
            if params.iter().any(|p| p == &var) {
                out.push(PathPart::Param(var));
                true
            } else {
                false
            }
        }
        ast::Expression::Parentheses { expression, .. } => {
            flatten_path_parts(expression, params, out)
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            if binop.to_string().trim() != ".." {
                return false;
            }
            flatten_path_parts(lhs, params, out) && flatten_path_parts(rhs, params, out)
        }
        _ => false,
    }
}
