use crate::Result;
use full_moon::ast::{self, Ast};

#[derive(Debug, Clone)]
pub struct VariableLocation {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub content: String,
    pub is_local: bool,
}

#[derive(Debug, Clone)]
pub struct FieldLocation {
    pub path: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct VariableRange {
    pub start_var_name: String,
    pub end_var_name: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct LuaParser;

impl LuaParser {
    pub fn new() -> Self {
        Self
    }

    pub fn locate_variable(source: &str, var_name: &str) -> Result<VariableLocation> {
        let ast = full_moon::parse(source).map_err(|e| {
            crate::Error::ParseError(format!("Lua parse error: {:?}", e))
        })?;

        Self::find_variable_in_ast(&ast, source, var_name)
    }

    pub fn locate_field_assignment(source: &str, field_path: &str) -> Result<FieldLocation> {
        let ast = full_moon::parse(source).map_err(|e| {
            crate::Error::ParseError(format!("Lua parse error: {:?}", e))
        })?;

        Self::find_field_assignment_in_ast(&ast, source, field_path)
    }

    pub fn locate_field_assignment_range(
        source: &str,
        start_field_path: &str,
        end_field_path: &str,
    ) -> Result<FieldLocation> {
        let ast = full_moon::parse(source).map_err(|e| {
            crate::Error::ParseError(format!("Lua parse error: {:?}", e))
        })?;

        let start_location = Self::find_field_assignment_in_ast(&ast, source, start_field_path)?;
        let end_location = Self::find_field_assignment_in_ast(&ast, source, end_field_path)?;

        if end_location.start_byte < start_location.end_byte {
            return Err(crate::Error::ParseError(format!(
                "End field '{}' must appear after start field '{}'",
                end_field_path, start_field_path
            )));
        }

        let content = source[start_location.start_byte..end_location.end_byte].to_string();

        Ok(FieldLocation {
            path: format!("{}..{}", start_field_path, end_field_path),
            start_byte: start_location.start_byte,
            end_byte: end_location.end_byte,
            content,
        })
    }

    fn find_field_assignment_in_ast(ast: &Ast, source: &str, field_path: &str) -> Result<FieldLocation> {
        for stmt in ast.nodes().stmts() {
            if let ast::Stmt::Assignment(assignment) = stmt {
                for var in assignment.variables().iter() {
                    if let ast::Var::Expression(var_expr) = var {
                        let var_str = var_expr.to_string();
                        let var_str_trimmed = var_str.trim();
                        if var_str_trimmed == field_path {
                            return Self::extract_field_assignment_location(
                                assignment, source, field_path,
                            );
                        }
                    }
                }
            }
        }

        Err(crate::Error::ParseError(format!(
            "Field assignment '{}' not found",
            field_path
        )))
    }

    fn extract_field_assignment_location(
        assignment: &ast::Assignment,
        source: &str,
        field_path: &str,
    ) -> Result<FieldLocation> {
        let actual_start = source.find(field_path).unwrap_or(0);

        let end_byte = if let Some(last_expr) = assignment.expressions().iter().last() {
            Self::find_expression_end_in_source(last_expr, source)
        } else {
            actual_start + field_path.len()
        };

        let content = source[actual_start..end_byte].to_string();

        Ok(FieldLocation {
            path: field_path.to_string(),
            start_byte: actual_start,
            end_byte,
            content,
        })
    }

    pub fn locate_variable_range(
        source: &str,
        start_var_name: &str,
        end_var_name: Option<&str>,
    ) -> Result<VariableRange> {
        let ast = full_moon::parse(source).map_err(|e| {
            crate::Error::ParseError(format!("Lua parse error: {:?}", e))
        })?;

        let start_location = Self::find_variable_in_ast(&ast, source, start_var_name)?;

        let (end_byte, end_var_name_str) = if let Some(end_name) = end_var_name {
            let end_location = Self::find_variable_in_ast(&ast, source, end_name)?;
            if end_location.start_byte < start_location.end_byte {
                return Err(crate::Error::ParseError(format!(
                    "End variable '{}' must appear after start variable '{}'",
                    end_name, start_var_name
                )));
            }
            (end_location.end_byte, Some(end_name.to_string()))
        } else {
            (start_location.end_byte, None)
        };

        let content = source[start_location.start_byte..end_byte].to_string();

        Ok(VariableRange {
            start_var_name: start_var_name.to_string(),
            end_var_name: end_var_name_str,
            start_byte: start_location.start_byte,
            end_byte,
            content,
        })
    }

    fn find_variable_in_ast(ast: &Ast, source: &str, var_name: &str) -> Result<VariableLocation> {
        for stmt in ast.nodes().stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assignment) => {
                    for name in assignment.names().iter() {
                        let token_name = name.token().to_string();
                        if token_name == var_name {
                            return Self::extract_local_assignment_location(
                                assignment, source, var_name, true,
                            );
                        }
                    }
                }
                ast::Stmt::Assignment(assignment) => {
                    for var in assignment.variables().iter() {
                        if let ast::Var::Name(name) = var {
                            if name.token().to_string() == var_name {
                                return Self::extract_assignment_location(
                                    assignment, source, var_name, false,
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Err(crate::Error::ParseError(format!(
            "Variable '{}' not found",
            var_name
        )))
    }

    fn extract_local_assignment_location(
        assignment: &ast::LocalAssignment,
        source: &str,
        var_name: &str,
        is_local: bool,
    ) -> Result<VariableLocation> {
        let local_token = assignment.local_token();
        let start_byte = local_token.token().start_position().bytes();

        let end_byte = if let Some(last_expr) = assignment.expressions().iter().last() {
            Self::find_expression_end_in_source(last_expr, source)
        } else if let Some(last_name) = assignment.names().iter().last() {
            last_name.token().end_position().bytes()
        } else {
            local_token.token().end_position().bytes()
        };

        let content = source[start_byte..end_byte].to_string();

        Ok(VariableLocation {
            name: var_name.to_string(),
            start_byte,
            end_byte,
            content,
            is_local,
        })
    }

    fn extract_assignment_location(
        assignment: &ast::Assignment,
        source: &str,
        var_name: &str,
        is_local: bool,
    ) -> Result<VariableLocation> {
        let start_byte = if let Some(first_var) = assignment.variables().iter().next() {
            if let ast::Var::Name(name) = first_var {
                name.token().start_position().bytes()
            } else {
                0
            }
        } else {
            0
        };

        let end_byte = if let Some(last_expr) = assignment.expressions().iter().last() {
            Self::find_expression_end_in_source(last_expr, source)
        } else if let Some(last_var) = assignment.variables().iter().last() {
            if let ast::Var::Name(name) = last_var {
                name.token().end_position().bytes()
            } else {
                start_byte
            }
        } else {
            start_byte
        };

        let content = source[start_byte..end_byte].to_string();

        Ok(VariableLocation {
            name: var_name.to_string(),
            start_byte,
            end_byte,
            content,
            is_local,
        })
    }

    fn find_expression_end_in_source(expr: &ast::Expression, source: &str) -> usize {
        let expr_str = expr.to_string();
        
        if let Some(pos) = source.find(&expr_str) {
            return pos + expr_str.len();
        }
        
        0
    }
}

impl Default for LuaParser {
    fn default() -> Self {
        Self::new()
    }
}

pub fn extract_variable(source: &str, var_name: &str) -> Result<VariableLocation> {
    LuaParser::locate_variable(source, var_name)
}

pub fn extract_variable_range(
    source: &str,
    start_var_name: &str,
    end_var_name: Option<&str>,
) -> Result<VariableRange> {
    LuaParser::locate_variable_range(source, start_var_name, end_var_name)
}

pub fn extract_field_assignment(source: &str, field_path: &str) -> Result<FieldLocation> {
    LuaParser::locate_field_assignment(source, field_path)
}

pub fn extract_field_assignment_range(
    source: &str,
    start_field_path: &str,
    end_field_path: &str,
) -> Result<FieldLocation> {
    LuaParser::locate_field_assignment_range(source, start_field_path, end_field_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locate_local_table() {
        let source = r#"local RECIPE_BUILDER_TAG_LOOKUP = {
    balloonomancer = "wes",
    basicengineer = "winona",
}
"#;
        let result = LuaParser::locate_variable(source, "RECIPE_BUILDER_TAG_LOOKUP");
        assert!(result.is_ok());
        let location = result.unwrap();
        assert!(location.is_local);
        assert!(location.content.contains("balloonomancer"));
        assert!(location.content.contains("winona"));
    }

    #[test]
    fn test_locate_global_table() {
        let source = r#"RECIPE_BUILDER_TAG_LOOKUP = {
    balloonomancer = "wes",
}
"#;
        let result = LuaParser::locate_variable(source, "RECIPE_BUILDER_TAG_LOOKUP");
        assert!(result.is_ok());
        let location = result.unwrap();
        assert!(!location.is_local);
    }

    #[test]
    fn test_variable_not_found() {
        let source = r#"local x = 1"#;
        let result = LuaParser::locate_variable(source, "NOT_EXIST");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_variable_range_single() {
        let source = r#"local FOO = {
    a = 1,
}
"#;
        let result = LuaParser::locate_variable_range(source, "FOO", None);
        assert!(result.is_ok());
        let range = result.unwrap();
        assert!(range.content.contains("FOO"));
        assert!(range.content.contains("a = 1"));
        assert!(range.end_var_name.is_none());
    }

    #[test]
    fn test_extract_variable_range_multiple() {
        let source = r#"local START_VAR = {
    a = 1,
}

local MIDDLE_VAR = {
    b = 2,
}

local END_VAR = {
    c = 3,
}
"#;
        let result = LuaParser::locate_variable_range(source, "START_VAR", Some("END_VAR"));
        assert!(result.is_ok());
        let range = result.unwrap();
        assert!(range.content.contains("START_VAR"));
        assert!(range.content.contains("MIDDLE_VAR"));
        assert!(range.content.contains("END_VAR"));
        assert_eq!(range.end_var_name, Some("END_VAR".to_string()));
    }

    #[test]
    fn test_extract_variable_range_invalid_order() {
        let source = r#"local FIRST = 1
local SECOND = 2
"#;
        let result = LuaParser::locate_variable_range(source, "SECOND", Some("FIRST"));
        assert!(result.is_err());
    }

    #[test]
    fn test_locate_field_assignment() {
        let source = r#"CRAFTING_FILTERS.CHARACTER.recipes = {
    { "character", "wilson" },
}
"#;
        let result = LuaParser::locate_field_assignment(source, "CRAFTING_FILTERS.CHARACTER.recipes");
        assert!(result.is_ok());
        let location = result.unwrap();
        assert!(location.content.contains("CRAFTING_FILTERS.CHARACTER.recipes"));
        assert!(location.content.contains("wilson"));
    }

    #[test]
    fn test_locate_field_assignment_range() {
        let source = r#"CRAFTING_FILTERS.CHARACTER.recipes = {
    { "character", "wilson" },
}

CRAFTING_FILTERS.DECOR.recipes = {
    { "decor", "item" },
}
"#;
        let result = LuaParser::locate_field_assignment_range(
            source,
            "CRAFTING_FILTERS.CHARACTER.recipes",
            "CRAFTING_FILTERS.DECOR.recipes",
        );
        assert!(result.is_ok());
        let location = result.unwrap();
        assert!(location.content.contains("CRAFTING_FILTERS.CHARACTER.recipes"));
        assert!(location.content.contains("CRAFTING_FILTERS.DECOR.recipes"));
    }
}
