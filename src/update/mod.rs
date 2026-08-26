//! Update impact assessment support (see docs/UPDATE_IMPACT_PLAN.md).
//!
//! Hosts the code association atlas (infrastructure A,
//! docs/CODE_ASSOCIATION_INFRA.md) plus M1 snapshot management and the
//! structured tree diff.

pub mod annotate;
pub mod consts;
pub mod diffdata;
pub mod fact;
pub mod grade;
pub mod impact;
pub mod index;
pub mod rules;
pub mod snapshot;
pub mod state;
pub mod stats;
pub mod takeup;

pub use annotate::{annotate_file, batch_annotate};
pub use consts::{collect_brain_consts, pair_const_changes};
pub use diffdata::{DiffStatus, FileDiff, Hunk, TreeDiff};
pub use fact::{
    collect_stat_records, pair_loot_changes, pair_stat_changes, EvidenceRef, FactChange, FactKind,
    Literal,
};
pub use grade::{
    grade_changes, summarize as summarize_grades, CorpusPageView, GradeTier, GradedChange,
};
pub use impact::{build_report, FileImpact, ImpactReport, TuningDiff};
pub use index::{
    build_atlas_from_dir, build_from_dir, build_from_sources, AtlasBuild, IndexArtifact,
    TuningTable,
};
pub use rules::{default_rules, evaluate as evaluate_rules, RuleHit, Tier0Rule};
pub use snapshot::SnapshotStore;
pub use stats::{extract_stats, StatFact, StatKind};
pub use takeup::{TakeupConfig, Tier};
