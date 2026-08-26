//! Core data types for the code association index.
//!
//! All paths are relative to the game scripts root using `/` separators
//! (e.g. `prefabs/hound.lua`, `components/combat.lua`).

use std::collections::BTreeMap;

use serde::Serialize;

/// Relative file key inside the scripts tree (`prefabs/hound.lua`).
pub type FileKey = String;

/// Role of a scanned file, derived from its location in the scripts tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Role {
    Prefab,
    Component,
    StateGraph,
    Brain,
    Behaviour,
    /// Root-level helper files (`standardcomponents.lua`, `prefabutil.lua`).
    Util,
    Other,
}

/// A literal value bound to a `local` name.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ConstVal {
    Num(String),
    Str(String),
}

/// A named function definition with source range.
#[derive(Debug, Clone, Serialize)]
pub struct FnDef {
    /// `fncommon` for locals; `Combat:GetDamage` style full text for globals.
    pub name: String,
    pub params: Vec<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    /// 1-based inclusive line range (0 when unknown).
    pub start_line: u32,
    pub end_line: u32,
    pub is_local: bool,
}

/// A global assignment `Name = Class(...)` exporting a constructor whose
/// parameter list is taken from the anonymous function argument of `Class`.
#[derive(Debug, Clone, Serialize)]
pub struct ExportedCtor {
    pub name: String,
    pub params: Vec<String>,
}

/// One argument expression of a call, classified during the AST walk.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ArgExpr {
    Str(String),
    Num(String),
    Ident(String),
    /// Field access such as `data.stategraph` (dynamic, not resolvable statically).
    Field(String),
    Or(Box<ArgExpr>, Box<ArgExpr>),
    And(Box<ArgExpr>, Box<ArgExpr>),
    /// Anonymous function literal (callback / location provider).
    FnRef,
    /// Anything else, including the `nil` literal.
    Other,
}

/// Calls relevant to association building, recorded per file.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallKind {
    AddComponent,
    SetStateGraph,
    SetBrain,
    /// `<alias>.components.<name>:<method>(...)`; only counted as an override
    /// when the receiver resolves to the constructed entity.
    ComponentMethod {
        component: String,
        method: String,
    },
    /// `MakeXxx(...)` helper invocation in a prefab file.
    Helper {
        name: String,
    },
    /// Lowercase local-function call in a prefab file (param-flow graph edge).
    LocalFnCall {
        callee: String,
    },
    /// Capitalized constructor call in a brain file (behaviour node candidate).
    CtorCall {
        name: String,
    },
}

/// A recorded call site.
#[derive(Debug, Clone, Serialize)]
pub struct AssocCall {
    pub kind: CallKind,
    pub args: Vec<ArgExpr>,
    /// Chain of enclosing *named* function names, outermost first.
    pub scope: Vec<String>,
    /// True when the receiver is the entity under construction (`inst` or a
    /// known alias). Always true for plain/ctor/helper calls.
    pub receiver_is_inst: bool,
    pub line: u32,
}

/// `return Prefab("hound", fndefault, assets, prefabs)` registration.
#[derive(Debug, Clone, Serialize)]
pub struct PrefabReg {
    /// Literal prefab name when statically known.
    pub name: Option<String>,
    /// Name of the variable holding the constructor function.
    pub fn_ref: Option<String>,
    /// Name of the deps table variable (4th argument), if any.
    pub deps_var: Option<String>,
    pub line: u32,
}

/// Everything Pass 1 collects from one file.
#[derive(Debug, Clone, Serialize)]
pub struct FileScan {
    pub path: FileKey,
    pub role: Role,
    /// `local brain = require("brains/houndbrain")` bindings.
    pub requires: Vec<(String, String)>,
    /// File-local numeric/string constants (`local SEE_DIST = 30`).
    pub consts: BTreeMap<String, ConstVal>,
    pub fns: Vec<FnDef>,
    pub exports: Vec<ExportedCtor>,
    /// Tables of string literals (`local prefabs = {"a", "b"}`).
    pub dep_tables: BTreeMap<String, Vec<String>>,
    pub prefab_regs: Vec<PrefabReg>,
    pub calls: Vec<AssocCall>,
    pub state_count: u32,
    pub event_handler_count: u32,
    pub parse_ok: bool,
}

impl FileScan {
    pub fn new(path: FileKey, role: Role) -> Self {
        Self {
            path,
            role,
            requires: Vec::new(),
            consts: BTreeMap::new(),
            fns: Vec::new(),
            exports: Vec::new(),
            dep_tables: BTreeMap::new(),
            prefab_regs: Vec::new(),
            calls: Vec::new(),
            state_count: 0,
            event_handler_count: 0,
            parse_ok: false,
        }
    }

    pub fn require_path(&self, var: &str) -> Option<&str> {
        self.requires
            .iter()
            .find(|(name, _)| name == var)
            .map(|(_, p)| p.as_str())
    }

    pub fn fn_def(&self, name: &str) -> Option<&FnDef> {
        self.fns.iter().find(|f| f.name == name)
    }
}
