//! Page ↔ prefab-variant registry (docs/CORPUS_CODE_ATLAS_CONTRACT.md §1).
//!
//! The prefab variant name is the single hard join key between the corpus and
//! the code-side association index. On the wiki side it comes from the
//! infobox template argument: `{{实体信息框/自动|dst|<prefab>…}}`. RichTab
//! aggregation pages carry one infobox per variant, so collecting every
//! instance in a page yields the full page→variants mapping.

use crate::corpus::model::PageMeta;
use crate::corpus::store::CorpusStore;
use crate::error::Result;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

/// Where a prefab argument was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefabSource {
    /// `{{实体信息框/自动|dst|X}}` — generated from DST Prefab data.
    Auto,
    /// `{{实体信息框|dst|X}}` — hand-written infobox.
    Manual,
}

/// Infobox template heads that carry a prefab argument as their second
/// parameter. The trailing `|dst|` delimiter keeps `/自动` pages from also
/// matching the manual pattern.
const PREFAB_HEADS: &[(&str, PrefabSource)] = &[
    ("{{实体信息框/自动|dst|", PrefabSource::Auto),
    ("{{实体信息框|dst|", PrefabSource::Manual),
];

/// Extracts every `(prefab_variant, source)` argument on the page, in
/// document order. Duplicates are kept — callers dedupe as needed.
pub fn extract_prefabs(wikitext: &str) -> Vec<(String, PrefabSource)> {
    let mut out = Vec::new();
    for (head, source) in PREFAB_HEADS {
        let mut from = 0usize;
        while let Some(rel) = wikitext[from..].find(head) {
            let arg_start = from + rel + head.len();
            let arg: String = wikitext[arg_start..]
                .chars()
                .take_while(|c| !matches!(c, '|' | '}' | '\n' | '['))
                .collect();
            let arg = arg.trim();
            if !arg.is_empty() {
                out.push((arg.to_string(), *source));
            }
            from = arg_start;
        }
    }
    out
}

/// Registry artifact written to `index/pages_by_prefab.json`.
#[derive(Debug, Serialize)]
pub struct PrefabRegistry {
    pub schema_version: u32,
    pub generated_at_ms: u64,
    /// prefab variant → sorted pageids using it.
    pub prefabs: BTreeMap<String, Vec<i64>>,
    /// Non-redirect pages without any infobox prefab argument (aggregation /
    /// mechanic pages; deliberately excluded from atlas joins, contract §1.3).
    pub pages_without_prefab: Vec<i64>,
    pub stats: RegistryStats,
}

#[derive(Debug, Serialize)]
pub struct RegistryStats {
    pub pages_scanned: usize,
    pub entity_pages: usize,
    pub multi_variant_pages: usize,
    pub manual_only_pages: usize,
    pub distinct_variants: usize,
}

/// Scans every non-redirect page's local wikitext and collects the
/// page ↔ prefab-variant mapping.
pub fn build_registry(
    store: &CorpusStore,
    metas: &BTreeMap<i64, PageMeta>,
    now_ms: u64,
) -> Result<PrefabRegistry> {
    let mut prefabs: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    let mut pages_without_prefab = Vec::new();
    let mut stats = RegistryStats {
        pages_scanned: metas.len(),
        entity_pages: 0,
        multi_variant_pages: 0,
        manual_only_pages: 0,
        distinct_variants: 0,
    };

    for meta in metas.values() {
        if meta.redirect {
            continue;
        }
        let text = match store.read_page(meta.pageid)? {
            Some(t) => t,
            None => {
                // Missing local file is a reconcile concern (self-healed by
                // the next fetch), not an index failure.
                pages_without_prefab.push(meta.pageid);
                continue;
            }
        };
        let found = extract_prefabs(&text);
        if found.is_empty() {
            pages_without_prefab.push(meta.pageid);
            continue;
        }
        stats.entity_pages += 1;
        let variants: BTreeSet<&str> = found.iter().map(|(v, _)| v.as_str()).collect();
        if variants.len() > 1 {
            stats.multi_variant_pages += 1;
        }
        if variants.len() == 1 && found.iter().all(|(_, s)| *s == PrefabSource::Manual) {
            stats.manual_only_pages += 1;
        }
        for variant in variants {
            prefabs
                .entry(variant.to_string())
                .or_default()
                .insert(meta.pageid);
        }
    }

    stats.distinct_variants = prefabs.len();
    Ok(PrefabRegistry {
        schema_version: SCHEMA_VERSION,
        generated_at_ms: now_ms,
        prefabs: prefabs
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect(),
        pages_without_prefab,
        stats,
    })
}

/// Writes the registry atomically under `<root>/index/pages_by_prefab.json`.
pub fn save_registry(root: &Path, registry: &PrefabRegistry) -> Result<std::path::PathBuf> {
    let dir = root.join("index");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join("pages_by_prefab.json");
    crate::platform::fs::write_json_atomic(&target, registry)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_auto_and_manual_variants_in_order() {
        let text =
            "{{实体信息框/自动|dst|hound\n|掉落 = x}}\ntext {{实体信息框|dst|beefalo |图像=1}} end";
        assert_eq!(
            extract_prefabs(text),
            vec![
                ("hound".to_string(), PrefabSource::Auto),
                ("beefalo".to_string(), PrefabSource::Manual),
            ]
        );
    }

    #[test]
    fn rich_tab_multi_variant_page_yields_every_tab() {
        // Condensed hound-page shape: three tabs inside RichTab/信息框.
        let text = "{{RichTab/信息框\n|普通猎犬|\n{{实体信息框/自动|dst|hound}}\n\
                    |火焰猎犬|\n{{实体信息框/自动|dst|firehound}}\n\
                    |冰冻猎犬|\n{{实体信息框/自动|dst|icehound}}\n}}";
        assert_eq!(extract_prefabs(text).len(), 3);
    }

    #[test]
    fn auto_head_does_not_double_match_manual_pattern() {
        assert_eq!(extract_prefabs("{{实体信息框/自动|dst|x}}").len(), 1);
        assert!(extract_prefabs("没有任何信息框的正文").is_empty());
    }

    #[test]
    fn build_registry_maps_pages_and_stats() -> Result<()> {
        let base = crate::corpus::store::tests_temp_dir("prefab-idx");
        let store = CorpusStore::new(&base, "test.example.com");
        let mk = |pageid: i64, title: &str, redirect: bool| PageMeta {
            pageid,
            title: title.to_string(),
            touched: None,
            len: None,
            redirect,
            rev_sha1: None,
            categories: Vec::new(),
            game_class: crate::corpus::model::GameClass::Dst,
            class_signals: Vec::new(),
            class_confidence: crate::corpus::model::ClassConfidence::High,
        };
        let mut metas = BTreeMap::new();
        metas.insert(1, mk(1, "猎犬", false));
        metas.insert(2, mk(2, "野牛", false));
        metas.insert(3, mk(3, "针线包", true)); // redirect: skipped
        metas.insert(4, mk(4, "首页", false)); // no infobox
        for (id, text) in [
            (
                1,
                "{{实体信息框/自动|dst|hound}}{{实体信息框/自动|dst|firehound}}",
            ),
            (2, "{{实体信息框|dst|beefalo}}"),
            (3, "#重定向 [[缝纫包]]"),
            (4, "纯正文"),
        ] {
            store.write_page(id, text)?;
        }

        let reg = build_registry(&store, &metas, 42)?;
        assert_eq!(reg.prefabs.get("hound"), Some(&vec![1]));
        assert_eq!(reg.prefabs.get("firehound"), Some(&vec![1]));
        assert_eq!(reg.prefabs.get("beefalo"), Some(&vec![2]));
        assert_eq!(reg.pages_without_prefab, vec![4]);
        assert_eq!(reg.stats.entity_pages, 2);
        assert_eq!(reg.stats.multi_variant_pages, 1);
        assert_eq!(reg.stats.manual_only_pages, 1);
        assert_eq!(reg.schema_version, SCHEMA_VERSION);

        let path = save_registry(store.root(), &reg)?;
        let raw = std::fs::read_to_string(path)?;
        assert!(raw.contains("\"schema_version\": 1"));
        std::fs::remove_dir_all(&base).ok();
        Ok(())
    }
}
