//! 平台层：与具体业务无关的基础设施。
//!
//! 依赖方向：`features（parser/models/wiki/corpus/scripts_sync/…）→ platform`；
//! `service → features`。platform 不依赖 service 与任何 feature 模块，
//! 底层功能模块因此不再反向依赖应用层（此前 13 个文件
//! `use crate::service::Reporter` 即依赖倒置）。

pub mod config;
pub mod fs;
pub mod progress;

pub use progress::{
    decide_write, CaptureReporter, ConfirmMode, JobEvent, Reporter, StdoutReporter, WriteDecision,
    WriteMode,
};
