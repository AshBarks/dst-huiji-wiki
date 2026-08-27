//! 符号知识文档管线(构想 a/b/c,见 docs/KNOWLEDGE_PIPELINE.md)。
//!
//! - [`types`]:SymbolDoc 契约
//! - [`store`]:knowledge/ 目录读写、sha 增量判据
//! - [`scan_symbols`]:M1 — LLM 阅读源码产出 component 知识文档

pub mod scan_symbols;
pub mod store;
pub mod types;

pub use scan_symbols::{run_scan_symbols, ScanSymbolsParams};
pub use types::{SymbolDoc, SymbolDocLlm, SymbolRefKey, PROMPT_REV, SCHEMA_VERSION};
