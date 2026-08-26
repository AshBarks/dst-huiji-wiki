//! Update impact assessment support (see docs/UPDATE_IMPACT_PLAN.md).
//!
//! Currently hosts the code association index (infrastructure A,
//! docs/CODE_ASSOCIATION_INFRA.md); snapshot/diff machinery lands with M1.

pub mod index;

pub use index::{build_from_dir, build_from_sources, IndexArtifact};
