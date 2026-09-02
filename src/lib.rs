pub mod context;
pub mod copyclip;
pub mod corpus;
pub mod error;
pub mod knowledge;
pub mod llm;
pub mod mapping;
pub mod models;
pub mod parser;
pub mod scripts_sync;
pub mod service;
pub mod update;
pub mod utils;
pub mod wiki;

pub use context::DstContext;
pub use context::SnapshotInfo;
pub use copyclip::{
    process_copyclip, process_copyclip_range, CopyClipConfig, CopyClipMapping, CopyClipMappings,
    CopyClipProcessor, CopyClipResult, MarkerRange,
};
pub use error::{Error, Result};
pub use models::TechReport;
pub use utils::{count_diff_stats, diff_lines, diff_lines_preserve_whitespace};
