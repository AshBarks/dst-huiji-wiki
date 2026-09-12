//! In-memory dataset used by the WebUI browse/visualize endpoints.
//!
//! The whole game dataset is small (a few MB of JSON), so it is parsed once
//! per snapshot and kept in memory; every API query is then a cheap lookup.

use crate::error::{Error, Result};
use crate::models::Recipe;
use crate::parser::{parse_skill_tree_with_tuning, PoParser, RecipeParser};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct IngredientDto {
    pub item: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeDto {
    pub name: String,
    pub tech: String,
    pub ingredients: Vec<IngredientDto>,
    pub numtogive: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<usize>,
}

impl RecipeDto {
    fn from_model(r: &Recipe) -> Self {
        Self {
            name: r.name.clone(),
            tech: r.tech.clone(),
            ingredients: r
                .ingredients
                .iter()
                .map(|i| IngredientDto {
                    item: i.item.clone(),
                    amount: i.amount,
                })
                .collect(),
            numtogive: r.options.numtogive,
            source_line: r.source_line,
        }
    }
}

/// A single PO entry for browsing/pagination from the WebUI.
#[derive(Debug, Clone, Serialize)]
pub struct PoEntryDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msgctxt: Option<String>,
    pub msgid: String,
    pub msgstr: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryStat {
    pub category: String,
    pub total: usize,
    pub translated: usize,
}

/// Chinese strings for skill titles/descriptions, keyed by PO msgctxt.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SkillStringsData {
    /// msgctxt -> msgstr (Chinese), only `STRINGS.SKILLTREE.*` entries.
    pub by_ctxt: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct Dataset {
    /// Patch version or snapshot directory name this data was built from.
    pub label: String,
    pub snapshot: Option<String>,
    pub recipes: Vec<RecipeDto>,
    /// ingredient prefab -> indices into `recipes`
    pub ingredient_index: BTreeMap<String, Vec<usize>>,
    pub po_categories: Vec<CategoryStat>,
    pub po_total_entries: usize,
    pub skill_characters: Vec<String>,
    pub tuning_count: usize,
    pub tech_levels: Vec<String>,
    /// Full PO entries for paginated browsing.
    pub po_entries: Vec<PoEntryDto>,
    /// TUNING constant table (raw values).
    pub tuning: Vec<TuningEntry>,
}

impl Dataset {
    pub fn search_recipes(
        &self,
        q: &str,
        tech: Option<&str>,
        ingredient: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> (usize, Vec<&RecipeDto>) {
        let ql = q.to_lowercase();
        let filtered: Vec<&RecipeDto> = self
            .recipes
            .iter()
            .filter(|r| {
                if !ql.is_empty() && !r.name.to_lowercase().contains(&ql) {
                    return false;
                }
                if let Some(t) = tech {
                    if !t.is_empty() && r.tech != t {
                        return false;
                    }
                }
                if let Some(ing) = ingredient {
                    if !ing.is_empty() && !r.ingredients.iter().any(|i| i.item == ing) {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total = filtered.len();
        let start = page.saturating_mul(page_size);
        let slice = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .collect::<Vec<_>>();
        (total, slice)
    }

    pub fn recipes_using(&self, ingredient: &str) -> Vec<&RecipeDto> {
        self.ingredient_index
            .get(ingredient)
            .map(|idxs| idxs.iter().filter_map(|i| self.recipes.get(*i)).collect())
            .unwrap_or_default()
    }
}

/// Loads a [`Dataset`] from game files for the given snapshot (None = latest).
fn load_dataset(snapshot: Option<String>) -> Result<Arc<Dataset>> {
    let mut ctx = crate::DstContext::new(
        crate::platform::config::dst_root_opt()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        snapshot.clone(),
    )?;

    // --- recipes ---
    let recipes_lua = ctx.read_script_file("scripts/recipes.lua")?;
    let mut parser = RecipeParser::new();
    let models = parser.parse(&recipes_lua, Some("scripts/recipes.lua"))?;
    let recipes: Vec<RecipeDto> = models.iter().map(RecipeDto::from_model).collect();

    let mut ingredient_index: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, r) in recipes.iter().enumerate() {
        for ing in &r.ingredients {
            ingredient_index
                .entry(ing.item.clone())
                .or_default()
                .push(idx);
        }
    }

    let mut tech_levels: Vec<String> = recipes
        .iter()
        .map(|r| r.tech.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    tech_levels.sort();

    // --- PO categories ---
    let po_content = ctx.read_script_file("scripts/languages/chinese_s.po")?;
    let po_file = PoParser::parse(&po_content)?;
    let po_total_entries = po_file.entries.len();

    let mut cat_map: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for e in &po_file.entries {
        let category = match &e.msgctxt {
            Some(c) => {
                let rest = c.strip_prefix("STRINGS.").unwrap_or(c);
                rest.split('.').next().unwrap_or("MISC").to_string()
            }
            None => "NONE".to_string(),
        };
        let entry = cat_map.entry(category).or_insert((0, 0));
        entry.0 += 1;
        if !e.msgstr.trim().is_empty() && e.msgstr != e.msgid {
            entry.1 += 1;
        }
    }
    let mut po_categories: Vec<CategoryStat> = cat_map
        .into_iter()
        .map(|(category, (total, translated))| CategoryStat {
            category,
            total,
            translated,
        })
        .collect();
    po_categories.sort_by_key(|c| std::cmp::Reverse(c.total));

    // --- skill tree characters ---
    let skill_characters = list_skill_characters(&mut ctx)?;

    // --- tuning constants ---
    let tuning_src = ctx
        .read_script_file("scripts/tuning.lua")
        .unwrap_or_default();
    let tuning_count = count_tuning_keys(&tuning_src);
    let tuning = tuning_from_source(&tuning_src);

    // --- full PO entries for browsing ---
    let po_entries: Vec<PoEntryDto> = po_file
        .entries
        .iter()
        .map(|e| PoEntryDto {
            msgctxt: e.msgctxt.clone(),
            msgid: e.msgid.clone(),
            msgstr: e.msgstr.clone(),
        })
        .collect();

    Ok(Arc::new(Dataset {
        label: ctx.sources(),
        snapshot,
        recipes,
        ingredient_index,
        po_categories,
        po_total_entries,
        skill_characters,
        tuning_count,
        tech_levels,
        po_entries,
        tuning,
    }))
}

/// Lists available skill tree characters from the prefabs directory.
pub fn list_skill_characters(ctx: &mut crate::DstContext) -> Result<Vec<String>> {
    // Snapshot mode: scan the directory directly.
    if let Some(snapshot) = &ctx.snapshot {
        let dir = std::path::Path::new(&ctx.dst_root)
            .join("data/databundles")
            .join(snapshot)
            .join("prefabs");
        let mut chars = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(rest) = name.strip_prefix("skilltree_") {
                    if let Some(char_name) = rest.strip_suffix(".lua") {
                        if char_name != "defs" {
                            chars.push(char_name.to_string());
                        }
                    }
                }
            }
        }
        chars.sort();
        return Ok(chars);
    }

    // Zip mode: iterate archive entries.
    let archive = ctx.open_scripts_zip()?;
    let mut chars = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if let Some(rest) = name.strip_prefix("scripts/prefabs/skilltree_") {
            if let Some(char_name) = rest.strip_suffix(".lua") {
                if char_name != "defs" {
                    chars.push(char_name.to_string());
                }
            }
        }
    }
    chars.sort();
    chars.dedup();
    Ok(chars)
}

/// Lists available skill tree characters from local extracted scripts only.
///
/// Unlike [`list_skill_characters`], this does not construct a wiki client,
/// so it can be used by local-only export jobs that should not require
/// `HUIJI__*` credentials.
pub fn list_skill_characters_local(snapshot: Option<&str>) -> Result<Vec<String>> {
    let entries = crate::platform::game_source::GameSource::from_env(snapshot.map(str::to_string))?
        .list_dir("prefabs")?;

    let mut chars = Vec::new();
    for name in entries {
        if let Some(rest) = name.strip_prefix("skilltree_") {
            if let Some(char_name) = rest.strip_suffix(".lua") {
                if char_name != "defs" {
                    chars.push(char_name.to_string());
                }
            }
        }
    }
    chars.sort();
    chars.dedup();
    Ok(chars)
}

fn count_tuning_keys(source: &str) -> usize {
    source
        .lines()
        .take_while(|l| !l.contains("--!") || l.trim_start().starts_with("TUNING"))
        .filter(|l| {
            let t = l.trim_start();
            let has_key = t
                .split(|c: char| !(c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()))
                .next()
                .map(|k| !k.is_empty() && k.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
                .unwrap_or(false);
            has_key && t.contains('=')
        })
        .count()
}

/// Parses `tuning.lua` numeric leaves (dotted keys without the `TUNING.`
/// prefix) so parsers can resolve `TUNING.*` references. Includes nested
/// tables, e.g. `SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD`.
pub fn load_tuning_numbers(snapshot: Option<&str>) -> Result<BTreeMap<String, f64>> {
    let source = read_game_file(snapshot, "tuning.lua")?;
    crate::update::index::tuning::build_tuning_leaves(&source)
}

/// Parses one character's skill tree, enriching nodes with zh titles/descs.
pub fn load_skill_tree(
    ctx_snapshot: Option<&str>,
    character: &str,
    strings: &SkillStringsData,
) -> Result<serde_json::Value> {
    let content = read_game_file(
        ctx_snapshot,
        &format!("prefabs/skilltree_{}.lua", character),
    )?;
    let tuning = load_tuning_numbers(ctx_snapshot)?;
    let tree = parse_skill_tree_with_tuning(&content, character, &tuning)?;

    let upper_char = character.to_uppercase();
    let prefix = format!("STRINGS.SKILLTREE.{}.", upper_char);

    let nodes: Vec<serde_json::Value> = tree
        .nodes
        .iter()
        .map(|n| {
            let title_key = format!("{}{}_TITLE", prefix, n.name.to_uppercase());
            let desc_key = format!("{}{}_DESC", prefix, n.name.to_uppercase());
            serde_json::json!({
                "name": n.name,
                "x": n.x,
                "y": n.y,
                "group": n.group,
                "root": n.root,
                "connects": n.connects,
                "icon": n.icon,
                "lock": n.lock,
                "locks": n.locks,
                "lock_open": n.lock_open,
                "tags": n.tags,
                "onactivate": n.onactivate,
                "ondeactivate": n.ondeactivate,
                "defaultfocus": n.defaultfocus,
                "infographic": n.infographic,
                "forced_focus": n.forced_focus,
                "button_decorations": n.button_decorations,
                "decorations": n.decorations,
                "title": strings.by_ctxt.get(&title_key),
                "desc": strings.by_ctxt.get(&desc_key),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "character": tree.character,
        "groups": tree.groups(),
        "background": tree.background,
        "nodes": nodes,
    }))
}

/// Reads an arbitrary file under the scripts root honouring snapshot choice.
///
/// 统一走 [`crate::platform::game_source::GameSource`]（snapshot → live →
/// scripts.zip）；未指定 snapshot 且解压树缺失时回退读取 zip。
pub fn read_game_file(snapshot: Option<&str>, rel_path: &str) -> Result<String> {
    crate::platform::game_source::GameSource::from_env(snapshot.map(str::to_string))?.read(rel_path)
}

/// Parses the SKILLTREE-related PO entries for zh lookups.
pub fn load_skill_strings(snapshot: Option<&str>) -> Result<SkillStringsData> {
    let content = read_game_file(snapshot, "languages/chinese_s.po")?;
    let po_file = PoParser::parse(&content)?;
    let mut by_ctxt = BTreeMap::new();
    for e in po_file.entries {
        if let Some(ctxt) = &e.msgctxt {
            if ctxt.starts_with("STRINGS.SKILLTREE.") && !e.msgstr.trim().is_empty() {
                by_ctxt.insert(ctxt.clone(), e.msgstr.clone());
            }
        }
    }
    Ok(SkillStringsData { by_ctxt })
}

/// Thread-safe cache of loaded datasets, keyed by snapshot selection.
#[derive(Default)]
pub struct DatasetCache {
    entries: Mutex<HashMap<Option<String>, Arc<Dataset>>>,
}

impl DatasetCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the cached dataset or loads it (blocking) on a worker thread.
    pub async fn get_or_load(&self, snapshot: Option<String>) -> Result<Arc<Dataset>> {
        {
            let entries = self.entries.lock().await;
            if let Some(ds) = entries.get(&snapshot) {
                return Ok(Arc::clone(ds));
            }
        }

        let snap = snapshot.clone();
        let loaded = tokio::task::spawn_blocking(move || load_dataset(snap))
            .await
            .map_err(|e| Error::Config(format!("dataset loader panicked: {}", e)))??;

        let mut entries = self.entries.lock().await;
        // Another request may have loaded the same key concurrently.
        let ds = entries
            .entry(snapshot)
            .or_insert_with(|| Arc::clone(&loaded));
        Ok(Arc::clone(ds))
    }

    pub async fn invalidate(&self) {
        self.entries.lock().await.clear();
    }
}

// ---------------------------------------------------------------------------
// TUNING constants extraction (line-based; values kept as raw text)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct TuningEntry {
    pub key: String,
    pub value: String,
}

/// Extracts top-level assignments from tuning.lua for a snapshot.
pub fn extract_tuning(snapshot: Option<&str>) -> Result<Vec<TuningEntry>> {
    let source = read_game_file(snapshot, "tuning.lua")?;
    Ok(tuning_from_source(&source))
}

/// Line-scans tuning.lua contents for entries of the top-level table
/// (`TUNING = { ... }`), i.e. assignments at brace depth 1.
fn tuning_from_source(source: &str) -> Vec<TuningEntry> {
    let mut entries = Vec::new();
    let mut depth: isize = 0;

    for line in source.lines() {
        let trimmed = line.trim();

        // Keys live directly inside the outermost table.
        if depth == 1 {
            if let Some((key, value)) = parse_tuning_line(trimmed) {
                entries.push(TuningEntry { key, value });
            }
        }

        let opens = trimmed.matches('{').count() as isize;
        let closes = trimmed.matches('}').count() as isize;
        depth += opens - closes;
    }
    let _ = depth; // trailing content after the outer table closes is ignored

    entries
}

fn parse_tuning_line(line: &str) -> Option<(String, String)> {
    // Match `KEY = <value>,` where KEY is uppercase-ish.
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '.')
    {
        return None;
    }
    if !key.chars().next()?.is_ascii_uppercase() {
        return None;
    }

    let mut value = line[eq + 1..].trim();
    if let Some(stripped) = value.strip_suffix(',') {
        value = stripped.trim_end();
    }
    const MAX_VALUE_LEN: usize = 160;
    if value.len() > MAX_VALUE_LEN {
        let mut truncated = value[..MAX_VALUE_LEN].to_string();
        truncated.push_str(" …");
        return Some((key.to_string(), truncated));
    }
    Some((key.to_string(), value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tuning_line_simple() {
        let (k, v) = parse_tuning_line("HAMBAT_DAMAGE = 51.2,").unwrap();
        assert_eq!(k, "HAMBAT_DAMAGE");
        assert_eq!(v, "51.2");
    }

    #[test]
    fn test_parse_tuning_line_expression() {
        let (k, v) = parse_tuning_line("ARMORWOOD_PERISHTIME = TOTAL_DAY_TIME * 2,").unwrap();
        assert_eq!(k, "ARMORWOOD_PERISHTIME");
        assert_eq!(v, "TOTAL_DAY_TIME * 2");
    }

    #[test]
    fn test_parse_tuning_line_rejects_lowercase() {
        assert!(parse_tuning_line("local something = 1,").is_none());
    }

    #[test]
    fn test_tuning_from_source() {
        let src = "TUNING = \n{\n A = 1,\n B = TOTAL_DAY_TIME * 2,\n -- comment\n c = 3,\n NESTED = {x=1},\n}\n";
        let entries = tuning_from_source(src);
        let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert!(keys.contains(&"A"), "got {:?}", keys);
        assert!(keys.contains(&"B"));
        assert!(keys.contains(&"NESTED"));
        assert!(!keys.contains(&"c"));
    }
}
