//! Snapshot-to-snapshot comparison engine (the "time machine").
//!
//! Compares `recipes.lua` and `chinese_s.po` between two game snapshots and
//! produces structured, renderable diffs.

use crate::error::Result;
use crate::parser::{PoParser, RecipeParser};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct IngredientDiffDto {
    pub item: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeChange {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_after: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ingredients_added: Vec<IngredientDiffDto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ingredients_removed: Vec<IngredientDiffDto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub amounts_changed: Vec<AmountChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AmountChange {
    pub item: String,
    pub before: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipesDiff {
    pub from: String,
    pub to: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<RecipeChange>,
    pub total_from: usize,
    pub total_to: usize,
}

fn parse_recipes_from_source(source: &str) -> Result<BTreeMap<String, crate::models::Recipe>> {
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(source, None)?;
    Ok(recipes.into_iter().map(|r| (r.name.clone(), r)).collect())
}

/// Compares recipes between two Lua sources.
pub fn diff_recipes_sources(from_src: &str, to_src: &str) -> Result<RecipesDiff> {
    let before = parse_recipes_from_source(from_src)?;
    let after = parse_recipes_from_source(to_src)?;

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for (name, old) in &before {
        match after.get(name) {
            None => removed.push(name.clone()),
            Some(new) => {
                let tech_changed = old.tech != new.tech;
                let old_ings: BTreeMap<&str, i32> = old
                    .ingredients
                    .iter()
                    .map(|i| (i.item.as_str(), i.amount))
                    .collect();
                let new_ings: BTreeMap<&str, i32> = new
                    .ingredients
                    .iter()
                    .map(|i| (i.item.as_str(), i.amount))
                    .collect();

                let mut change = RecipeChange {
                    name: name.clone(),
                    tech_before: None,
                    tech_after: None,
                    ingredients_added: Vec::new(),
                    ingredients_removed: Vec::new(),
                    amounts_changed: Vec::new(),
                };

                if tech_changed {
                    change.tech_before = Some(old.tech.clone());
                    change.tech_after = Some(new.tech.clone());
                }

                for (item, amount) in &new_ings {
                    match old_ings.get(item) {
                        None => change.ingredients_added.push(IngredientDiffDto {
                            item: (*item).to_string(),
                            amount: *amount,
                        }),
                        Some(before_amount) if before_amount != amount => {
                            change.amounts_changed.push(AmountChange {
                                item: (*item).to_string(),
                                before: *before_amount,
                                after: *amount,
                            });
                        }
                        _ => {}
                    }
                }
                for (item, amount) in &old_ings {
                    if !new_ings.contains_key(item) {
                        change.ingredients_removed.push(IngredientDiffDto {
                            item: (*item).to_string(),
                            amount: *amount,
                        });
                    }
                }

                if tech_changed
                    || !change.ingredients_added.is_empty()
                    || !change.ingredients_removed.is_empty()
                    || !change.amounts_changed.is_empty()
                {
                    changed.push(change);
                }
            }
        }
    }
    for name in after.keys() {
        if !before.contains_key(name) {
            added.push(name.clone());
        }
    }

    let total_from = before.len();
    let total_to = after.len();
    added.sort();
    removed.sort();

    Ok(RecipesDiff {
        from: String::new(),
        to: String::new(),
        added,
        removed,
        changed,
        total_from,
        total_to,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct PoDiff {
    pub from: String,
    pub to: String,
    pub keys_added: usize,
    pub keys_removed: usize,
    pub translations_changed: usize,
    pub newly_translated: usize,
    pub total_from: usize,
    pub total_to: usize,
    pub added_samples: Vec<String>,
    pub removed_samples: Vec<String>,
}

const SAMPLE_LIMIT: usize = 50;

fn po_key_map(source: &str) -> Result<BTreeMap<String, (Option<String>, String)>> {
    let file = PoParser::parse(source)?;
    Ok(file
        .entries
        .into_iter()
        .map(|e| {
            let key = format!("{}\x00{}", e.msgctxt.as_deref().unwrap_or(""), e.msgid);
            (key, (e.msgctxt, e.msgstr))
        })
        .collect())
}

/// Compares two PO sources by (msgctxt, msgid) key.
pub fn diff_po_sources(from_src: &str, to_src: &str) -> Result<PoDiff> {
    let before = po_key_map(from_src)?;
    let after = po_key_map(to_src)?;

    let before_keys: HashSet<&String> = before.keys().collect();
    let after_keys: HashSet<&String> = after.keys().collect();

    let mut added_samples = Vec::new();
    for key in after_keys.difference(&before_keys) {
        if added_samples.len() >= SAMPLE_LIMIT {
            break;
        }
        // Show only the msgid part for readability.
        added_samples.push(key.split('\x00').nth(1).unwrap_or(key).to_string());
    }

    let mut removed_samples = Vec::new();
    for key in before_keys.difference(&after_keys) {
        if removed_samples.len() >= SAMPLE_LIMIT {
            break;
        }
        removed_samples.push(key.split('\x00').nth(1).unwrap_or(key).to_string());
    }

    let mut translations_changed = 0;
    let mut newly_translated = 0;
    for (key, (_, to_str)) in after.iter() {
        if let Some((_, from_str)) = before.get(key) {
            if from_str != to_str {
                translations_changed += 1;
                if from_str.trim().is_empty() && !to_str.trim().is_empty() {
                    newly_translated += 1;
                }
            }
        }
    }

    Ok(PoDiff {
        from: String::new(),
        to: String::new(),
        keys_added: after_keys.difference(&before_keys).count(),
        keys_removed: before_keys.difference(&after_keys).count(),
        translations_changed,
        newly_translated,
        total_from: before.len(),
        total_to: after.len(),
        added_samples,
        removed_samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPE_A: &str = r#"
Recipe2("axe", {Ingredient("twigs", 1), Ingredient("flint", 1)}, TECH.NONE)
Recipe2("hammer", {Ingredient("twigs", 3), Ingredient("rocks", 1), Ingredient("goldnugget", 3)}, TECH.NONE)
"#;

    const RECIPE_B: &str = r#"
Recipe2("axe", {Ingredient("twigs", 1), Ingredient("flint", 2)}, TECH.SCIENCE_ONE)
Recipe2("torch", {Ingredient("twigs", 2), Ingredient("cutgrass", 2)}, TECH.NONE)
"#;

    #[test]
    fn test_diff_recipes_basic() {
        let diff = diff_recipes_sources(RECIPE_A, RECIPE_B).unwrap();
        assert_eq!(diff.added, vec!["torch".to_string()]);
        assert_eq!(diff.removed, vec!["hammer".to_string()]);
        assert_eq!(diff.changed.len(), 1);
        let axe = &diff.changed[0];
        assert_eq!(axe.name, "axe");
        assert_eq!(axe.tech_before.as_deref(), Some("TECH.NONE"));
        assert_eq!(axe.tech_after.as_deref(), Some("TECH.SCIENCE_ONE"));
        assert_eq!(axe.amounts_changed.len(), 1);
        assert_eq!(axe.amounts_changed[0].item, "flint");
        assert_eq!(axe.amounts_changed[0].before, 1);
        assert_eq!(axe.amounts_changed[0].after, 2);
    }

    #[test]
    fn test_diff_recipes_totals() {
        let diff = diff_recipes_sources(RECIPE_A, RECIPE_B).unwrap();
        assert_eq!(diff.total_from, 2);
        assert_eq!(diff.total_to, 2);
    }

    const PO_A: &str = r#"msgctxt "STRINGS.NAMES.AXE"
msgid "Axe"
msgstr ""

msgctxt "STRINGS.NAMES.OLD"
msgid "Old"
msgstr "旧的"
"#;

    const PO_B: &str = r#"msgctxt "STRINGS.NAMES.AXE"
msgid "Axe"
msgstr "斧头"

msgctxt "STRINGS.NAMES.NEW"
msgid "New"
msgstr "新"
"#;

    #[test]
    fn test_diff_po() {
        let diff = diff_po_sources(PO_A, PO_B).unwrap();
        assert_eq!(diff.keys_added, 1);
        assert_eq!(diff.keys_removed, 1);
        assert_eq!(diff.newly_translated, 1); // AXE went empty -> translated
        assert!(diff.added_samples.contains(&"New".to_string()));
        assert!(diff.removed_samples.contains(&"Old".to_string()));
    }
}
