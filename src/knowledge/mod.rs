//! 符号知识文档管线(构想 a/b/c,见 docs/KNOWLEDGE_PIPELINE.md)。
//!
//! - [`types`]:SymbolDoc 契约
//! - [`store`]:knowledge/ 目录读写、sha 增量判据
//! - [`scan_symbols`]:M1 — LLM 阅读源码产出 component 知识文档
//! - [`scan_wiki`]:M2a — PageSymbolMap 确定性骨架(路由/反转/数值配对)

pub mod auto_infobox;
pub mod page_assist;
pub mod scan_symbols;
pub mod scan_wiki;
pub mod store;
pub mod types;

pub use page_assist::{run_page_assist, PageAssistParams};
pub use scan_symbols::{run_scan_symbols, ScanSymbolsParams};
pub use scan_wiki::{run_scan_wiki, ScanWikiParams};
pub use types::{SymbolDoc, SymbolDocLlm, SymbolRefKey, PROMPT_REV, SCHEMA_VERSION};
