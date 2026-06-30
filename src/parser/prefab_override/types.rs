use full_moon::ast;

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
pub(crate) struct FunctionInfo {
    pub body: ast::FunctionBody,
}

#[derive(Debug, Clone)]
pub(crate) struct VariableValue {
    pub value: Option<String>,
}
