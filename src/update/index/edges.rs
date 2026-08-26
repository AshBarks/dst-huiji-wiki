//! Resolved association edges and the final index artifact.

use std::collections::BTreeMap;

use serde::Serialize;

use super::symbols::FileKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum EdgeKind {
    Component,
    StateGraph,
    Brain,
    PrefabDep,
    Behaviour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Confidence {
    /// Single statically-known target.
    Direct,
    /// Multiple targets from `and`/`or` folding or parameter flow.
    Folded,
    /// Expanded from a `MakeXxx` helper profile.
    HelperExpanded,
}

/// One resolved association edge, attributed to every prefab variant that
/// owns the enclosing function scope.
#[derive(Debug, Clone, Serialize)]
pub struct AssocEdge {
    pub kind: EdgeKind,
    pub prefab_file: FileKey,
    pub prefab_variant: String,
    /// Normalized target path (`components/combat.lua`, `stategraphs/SGhound.lua`,
    /// `brains/houndbrain.lua`) or the raw prefab name for `PrefabDep`.
    pub target: String,
    pub anchor_line: u32,
    pub confidence: Confidence,
    /// Helper name when [`Confidence::HelperExpanded`].
    pub via: Option<String>,
}

/// A component method invoked on the constructed entity (override evidence).
#[derive(Debug, Clone, Serialize)]
pub struct OverrideMark {
    pub component: String,
    pub method: String,
    pub line: u32,
}

/// A behaviour constructor call inside a brain file with its arguments
/// aligned to the ctor signature (`self`/`inst` leading params skipped).
#[derive(Debug, Clone, Serialize)]
pub struct BehaviourCallRecord {
    pub brain_file: FileKey,
    pub ctor: String,
    pub line: u32,
    pub args: Vec<(String, ArgValue)>,
    /// Prefab variants whose brain chain reaches this file.
    pub prefab_variants: Vec<String>,
}

/// Statically resolved value of a behaviour call argument.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ArgValue {
    Str(String),
    Num(String),
    FnRef,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnresolvedNote {
    pub file: FileKey,
    pub line: u32,
    pub kind: &'static str,
    pub detail: String,
}

/// Usage tier of a component by number of distinct prefab variants using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Tier {
    Universal,
    Common,
    Niche,
    Singleton,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentTier {
    pub component: String,
    pub prefab_count: usize,
    pub tier: Tier,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GenericityReport {
    pub components: Vec<ComponentTier>,
    /// Components excluded from page-fact extraction by L0 heuristics.
    pub blacklisted: Vec<String>,
}

/// Final output of one index build.
#[derive(Debug, Clone, Serialize, Default)]
pub struct IndexArtifact {
    /// Number of files scanned and how many failed to parse.
    pub scanned_files: usize,
    pub parse_failures: Vec<UnresolvedNote>,
    pub edges: Vec<AssocEdge>,
    /// Reverse index: target file -> owning prefab variants.
    pub reverse: BTreeMap<FileKey, Vec<String>>,
    /// Component overrides per prefab file (for blast-radius narrowing).
    pub overrides: BTreeMap<FileKey, Vec<OverrideMark>>,
    pub behaviour_calls: Vec<BehaviourCallRecord>,
    pub unresolved: Vec<UnresolvedNote>,
    pub genericity: GenericityReport,
}
