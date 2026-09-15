//! Shared cooking asset/name helpers for the WebUI endpoint and the
//! standalone game package exporter.
//!
//! Keeping icon/name resolution here avoids the API and exporter drifting on
//! special image exceptions such as `onion` -> `quagmire_onion.png`.

use crate::error::Result;
use crate::models::PoEntry;
use crate::parser::PoParser;
use crate::platform::game_source::GameSource;
#[cfg(test)]
use crate::scripts_sync::images::icons::IconEntry;
use crate::scripts_sync::images::icons::IconsIndex;
use crate::scripts_sync::images::meta::IconMeta;
use std::collections::{HashMap, HashSet};

/// Image names that differ from the prefab name and cannot be resolved by the
/// PO English-name fallback (the icon metadata uses slightly different names,
/// e.g. `Roast Onion` vs `Roasted Onion`).
pub const ICON_EXCEPTIONS: &[(&str, &str)] = &[
    ("onion", "quagmire_onion.png"),
    ("onion_cooked", "quagmire_onion_cooked.png"),
    ("tomato", "quagmire_tomato.png"),
    ("tomato_cooked", "quagmire_tomato_cooked.png"),
];

/// Upper-snake `STRINGS.NAMES.*` key -> (en, zh).
pub type NameIndex = HashMap<String, (Option<String>, Option<String>)>;

/// Pre-indexed inventory icons for `resolve()`.
#[derive(Debug, Default)]
pub struct IconResolver {
    inventory_files: HashSet<String>,
    by_name: HashMap<String, String>,
}

impl IconResolver {
    pub fn from_index(index: &IconsIndex, meta: &IconMeta) -> Self {
        let mut inventory_files = HashSet::new();
        for entry in &index.entries {
            if entry.source == "inventory" {
                inventory_files.insert(entry.file.clone());
            }
        }

        let mut by_name: HashMap<String, String> = HashMap::new();
        for (file, entry) in &meta.icons {
            if entry.source.as_deref().unwrap_or("inventory") != "inventory" {
                continue;
            }
            if let Some(name) = &entry.name_en {
                by_name
                    .entry(name.to_lowercase())
                    .or_insert_with(|| file.clone());
            }
        }

        Self {
            inventory_files,
            by_name,
        }
    }

    #[cfg(test)]
    pub fn from_entries(entries: &[IconEntry], meta: &IconMeta) -> Self {
        let index = IconsIndex {
            entries: entries.to_vec(),
            versions: HashMap::new(),
            latest_build: None,
            latest_synced_at: None,
        };
        Self::from_index(&index, meta)
    }

    pub fn resolve(&self, prefab: &str, name_en: Option<&str>) -> Option<String> {
        let exception = ICON_EXCEPTIONS
            .iter()
            .find_map(|(key, file)| (*key == prefab).then_some((*file).to_string()));
        let exact = format!("{}.png", prefab);
        let exact = self.inventory_files.contains(&exact).then_some(exact);
        let named = name_en.and_then(|name| self.by_name.get(&name.to_lowercase()).cloned());
        exception
            .or(exact)
            .or(named)
            .filter(|file| self.inventory_files.contains(file))
    }

    pub fn contains_file(&self, file: &str) -> bool {
        self.inventory_files.contains(file)
    }
}

/// `STRINGS.NAMES.*` PO entries -> upper-snake key -> (en, zh).
pub fn po_name_index(
    entries: &[crate::service::dataset::PoEntryDto],
) -> HashMap<String, (Option<String>, Option<String>)> {
    let mut out = HashMap::new();
    for entry in entries {
        let Some(ctxt) = &entry.msgctxt else {
            continue;
        };
        let Some(key) = ctxt.strip_prefix("STRINGS.NAMES.") else {
            continue;
        };
        if !key.is_empty() {
            let en = (!entry.msgid.trim().is_empty()).then(|| entry.msgid.clone());
            let zh = (!entry.msgstr.trim().is_empty() && entry.msgstr != entry.msgid)
                .then(|| entry.msgstr.trim().to_string());
            out.insert(key.to_ascii_uppercase(), (en, zh));
        }
    }
    out
}

/// Load the same index for local-only commands (no wiki client / credentials).
pub fn load_po_name_index(snapshot: Option<&str>) -> Result<NameIndex> {
    let source = GameSource::from_env(snapshot.map(str::to_string))?;
    let content = source.read("languages/chinese_s.po")?;
    let po = PoParser::parse(&content)?;
    let entries: Vec<crate::service::dataset::PoEntryDto> = po
        .entries
        .into_iter()
        .map(|e: PoEntry| crate::service::dataset::PoEntryDto {
            msgctxt: e.msgctxt,
            msgid: e.msgid,
            msgstr: e.msgstr,
        })
        .collect();
    Ok(po_name_index(&entries))
}

/// Resolve an upper-snake `STRINGS.NAMES.*` key through a name index.
pub fn names_for(index: &NameIndex, prefab: &str) -> (Option<String>, Option<String>) {
    index
        .get(&prefab.to_ascii_uppercase())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::dataset::PoEntryDto;

    #[test]
    fn po_name_index_uses_upper_snake_key() {
        let entries = vec![PoEntryDto {
            msgctxt: Some("STRINGS.NAMES.BUTTERFLYMUFFIN".into()),
            msgid: "Butter Muffin".into(),
            msgstr: "蝴蝶松饼".into(),
        }];
        let index = po_name_index(&entries);
        let (en, zh) = names_for(&index, "butterflymuffin");
        assert_eq!(en.as_deref(), Some("Butter Muffin"));
        assert_eq!(zh.as_deref(), Some("蝴蝶松饼"));
    }

    #[test]
    fn icon_exceptions_are_used_only_when_file_exists() {
        let entry = IconEntry {
            file: "quagmire_onion.png".into(),
            source: "inventory".into(),
            hash: "x".into(),
            first_build: "1".into(),
            first_synced_at: None,
        };
        let resolver = IconResolver::from_entries(&[entry], &IconMeta::default());
        assert_eq!(
            resolver.resolve("onion", Some("Onion")).as_deref(),
            Some("quagmire_onion.png")
        );

        let empty = IconResolver::from_entries(&[], &IconMeta::default());
        assert_eq!(empty.resolve("onion", Some("Onion")), None);
    }

    #[test]
    fn loader_helpers_construct() {
        let index: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
        assert_eq!(names_for(&index, "x"), (None, None));
    }
}
