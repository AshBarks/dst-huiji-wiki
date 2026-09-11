use crate::Result;
use full_moon::ast::{self, Ast};
use full_moon::node::Node;
use std::collections::HashMap;

use super::types::{
    FunctionInfo, OverrideValue, PrefabNameOverride, SourceLocation, VariableValue,
};

pub struct PrefabOverrideParser {
    source: String,
    ast: Ast,
    functions: HashMap<String, FunctionInfo>,
    variables: HashMap<String, VariableValue>,
}

impl PrefabOverrideParser {
    pub fn new(source: &str) -> Result<Self> {
        let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;

        let mut parser = Self {
            source: source.to_string(),
            ast,
            functions: HashMap::new(),
            variables: HashMap::new(),
        };

        parser.collect_definitions();
        Ok(parser)
    }

    // ============================================================================
    // Section 1: Definition Collection
    // ============================================================================
    fn collect_definitions(&mut self) {
        let mut blocks_to_visit: Vec<&ast::Block> = vec![self.ast.nodes()];

        while let Some(block) = blocks_to_visit.pop() {
            for stmt in block.stmts() {
                match stmt {
                    ast::Stmt::LocalAssignment(assignment) => {
                        let name_list = assignment.names();
                        let expr_list = assignment.expressions();
                        for (name, expr) in name_list.iter().zip(expr_list.iter()) {
                            let var_name = name.token().to_string();
                            if let Some(value) = self.extract_string_value(expr) {
                                self.variables
                                    .insert(var_name, VariableValue { value: Some(value) });
                            }
                            if let ast::Expression::Function(func) = expr {
                                blocks_to_visit.push(func.body().block());
                            }
                        }
                    }
                    ast::Stmt::LocalFunction(local_fn) => {
                        let name = local_fn.name().to_string();
                        self.functions.insert(
                            name,
                            FunctionInfo {
                                body: local_fn.body().clone(),
                            },
                        );
                        blocks_to_visit.push(local_fn.body().block());
                    }
                    ast::Stmt::FunctionDeclaration(func_decl) => {
                        let name = func_decl.name().to_string();
                        self.functions.insert(
                            name,
                            FunctionInfo {
                                body: func_decl.body().clone(),
                            },
                        );
                        blocks_to_visit.push(func_decl.body().block());
                    }
                    ast::Stmt::GenericFor(for_stmt) => {
                        blocks_to_visit.push(for_stmt.block());
                    }
                    ast::Stmt::NumericFor(for_stmt) => {
                        blocks_to_visit.push(for_stmt.block());
                    }
                    ast::Stmt::While(while_stmt) => {
                        blocks_to_visit.push(while_stmt.block());
                    }
                    ast::Stmt::Repeat(repeat_stmt) => {
                        blocks_to_visit.push(repeat_stmt.block());
                    }
                    ast::Stmt::If(if_stmt) => {
                        blocks_to_visit.push(if_stmt.block());
                        if let Some(else_ifs) = if_stmt.else_if() {
                            for else_if_block in else_ifs {
                                blocks_to_visit.push(else_if_block.block());
                            }
                        }
                        if let Some(else_block) = if_stmt.else_block() {
                            blocks_to_visit.push(else_block);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ============================================================================
    // Section 2: String Value Extraction
    // ============================================================================
    fn extract_string_value(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
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
                    let left = self.extract_string_value(lhs)?;
                    let right = self.extract_string_value(rhs)?;
                    Some(format!("{}{}", left, right))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn parse(&self) -> Result<Vec<PrefabNameOverride>> {
        let mut results = Vec::new();

        for prefab_call in self.find_prefab_calls() {
            if let Some(override_info) = self.analyze_prefab_call(&prefab_call) {
                results.push(override_info);
            }
        }

        for factory_call in self.find_factory_calls() {
            if let Some(override_info) = self.analyze_factory_call(&factory_call) {
                results.push(override_info);
            }
        }

        for table_insert_prefab in self.find_table_insert_prefabs() {
            if let Some(override_info) = self.analyze_prefab_call(&table_insert_prefab) {
                results.push(override_info);
            }
        }

        for return_factory in self.find_return_factory_calls() {
            if let Some(overrides) = self.analyze_return_factory_call(&return_factory) {
                results.extend(overrides);
            }
        }

        for table_insert_factory in self.find_table_insert_factory_calls() {
            if let Some(overrides) = self.analyze_return_factory_call(&table_insert_factory) {
                results.extend(overrides);
            }
        }

        for table_literal_factory in self.find_factory_calls_in_table_literals() {
            if let Some(overrides) = self.analyze_return_factory_call(&table_literal_factory) {
                results.extend(overrides);
            }
        }

        for (call, iter_tables) in self.find_ipairs_factory_calls() {
            if let Some(overrides) = self.analyze_ipairs_factory_call(&call, &iter_tables) {
                results.extend(overrides);
            }
        }

        Ok(results)
    }

    // ============================================================================
    // Section 3: Factory Call Analysis (ipairs patterns)
    // ============================================================================
    fn analyze_ipairs_factory_call(
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

    fn find_factory_calls_in_table_literals(&self) -> Vec<ast::FunctionCall> {
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

    fn find_factory_calls_in_table_constructor(
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

    fn find_table_insert_factory_calls(&self) -> Vec<ast::FunctionCall> {
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
    fn find_table_insert_prefabs(&self) -> Vec<ast::FunctionCall> {
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

    fn is_table_insert_with_prefab(&self, call: &ast::FunctionCall) -> bool {
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

    fn is_table_insert_with_factory_call(&self, call: &ast::FunctionCall) -> bool {
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

    fn extract_factory_from_table_insert(
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

    fn extract_prefab_from_table_insert(
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

    fn find_return_factory_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();
        self.find_factory_calls_in_block(self.ast.nodes(), &mut calls);
        calls
    }

    // ============================================================================
    // Section 5: Return Factory Call Analysis
    // ============================================================================
    fn analyze_return_factory_call(
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
    fn is_external_factory_function(&self, name: &str) -> bool {
        matches!(
            name,
            "MakeBundle"
                | "MakeWrap"
                | "MakeContainer"
                | "AddWinterTree"
                | "MakeGlobalTrackingIcons"
        )
    }

    fn analyze_external_factory_call(
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

    fn visit_control_flow_blocks<F>(stmt: &ast::Stmt, mut f: F)
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
    fn find_set_prefab_name_override_in_function_body(
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

    fn find_set_prefab_name_override_in_block_with_default(
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

    fn extract_override_from_call_with_default(
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

    fn extract_override_value_from_expr(&self, arg: &ast::Expression) -> Option<OverrideValue> {
        if let Some(s) = self.extract_string_from_arg(arg) {
            return Some(OverrideValue::Static(s));
        }
        Some(OverrideValue::Dynamic(arg.to_string()))
    }

    // ============================================================================
    // Section 8: Function Body Prefab Analysis with Tracking
    // ============================================================================
    fn find_prefabs_in_function_body_with_fn_tracking_and_params(
        &self,
        body: &ast::FunctionBody,
        results: &mut Vec<PrefabNameOverride>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) {
        let mut current_local_functions = local_functions.clone();
        let mut local_variables: HashMap<String, String> = HashMap::new();

        for stmt in body.block().stmts() {
            if let ast::Stmt::LocalAssignment(assignment) = stmt {
                let names: Vec<_> = assignment.names().iter().collect();
                let exprs: Vec<_> = assignment.expressions().iter().collect();
                for (name, expr) in names.iter().zip(exprs.iter()) {
                    let var_name = name.token().to_string();
                    if let Some(value) =
                        self.extract_string_from_arg_with_params(expr, param_values)
                    {
                        local_variables.insert(var_name.clone(), value);
                    }
                    if let ast::Expression::Function(nested_func) = expr {
                        current_local_functions.insert(var_name, nested_func.body().clone());
                        self.find_prefabs_in_function_body_with_fn_tracking_and_params(
                            nested_func.body(),
                            results,
                            &current_local_functions,
                            param_values,
                        );
                    }
                }
            } else if let ast::Stmt::LocalFunction(local_fn) = stmt {
                let fn_name = local_fn.name().to_string();
                current_local_functions.insert(fn_name.clone(), local_fn.body().clone());
                self.find_prefabs_in_function_body_with_fn_tracking_and_params(
                    local_fn.body(),
                    results,
                    &current_local_functions,
                    param_values,
                );
            }
        }

        let combined_values: HashMap<String, String> = param_values
            .iter()
            .chain(local_variables.iter())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for stmt in body.block().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_prefab_call(call) {
                    if let Some(override_info) = self.analyze_prefab_call_with_local_fns_and_params(
                        call,
                        &current_local_functions,
                        &combined_values,
                    ) {
                        results.push(override_info);
                    }
                } else if self.is_table_insert_with_prefab(call) {
                    if let Some(prefab_call) = self.extract_prefab_from_table_insert(call) {
                        if let Some(override_info) = self
                            .analyze_prefab_call_with_local_fns_and_params(
                                &prefab_call,
                                &current_local_functions,
                                &combined_values,
                            )
                        {
                            results.push(override_info);
                        }
                    }
                }
            }
        }

        if let Some(ast::LastStmt::Return(ret)) = body.block().last_stmt() {
            for expr in ret.returns().iter() {
                if let ast::Expression::FunctionCall(call) = expr {
                    if self.is_prefab_call(call) {
                        if let Some(override_info) = self
                            .analyze_prefab_call_with_local_fns_and_params(
                                call,
                                &current_local_functions,
                                &combined_values,
                            )
                        {
                            results.push(override_info);
                        }
                    }
                }
            }
        }
    }

    fn find_table_insert_prefabs_in_function_body_with_params(
        &self,
        body: &ast::FunctionBody,
        results: &mut Vec<PrefabNameOverride>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
        param_tables: &HashMap<String, ast::Expression>,
    ) {
        let mut current_local_functions = local_functions.clone();

        for stmt in body.block().stmts() {
            if let ast::Stmt::LocalFunction(local_fn) = stmt {
                let fn_name = local_fn.name().to_string();
                current_local_functions.insert(fn_name.clone(), local_fn.body().clone());
            } else if let ast::Stmt::LocalAssignment(assignment) = stmt {
                let names: Vec<_> = assignment.names().iter().collect();
                let exprs: Vec<_> = assignment.expressions().iter().collect();
                for (name, expr) in names.iter().zip(exprs.iter()) {
                    if let ast::Expression::Function(nested_func) = expr {
                        let fn_name = name.token().to_string();
                        current_local_functions.insert(fn_name, nested_func.body().clone());
                    }
                }
            }
        }

        for stmt in body.block().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_table_insert_with_prefab(call) {
                    if let Some(prefab_call) = self.extract_prefab_from_table_insert(call) {
                        if let Some(override_info) = self.analyze_prefab_call_with_tables(
                            &prefab_call,
                            &current_local_functions,
                            param_values,
                            param_tables,
                        ) {
                            results.push(override_info);
                        }
                    }
                }
            } else if let ast::Stmt::LocalAssignment(assignment) = stmt {
                let names: Vec<_> = assignment.names().iter().collect();
                let exprs: Vec<_> = assignment.expressions().iter().collect();
                for (_name, expr) in names.iter().zip(exprs.iter()) {
                    if let ast::Expression::Function(nested_func) = expr {
                        self.find_table_insert_prefabs_in_function_body_with_params(
                            nested_func.body(),
                            results,
                            &current_local_functions,
                            param_values,
                            param_tables,
                        );
                    }
                }
            } else if let ast::Stmt::LocalFunction(local_fn) = stmt {
                self.find_table_insert_prefabs_in_function_body_with_params(
                    local_fn.body(),
                    results,
                    &current_local_functions,
                    param_values,
                    param_tables,
                );
            } else {
                Self::visit_control_flow_blocks(stmt, |block| {
                    self.find_table_insert_prefabs_in_block_with_params(
                        block,
                        results,
                        &current_local_functions,
                        param_values,
                        param_tables,
                    );
                });
            }
        }

        if let Some(ast::LastStmt::Return(ret)) = body.block().last_stmt() {
            for expr in ret.returns().iter() {
                if let ast::Expression::FunctionCall(call) = expr {
                    if self.is_prefab_call(call) {
                        if let Some(override_info) = self.analyze_prefab_call_with_tables(
                            call,
                            &current_local_functions,
                            param_values,
                            param_tables,
                        ) {
                            results.push(override_info);
                        }
                    }
                }
            }
        }
    }

    fn find_table_insert_prefabs_in_block_with_params(
        &self,
        block: &ast::Block,
        results: &mut Vec<PrefabNameOverride>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
        param_tables: &HashMap<String, ast::Expression>,
    ) {
        for stmt in block.stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_table_insert_with_prefab(call) {
                    if let Some(prefab_call) = self.extract_prefab_from_table_insert(call) {
                        if let Some(override_info) = self.analyze_prefab_call_with_tables(
                            &prefab_call,
                            local_functions,
                            param_values,
                            param_tables,
                        ) {
                            results.push(override_info);
                        }
                    }
                }
            } else if let ast::Stmt::LocalAssignment(assignment) = stmt {
                let names: Vec<_> = assignment.names().iter().collect();
                let exprs: Vec<_> = assignment.expressions().iter().collect();
                for (_name, expr) in names.iter().zip(exprs.iter()) {
                    if let ast::Expression::Function(nested_func) = expr {
                        self.find_table_insert_prefabs_in_function_body_with_params(
                            nested_func.body(),
                            results,
                            local_functions,
                            param_values,
                            param_tables,
                        );
                    }
                }
            } else if let ast::Stmt::LocalFunction(local_fn) = stmt {
                self.find_table_insert_prefabs_in_function_body_with_params(
                    local_fn.body(),
                    results,
                    local_functions,
                    param_values,
                    param_tables,
                );
            } else {
                Self::visit_control_flow_blocks(stmt, |block| {
                    self.find_table_insert_prefabs_in_block_with_params(
                        block,
                        results,
                        local_functions,
                        param_values,
                        param_tables,
                    );
                });
            }
        }
    }

    fn analyze_prefab_call_with_tables(
        &self,
        call: &ast::FunctionCall,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
        param_tables: &HashMap<String, ast::Expression>,
    ) -> Option<PrefabNameOverride> {
        let args = self.get_call_args(call)?;
        let args_vec: Vec<_> = args.iter().collect();

        if args_vec.is_empty() {
            return None;
        }

        let prefab_name =
            self.extract_prefab_name_from_arg(args_vec[0], param_values, param_tables);

        if args_vec.len() < 2 {
            return None;
        }

        let prefab_name = prefab_name?;

        let fn_arg = args_vec[1];
        let override_value = self.find_override_in_fn_arg_with_local_fns_and_params(
            fn_arg,
            local_functions,
            param_values,
        );

        let override_value = override_value?;

        let location = self.get_call_location(call);
        Some(PrefabNameOverride {
            prefab_name,
            override_name: override_value,
            location,
        })
    }

    fn extract_prefab_name_from_arg(
        &self,
        arg: &ast::Expression,
        param_values: &HashMap<String, String>,
        param_tables: &HashMap<String, ast::Expression>,
    ) -> Option<String> {
        match arg {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Number(n) => Some(n.token().to_string().trim().to_string()),
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    if let Some(value) = param_values.get(&var_name) {
                        return Some(value.clone());
                    }
                    if let Some(table_expr) = param_tables.get(&var_name) {
                        return self.extract_name_from_table_expr(table_expr);
                    }
                    self.variables.get(&var_name).and_then(|v| v.value.clone())
                } else if let ast::Var::Expression(var_expr) = var {
                    let prefix = var_expr.prefix();
                    if let ast::Prefix::Name(name) = prefix {
                        let base_name = name.token().to_string();
                        if let Some(table_expr) = param_tables.get(&base_name) {
                            return self.extract_name_from_table_expr(table_expr);
                        }
                    }
                    None
                } else {
                    None
                }
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op_str = binop.to_string().trim().to_string();
                if op_str == ".." {
                    let left =
                        self.extract_prefab_name_from_arg(lhs, param_values, param_tables)?;
                    let right =
                        self.extract_prefab_name_from_arg(rhs, param_values, param_tables)?;
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
                                return self.extract_prefab_name_from_arg(
                                    args_vec[0],
                                    param_values,
                                    param_tables,
                                );
                            }
                        }
                    }
                }
                None
            }
            ast::Expression::TableConstructor(table) => {
                self.extract_name_from_table_constructor(table)
            }
            _ => None,
        }
    }

    // ============================================================================
    // Section 9: Table Expression Name Extraction
    // ============================================================================
    fn extract_name_from_table_expr(&self, expr: &ast::Expression) -> Option<String> {
        match expr {
            ast::Expression::TableConstructor(table) => {
                self.extract_name_from_table_constructor(table)
            }
            _ => None,
        }
    }

    fn extract_name_from_table_constructor(&self, table: &ast::TableConstructor) -> Option<String> {
        for field in table.fields() {
            if let ast::Field::NameKey { key, value, .. } = field {
                if key.token().to_string() == "name" {
                    return self.extract_string_from_arg(value);
                }
            }
        }
        None
    }

    fn extract_string_from_arg(&self, arg: &ast::Expression) -> Option<String> {
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
    // Section 10: Prefab Call Analysis with Local Functions and Parameters
    // ============================================================================
    fn analyze_prefab_call_with_local_fns_and_params(
        &self,
        call: &ast::FunctionCall,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) -> Option<PrefabNameOverride> {
        let args = self.get_call_args(call)?;
        let args_vec: Vec<_> = args.iter().collect();

        if args_vec.is_empty() {
            return None;
        }

        let prefab_name = self.extract_string_from_arg_with_params(args_vec[0], param_values);

        if args_vec.len() < 2 {
            return None;
        }

        let prefab_name = prefab_name?;

        let fn_arg = args_vec[1];
        let override_value = self.find_override_in_fn_arg_with_local_fns_and_params(
            fn_arg,
            local_functions,
            param_values,
        );

        let override_value = override_value?;

        let location = self.get_call_location(call);
        Some(PrefabNameOverride {
            prefab_name,
            override_name: override_value,
            location,
        })
    }

    fn extract_string_from_arg_with_params(
        &self,
        arg: &ast::Expression,
        param_values: &HashMap<String, String>,
    ) -> Option<String> {
        match arg {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Number(n) => Some(n.token().to_string().trim().to_string()),
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    if let Some(value) = param_values.get(&var_name) {
                        return Some(value.clone());
                    }
                    self.variables.get(&var_name).and_then(|v| v.value.clone())
                } else {
                    None
                }
            }
            ast::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op_str = binop.to_string().trim().to_string();
                if op_str == ".." {
                    let left = self.extract_string_from_arg_with_params(lhs, param_values)?;
                    let right = self.extract_string_from_arg_with_params(rhs, param_values)?;
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
                                return self.extract_string_from_arg_with_params(
                                    args_vec[0],
                                    param_values,
                                );
                            }
                        }
                    }
                }
                None
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    if let ast::Field::NameKey { key, value, .. } = field {
                        if key.token().to_string() == "name" {
                            if let Some(val) =
                                self.extract_string_from_arg_with_params(value, param_values)
                            {
                                return Some(val);
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
    // Section 11: Override Finding with Local Functions and Parameters
    // ============================================================================
    fn find_override_in_fn_arg_with_local_fns_and_params(
        &self,
        arg: &ast::Expression,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        match arg {
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let fn_name = name.token().to_string();
                    if let Some(body) = local_functions.get(&fn_name) {
                        let mut visited = vec![fn_name.clone()];
                        self.find_override_in_function_body_deep_with_local_fns_and_params(
                            body,
                            &mut visited,
                            local_functions,
                            param_values,
                        )
                    } else if let Some(func_info) = self.functions.get(&fn_name) {
                        let mut visited = vec![fn_name.clone()];
                        self.find_override_in_function_body_deep_with_params(
                            &func_info.body,
                            &mut visited,
                            param_values,
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ast::Expression::Function(func) => {
                self.find_override_in_function_body_with_params(func.body(), param_values)
            }
            _ => None,
        }
    }

    fn find_override_in_function_body_deep_with_local_fns_and_params(
        &self,
        body: &ast::FunctionBody,
        visited: &mut Vec<String>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        for stmt in body.block().stmts() {
            if let Some(override_val) = self.find_override_in_stmt_deep_with_local_fns_and_params(
                stmt,
                visited,
                local_functions,
                param_values,
            ) {
                return Some(override_val);
            }
        }
        // 常见包装模式：`local function redfn() return common_fn("red") end`
        if let Some(ast::LastStmt::Return(ret)) = body.block().last_stmt() {
            for expr in ret.returns().iter() {
                if let Some(override_val) = self
                    .find_override_in_expr_deep_with_local_fns_and_params(
                        expr,
                        visited,
                        local_functions,
                        param_values,
                    )
                {
                    return Some(override_val);
                }
            }
        }
        None
    }

    fn find_override_in_function_body_deep_with_params(
        &self,
        body: &ast::FunctionBody,
        visited: &mut Vec<String>,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        let mut local_functions: HashMap<String, ast::FunctionBody> = HashMap::new();

        for stmt in body.block().stmts() {
            match stmt {
                ast::Stmt::LocalFunction(local_fn) => {
                    let fn_name = local_fn.name().to_string();
                    local_functions.insert(fn_name, local_fn.body().clone());
                }
                ast::Stmt::LocalAssignment(assignment) => {
                    let names: Vec<_> = assignment.names().iter().collect();
                    let exprs: Vec<_> = assignment.expressions().iter().collect();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        if let ast::Expression::Function(nested_func) = expr {
                            let fn_name = name.token().to_string();
                            local_functions.insert(fn_name, nested_func.body().clone());
                        }
                    }
                }
                ast::Stmt::NumericFor(numeric_for_stmt) => {
                    for stmt in numeric_for_stmt.block().stmts() {
                        if let ast::Stmt::LocalFunction(local_fn) = stmt {
                            let fn_name = local_fn.name().to_string();
                            local_functions.insert(fn_name, local_fn.body().clone());
                        }
                    }
                }
                ast::Stmt::GenericFor(generic_for_stmt) => {
                    for stmt in generic_for_stmt.block().stmts() {
                        if let ast::Stmt::LocalFunction(local_fn) = stmt {
                            let fn_name = local_fn.name().to_string();
                            local_functions.insert(fn_name, local_fn.body().clone());
                        }
                    }
                }
                _ => {}
            }
        }

        self.find_override_in_function_body_deep_with_local_fns_and_params(
            body,
            visited,
            &local_functions,
            param_values,
        )
    }

    fn find_override_in_function_body_with_params(
        &self,
        body: &ast::FunctionBody,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        let mut visited = Vec::new();
        self.find_override_in_function_body_deep_with_params(body, &mut visited, param_values)
    }

    fn find_override_in_stmt_deep_with_local_fns_and_params(
        &self,
        stmt: &ast::Stmt,
        visited: &mut Vec<String>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        match stmt {
            ast::Stmt::FunctionCall(call) => {
                if self.is_set_prefab_name_override_call(call) {
                    self.extract_override_from_call_with_params(call, param_values)
                } else {
                    self.try_find_override_in_call_chain_with_local_fns_and_params(
                        call,
                        visited,
                        local_functions,
                        param_values,
                    )
                }
            }
            ast::Stmt::LocalAssignment(assignment) => {
                for expr in assignment.expressions().iter() {
                    if let Some(override_val) = self
                        .find_override_in_expr_deep_with_local_fns_and_params(
                            expr,
                            visited,
                            local_functions,
                            param_values,
                        )
                    {
                        return Some(override_val);
                    }
                }
                None
            }
            ast::Stmt::If(if_stmt) => {
                for stmt in if_stmt.block().stmts() {
                    if let Some(override_val) = self
                        .find_override_in_stmt_deep_with_local_fns_and_params(
                            stmt,
                            visited,
                            local_functions,
                            param_values,
                        )
                    {
                        return Some(override_val);
                    }
                }
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        for stmt in else_if.block().stmts() {
                            if let Some(override_val) = self
                                .find_override_in_stmt_deep_with_local_fns_and_params(
                                    stmt,
                                    visited,
                                    local_functions,
                                    param_values,
                                )
                            {
                                return Some(override_val);
                            }
                        }
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    for stmt in else_block.stmts() {
                        if let Some(override_val) = self
                            .find_override_in_stmt_deep_with_local_fns_and_params(
                                stmt,
                                visited,
                                local_functions,
                                param_values,
                            )
                        {
                            return Some(override_val);
                        }
                    }
                }
                None
            }
            ast::Stmt::While(while_stmt) => {
                for stmt in while_stmt.block().stmts() {
                    if let Some(override_val) = self
                        .find_override_in_stmt_deep_with_local_fns_and_params(
                            stmt,
                            visited,
                            local_functions,
                            param_values,
                        )
                    {
                        return Some(override_val);
                    }
                }
                None
            }
            ast::Stmt::Repeat(repeat_stmt) => {
                for stmt in repeat_stmt.block().stmts() {
                    if let Some(override_val) = self
                        .find_override_in_stmt_deep_with_local_fns_and_params(
                            stmt,
                            visited,
                            local_functions,
                            param_values,
                        )
                    {
                        return Some(override_val);
                    }
                }
                None
            }
            ast::Stmt::GenericFor(generic_for_stmt) => {
                for stmt in generic_for_stmt.block().stmts() {
                    if let Some(override_val) = self
                        .find_override_in_stmt_deep_with_local_fns_and_params(
                            stmt,
                            visited,
                            local_functions,
                            param_values,
                        )
                    {
                        return Some(override_val);
                    }
                }
                None
            }
            ast::Stmt::NumericFor(numeric_for_stmt) => {
                for stmt in numeric_for_stmt.block().stmts() {
                    if let Some(override_val) = self
                        .find_override_in_stmt_deep_with_local_fns_and_params(
                            stmt,
                            visited,
                            local_functions,
                            param_values,
                        )
                    {
                        return Some(override_val);
                    }
                }
                None
            }
            ast::Stmt::LocalFunction(local_fn) => self
                .find_override_in_function_body_deep_with_local_fns_and_params(
                    local_fn.body(),
                    visited,
                    local_functions,
                    param_values,
                ),
            _ => None,
        }
    }

    fn find_override_in_expr_deep_with_local_fns_and_params(
        &self,
        expr: &ast::Expression,
        visited: &mut Vec<String>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        match expr {
            ast::Expression::FunctionCall(call) => self
                .try_find_override_in_call_chain_with_local_fns_and_params(
                    call,
                    visited,
                    local_functions,
                    param_values,
                ),
            ast::Expression::Function(func) => self
                .find_override_in_function_body_deep_with_local_fns_and_params(
                    func.body(),
                    visited,
                    local_functions,
                    param_values,
                ),
            _ => None,
        }
    }

    fn try_find_override_in_call_chain_with_local_fns_and_params(
        &self,
        call: &ast::FunctionCall,
        visited: &mut Vec<String>,
        local_functions: &HashMap<String, ast::FunctionBody>,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let fn_name = name.token().to_string();
            if visited.contains(&fn_name) {
                return None;
            }
            if let Some(body) = local_functions.get(&fn_name) {
                visited.push(fn_name);
                let result = self.find_override_in_function_body_deep_with_local_fns_and_params(
                    body,
                    visited,
                    local_functions,
                    param_values,
                );
                visited.pop();
                return result;
            }
            if let Some(func_info) = self.functions.get(&fn_name) {
                visited.push(fn_name);
                let result = self.find_override_in_function_body_deep_with_params(
                    &func_info.body,
                    visited,
                    param_values,
                );
                visited.pop();
                return result;
            }
        }
        None
    }

    fn extract_override_from_call_with_params(
        &self,
        call: &ast::FunctionCall,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        let suffixes: Vec<_> = call.suffixes().collect();
        if let Some(ast::Suffix::Call(ast::Call::MethodCall(method_call))) = suffixes.first() {
            let args = method_call.args();
            if let ast::FunctionArgs::Parentheses { arguments, .. } = args {
                let args_vec: Vec<_> = arguments.iter().collect();
                if !args_vec.is_empty() {
                    if let Some(value) =
                        self.extract_string_from_arg_with_params(args_vec[0], param_values)
                    {
                        return Some(OverrideValue::Static(value));
                    } else {
                        return Some(OverrideValue::Dynamic(args_vec[0].to_string()));
                    }
                }
            }
        }
        None
    }

    fn find_prefab_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();

        for stmt in self.ast.nodes().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_prefab_call(call) {
                    calls.push(call.clone());
                }
            }
        }

        if let Some(ast::LastStmt::Return(ret)) = self.ast.nodes().last_stmt() {
            for expr in ret.returns().iter() {
                if let ast::Expression::FunctionCall(call) = expr {
                    if self.is_prefab_call(call) {
                        calls.push(call.clone());
                    }
                }
            }
        }

        calls
    }

    fn is_prefab_call(&self, call: &ast::FunctionCall) -> bool {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            name.token().to_string() == "Prefab"
        } else {
            false
        }
    }

    // ============================================================================
    // Section 12: Factory Call Collection
    // ============================================================================
    fn find_factory_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();
        self.find_factory_calls_in_block(self.ast.nodes(), &mut calls);
        calls
    }

    fn find_factory_calls_in_block(&self, block: &ast::Block, calls: &mut Vec<ast::FunctionCall>) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call)
                    if (self.is_factory_call(call) || self.is_known_function_call(call)) =>
                {
                    calls.push(call.clone());
                }
                ast::Stmt::LocalAssignment(assignment) => {
                    for expr in assignment.expressions().iter() {
                        self.find_factory_calls_in_expr(expr, calls);
                    }
                }
                ast::Stmt::If(if_stmt) => {
                    self.find_factory_calls_in_block(if_stmt.block(), calls);
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if_block in else_ifs {
                            self.find_factory_calls_in_block(else_if_block.block(), calls);
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.find_factory_calls_in_block(else_block, calls);
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.find_factory_calls_in_block(while_stmt.block(), calls);
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.find_factory_calls_in_block(repeat_stmt.block(), calls);
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    let names: Vec<_> = for_stmt
                        .names()
                        .iter()
                        .map(|n| n.token().to_string())
                        .collect();
                    let exprs: Vec<_> = for_stmt.expressions().iter().collect();
                    if !names.is_empty() && !exprs.is_empty() {
                        let mut iter_tables: HashMap<String, ast::Expression> = HashMap::new();
                        for (name, expr) in names.iter().zip(exprs.iter()) {
                            if let ast::Expression::FunctionCall(call_expr) = expr {
                                if let Some(table) = self.extract_table_from_ipairs(call_expr) {
                                    iter_tables.insert(name.clone(), table);
                                }
                            }
                        }
                        self.find_factory_calls_in_block_with_iter_tables(
                            for_stmt.block(),
                            calls,
                            &iter_tables,
                        );
                    } else {
                        self.find_factory_calls_in_block(for_stmt.block(), calls);
                    }
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.find_factory_calls_in_block(for_stmt.block(), calls);
                }
                ast::Stmt::LocalFunction(local_fn) => {
                    self.find_factory_calls_in_block(local_fn.body().block(), calls);
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    self.find_factory_calls_in_block(func_decl.body().block(), calls);
                }
                _ => {}
            }
        }

        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns().iter() {
                self.find_factory_calls_in_expr(expr, calls);
            }
        }
    }

    fn is_known_function_call(&self, call: &ast::FunctionCall) -> bool {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let name_str = name.token().to_string();
            self.functions.contains_key(&name_str)
        } else {
            false
        }
    }

    /// 函数体是否真的注册了 prefab（`Prefab(...)` / `table.insert(_, Prefab(...))`）。
    ///
    /// 用于区分工厂函数与普通构造函数：后者也会调用其他局部函数并设置
    /// `SetPrefabNameOverride`，但其首个实参是动画/形态名，不是 prefab 名。
    fn body_registers_prefab(&self, body: &ast::FunctionBody) -> bool {
        self.block_registers_prefab(body.block())
    }

    fn block_registers_prefab(&self, block: &ast::Block) -> bool {
        for stmt in block.stmts() {
            let found = match stmt {
                ast::Stmt::FunctionCall(call) => {
                    self.call_registers_prefab(call) || self.call_args_register_prefab(call)
                }
                ast::Stmt::LocalAssignment(assignment) => assignment
                    .expressions()
                    .iter()
                    .any(|expr| self.expr_registers_prefab(expr)),
                ast::Stmt::Assignment(assignment) => assignment
                    .expressions()
                    .iter()
                    .any(|expr| self.expr_registers_prefab(expr)),
                ast::Stmt::LocalFunction(local_fn) => {
                    self.block_registers_prefab(local_fn.body().block())
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    self.block_registers_prefab(func_decl.body().block())
                }
                ast::Stmt::If(if_stmt) => {
                    self.block_registers_prefab(if_stmt.block())
                        || if_stmt
                            .else_if()
                            .map(|list| {
                                list.iter()
                                    .any(|else_if| self.block_registers_prefab(else_if.block()))
                            })
                            .unwrap_or(false)
                        || if_stmt
                            .else_block()
                            .map(|else_block| self.block_registers_prefab(else_block))
                            .unwrap_or(false)
                }
                ast::Stmt::Do(do_stmt) => self.block_registers_prefab(do_stmt.block()),
                ast::Stmt::While(while_stmt) => self.block_registers_prefab(while_stmt.block()),
                ast::Stmt::Repeat(repeat_stmt) => self.block_registers_prefab(repeat_stmt.block()),
                ast::Stmt::NumericFor(for_stmt) => self.block_registers_prefab(for_stmt.block()),
                ast::Stmt::GenericFor(for_stmt) => self.block_registers_prefab(for_stmt.block()),
                _ => false,
            };
            if found {
                return true;
            }
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns().iter() {
                if self.expr_registers_prefab(expr) {
                    return true;
                }
            }
        }
        false
    }

    fn expr_registers_prefab(&self, expr: &ast::Expression) -> bool {
        match expr {
            ast::Expression::FunctionCall(call) => {
                self.call_registers_prefab(call) || self.call_args_register_prefab(call)
            }
            ast::Expression::TableConstructor(table) => {
                table.fields().iter().any(|field| match field {
                    ast::Field::NameKey { value, .. } => self.expr_registers_prefab(value),
                    ast::Field::ExpressionKey { value, .. } => self.expr_registers_prefab(value),
                    ast::Field::NoKey(value) => self.expr_registers_prefab(value),
                    _ => false,
                })
            }
            ast::Expression::Function(func) => self.block_registers_prefab(func.body().block()),
            ast::Expression::Parentheses { expression, .. } => {
                self.expr_registers_prefab(expression)
            }
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.expr_registers_prefab(lhs) || self.expr_registers_prefab(rhs)
            }
            ast::Expression::UnaryOperator { expression, .. } => {
                self.expr_registers_prefab(expression)
            }
            _ => false,
        }
    }

    fn call_registers_prefab(&self, call: &ast::FunctionCall) -> bool {
        self.is_prefab_call(call) || self.is_table_insert_with_prefab(call)
    }

    fn call_args_register_prefab(&self, call: &ast::FunctionCall) -> bool {
        self.get_call_args(call)
            .map(|args| args.iter().any(|arg| self.expr_registers_prefab(arg)))
            .unwrap_or(false)
    }

    fn find_factory_calls_in_block_with_iter_tables(
        &self,
        block: &ast::Block,
        calls: &mut Vec<ast::FunctionCall>,
        iter_tables: &HashMap<String, ast::Expression>,
    ) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call)
                    if (self.is_factory_call(call) || self.is_known_function_call(call)) =>
                {
                    calls.push(call.clone());
                }
                ast::Stmt::LocalAssignment(assignment) => {
                    for expr in assignment.expressions().iter() {
                        self.find_factory_calls_in_expr_with_iter_tables(expr, calls, iter_tables);
                    }
                }
                ast::Stmt::LocalFunction(local_fn) => {
                    self.find_factory_calls_in_block_with_iter_tables(
                        local_fn.body().block(),
                        calls,
                        iter_tables,
                    );
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    self.find_factory_calls_in_block_with_iter_tables(
                        func_decl.body().block(),
                        calls,
                        iter_tables,
                    );
                }
                ast::Stmt::If(if_stmt) => {
                    self.find_factory_calls_in_block_with_iter_tables(
                        if_stmt.block(),
                        calls,
                        iter_tables,
                    );
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if_block in else_ifs {
                            self.find_factory_calls_in_block_with_iter_tables(
                                else_if_block.block(),
                                calls,
                                iter_tables,
                            );
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.find_factory_calls_in_block_with_iter_tables(
                            else_block,
                            calls,
                            iter_tables,
                        );
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.find_factory_calls_in_block_with_iter_tables(
                        while_stmt.block(),
                        calls,
                        iter_tables,
                    );
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.find_factory_calls_in_block_with_iter_tables(
                        repeat_stmt.block(),
                        calls,
                        iter_tables,
                    );
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    let names: Vec<_> = for_stmt
                        .names()
                        .iter()
                        .map(|n| n.token().to_string())
                        .collect();
                    let exprs: Vec<_> = for_stmt.expressions().iter().collect();
                    if !names.is_empty() && !exprs.is_empty() {
                        let mut new_iter_tables = iter_tables.clone();
                        for (name, expr) in names.iter().zip(exprs.iter()) {
                            if let ast::Expression::FunctionCall(call_expr) = expr {
                                if let Some(table) = self.extract_table_from_ipairs(call_expr) {
                                    new_iter_tables.insert(name.clone(), table);
                                }
                            }
                        }
                        self.find_factory_calls_in_block_with_iter_tables(
                            for_stmt.block(),
                            calls,
                            &new_iter_tables,
                        );
                    } else {
                        self.find_factory_calls_in_block_with_iter_tables(
                            for_stmt.block(),
                            calls,
                            iter_tables,
                        );
                    }
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.find_factory_calls_in_block_with_iter_tables(
                        for_stmt.block(),
                        calls,
                        iter_tables,
                    );
                }
                _ => {}
            }
        }

        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns().iter() {
                self.find_factory_calls_in_expr_with_iter_tables(expr, calls, iter_tables);
            }
        }
    }

    fn find_factory_calls_in_expr_with_iter_tables(
        &self,
        expr: &ast::Expression,
        calls: &mut Vec<ast::FunctionCall>,
        _iter_tables: &HashMap<String, ast::Expression>,
    ) {
        match expr {
            ast::Expression::FunctionCall(call)
                if (self.is_factory_call(call) || self.is_known_function_call(call)) =>
            {
                calls.push(call.clone());
            }
            ast::Expression::Function(func) => {
                self.find_factory_calls_in_block_with_iter_tables(
                    func.body().block(),
                    calls,
                    _iter_tables,
                );
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => {
                            self.find_factory_calls_in_expr_with_iter_tables(
                                expression,
                                calls,
                                _iter_tables,
                            );
                        }
                        ast::Field::NameKey { value, .. } => {
                            self.find_factory_calls_in_expr_with_iter_tables(
                                value,
                                calls,
                                _iter_tables,
                            );
                        }
                        ast::Field::ExpressionKey { value, .. } => {
                            self.find_factory_calls_in_expr_with_iter_tables(
                                value,
                                calls,
                                _iter_tables,
                            );
                        }
                        _ => {}
                    }
                }
            }
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.find_factory_calls_in_expr_with_iter_tables(lhs, calls, _iter_tables);
                self.find_factory_calls_in_expr_with_iter_tables(rhs, calls, _iter_tables);
            }
            _ => {}
        }
    }

    fn find_ipairs_factory_calls(
        &self,
    ) -> Vec<(ast::FunctionCall, HashMap<String, ast::Expression>)> {
        let mut results = Vec::new();
        self.find_ipairs_factory_calls_in_block(self.ast.nodes(), &mut results, &HashMap::new());
        results
    }

    fn find_ipairs_factory_calls_in_block(
        &self,
        block: &ast::Block,
        results: &mut Vec<(ast::FunctionCall, HashMap<String, ast::Expression>)>,
        iter_tables: &HashMap<String, ast::Expression>,
    ) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call) if self.is_factory_call(call) => {
                    let call_args = self.get_call_args(call);
                    if let Some(args) = call_args {
                        let args_vec: Vec<_> = args.iter().collect();
                        if args_vec.len() == 1 {
                            if let ast::Expression::Var(ast::Var::Name(name)) = args_vec[0] {
                                let var_name = name.token().to_string();
                                if let Some(ast::Expression::TableConstructor(table_constructor)) =
                                    iter_tables.get(&var_name)
                                {
                                    for field in table_constructor.fields() {
                                        if let ast::Field::NoKey(
                                            ast::Expression::TableConstructor(item_table),
                                        ) = field
                                        {
                                            let mut synthetic_tables: HashMap<
                                                String,
                                                ast::Expression,
                                            > = HashMap::new();
                                            synthetic_tables.insert(
                                                var_name.clone(),
                                                ast::Expression::TableConstructor(
                                                    item_table.clone(),
                                                ),
                                            );
                                            results.push((call.clone(), synthetic_tables));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ast::Stmt::LocalAssignment(assignment) => {
                    for expr in assignment.expressions().iter() {
                        self.find_factory_calls_in_expr(expr, &mut Vec::new());
                    }
                }
                ast::Stmt::If(if_stmt) => {
                    self.find_ipairs_factory_calls_in_block(if_stmt.block(), results, iter_tables);
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if_block in else_ifs {
                            self.find_ipairs_factory_calls_in_block(
                                else_if_block.block(),
                                results,
                                iter_tables,
                            );
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.find_ipairs_factory_calls_in_block(else_block, results, iter_tables);
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.find_ipairs_factory_calls_in_block(
                        while_stmt.block(),
                        results,
                        iter_tables,
                    );
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.find_ipairs_factory_calls_in_block(
                        repeat_stmt.block(),
                        results,
                        iter_tables,
                    );
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    let names: Vec<_> = for_stmt
                        .names()
                        .iter()
                        .map(|n| n.token().to_string())
                        .collect();
                    let exprs: Vec<_> = for_stmt.expressions().iter().collect();
                    if !names.is_empty() && !exprs.is_empty() {
                        let mut new_iter_tables = iter_tables.clone();
                        if exprs.len() == 1 {
                            if let ast::Expression::FunctionCall(call_expr) = &exprs[0] {
                                if let Some(table) = self.extract_table_from_ipairs(call_expr) {
                                    if names.len() >= 2 {
                                        new_iter_tables.insert(names[1].clone(), table.clone());
                                    }
                                }
                            }
                        }
                        for (name, expr) in names.iter().zip(exprs.iter()) {
                            if let ast::Expression::FunctionCall(call_expr) = expr {
                                if let Some(table) = self.extract_table_from_ipairs(call_expr) {
                                    new_iter_tables.insert(name.clone(), table);
                                }
                            }
                        }
                        self.find_ipairs_factory_calls_in_block(
                            for_stmt.block(),
                            results,
                            &new_iter_tables,
                        );
                    } else {
                        self.find_ipairs_factory_calls_in_block(
                            for_stmt.block(),
                            results,
                            iter_tables,
                        );
                    }
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.find_ipairs_factory_calls_in_block(for_stmt.block(), results, iter_tables);
                }
                _ => {}
            }
        }

        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns().iter() {
                self.find_factory_calls_in_expr(expr, &mut Vec::new());
            }
        }
    }

    fn extract_table_from_ipairs(&self, call: &ast::FunctionCall) -> Option<ast::Expression> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            if name.token().to_string() == "ipairs" {
                let suffixes: Vec<_> = call.suffixes().collect();
                if suffixes.len() != 1 {
                    return None;
                }
                if let ast::Suffix::Call(ast::Call::AnonymousCall(
                    ast::FunctionArgs::Parentheses { arguments, .. },
                )) = &suffixes[0]
                {
                    let args_vec: Vec<_> = arguments.iter().collect();
                    if !args_vec.is_empty() {
                        return Some(args_vec[0].clone());
                    }
                }
            }
        }
        None
    }

    fn find_factory_calls_in_expr(
        &self,
        expr: &ast::Expression,
        calls: &mut Vec<ast::FunctionCall>,
    ) {
        match expr {
            ast::Expression::FunctionCall(call)
                if (self.is_factory_call(call) || self.is_known_function_call(call)) =>
            {
                calls.push(call.clone());
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => {
                            self.find_factory_calls_in_expr(expression, calls);
                        }
                        ast::Field::NameKey { value, .. } => {
                            self.find_factory_calls_in_expr(value, calls);
                        }
                        ast::Field::ExpressionKey { key: _, value, .. } => {
                            self.find_factory_calls_in_expr(value, calls);
                        }
                        _ => {}
                    }
                }
            }
            ast::Expression::Function(func) => {
                self.find_factory_calls_in_block(func.body().block(), calls);
            }
            _ => {}
        }
    }

    // ============================================================================
    // Section 13: Prefab Call Analysis
    // ============================================================================
    fn is_factory_call(&self, call: &ast::FunctionCall) -> bool {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let name_str = name.token().to_string();
            matches!(
                name_str.as_str(),
                "MakeBundle"
                    | "MakeWrap"
                    | "MakeContainer"
                    | "AddWinterTree"
                    | "MakeGlobalTrackingIcons"
            )
        } else {
            false
        }
    }

    fn analyze_prefab_call(&self, call: &ast::FunctionCall) -> Option<PrefabNameOverride> {
        let args = self.get_call_args(call)?;
        let args_vec: Vec<_> = args.iter().collect();

        if args_vec.is_empty() {
            return None;
        }

        let prefab_name = self.extract_string_from_arg(args_vec[0])?;

        if args_vec.len() < 2 {
            return None;
        }

        let fn_arg = args_vec[1];
        let override_value = self.find_override_in_fn_arg(fn_arg)?;

        let location = self.get_call_location(call);
        Some(PrefabNameOverride {
            prefab_name,
            override_name: override_value,
            location,
        })
    }

    fn get_call_args(&self, call: &ast::FunctionCall) -> Option<Vec<ast::Expression>> {
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

    fn find_override_in_fn_arg(&self, arg: &ast::Expression) -> Option<OverrideValue> {
        match arg {
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let fn_name = name.token().to_string();
                    if let Some(func_info) = self.functions.get(&fn_name) {
                        let mut visited = vec![fn_name.clone()];
                        self.find_override_in_function_body_deep(&func_info.body, &mut visited)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ast::Expression::Function(func) => self.find_override_in_function_body(func.body()),
            _ => None,
        }
    }

    fn find_override_in_function_body(&self, body: &ast::FunctionBody) -> Option<OverrideValue> {
        for stmt in body.block().stmts() {
            if let Some(override_val) = self.find_override_in_stmt(stmt) {
                return Some(override_val);
            }
        }
        None
    }

    fn find_override_in_function_body_deep(
        &self,
        body: &ast::FunctionBody,
        visited: &mut Vec<String>,
    ) -> Option<OverrideValue> {
        let mut local_functions: HashMap<String, ast::FunctionBody> = HashMap::new();

        for stmt in body.block().stmts() {
            match stmt {
                ast::Stmt::LocalFunction(local_fn) => {
                    let fn_name = local_fn.name().to_string();
                    local_functions.insert(fn_name, local_fn.body().clone());
                }
                ast::Stmt::LocalAssignment(assignment) => {
                    let names: Vec<_> = assignment.names().iter().collect();
                    let exprs: Vec<_> = assignment.expressions().iter().collect();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        if let ast::Expression::Function(nested_func) = expr {
                            let fn_name = name.token().to_string();
                            local_functions.insert(fn_name, nested_func.body().clone());
                        }
                    }
                }
                ast::Stmt::NumericFor(numeric_for_stmt) => {
                    for stmt in numeric_for_stmt.block().stmts() {
                        if let ast::Stmt::LocalFunction(local_fn) = stmt {
                            let fn_name = local_fn.name().to_string();
                            local_functions.insert(fn_name, local_fn.body().clone());
                        }
                    }
                }
                ast::Stmt::GenericFor(generic_for_stmt) => {
                    for stmt in generic_for_stmt.block().stmts() {
                        if let ast::Stmt::LocalFunction(local_fn) = stmt {
                            let fn_name = local_fn.name().to_string();
                            local_functions.insert(fn_name, local_fn.body().clone());
                        }
                    }
                }
                _ => {}
            }
        }

        self.find_override_in_function_body_deep_with_local_fns_and_params(
            body,
            visited,
            &local_functions,
            &HashMap::new(),
        )
    }

    fn find_override_in_stmt(&self, stmt: &ast::Stmt) -> Option<OverrideValue> {
        match stmt {
            ast::Stmt::FunctionCall(call) => {
                if self.is_set_prefab_name_override_call(call) {
                    self.extract_override_from_call(call)
                } else {
                    None
                }
            }
            ast::Stmt::If(if_stmt) => {
                for stmt in if_stmt.block().stmts() {
                    if let Some(override_val) = self.find_override_in_stmt(stmt) {
                        return Some(override_val);
                    }
                }
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        for stmt in else_if.block().stmts() {
                            if let Some(override_val) = self.find_override_in_stmt(stmt) {
                                return Some(override_val);
                            }
                        }
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    for stmt in else_block.stmts() {
                        if let Some(override_val) = self.find_override_in_stmt(stmt) {
                            return Some(override_val);
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
    fn is_set_prefab_name_override_call(&self, call: &ast::FunctionCall) -> bool {
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

    fn extract_override_from_call(&self, call: &ast::FunctionCall) -> Option<OverrideValue> {
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

    fn extract_override_value(&self, expr: &ast::Expression) -> Option<OverrideValue> {
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
    fn analyze_factory_call(&self, call: &ast::FunctionCall) -> Option<PrefabNameOverride> {
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

    fn find_config_table_in_args(&self, args: &[ast::Expression]) -> Option<String> {
        for arg in args {
            if let ast::Expression::Var(ast::Var::Name(name)) = arg {
                return Some(name.token().to_string());
            }
        }
        None
    }

    fn find_override_in_config_table(&self, table_name: &str) -> Option<OverrideValue> {
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

    fn find_override_in_table_expr(&self, expr: &ast::Expression) -> Option<OverrideValue> {
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
    fn get_call_location(&self, call: &ast::FunctionCall) -> SourceLocation {
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

    fn byte_to_line(&self, byte: usize) -> usize {
        self.source[..byte.min(self.source.len())]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1
    }
}

fn extract_string_literal(s: &str) -> String {
    let s = s.trim();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 {
        let first = chars[0];
        let last = chars[chars.len() - 1];
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return chars[1..chars.len() - 1].iter().collect();
        }
        if first == '[' && last == ']' {
            let inner: String = chars[1..chars.len() - 1].iter().collect();
            if let Some(pos) = inner.find('[') {
                return inner[pos + 1..].to_string();
            }
        }
    }
    s.to_string()
}

pub fn parse_prefab_overrides(source: &str) -> Result<Vec<PrefabNameOverride>> {
    let parser = PrefabOverrideParser::new(source)?;
    parser.parse()
}

#[cfg(test)]
// ============================================================================
// Section 17: Tests
// ============================================================================
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_prefab_with_override() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("ancient_altar")
    return inst
end

return Prefab("ancient_altar_broken", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "ancient_altar_broken");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("ancient_altar".to_string())
        );
    }

    #[test]
    fn test_parse_prefab_with_variable_override() {
        let source = r#"
local override_name = "test_override"

local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride(override_name)
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "test_prefab");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("test_override".to_string())
        );
    }

    #[test]
    fn test_parse_prefab_with_dynamic_override() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride(get_override())
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "test_prefab");
        assert!(matches!(result[0].override_name, OverrideValue::Dynamic(_)));
    }

    #[test]
    fn test_parse_prefab_without_override() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_factory_pattern() {
        let source = r#"
local config = {
    common_postinit = function(inst)
        inst:SetPrefabNameOverride("redpouch")
    end,
}

return MakeBundle("redpouch_yotp", 3, nil, nil, true, config)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "redpouch_yotp");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("redpouch".to_string())
        );
    }

    #[test]
    fn test_parse_multiple_prefabs() {
        let source = r#"
local function fn1()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("override1")
    return inst
end

local function fn2()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("override2")
    return inst
end

return Prefab("prefab1", fn1), Prefab("prefab2", fn2)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].prefab_name, "prefab1");
        assert_eq!(result[1].prefab_name, "prefab2");
    }

    #[test]
    fn test_parse_inline_function() {
        let source = r#"
return Prefab("test_prefab", function()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride("inline_override")
    return inst
end)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prefab_name, "test_prefab");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("inline_override".to_string())
        );
    }

    #[test]
    fn test_string_concatenation() {
        let source = r#"
local prefix = "ancient_"

local function fn()
    local inst = CreateEntity()
    inst:SetPrefabNameOverride(prefix .. "altar")
    return inst
end

return Prefab("test_prefab", fn)
"#;
        let result = parse_prefab_overrides(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("ancient_altar".to_string())
        );
    }

    #[test]
    fn test_altar_prototyper_example() {
        let source = include_str!("../../../examples/prefabs/altar_prototyper.lua");
        let result = parse_prefab_overrides(source).unwrap();

        assert!(result.len() >= 2, "Expected at least 2 prefab overrides");

        let ancient_altar = result.iter().find(|r| r.prefab_name == "ancient_altar");
        let ancient_altar_broken = result
            .iter()
            .find(|r| r.prefab_name == "ancient_altar_broken");

        assert!(ancient_altar.is_some(), "Should find ancient_altar prefab");
        assert!(
            ancient_altar_broken.is_some(),
            "Should find ancient_altar_broken prefab"
        );

        if let Some(override_info) = ancient_altar {
            assert_eq!(
                override_info.override_name,
                OverrideValue::Static("ancient_altar".to_string())
            );
        }
        if let Some(override_info) = ancient_altar_broken {
            assert_eq!(
                override_info.override_name,
                OverrideValue::Static("ancient_altar".to_string())
            );
        }
    }

    #[test]
    fn test_bundle_example() {
        let source = include_str!("../../../examples/prefabs/bundle.lua");
        let result = parse_prefab_overrides(source).unwrap();

        let redpouch_yotp = result.iter().find(|r| r.prefab_name == "redpouch_yotp");
        let redpouch_yotc = result.iter().find(|r| r.prefab_name == "redpouch_yotc");
        let redpouch_yotb = result.iter().find(|r| r.prefab_name == "redpouch_yotb");

        assert!(redpouch_yotp.is_some(), "Should find redpouch_yotp prefab");
        assert!(redpouch_yotc.is_some(), "Should find redpouch_yotc prefab");
        assert!(redpouch_yotb.is_some(), "Should find redpouch_yotb prefab");

        for prefab in [
            "redpouch_yotp",
            "redpouch_yotc",
            "redpouch_yotb",
            "redpouch_yotr",
            "redpouch_yotd",
            "redpouch_yoth",
            "redpouch_yot_catcoon",
        ] {
            if let Some(override_info) = result.iter().find(|r| r.prefab_name == prefab) {
                assert_eq!(
                    override_info.override_name,
                    OverrideValue::Static("redpouch".to_string()),
                    "Prefab {} should have override 'redpouch'",
                    prefab
                );
            }
        }
    }

    #[test]
    fn test_wormhole_limited_example() {
        let source = include_str!("../../../examples/prefabs/wormhole_limited.lua");
        let result = parse_prefab_overrides(source).unwrap();

        assert!(
            !result.is_empty(),
            "Should find at least one prefab override in wormhole_limited.lua"
        );

        let wormhole = result
            .iter()
            .find(|r| r.prefab_name == "wormhole_limited_1");
        assert!(
            wormhole.is_some(),
            "Should find wormhole_limited_1 prefab (factory arg expansion)"
        );

        if let Some(override_info) = wormhole {
            assert_eq!(
                override_info.override_name,
                OverrideValue::Static("wormhole_limited".to_string())
            );
        }
    }

    #[test]
    fn test_wx78_drone_delivery_example() {
        let source = include_str!("../../../examples/prefabs/wx78_drone_delivery.lua");
        let result = parse_prefab_overrides(source).unwrap();

        assert!(
            !result.is_empty(),
            "Should find at least one prefab override"
        );

        let delivery = result
            .iter()
            .find(|r| r.prefab_name == "wx78_drone_delivery");
        assert!(delivery.is_some(), "Should find wx78_drone_delivery prefab");

        let delivery_small = result
            .iter()
            .find(|r| r.prefab_name == "wx78_drone_delivery_small");
        assert!(
            delivery_small.is_some(),
            "Should find wx78_drone_delivery_small prefab"
        );
    }

    #[test]
    fn test_nested_function_factory() {
        let source = r#"
local function makewormhole(uses)
    local function fn()
        local inst = CreateEntity()
        inst:SetPrefabNameOverride("wormhole_limited")
        return inst
    end

    return Prefab("wormhole_limited_"..uses, fn, assets)
end

return makewormhole(1)
"#;
        let parser = PrefabOverrideParser::new(source).unwrap();

        let result = parser.parse().unwrap();

        assert!(
            !result.is_empty(),
            "Should find at least one prefab override"
        );
        assert_eq!(result[0].prefab_name, "wormhole_limited_1");
        assert_eq!(
            result[0].override_name,
            OverrideValue::Static("wormhole_limited".to_string())
        );
    }

    #[test]
    fn test_all_prefabs_files() {
        let prefab_files = [
            "yots_worm_lantern",
            "wx78_taser_projectile",
            "wx78_drone_scout",
            "wormwood_lightflier",
            "wx78_drone_delivery",
            "wormwood_fruitdragon",
            "wormwood_carrat",
            "wormhole_limited",
            "worm_boss",
            "winter_tree",
            "wobster",
            "winter_ornaments",
            "winona_teleport_pad",
            "winona_spotlight",
            "winona_catapult_projectile",
            "winona_catapult",
            "winona_battery_low",
            "waterplant_seed",
            "winona_battery_high",
            "waterplant_rock",
            "wagstaff_npc",
            "wagdrone_projectile",
            "wagdrone_laserwire",
            "wagboss_beam",
            "veggies",
            "tree_rocks",
            "vault_switch",
            "support_pillar",
            "statueruins",
            "statue_marble",
            "stalker",
            "stalker_minions",
            "stalker_ferns",
            "stalker_bulb",
            "stalker_berry",
            "stalagmite_tall",
            "spiderhole",
            "stalagmite",
            "slingshot",
            "skeleton",
            "slingshotammo_debuffs",
            "shadowwaxwell",
            "sharkboi_ice_hazard",
            "shadowthrall_centipede",
            "scrapbook_page",
            "scrapbook_notes",
            "sapling",
            "sand_spike",
            "rock_ice_temperature",
            "redlantern",
            "rock_avocado_fruit",
            "quagmire_shadowwaxwell",
            "quagmire_parkspike",
            "quagmire_plantables",
            "quagmire_food_burnt",
            "quagmire_book_shadow",
            "quagmire_evergreen",
            "propsign",
            "quagmire_book_fertilizer",
            "preparedfoods",
            "portablespicer",
            "portablefirepit",
            "portablecookpot",
            "portableblender",
            "pocketwatch_portal",
            "pigman",
            "oceanfish",
            "nightmarefissure",
            "oceanfishingbobber",
            "multiplayer_portal",
            "moonstorm_glass",
            "moon_device",
            "minisign",
            "miniflare",
            "merm_fx",
            "mast_broken",
            "megaflare",
            "lunarthrall_plant",
            "livingtree_halloween",
            "lightflier_flower",
            "lavaarena_trails",
            "lava_pond",
            "lavaarena_peghook",
            "lavaarena_fossilizing",
            "lavaarena_groundlifts",
            "lavaarena_blooms",
            "lavaarena_abigail",
            "lavaarena_battlestandard",
            "lavaarena_abigail_flower",
            "hound",
            "hermithouse",
            "hats",
            "grotto_pool_moonglass",
            "grotto_waterfall_small",
            "gnarwail",
            "goosplash",
            "glass_spike",
            "gelblob",
            "gestalt_cage",
            "gargoyles",
            "flower_cave",
            "fused_shadeling_bomb",
            "firepit",
            "deer",
            "driftwood_trees",
            "deerclops_laser",
            "deer_antler",
            "collapsedchest",
            "cave_vents",
            "cave_banana_tree",
            "carrat",
            "carnivaldecor_figure",
            "cactus",
            "campfire",
            "bundle",
            "bramblefx",
            "bullkelp_beached",
            "bishop_charge",
            "archive_props",
            "atrium_statue",
            "alterguardian_laser",
            "altar_prototyper",
        ];

        let mut success_count = 0;
        let mut fail_count = 0;
        let mut empty_count = 0;

        for name in &prefab_files {
            let source = match std::fs::read_to_string(format!("examples/prefabs/{}.lua", name)) {
                Ok(s) => s,
                Err(e) => {
                    println!("FAIL {}: Could not read file: {}", name, e);
                    fail_count += 1;
                    continue;
                }
            };

            match parse_prefab_overrides(&source) {
                Ok(result) => {
                    if result.is_empty() {
                        println!("WARN {}: No prefab overrides found", name);
                        empty_count += 1;
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    println!("FAIL {}: Parse error: {}", name, e);
                    fail_count += 1;
                }
            }
        }

        println!(
            "\nParsing results: {} succeeded, {} empty, {} failed out of {} files",
            success_count,
            empty_count,
            fail_count,
            prefab_files.len()
        );

        assert!(fail_count == 0, "Some files failed to parse");
    }

    #[test]
    fn test_deer_debug() {
        let source = include_str!("../../../examples/prefabs/deer.lua");
        let parser = PrefabOverrideParser::new(source).unwrap();
        let result = parser.parse().unwrap();
        for name in ["deer", "deer_red", "deer_blue"] {
            assert!(
                result.iter().any(|r| r.prefab_name == name),
                "missing {name}: {result:?}"
            );
        }
        assert!(
            !result
                .iter()
                .any(|r| r.prefab_name == "red" || r.prefab_name == "blue"),
            "wrapper arguments must not become prefab names: {result:?}"
        );
    }
}
