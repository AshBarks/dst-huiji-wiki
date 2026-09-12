//! Factory/table/prefab 分析（由 parser.rs 机械拆分，行为不变）。

use std::collections::HashMap;

use full_moon::ast;
use full_moon::node::Node;

use super::super::types::{OverrideValue, PrefabNameOverride, SourceLocation};
use super::extract_string_literal;
use super::PrefabOverrideParser;

impl PrefabOverrideParser {
    // ============================================================================
    // Section 3: Factory Call Analysis (ipairs patterns)
    // ============================================================================
    pub(super) fn analyze_ipairs_factory_call(
        &self,
        call: &ast::FunctionCall,
        iter_tables: &HashMap<String, ast::Expression>,
    ) -> Option<Vec<PrefabNameOverride>> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let fn_name = name.token().to_string();
            if let Some(func_info) = self.functions.get(&fn_name) {
                let mut results = Vec::new();
                let local_functions = HashMap::new();

                let call_args = self.get_call_args(call);
                let call_args_vec: Vec<_> = call_args.map(|a| a.to_vec()).unwrap_or_default();

                let params: Vec<_> = func_info.body.parameters().iter().collect();
                let mut param_tables: HashMap<String, ast::Expression> = HashMap::new();

                for (i, param) in params.iter().enumerate() {
                    if let ast::Parameter::Name(param_name) = param {
                        let param_name_str = param_name.token().to_string();
                        if i < call_args_vec.len() {
                            if let ast::Expression::Var(ast::Var::Name(var_name)) =
                                &call_args_vec[i]
                            {
                                let var_name_str = var_name.token().to_string();
                                if let Some(table_expr) = iter_tables.get(&var_name_str) {
                                    param_tables.insert(param_name_str.clone(), table_expr.clone());
                                }
                            }
                        }
                    }
                }

                let param_values: HashMap<String, String> = HashMap::new();

                self.find_table_insert_prefabs_in_function_body_with_params(
                    &func_info.body,
                    &mut results,
                    &local_functions,
                    &param_values,
                    &param_tables,
                );

                if !results.is_empty() {
                    return Some(results);
                }
            }
        }
        None
    }

    pub(super) fn find_factory_calls_in_table_literals(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();

        for stmt in self.ast.nodes().stmts() {
            if let ast::Stmt::LocalAssignment(assignment) = stmt {
                for expr in assignment.expressions().iter() {
                    self.find_factory_calls_in_table_constructor(expr, &mut calls);
                }
            }
        }

        calls
    }

    pub(super) fn find_factory_calls_in_table_constructor(
        &self,
        expr: &ast::Expression,
        calls: &mut Vec<ast::FunctionCall>,
    ) {
        if let ast::Expression::TableConstructor(table) = expr {
            for field in table.fields() {
                match field {
                    ast::Field::NoKey(expression) => {
                        if let ast::Expression::FunctionCall(call) = expression {
                            let prefix = call.prefix();
                            if let ast::Prefix::Name(name) = prefix {
                                let fn_name = name.token().to_string();
                                if self.functions.contains_key(&fn_name) {
                                    calls.push(call.clone());
                                }
                            }
                        } else {
                            self.find_factory_calls_in_table_constructor(expression, calls);
                        }
                    }
                    ast::Field::NameKey { value, .. } => {
                        if let ast::Expression::FunctionCall(call) = value {
                            let prefix = call.prefix();
                            if let ast::Prefix::Name(name) = prefix {
                                let fn_name = name.token().to_string();
                                if self.functions.contains_key(&fn_name) {
                                    calls.push(call.clone());
                                }
                            }
                        } else {
                            self.find_factory_calls_in_table_constructor(value, calls);
                        }
                    }
                    ast::Field::ExpressionKey { value, .. } => {
                        if let ast::Expression::FunctionCall(call) = value {
                            let prefix = call.prefix();
                            if let ast::Prefix::Name(name) = prefix {
                                let fn_name = name.token().to_string();
                                if self.functions.contains_key(&fn_name) {
                                    calls.push(call.clone());
                                }
                            }
                        } else {
                            self.find_factory_calls_in_table_constructor(value, calls);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub(super) fn find_table_insert_factory_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();

        for stmt in self.ast.nodes().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_table_insert_with_factory_call(call) {
                    if let Some(factory_call) = self.extract_factory_from_table_insert(call) {
                        calls.push(factory_call);
                    }
                }
            }
        }

        calls
    }

    // ============================================================================
    // Section 4: Table Insert Pattern Analysis
    // ============================================================================
    pub(super) fn find_table_insert_prefabs(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();

        for stmt in self.ast.nodes().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_table_insert_with_prefab(call) {
                    if let Some(prefab_call) = self.extract_prefab_from_table_insert(call) {
                        calls.push(prefab_call);
                    }
                }
            }
        }

        calls
    }

    pub(super) fn is_table_insert_with_prefab(&self, call: &ast::FunctionCall) -> bool {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let prefix_name = name.token().to_string();
            if prefix_name == "table" {
                let suffixes: Vec<_> = call.suffixes().collect();
                if suffixes.len() >= 2 {
                    let has_insert_index = suffixes.iter().any(|s| {
                        if let ast::Suffix::Index(ast::Index::Dot { name, .. }) = s {
                            name.token().to_string() == "insert"
                        } else {
                            false
                        }
                    });

                    if has_insert_index {
                        for suffix in &suffixes {
                            if let ast::Suffix::Call(ast::Call::AnonymousCall(
                                ast::FunctionArgs::Parentheses { arguments, .. },
                            )) = suffix
                            {
                                let args_vec: Vec<_> = arguments.iter().collect();
                                if args_vec.len() >= 2 {
                                    if let ast::Expression::FunctionCall(inner_call) = &args_vec[1]
                                    {
                                        if self.is_prefab_call(inner_call) {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub(super) fn is_table_insert_with_factory_call(&self, call: &ast::FunctionCall) -> bool {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let prefix_name = name.token().to_string();
            if prefix_name == "table" {
                let suffixes: Vec<_> = call.suffixes().collect();
                if suffixes.len() >= 2 {
                    let has_insert_index = suffixes.iter().any(|s| {
                        if let ast::Suffix::Index(ast::Index::Dot { name, .. }) = s {
                            name.token().to_string() == "insert"
                        } else {
                            false
                        }
                    });

                    if has_insert_index {
                        for suffix in &suffixes {
                            if let ast::Suffix::Call(ast::Call::AnonymousCall(
                                ast::FunctionArgs::Parentheses { arguments, .. },
                            )) = suffix
                            {
                                let args_vec: Vec<_> = arguments.iter().collect();
                                if args_vec.len() >= 2 {
                                    if let ast::Expression::FunctionCall(inner_call) = &args_vec[1]
                                    {
                                        let inner_prefix = inner_call.prefix();
                                        if let ast::Prefix::Name(inner_name) = inner_prefix {
                                            let inner_fn_name = inner_name.token().to_string();
                                            if self.functions.contains_key(&inner_fn_name) {
                                                return true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub(super) fn extract_factory_from_table_insert(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<ast::FunctionCall> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let prefix_name = name.token().to_string();
            if prefix_name == "table" {
                let suffixes: Vec<_> = call.suffixes().collect();
                let has_insert_index = suffixes.iter().any(|s| {
                    if let ast::Suffix::Index(ast::Index::Dot { name, .. }) = s {
                        name.token().to_string() == "insert"
                    } else {
                        false
                    }
                });

                if has_insert_index {
                    for suffix in &suffixes {
                        if let ast::Suffix::Call(ast::Call::AnonymousCall(
                            ast::FunctionArgs::Parentheses { arguments, .. },
                        )) = suffix
                        {
                            let args_vec: Vec<_> = arguments.iter().collect();
                            if args_vec.len() >= 2 {
                                if let ast::Expression::FunctionCall(inner_call) = &args_vec[1] {
                                    return Some(inner_call.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_prefab_from_table_insert(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<ast::FunctionCall> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let prefix_name = name.token().to_string();
            if prefix_name == "table" {
                let suffixes: Vec<_> = call.suffixes().collect();
                let has_insert_index = suffixes.iter().any(|s| {
                    if let ast::Suffix::Index(ast::Index::Dot { name, .. }) = s {
                        name.token().to_string() == "insert"
                    } else {
                        false
                    }
                });

                if has_insert_index {
                    for suffix in &suffixes {
                        if let ast::Suffix::Call(ast::Call::AnonymousCall(
                            ast::FunctionArgs::Parentheses { arguments, .. },
                        )) = suffix
                        {
                            let args_vec: Vec<_> = arguments.iter().collect();
                            if args_vec.len() >= 2 {
                                if let ast::Expression::FunctionCall(inner_call) = &args_vec[1] {
                                    return Some(inner_call.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn find_return_factory_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();
        self.find_factory_calls_in_block(self.ast.nodes(), &mut calls);
        calls
    }

    // ============================================================================
    // Section 5: Return Factory Call Analysis
    // ============================================================================
    pub(super) fn analyze_return_factory_call(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<Vec<PrefabNameOverride>> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let fn_name = name.token().to_string();
            if let Some(func_info) = self.functions.get(&fn_name) {
                // 普通构造函数（只设 override、不注册 prefab）不是工厂：
                // 其首参是动画/形态名，不能当作 prefab 名。
                if !self.body_registers_prefab(&func_info.body) {
                    return None;
                }
                let mut results = Vec::new();
                let local_functions = HashMap::new();

                let call_args = self.get_call_args(call);
                let mut param_values: HashMap<String, String> = HashMap::new();
                let mut param_tables: HashMap<String, ast::Expression> = HashMap::new();

                let mut prefab_name_from_call: Option<String> = None;

                if let Some(args) = call_args {
                    let args_vec: Vec<_> = args.iter().collect();
                    let params: Vec<_> = func_info.body.parameters().iter().collect();

                    if !args_vec.is_empty() {
                        prefab_name_from_call =
                            self.extract_string_from_arg_with_params(args_vec[0], &param_values);
                    }

                    for (i, param) in params.iter().enumerate() {
                        if i < args_vec.len() {
                            let param_name = match param {
                                ast::Parameter::Name(name) => name.token().to_string(),
                                _ => continue,
                            };
                            if let Some(value) =
                                self.extract_string_from_arg_with_params(args_vec[i], &param_values)
                            {
                                param_values.insert(param_name.clone(), value);
                            }
                            if let ast::Expression::TableConstructor(_) = args_vec[i] {
                                param_tables.insert(param_name, args_vec[i].clone());
                            }
                        }
                    }
                }

                self.find_prefabs_in_function_body_with_fn_tracking_and_params(
                    &func_info.body,
                    &mut results,
                    &local_functions,
                    &param_values,
                );
                self.find_table_insert_prefabs_in_function_body_with_params(
                    &func_info.body,
                    &mut results,
                    &local_functions,
                    &param_values,
                    &param_tables,
                );
                self.find_set_prefab_name_override_in_function_body(
                    &func_info.body,
                    prefab_name_from_call.as_deref(),
                    &local_functions,
                    &param_values,
                    &mut results,
                );
                if !results.is_empty() {
                    return Some(results);
                }
            } else if self.is_external_factory_function(&fn_name) {
                return self.analyze_external_factory_call(call);
            }
        }
        None
    }

    // ============================================================================
    // Section 6: External Factory Call Analysis
    // ============================================================================
    pub(super) fn is_external_factory_function(&self, name: &str) -> bool {
        matches!(
            name,
            "MakeBundle"
                | "MakeWrap"
                | "MakeContainer"
                | "AddWinterTree"
                | "MakeGlobalTrackingIcons"
        )
    }

    pub(super) fn analyze_external_factory_call(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<Vec<PrefabNameOverride>> {
        let args = self.get_call_args(call)?;
        let args_vec: Vec<_> = args.iter().collect();

        if args_vec.is_empty() {
            return None;
        }

        let prefab_name = self.extract_string_from_arg(args_vec[0]);

        let mut results = Vec::new();
        let local_functions = HashMap::new();
        let param_values = HashMap::new();

        for arg in &args_vec {
            if let ast::Expression::TableConstructor(table) = arg {
                for field in table.fields() {
                    match field {
                        ast::Field::NameKey { key, value, .. } => {
                            let key_name = key.token().to_string();
                            if key_name == "global_common_postinit"
                                || key_name == "postinit"
                                || key_name == "fn"
                            {
                                if let ast::Expression::Function(func) = value {
                                    self.find_set_prefab_name_override_in_function_body(
                                        func.body(),
                                        prefab_name.as_deref(),
                                        &local_functions,
                                        &param_values,
                                        &mut results,
                                    );
                                }
                            }
                        }
                        ast::Field::NoKey(ast::Expression::Function(func)) => {
                            self.find_set_prefab_name_override_in_function_body(
                                func.body(),
                                prefab_name.as_deref(),
                                &local_functions,
                                &param_values,
                                &mut results,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        if !results.is_empty() {
            Some(results)
        } else {
            None
        }
    }

    pub(super) fn visit_control_flow_blocks<F>(stmt: &ast::Stmt, mut f: F)
    where
        F: FnMut(&ast::Block),
    {
        match stmt {
            ast::Stmt::If(if_stmt) => {
                f(if_stmt.block());
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        f(else_if.block());
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    f(else_block);
                }
            }
            ast::Stmt::While(while_stmt) => {
                f(while_stmt.block());
            }
            ast::Stmt::Repeat(repeat_stmt) => {
                f(repeat_stmt.block());
            }
            ast::Stmt::GenericFor(generic_for_stmt) => {
                f(generic_for_stmt.block());
            }
            ast::Stmt::NumericFor(numeric_for_stmt) => {
                f(numeric_for_stmt.block());
            }
            _ => {}
        }
    }

    // ============================================================================
    // Section 7: SetPrefabNameOverride Analysis
    // ============================================================================
    pub(super) fn find_set_prefab_name_override_in_function_body(
        &self,
        body: &ast::FunctionBody,
        default_prefab_name: Option<&str>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
        results: &mut Vec<PrefabNameOverride>,
    ) {
        for stmt in body.block().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_set_prefab_name_override_call(call) {
                    if let Some(override_info) =
                        self.extract_override_from_call_with_default(call, default_prefab_name)
                    {
                        results.push(override_info);
                    }
                }
            } else {
                Self::visit_control_flow_blocks(stmt, |block| {
                    self.find_set_prefab_name_override_in_block_with_default(
                        block,
                        default_prefab_name,
                        local_functions,
                        param_values,
                        results,
                    );
                });
            }
        }
    }

    pub(super) fn find_set_prefab_name_override_in_block_with_default(
        &self,
        block: &ast::Block,
        default_prefab_name: Option<&str>,
        _local_functions: &HashMap<String, ast::FunctionBody>,
        _param_values: &HashMap<String, String>,
        results: &mut Vec<PrefabNameOverride>,
    ) {
        for stmt in block.stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_set_prefab_name_override_call(call) {
                    if let Some(override_info) =
                        self.extract_override_from_call_with_default(call, default_prefab_name)
                    {
                        results.push(override_info);
                    }
                }
            }
        }
    }

    pub(super) fn extract_override_from_call_with_default(
        &self,
        call: &ast::FunctionCall,
        default_prefab_name: Option<&str>,
    ) -> Option<PrefabNameOverride> {
        let args = self.get_call_args(call)?;
        let args_vec: Vec<_> = args.iter().collect();

        if args_vec.is_empty() {
            return None;
        }

        let override_value = self.extract_override_value_from_expr(args_vec[0])?;

        let prefab_name = default_prefab_name.map(String::from);

        let location = self.get_call_location(call);

        Some(PrefabNameOverride {
            prefab_name: prefab_name.unwrap_or_else(|| "unknown".to_string()),
            override_name: override_value,
            location,
        })
    }

    pub(super) fn extract_override_value_from_expr(
        &self,
        arg: &ast::Expression,
    ) -> Option<OverrideValue> {
        if let Some(s) = self.extract_string_from_arg(arg) {
            return Some(OverrideValue::Static(s));
        }
        Some(OverrideValue::Dynamic(arg.to_string()))
    }

    // ============================================================================
    // Section 9: Table Expression Name Extraction
    // ============================================================================
    pub(super) fn extract_name_from_table_expr(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::TableConstructor(table) => {
                self.extract_name_from_table_constructor(table)
            }
            _ => None,
        }
    }

    pub(super) fn extract_name_from_table_constructor(
        &self,
        table: &ast::TableConstructor,
    ) -> Option<String> {
        for field in table.fields() {
            if let ast::Field::NameKey { key, value, .. } = field {
                if key.token().to_string() == "name" {
                    return self.extract_string_from_arg(value);
                }
            }
        }
        None
    }

    pub(super) fn extract_string_from_arg(&self, arg: &ast::Expression) -> Option<String> {
        match arg {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Number(n) => Some(n.token().to_string().trim().to_string()),
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    self.variables.get(&var_name).and_then(|v| v.value.clone())
                } else {
                    None
                }
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op_str = binop.to_string().trim().to_string();
                if op_str == ".." {
                    let left = self.extract_string_from_arg(lhs)?;
                    let right = self.extract_string_from_arg(rhs)?;
                    Some(format!("{}{}", left, right))
                } else {
                    None
                }
            }
            ast::Expression::FunctionCall(call) => {
                let prefix = call.prefix();
                if let ast::Prefix::Name(name) = prefix {
                    let fn_name = name.token().to_string();
                    if fn_name == "tostring" {
                        if let Some(args) = self.get_call_args(call) {
                            let args_vec: Vec<_> = args.iter().collect();
                            if !args_vec.is_empty() {
                                return self.extract_string_from_arg(args_vec[0]);
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    // ============================================================================
    // Section 14: SetPrefabNameOverride Call Analysis
    // ============================================================================
    pub(super) fn is_set_prefab_name_override_call(&self, call: &ast::FunctionCall) -> bool {
        let suffixes: Vec<_> = call.suffixes().collect();
        if suffixes.len() != 1 {
            return false;
        }

        match &suffixes[0] {
            ast::Suffix::Call(ast::Call::MethodCall(method_call)) => {
                let method_name = method_call.name().token().to_string();
                method_name == "SetPrefabNameOverride"
            }
            _ => false,
        }
    }

    pub(super) fn extract_override_from_call(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<OverrideValue> {
        let suffixes: Vec<_> = call.suffixes().collect();
        match &suffixes[0] {
            ast::Suffix::Call(ast::Call::MethodCall(method_call)) => {
                let args = method_call.args();
                match args {
                    ast::FunctionArgs::Parentheses { arguments, .. } => {
                        if let Some(first_arg) = arguments.iter().next() {
                            return self.extract_override_value(first_arg);
                        }
                    }
                    ast::FunctionArgs::String(s) => {
                        let value = extract_string_literal(&s.to_string());
                        return Some(OverrideValue::Static(value));
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    pub(super) fn extract_override_value(&self, expr: &ast::Expression) -> Option<OverrideValue> {
        match expr {
            ast::Expression::String(s) => {
                let value = extract_string_literal(&s.to_string());
                Some(OverrideValue::Static(value))
            }
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    if let Some(var_val) = self.variables.get(&var_name) {
                        if let Some(value) = &var_val.value {
                            return Some(OverrideValue::Static(value.clone()));
                        }
                    }
                }
                let expr_str = expr.to_string();
                Some(OverrideValue::Dynamic(expr_str))
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op_str = binop.to_string().trim().to_string();
                if op_str == ".." {
                    if let (Some(left), Some(right)) = (
                        self.extract_override_value(lhs),
                        self.extract_override_value(rhs),
                    ) {
                        if let (OverrideValue::Static(l), OverrideValue::Static(r)) = (left, right)
                        {
                            return Some(OverrideValue::Static(format!("{}{}", l, r)));
                        }
                    }
                }
                let expr_str = expr.to_string();
                Some(OverrideValue::Dynamic(expr_str))
            }
            _ => {
                let expr_str = expr.to_string();
                Some(OverrideValue::Dynamic(expr_str))
            }
        }
    }

    // ============================================================================
    // Section 15: Config Table Analysis
    // ============================================================================
    pub(super) fn analyze_factory_call(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<PrefabNameOverride> {
        let args = self.get_call_args(call)?;
        let args_vec: Vec<_> = args.iter().collect();

        if args_vec.is_empty() {
            return None;
        }

        let prefab_name = self.extract_string_from_arg(args_vec[0])?;

        let config_table = self.find_config_table_in_args(&args);
        if let Some(config_name) = config_table {
            if let Some(override_val) = self.find_override_in_config_table(&config_name) {
                let location = self.get_call_location(call);
                return Some(PrefabNameOverride {
                    prefab_name,
                    override_name: override_val,
                    location,
                });
            }
        }

        None
    }

    pub(super) fn find_config_table_in_args(&self, args: &[ast::Expression]) -> Option<String> {
        for arg in args {
            if let ast::Expression::Var(ast::Var::Name(name)) = arg {
                return Some(name.token().to_string());
            }
        }
        None
    }

    pub(super) fn find_override_in_config_table(&self, table_name: &str) -> Option<OverrideValue> {
        for stmt in self.ast.nodes().stmts() {
            if let ast::Stmt::LocalAssignment(assignment) = stmt {
                let name_list = assignment.names();
                let expr_list = assignment.expressions();
                for (name, expr) in name_list.iter().zip(expr_list.iter()) {
                    if name.token().to_string() == table_name {
                        return self.find_override_in_table_expr(expr);
                    }
                }
            }
        }
        None
    }

    pub(super) fn find_override_in_table_expr(
        &self,
        expr: &ast::Expression,
    ) -> Option<OverrideValue> {
        if let ast::Expression::TableConstructor(table) = expr {
            for field in table.fields() {
                if let ast::Field::NameKey { key, value, .. } = field {
                    let key_str = key.token().to_string();
                    if key_str == "common_postinit" || key_str == "master_postinit" {
                        if let Some(override_val) = self.find_override_in_fn_arg(value) {
                            return Some(override_val);
                        }
                    }
                }
            }
        }
        None
    }

    // ============================================================================
    // Section 16: Location and Utility Functions
    // ============================================================================
    pub(super) fn get_call_location(&self, call: &ast::FunctionCall) -> SourceLocation {
        let start_byte = call.start_position().map(|p| p.bytes()).unwrap_or(0);
        let end_byte = call.end_position().map(|p| p.bytes()).unwrap_or(0);

        let start_line = self.byte_to_line(start_byte);
        let end_line = self.byte_to_line(end_byte);

        SourceLocation {
            start_byte,
            end_byte,
            start_line,
            end_line,
        }
    }

    pub(super) fn byte_to_line(&self, byte: usize) -> usize {
        self.source[..byte.min(self.source.len())]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1
    }
}
