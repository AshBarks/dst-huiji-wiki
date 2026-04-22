use crate::Result;
use full_moon::ast::{self, Ast};
use full_moon::node::Node;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct SourceLocation {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabNameOverride {
    pub prefab_name: String,
    pub override_name: OverrideValue,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverrideValue {
    Static(String),
    Dynamic(String),
    Unknown,
}

impl OverrideValue {
    pub fn value(&self) -> Option<&str> {
        match self {
            OverrideValue::Static(s) => Some(s),
            OverrideValue::Dynamic(s) => Some(s),
            OverrideValue::Unknown => None,
        }
    }
}

#[derive(Debug, Clone)]
struct FunctionInfo {
    body: ast::FunctionBody,
}

#[derive(Debug, Clone)]
struct VariableValue {
    value: Option<String>,
}

pub struct PrefabOverrideParser {
    source: String,
    ast: Ast,
    functions: HashMap<String, FunctionInfo>,
    variables: HashMap<String, VariableValue>,
}

impl PrefabOverrideParser {
    pub fn new(source: &str) -> Result<Self> {
        let ast = full_moon::parse(source)
            .map_err(|e| crate::Error::ParseError(format!("Lua parse error: {:?}", e)))?;

        let mut parser = Self {
            source: source.to_string(),
            ast,
            functions: HashMap::new(),
            variables: HashMap::new(),
        };

        parser.collect_definitions();
        Ok(parser)
    }

    fn collect_definitions(&mut self) {
        for stmt in self.ast.nodes().stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assignment) => {
                    let name_list = assignment.names();
                    let expr_list = assignment.expressions();
                    for (name, expr) in name_list.iter().zip(expr_list.iter()) {
                        let var_name = name.token().to_string();
                        if let Some(value) = self.extract_string_value(expr) {
                            self.variables.insert(
                                var_name,
                                VariableValue {
                                    value: Some(value),
                                },
                            );
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
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    let name = func_decl.name().to_string();
                    self.functions.insert(
                        name,
                        FunctionInfo {
                            body: func_decl.body().clone(),
                        },
                    );
                }
                _ => {}
            }
        }
    }

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
                    let right = self.extract_string_value(rhs).unwrap_or_default();
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

        Ok(results)
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

        if let Some(last_stmt) = self.ast.nodes().last_stmt() {
            if let ast::LastStmt::Return(ret) = last_stmt {
                for expr in ret.returns().iter() {
                    if let ast::Expression::FunctionCall(call) = expr {
                        if self.is_prefab_call(call) {
                            calls.push(call.clone());
                        }
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

    fn find_factory_calls(&self) -> Vec<ast::FunctionCall> {
        let mut calls = Vec::new();

        for stmt in self.ast.nodes().stmts() {
            if let ast::Stmt::FunctionCall(call) = stmt {
                if self.is_factory_call(call) {
                    calls.push(call.clone());
                }
            }
        }

        if let Some(last_stmt) = self.ast.nodes().last_stmt() {
            if let ast::LastStmt::Return(ret) = last_stmt {
                for expr in ret.returns().iter() {
                    if let ast::Expression::FunctionCall(call) = expr {
                        if self.is_factory_call(call) {
                            calls.push(call.clone());
                        }
                    }
                }
            }
        }

        calls
    }

    fn is_factory_call(&self, call: &ast::FunctionCall) -> bool {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let name_str = name.token().to_string();
            matches!(
                name_str.as_str(),
                "MakeBundle" | "MakeWrap" | "MakeContainer"
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

    fn get_call_args(
        &self,
        call: &ast::FunctionCall,
    ) -> Option<Vec<ast::Expression>> {
        let suffixes: Vec<_> = call.suffixes().collect();
        if suffixes.len() != 1 {
            return None;
        }

        match &suffixes[0] {
            ast::Suffix::Call(ast::Call::AnonymousCall(args)) => match args {
                ast::FunctionArgs::Parentheses { arguments, .. } => {
                    Some(arguments.iter().cloned().collect())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn extract_string_from_arg(&self, arg: &ast::Expression) -> Option<String> {
        match arg {
            ast::Expression::String(s) => Some(extract_string_literal(&s.to_string())),
            ast::Expression::Var(var) => {
                if let ast::Var::Name(name) = var {
                    let var_name = name.token().to_string();
                    self.variables.get(&var_name).and_then(|v| v.value.clone())
                } else {
                    None
                }
            }
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
            ast::Expression::Function(func) => {
                self.find_override_in_function_body(func.body())
            }
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
        for stmt in body.block().stmts() {
            if let Some(override_val) = self.find_override_in_stmt_deep(stmt, visited) {
                return Some(override_val);
            }
        }
        None
    }

    fn find_override_in_stmt_deep(
        &self,
        stmt: &ast::Stmt,
        visited: &mut Vec<String>,
    ) -> Option<OverrideValue> {
        match stmt {
            ast::Stmt::FunctionCall(call) => {
                if self.is_set_prefab_name_override_call(call) {
                    self.extract_override_from_call(call)
                } else {
                    self.try_find_override_in_call_chain(call, visited)
                }
            }
            ast::Stmt::LocalAssignment(assignment) => {
                for expr in assignment.expressions().iter() {
                    if let Some(override_val) = self.find_override_in_expr_deep(expr, visited) {
                        return Some(override_val);
                    }
                }
                None
            }
            ast::Stmt::If(if_stmt) => {
                for stmt in if_stmt.block().stmts() {
                    if let Some(override_val) = self.find_override_in_stmt_deep(stmt, visited) {
                        return Some(override_val);
                    }
                }
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        for stmt in else_if.block().stmts() {
                            if let Some(override_val) = self.find_override_in_stmt_deep(stmt, visited)
                            {
                                return Some(override_val);
                            }
                        }
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    for stmt in else_block.stmts() {
                        if let Some(override_val) = self.find_override_in_stmt_deep(stmt, visited) {
                            return Some(override_val);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn find_override_in_expr_deep(
        &self,
        expr: &ast::Expression,
        visited: &mut Vec<String>,
    ) -> Option<OverrideValue> {
        match expr {
            ast::Expression::FunctionCall(call) => {
                self.try_find_override_in_call_chain(call, visited)
            }
            _ => None,
        }
    }

    fn try_find_override_in_call_chain(
        &self,
        call: &ast::FunctionCall,
        visited: &mut Vec<String>,
    ) -> Option<OverrideValue> {
        let prefix = call.prefix();
        if let ast::Prefix::Name(name) = prefix {
            let fn_name = name.token().to_string();
            if visited.contains(&fn_name) {
                return None;
            }
            if let Some(func_info) = self.functions.get(&fn_name) {
                visited.push(fn_name);
                let result = self.find_override_in_function_body_deep(&func_info.body, visited);
                visited.pop();
                return result;
            }
        }
        None
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
            if let ast::Expression::Var(var) = arg {
                if let ast::Var::Name(name) = var {
                    return Some(name.token().to_string());
                }
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

    fn get_call_location(&self, call: &ast::FunctionCall) -> SourceLocation {
        let start_byte = call
            .start_position()
            .map(|p| p.bytes())
            .unwrap_or(0);
        let end_byte = call
            .end_position()
            .map(|p| p.bytes())
            .unwrap_or(0);

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
        let source = include_str!("../../examples/prefabs/altar_prototyper.lua");
        let result = parse_prefab_overrides(source).unwrap();

        assert!(result.len() >= 2, "Expected at least 2 prefab overrides");

        let ancient_altar = result.iter().find(|r| r.prefab_name == "ancient_altar");
        let ancient_altar_broken = result
            .iter()
            .find(|r| r.prefab_name == "ancient_altar_broken");

        assert!(
            ancient_altar.is_some(),
            "Should find ancient_altar prefab"
        );
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
        let source = include_str!("../../examples/prefabs/bundle.lua");
        let result = parse_prefab_overrides(source).unwrap();

        let redpouch_yotp = result.iter().find(|r| r.prefab_name == "redpouch_yotp");
        let redpouch_yotc = result.iter().find(|r| r.prefab_name == "redpouch_yotc");
        let redpouch_yotb = result.iter().find(|r| r.prefab_name == "redpouch_yotb");

        assert!(
            redpouch_yotp.is_some(),
            "Should find redpouch_yotp prefab"
        );
        assert!(
            redpouch_yotc.is_some(),
            "Should find redpouch_yotc prefab"
        );
        assert!(
            redpouch_yotb.is_some(),
            "Should find redpouch_yotb prefab"
        );

        for prefab in ["redpouch_yotp", "redpouch_yotc", "redpouch_yotb", "redpouch_yotr", "redpouch_yotd", "redpouch_yoth", "redpouch_yot_catcoon"] {
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
}
