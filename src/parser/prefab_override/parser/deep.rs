//! 跨函数边界深度解析（由 parser.rs 机械拆分，行为不变）。

use std::collections::HashMap;

use full_moon::ast;

use super::super::types::{OverrideValue, PrefabNameOverride};
use super::extract_string_literal;
use super::PrefabOverrideParser;

impl PrefabOverrideParser {
    // ============================================================================
    // Section 8: Function Body Prefab Analysis with Tracking
    // ============================================================================
    pub(super) fn find_prefabs_in_function_body_with_fn_tracking_and_params(
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

    pub(super) fn find_table_insert_prefabs_in_function_body_with_params(
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

    pub(super) fn find_table_insert_prefabs_in_block_with_params(
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

    pub(super) fn analyze_prefab_call_with_tables(
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

    pub(super) fn extract_prefab_name_from_arg(
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
    // Section 10: Prefab Call Analysis with Local Functions and Parameters
    // ============================================================================
    pub(super) fn analyze_prefab_call_with_local_fns_and_params(
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

    pub(super) fn extract_string_from_arg_with_params(
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
    pub(super) fn find_override_in_fn_arg_with_local_fns_and_params(
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

    pub(super) fn find_override_in_function_body_deep_with_local_fns_and_params(
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

    pub(super) fn find_override_in_function_body_deep_with_params(
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

    pub(super) fn find_override_in_function_body_with_params(
        &self,
        body: &ast::FunctionBody,
        param_values: &HashMap<String, String>,
    ) -> Option<OverrideValue> {
        let mut visited = Vec::new();
        self.find_override_in_function_body_deep_with_params(body, &mut visited, param_values)
    }

    pub(super) fn find_override_in_stmt_deep_with_local_fns_and_params(
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

    pub(super) fn find_override_in_expr_deep_with_local_fns_and_params(
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

    pub(super) fn try_find_override_in_call_chain_with_local_fns_and_params(
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

    pub(super) fn extract_override_from_call_with_params(
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

    pub(super) fn find_prefab_calls(&self) -> Vec<ast::FunctionCall> {
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

    pub(super) fn is_prefab_call(&self, call: &ast::FunctionCall) -> bool {
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
    pub(super) fn find_factory_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();
        self.find_factory_calls_in_block(self.ast.nodes(), &mut calls);
        calls
    }

    pub(super) fn find_factory_calls_in_block(
        &self,
        block: &ast::Block,
        calls: &mut Vec<ast::FunctionCall>,
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

    pub(super) fn is_known_function_call(&self, call: &ast::FunctionCall) -> bool {
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
    pub(super) fn body_registers_prefab(&self, body: &ast::FunctionBody) -> bool {
        self.block_registers_prefab(body.block())
    }

    pub(super) fn block_registers_prefab(&self, block: &ast::Block) -> bool {
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

    pub(super) fn expr_registers_prefab(&self, expr: &ast::Expression) -> bool {
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

    pub(super) fn call_registers_prefab(&self, call: &ast::FunctionCall) -> bool {
        self.is_prefab_call(call) || self.is_table_insert_with_prefab(call)
    }

    pub(super) fn call_args_register_prefab(&self, call: &ast::FunctionCall) -> bool {
        self.get_call_args(call)
            .map(|args| args.iter().any(|arg| self.expr_registers_prefab(arg)))
            .unwrap_or(false)
    }

    pub(super) fn find_factory_calls_in_block_with_iter_tables(
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

    pub(super) fn find_factory_calls_in_expr_with_iter_tables(
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

    pub(super) fn find_ipairs_factory_calls(
        &self,
    ) -> Vec<(ast::FunctionCall, HashMap<String, ast::Expression>)> {
        let mut results = Vec::new();
        self.find_ipairs_factory_calls_in_block(self.ast.nodes(), &mut results, &HashMap::new());
        results
    }

    pub(super) fn find_ipairs_factory_calls_in_block(
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

    pub(super) fn extract_table_from_ipairs(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<ast::Expression> {
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

    pub(super) fn find_factory_calls_in_expr(
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
    pub(super) fn is_factory_call(&self, call: &ast::FunctionCall) -> bool {
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

    pub(super) fn analyze_prefab_call(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<PrefabNameOverride> {
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

    pub(super) fn get_call_args(&self, call: &ast::FunctionCall) -> Option<Vec<ast::Expression>> {
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

    pub(super) fn find_override_in_fn_arg(&self, arg: &ast::Expression) -> Option<OverrideValue> {
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

    pub(super) fn find_override_in_function_body(
        &self,
        body: &ast::FunctionBody,
    ) -> Option<OverrideValue> {
        for stmt in body.block().stmts() {
            if let Some(override_val) = self.find_override_in_stmt(stmt) {
                return Some(override_val);
            }
        }
        None
    }

    pub(super) fn find_override_in_function_body_deep(
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

    pub(super) fn find_override_in_stmt(&self, stmt: &ast::Stmt) -> Option<OverrideValue> {
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
}
