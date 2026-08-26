//! Update impact assessment support (see docs/UPDATE_IMPACT_PLAN.md).
//!
//! Hosts the code association atlas (infrastructure A,
//! docs/CODE_ASSOCIATION_INFRA.md) plus M1 snapshot management and the
//! structured tree diff.

pub mod diffdata;
pub mod impact;
pub mod index;
pub mod snapshot;

pub use diffdata::{DiffStatus, FileDiff, Hunk, TreeDiff};
pub use impact::{build_report, FileImpact, ImpactReport, TuningDiff};
pub use index::{
    build_atlas_from_dir, build_from_dir, build_from_sources, AtlasBuild, IndexArtifact,
    TuningTable,
};
pub use snapshot::SnapshotStore;
