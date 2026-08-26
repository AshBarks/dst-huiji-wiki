//! Corpus data models: per-page metadata, classification labels and the
//! per-round manifest.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version/type label assigned by the classifier (see `classify.rs`).
///
/// The corpus keeps every main-namespace page; this label only decides how
/// downstream consumers (segmenter / Atlas) may treat a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameClass {
    /// Redirect page — kept for its alias → canonical-page mapping.
    Redirect,
    /// Disambiguation page (`{{消歧义…}}`).
    Disambig,
    /// Aggregation page covering both game versions.
    Mixed,
    /// Don't Starve Together (联机版) content.
    Dst,
    /// Single-player / DLC content (单机版、海难、巨人国、猪镇…).
    Ds,
    /// No signal matched; goes to the manual triage queue.
    Unknown,
}

impl GameClass {
    pub fn name(self) -> &'static str {
        match self {
            GameClass::Redirect => "redirect",
            GameClass::Disambig => "disambig",
            GameClass::Mixed => "mixed",
            GameClass::Dst => "dst",
            GameClass::Ds => "ds",
            GameClass::Unknown => "unknown",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "redirect" => GameClass::Redirect,
            "disambig" => GameClass::Disambig,
            "mixed" => GameClass::Mixed,
            "dst" => GameClass::Dst,
            "ds" => GameClass::Ds,
            "unknown" => GameClass::Unknown,
            _ => return None,
        })
    }
}

/// How strong the evidence behind a [`GameClass`] assignment is.
///
/// Declaration order is the derived ordering: Low < Medium < High.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassConfidence {
    /// Nothing matched; triage required.
    Low,
    /// Version categories only (联机版/单机版/DLC 分类成员)。
    Medium,
    /// Template self-declaration (`|dst|` / `|ds|` infobox params).
    High,
}

/// Authoritative per-page metadata (one line of `meta.jsonl`).
///
/// Everything else on disk (page files aside) is derived from the map of
/// these records, so the format must stay append-friendly and stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    pub pageid: i64,
    pub title: String,
    /// MediaWiki `touched` timestamp — the incremental-sync key.
    pub touched: Option<String>,
    /// Revision byte length as reported by the API (len sanity check).
    pub len: Option<u64>,
    pub redirect: bool,
    pub rev_sha1: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub game_class: GameClass,
    #[serde(default)]
    pub class_signals: Vec<String>,
    pub class_confidence: ClassConfidence,
}

impl PageMeta {
    /// Builds the metadata of a freshly fetched page (classification is
    /// filled in separately via [`crate::corpus::classify::classify`]).
    pub fn from_listing(entry: &crate::wiki::PageListingEntry) -> Self {
        Self {
            pageid: entry.pageid,
            title: entry.title.clone(),
            touched: entry.touched.clone(),
            len: entry.len,
            redirect: entry.redirect,
            rev_sha1: None,
            categories: Vec::new(),
            game_class: GameClass::Unknown,
            class_signals: Vec::new(),
            class_confidence: ClassConfidence::Low,
        }
    }
}

/// Per-round run summary written to `<root>/manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusManifest {
    pub generated_at_ms: u64,
    pub host: String,
    /// `"full"` or `"incremental"`.
    pub mode: String,
    /// Local write was suppressed (`--dry-run`).
    pub dry_run: bool,
    pub enumerated: usize,
    pub fetched: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub missing_on_fetch: usize,
    pub len_mismatches: usize,
    #[serde(default)]
    pub class_counts: BTreeMap<String, usize>,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_class_serde_roundtrip() {
        for class in [
            GameClass::Redirect,
            GameClass::Disambig,
            GameClass::Mixed,
            GameClass::Dst,
            GameClass::Ds,
            GameClass::Unknown,
        ] {
            let json = serde_json::to_string(&class).unwrap();
            let back: GameClass = serde_json::from_str(&json).unwrap();
            assert_eq!(back, class);
            assert_eq!(GameClass::from_name(class.name()), Some(class));
        }
        assert_eq!(GameClass::from_name("nonsense"), None);
    }

    #[test]
    fn page_meta_jsonl_roundtrip() {
        let meta = PageMeta {
            pageid: 42,
            title: "猎犬".to_string(),
            touched: Some("2026-08-01T00:00:00Z".to_string()),
            len: Some(2805),
            redirect: false,
            rev_sha1: Some("abc123".to_string()),
            categories: vec!["分类:联机版".to_string()],
            game_class: GameClass::Dst,
            class_signals: vec!["tpl_param:dst".to_string()],
            class_confidence: ClassConfidence::High,
        };
        let line = serde_json::to_string(&meta).unwrap();
        assert!(line.contains("\"game_class\":\"dst\""), "{}", line);
        let back: PageMeta = serde_json::from_str(&line).unwrap();
        assert_eq!(back.pageid, meta.pageid);
        assert_eq!(back.game_class, GameClass::Dst);
        // Older lines without optional fields keep deserializing.
        let legacy = r#"{"pageid":1,"title":"X","touched":null,"len":null,"redirect":true,"rev_sha1":null,"game_class":"redirect","class_confidence":"high"}"#;
        let back: PageMeta = serde_json::from_str(legacy).unwrap();
        assert!(back.redirect);
        assert!(back.categories.is_empty());
    }

    #[test]
    fn confidence_ordering() {
        // Derived order follows declaration: Low < Medium < High.
        assert!(ClassConfidence::Low < ClassConfidence::Medium);
        assert!(ClassConfidence::Medium < ClassConfidence::High);
    }
}
