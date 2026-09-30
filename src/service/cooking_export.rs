//! Standalone cooking-game package exporter.
//!
//! Produces a self-contained directory (and optionally a zip) consumed by the
//! independent `dst-cooking-game` project. The package contains the compiled
//! recipe AST, selectable ingredients, icons, each dish's food type, the
//! playable-character roster with portraits, and one verified `example_combo`
//! per recipe so the game backend can guarantee every refresh is solvable.

use crate::error::{Error, Result};
use crate::models::{CookingStats, CookingTemperature};
use crate::platform::fs::write_json_atomic;
use crate::platform::progress::Reporter;
use crate::scripts_sync::anim::history::hash_file;
use crate::scripts_sync::images::icons::build_icons_index;
use crate::scripts_sync::images::meta as icon_meta;
use crate::service::cooking::load_cooking_data;
use crate::service::cooking_assets::{
    character_name, foodtype_label, load_po_indexes, names_for, CharacterNames, IconResolver,
};
use crate::service::cooking_desc::describe_recipe;
use crate::service::cooking_eval::{collect_field_refs, eval_truthy, EvalEnv};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Bundle payload version. `3` adds recipe `foodtype` / `secondary_foodtype`
/// (plus their zh labels), `4` adds `data/characters.json` and the character
/// portraits under `ui/characters/`, `5` adds the entry-instant effect numbers
/// (`stats`, `temperature`); the AST contract itself is unchanged.
pub const BUNDLE_SCHEMA_VERSION: u32 = 5;

/// `data/characters.json` contract version.
pub const CHARACTERS_SCHEMA_VERSION: u32 = 1;

/// UI assets copied into the bundle, independent of the item icon set.
/// `(bundle_relative_path, path_under_current/split/)`
pub const UI_ASSETS: &[(&str, &str)] = &[
    ("ui/cookpot.png", "inventoryimages/cookpot.png"),
    (
        "ui/portablecookpot.png",
        "inventoryimages/portablecookpot_item.png",
    ),
    ("ui/warly.png", "saveslot_portraits/warly.png"),
];

/// Character portraits: source directory under `current/split/` and the bundle
/// directory they are copied into. Every character is 120x104 RGBA, the same
/// art the game shows in save slots (and the same art as `ui/warly.png`).
pub const PORTRAIT_SRC_DIR: &str = "saveslot_portraits";
pub const PORTRAIT_BUNDLE_DIR: &str = "ui/characters";

/// `saveslot_portraits/` entries that are UI placeholders rather than playable
/// characters. Keep in sync with the atlas if the game adds placeholder art.
pub const PORTRAIT_PLACEHOLDERS: &[&str] = &["background", "mod", "mod_small", "random", "unknown"];
pub const DEFAULT_CONFIG_TOML: &str = r#"# dst-cooking-game default configuration.
n_ingredients = 9
time_limit_secs = 30
score_per_dish = 100
# cookpot | portablecookpot
default_cooker = "portablecookpot"
# simple | normal
default_mode = "normal"
grace_ms = 300
max_sessions = 10000
session_ttl_secs = 1800
"#;

#[derive(Debug, Clone)]
pub struct CookingGameExportParams {
    pub output: Option<String>,
    pub snapshot: Option<String>,
    pub zip: bool,
    pub allow_missing_icons: bool,
}

#[derive(Debug, Serialize, Clone)]
struct BundleSource {
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot: Option<String>,
}

#[derive(Debug, Serialize)]
struct BundleIngredient {
    id: u32,
    prefab: String,
    key: String,
    tags: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_zh: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

#[derive(Debug, Serialize)]
struct BundleRecipe {
    id: u32,
    name: String,
    priority: f64,
    weight: f64,
    test: serde_json::Value,
    /// 属性要求（wiki 料理描述表同列语义），tag 约束以「，」连接；无约束时为空串。
    desc_attrs: String,
    /// 特殊要求（具体食材计数），以「；」连接；无要求时为空串。
    desc_special: String,
    /// 入口瞬时数值：生命 / 饥饿 / 理智。非食物产物（`preparednonfoods.lua`）
    /// 一律为 0（真实数值由各自的 prefab 决定，不在本包内）。
    stats: CookingStats,
    /// 冷热食物的温度 delta 与持续时间（仅部分料理有）。
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<CookingTemperature>,
    /// 食物类型（`FOODTYPE.*` 名，如 `MEAT`）。`preparednonfoods.lua` 的非食物
    /// 产物（如 `batnosehat`）没有类型，此时两个字段都不输出。
    #[serde(skip_serializing_if = "Option::is_none")]
    foodtype: Option<String>,
    /// 食物类型中文名（`STRINGS.SCRAPBOOK.FOODTYPE.*`，如 `肉`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    foodtype_zh: Option<String>,
    /// 次要食物类型，仅少数料理有（怪物千层饼、怪物鞑靼、牛饲料）。
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_foodtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_foodtype_zh: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_zh: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    examples: Vec<BundleExample>,
}

#[derive(Debug, Serialize, Clone)]
struct BundleExample {
    cooker: String,
    ingredients: Vec<BundleExampleItem>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
struct BundleExampleItem {
    id: u32,
    count: u32,
}

#[derive(Debug, Serialize)]
struct BundleCharacters {
    schema_version: u32,
    source: BundleSource,
    characters: Vec<BundleCharacter>,
}

#[derive(Debug, Serialize)]
struct BundleCharacter {
    /// `saveslot_portraits` file stem, i.e. the in-game character prefab
    /// (`wathgrithr`, `waxwell`, ...).
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_zh: Option<String>,
    /// Bundle-relative portrait path (`ui/characters/<key>.png`).
    portrait: String,
}

#[derive(Debug, Serialize)]
struct BundleData {
    schema_version: u32,
    cooking_schema_version: u32,
    source: BundleSource,
    cookers: BTreeMap<String, Vec<u32>>,
    ingredients: Vec<BundleIngredient>,
    recipes: Vec<BundleRecipe>,
}

#[derive(Debug, Serialize)]
struct ManifestFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Serialize)]
struct BundleManifest {
    bundle_schema_version: u32,
    cooking_schema_version: u32,
    game_contract_version: u32,
    generated_at_ms: u64,
    source: BundleSource,
    counts: BundleCounts,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missing_icons: Vec<String>,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize)]
struct BundleCounts {
    ingredients: usize,
    recipes: usize,
    characters: usize,
    images: usize,
    examples: usize,
}

#[derive(Debug, Serialize)]
struct ExportReport {
    status: &'static str,
    output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<String>,
    counts: BundleCounts,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missing_icons: Vec<String>,
}

pub fn run_cooking_export(
    params: &CookingGameExportParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("导出烹饪小游戏数据包");
    let out_dir = params
        .output
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/cooking-game-bundle"));

    let data = load_cooking_data(params.snapshot.clone())?;
    reporter.log(format!(
        "已编译：{} 种食材、{} 个食谱（{}）",
        data.ingredients.len(),
        data.recipes.len(),
        data.label
    ));

    let po = load_po_indexes(params.snapshot.as_deref()).unwrap_or_else(|e| {
        reporter.log(format!("PO 名称加载失败，将只输出 prefab 名：{}", e));
        Default::default()
    });
    let names = po.names;
    let foodtype_labels = po.foodtypes;

    let ktools = crate::platform::config::ktools_out_dir();
    let manifests_dir = ktools.join("history/manifests");
    let icons_index = build_icons_index(&manifests_dir)?;
    let icon_meta = icon_meta::load(&ktools).unwrap_or_else(|e| {
        reporter.log(format!("icon_meta 加载失败（仅用同名图标兜底）：{}", e));
        Default::default()
    });
    let icon_resolver = IconResolver::from_index(&icons_index, &icon_meta);
    let split_root = ktools.join("current/split");
    let image_source = split_root.join("inventoryimages");
    let ui_assets: Vec<(String, PathBuf)> = UI_ASSETS
        .iter()
        .map(|(rel, src)| {
            let path = split_root.join(src);
            if path.is_file() {
                Ok(((*rel).to_string(), path))
            } else {
                Err(Error::Config(format!(
                    "缺少小游戏 UI 素材：{}（来自 {}/{}）",
                    rel,
                    ktools.display(),
                    src
                )))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    // --- characters -----------------------------------------------------------------
    let portraits = character_portraits(&split_root)?;
    let (bundle_characters, unnamed_characters) = bundle_characters(&portraits, &po.characters);
    if !unnamed_characters.is_empty() {
        reporter.log(format!(
            "警告：{} 个角色头像在 PO 中没有名字（仍会导出）：{}",
            unnamed_characters.len(),
            unnamed_characters.join(", ")
        ));
    }
    let character_count = bundle_characters.len();
    let mut bundle_ui_assets = ui_assets;
    bundle_ui_assets.extend(
        portraits
            .iter()
            .map(|(key, path)| (format!("{}/{}.png", PORTRAIT_BUNDLE_DIR, key), path.clone())),
    );

    // --- stable IDs -----------------------------------------------------------------
    let mut ingredient_key_to_idx: HashMap<String, usize> = HashMap::new();
    let mut ingredient_prefab_to_idx: HashMap<String, usize> = HashMap::new();
    let mut bundle_ingredients = Vec::with_capacity(data.ingredients.len());
    let mut missing_icons = Vec::new();
    for (idx, ingredient) in data.ingredients.iter().enumerate() {
        let (name_en, name_zh) = names_for(&names, &ingredient.prefab);
        let icon = icon_resolver.resolve(&ingredient.prefab, name_en.as_deref());
        if let Some(file) = &icon {
            if !image_source.join(file).is_file() {
                missing_icons.push(format!("{} -> {}", ingredient.prefab, file));
            }
        } else {
            missing_icons.push(ingredient.prefab.clone());
        }
        ingredient_key_to_idx.insert(ingredient.key.clone(), idx);
        ingredient_prefab_to_idx.insert(ingredient.prefab.clone(), idx);
        bundle_ingredients.push(BundleIngredient {
            id: idx as u32,
            prefab: ingredient.prefab.clone(),
            key: ingredient.key.clone(),
            tags: ingredient.tags.clone(),
            name_en,
            name_zh,
            icon: icon.filter(|file| image_source.join(file).is_file()),
        });
    }

    let mut recipe_name_to_idx: HashMap<String, usize> = HashMap::new();
    let mut bundle_recipes = Vec::with_capacity(data.recipes.len());
    // 描述渲染的名字表：别名 key 与 prefab 都指向显示名（中文优先，缺失回退 prefab）。
    let mut desc_names: HashMap<String, String> = HashMap::new();
    for ingredient in &bundle_ingredients {
        let display = ingredient
            .name_zh
            .clone()
            .unwrap_or_else(|| ingredient.prefab.clone());
        desc_names
            .entry(ingredient.key.clone())
            .or_insert_with(|| display.clone());
        desc_names
            .entry(ingredient.prefab.clone())
            .or_insert_with(|| display.clone());
    }
    for (idx, recipe) in data.recipes.values().enumerate() {
        let (name_en, name_zh) = names_for(&names, &recipe.name);
        let icon = icon_resolver.resolve(&recipe.name, name_en.as_deref());
        if icon.is_none() {
            missing_icons.push(recipe.name.clone());
        }
        let desc = describe_recipe(&recipe.name, &recipe.test, &desc_names)?;
        let foodtype_zh = recipe
            .foodtype
            .as_deref()
            .and_then(|ty| foodtype_label(&foodtype_labels, ty))
            .map(str::to_string);
        let secondary_foodtype_zh = recipe
            .secondary_foodtype
            .as_deref()
            .and_then(|ty| foodtype_label(&foodtype_labels, ty))
            .map(str::to_string);
        recipe_name_to_idx.insert(recipe.name.clone(), idx);
        bundle_recipes.push(BundleRecipe {
            id: idx as u32,
            name: recipe.name.clone(),
            priority: recipe.priority,
            weight: recipe.weight,
            test: recipe.test.clone(),
            desc_attrs: desc.attrs,
            desc_special: desc.specials,
            stats: recipe.stats.clone(),
            temperature: recipe.temperature.clone(),
            foodtype: recipe.foodtype.clone(),
            foodtype_zh,
            secondary_foodtype: recipe.secondary_foodtype.clone(),
            secondary_foodtype_zh,
            name_en,
            name_zh,
            icon: icon.filter(|file| image_source.join(file).is_file()),
            examples: Vec::new(),
        });
    }

    if !missing_icons.is_empty() && !params.allow_missing_icons {
        return Err(Error::Config(format!(
            "有 {} 个食材/食谱缺少图标；修正图标映射或使用 --allow-missing-icons：{}",
            missing_icons.len(),
            missing_icons.join(", ")
        )));
    }

    // --- recipe membership ----------------------------------------------------------
    let cookpot_indices = cooker_indices(&data, "cookpot", &recipe_name_to_idx)?;
    let portable_indices = cooker_indices(&data, "portablecookpot", &recipe_name_to_idx)?;
    let cookpot_set: HashSet<usize> = cookpot_indices.iter().copied().collect();
    let warly_only_indices: Vec<usize> = portable_indices
        .iter()
        .copied()
        .filter(|idx| !cookpot_set.contains(idx))
        .collect();

    // --- example combos -------------------------------------------------------------
    reporter.stage("搜索并验证 example_combo");
    let examples = find_examples(
        &bundle_ingredients,
        &bundle_recipes,
        &cookpot_indices,
        &warly_only_indices,
        reporter,
    )?;
    for (idx, example) in examples.iter().enumerate() {
        if let Some(example) = example {
            bundle_recipes[idx].examples.push(example.clone());
        }
    }
    let example_count = examples.iter().filter(|e| e.is_some()).count();
    if example_count != bundle_recipes.len() {
        let missing: Vec<&str> = examples
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_none())
            .map(|(idx, _)| bundle_recipes[idx].name.as_str())
            .collect();
        return Err(Error::ParseError(format!(
            "无法为 {} 个食谱找到 example_combo：{}",
            missing.len(),
            missing.join(", ")
        )));
    }

    // --- cookers --------------------------------------------------------------------
    let mut cookers = BTreeMap::new();
    cookers.insert(
        "cookpot".to_string(),
        cookpot_indices
            .iter()
            .map(|idx| bundle_recipes[*idx].id)
            .collect::<Vec<_>>(),
    );
    cookers.insert(
        "portablecookpot".to_string(),
        portable_indices
            .iter()
            .map(|idx| bundle_recipes[*idx].id)
            .collect::<Vec<_>>(),
    );

    let bundle = BundleData {
        schema_version: BUNDLE_SCHEMA_VERSION,
        cooking_schema_version: data.schema_version,
        source: BundleSource {
            label: data.label.clone(),
            snapshot: data.snapshot.clone(),
        },
        cookers,
        ingredients: bundle_ingredients,
        recipes: bundle_recipes,
    };

    // --- write ----------------------------------------------------------------------
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)?;
    }
    std::fs::create_dir_all(out_dir.join("data"))?;
    std::fs::create_dir_all(out_dir.join("images"))?;
    std::fs::create_dir_all(out_dir.join("ui"))?;
    std::fs::create_dir_all(out_dir.join(PORTRAIT_BUNDLE_DIR))?;

    for (rel, src) in &bundle_ui_assets {
        std::fs::copy(src, out_dir.join(rel))?;
    }

    let mut image_files = BTreeSet::new();
    for ingredient in &bundle.ingredients {
        if let Some(icon) = &ingredient.icon {
            image_files.insert(icon.clone());
        }
    }
    for recipe in &bundle.recipes {
        if let Some(icon) = &recipe.icon {
            image_files.insert(icon.clone());
        }
    }
    for file in &image_files {
        std::fs::copy(image_source.join(file), out_dir.join("images").join(file))?;
    }

    write_json_atomic(out_dir.join("data/cooking.json"), &bundle)?;
    write_json_atomic(
        out_dir.join("data/characters.json"),
        &BundleCharacters {
            schema_version: CHARACTERS_SCHEMA_VERSION,
            source: bundle.source.clone(),
            characters: bundle_characters,
        },
    )?;
    std::fs::write(out_dir.join("default-config.toml"), DEFAULT_CONFIG_TOML)?;

    // Manifest hashes are computed after data/images exist.
    let mut files = Vec::new();
    for rel in walk_relative_files(&out_dir)? {
        if rel == "manifest.json" {
            continue;
        }
        let path = out_dir.join(&rel);
        let meta = std::fs::metadata(&path)?;
        files.push(ManifestFile {
            path: rel,
            sha256: hash_file(&path)?,
            size: meta.len(),
        });
    }
    let image_count = image_files.len() + bundle_ui_assets.len();
    let manifest = BundleManifest {
        bundle_schema_version: BUNDLE_SCHEMA_VERSION,
        cooking_schema_version: data.schema_version,
        game_contract_version: 1,
        generated_at_ms: crate::scripts_sync::anim::history::now_ms(),
        source: BundleSource {
            label: data.label.clone(),
            snapshot: data.snapshot.clone(),
        },
        counts: BundleCounts {
            ingredients: bundle.ingredients.len(),
            recipes: bundle.recipes.len(),
            characters: character_count,
            images: image_count,
            examples: example_count,
        },
        missing_icons: missing_icons.clone(),
        files,
    };
    write_json_atomic(out_dir.join("manifest.json"), &manifest)?;

    let zip_path = if params.zip {
        let zip_path = zip_path_for(&out_dir);
        write_zip(&out_dir, &zip_path)?;
        Some(zip_path)
    } else {
        None
    };

    reporter.log(format!("已写入 {}", out_dir.display()));
    if let Some(zip) = &zip_path {
        reporter.log(format!("已写入 {}", zip.display()));
    }
    if !missing_icons.is_empty() {
        reporter.log(format!(
            "警告：{} 个条目缺少图标，已按 --allow-missing-icons 继续",
            missing_icons.len()
        ));
    }
    let report = ExportReport {
        status: "success",
        output: out_dir.display().to_string(),
        zip: zip_path.map(|p| p.display().to_string()),
        counts: BundleCounts {
            ingredients: bundle.ingredients.len(),
            recipes: bundle.recipes.len(),
            characters: character_count,
            images: image_count,
            examples: example_count,
        },
        missing_icons,
    };
    Ok(serde_json::to_value(report)?)
}

fn cooker_indices(
    data: &crate::models::CookingData,
    cooker: &str,
    recipe_name_to_idx: &HashMap<String, usize>,
) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    for name in data.cookers.get(cooker).into_iter().flatten() {
        let idx = recipe_name_to_idx.get(name).ok_or_else(|| {
            Error::ParseError(format!("cooker `{}` 引用了未解析的食谱 `{}`", cooker, name))
        })?;
        out.push(*idx);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// character portraits
// ---------------------------------------------------------------------------

/// Portrait file name -> character key, or `None` for non-PNG entries and UI
/// placeholders (`background`, `random`, ...).
fn portrait_key(file_name: &str) -> Option<&str> {
    let key = file_name.strip_suffix(".png")?;
    if key.is_empty()
        || PORTRAIT_PLACEHOLDERS.contains(&key)
        || !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    Some(key)
}

/// Enumerate the character portraits produced by the image pipeline, sorted by
/// key. The directory is authoritative: a character added by a game update is
/// picked up without touching this exporter.
fn character_portraits(split_root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let dir = split_root.join(PORTRAIT_SRC_DIR);
    let entries = std::fs::read_dir(&dir).map_err(|e| {
        Error::Config(format!(
            "缺少角色头像素材目录：{}（来自 {}/{}，{}）",
            dir.display(),
            split_root.display(),
            PORTRAIT_SRC_DIR,
            e
        ))
    })?;
    let mut out = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(key) = portrait_key(name) {
            out.push((key.to_string(), path));
        }
    }
    out.sort();
    if out.is_empty() {
        return Err(Error::Config(format!(
            "角色头像素材目录里没有可用头像：{}",
            dir.display()
        )));
    }
    Ok(out)
}

/// Build `characters.json` entries. Returns the entries plus the keys whose
/// names are missing from the PO (exported anyway, reported by the caller).
fn bundle_characters(
    portraits: &[(String, PathBuf)],
    names: &CharacterNames,
) -> (Vec<BundleCharacter>, Vec<String>) {
    let mut characters = Vec::with_capacity(portraits.len());
    let mut unnamed = Vec::new();
    for (key, _) in portraits {
        let (name_en, name_zh) = character_name(names, key);
        if name_en.is_none() && name_zh.is_none() {
            unnamed.push(key.clone());
        }
        characters.push(BundleCharacter {
            key: key.clone(),
            name_en,
            name_zh,
            portrait: format!("{}/{}.png", PORTRAIT_BUNDLE_DIR, key),
        });
    }
    (characters, unnamed)
}

// ---------------------------------------------------------------------------
// example search
// ---------------------------------------------------------------------------

fn find_examples(
    ingredients: &[BundleIngredient],
    recipes: &[BundleRecipe],
    cookpot_indices: &[usize],
    warly_only_indices: &[usize],
    reporter: &dyn Reporter,
) -> Result<Vec<Option<BundleExample>>> {
    let mut name_refs = BTreeSet::new();
    let mut tag_refs = BTreeSet::new();
    for recipe in recipes {
        collect_field_refs(&recipe.test, &mut name_refs, &mut tag_refs);
    }

    let mut key_to_idx = HashMap::new();
    let mut prefab_to_idx = HashMap::new();
    for (idx, ingredient) in ingredients.iter().enumerate() {
        key_to_idx.insert(ingredient.key.as_str(), idx);
        prefab_to_idx.insert(ingredient.prefab.as_str(), idx);
    }

    let mut pool: Vec<usize> = Vec::new();
    for name in &name_refs {
        if let Some(idx) = key_to_idx
            .get(name.as_str())
            .or_else(|| prefab_to_idx.get(name.as_str()))
        {
            pool.push(*idx);
        }
    }
    for tag in &tag_refs {
        if let Some((idx, _)) = ingredients
            .iter()
            .enumerate()
            .find(|(_, i)| i.tags.get(tag).is_some_and(|v| *v > 0.0))
        {
            pool.push(idx);
        }
    }
    pool.sort_unstable();
    pool.dedup();

    reporter.log(format!(
        "参考字段：{} 个食材名、{} 个 tag；候选池 {} 个食材",
        name_refs.len(),
        tag_refs.len(),
        pool.len()
    ));

    let examples = search_examples(
        ingredients,
        recipes,
        cookpot_indices,
        warly_only_indices,
        &pool,
    )?;
    if examples.iter().all(Option::is_some) {
        return Ok(examples);
    }

    // 初始候选池若覆盖不足，退化为全食材搜索；只在游戏版本新增了
    // 旧池未覆盖的组合时需要走这里。
    let full_pool: Vec<usize> = (0..ingredients.len()).collect();
    if full_pool.len() > pool.len() {
        reporter.log("初始候选池未覆盖全部食谱，改用全食材搜索".to_string());
        return search_examples(
            ingredients,
            recipes,
            cookpot_indices,
            warly_only_indices,
            &full_pool,
        );
    }
    Ok(examples)
}

fn search_examples(
    ingredients: &[BundleIngredient],
    recipes: &[BundleRecipe],
    cookpot_indices: &[usize],
    warly_only_indices: &[usize],
    pool: &[usize],
) -> Result<Vec<Option<BundleExample>>> {
    let mut examples: Vec<Option<BundleExample>> = vec![None; recipes.len()];
    let mut found = 0usize;
    let total = recipes.len();
    let mut combo = Vec::with_capacity(4);

    #[allow(clippy::too_many_arguments)]
    fn visit(
        depth: usize,
        start: usize,
        pool: &[usize],
        combo: &mut Vec<usize>,
        ingredients: &[BundleIngredient],
        recipes: &[BundleRecipe],
        cookpot_indices: &[usize],
        warly_only_indices: &[usize],
        examples: &mut [Option<BundleExample>],
        found: &mut usize,
        total: usize,
    ) -> Result<bool> {
        if depth == 4 {
            let mut env = EvalEnv::new("cookpot");
            for &idx in combo.iter() {
                let ingredient = &ingredients[idx];
                env.add_name(&ingredient.key);
                for (tag, value) in &ingredient.tags {
                    env.add_tag(tag, *value);
                }
            }
            let (base_max, base_top) = top_recipes(cookpot_indices, recipes, &env)?;
            let (warly_max, warly_top) = top_recipes(warly_only_indices, recipes, &env)?;
            if !base_top.is_empty() || !warly_top.is_empty() {
                let combo_ids = combo
                    .iter()
                    .map(|idx| ingredients[*idx].id)
                    .collect::<Vec<_>>();
                if !base_top.is_empty() {
                    let example = example_from(&combo_ids, "cookpot");
                    for &idx in &base_top {
                        if examples[idx].is_none() {
                            examples[idx] = Some(example.clone());
                            *found += 1;
                        }
                    }
                }
                if !warly_top.is_empty() && warly_max > base_max {
                    // Warly-only recipes are the only portable additions; base
                    // recipes still use their valid cookpot example.
                    let example = example_from(&combo_ids, "portablecookpot");
                    for &idx in &warly_top {
                        if examples[idx].is_none() {
                            examples[idx] = Some(example.clone());
                            *found += 1;
                        }
                    }
                } else if !warly_top.is_empty() && warly_max == base_max {
                    // Equal priority: both cookers can yield these; cookpot
                    // examples were already recorded for base_top.
                    let example = example_from(&combo_ids, "portablecookpot");
                    for &idx in &warly_top {
                        if examples[idx].is_none() {
                            examples[idx] = Some(example.clone());
                            *found += 1;
                        }
                    }
                }
                if *found == total {
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        for i in start..pool.len() {
            combo.push(pool[i]);
            if visit(
                depth + 1,
                i,
                pool,
                combo,
                ingredients,
                recipes,
                cookpot_indices,
                warly_only_indices,
                examples,
                found,
                total,
            )? {
                return Ok(true);
            }
            combo.pop();
        }
        Ok(false)
    }

    visit(
        0,
        0,
        pool,
        &mut combo,
        ingredients,
        recipes,
        cookpot_indices,
        warly_only_indices,
        &mut examples,
        &mut found,
        total,
    )?;

    // Verify every example really yields the target recipe under its cooker.
    for (idx, example) in examples.iter().enumerate() {
        if let Some(example) = example {
            let mut env = EvalEnv::new(example.cooker.clone());
            for item in &example.ingredients {
                let ingredient = ingredients
                    .iter()
                    .find(|i| i.id == item.id)
                    .ok_or_else(|| {
                        Error::ParseError(format!("example 引用未知食材 id {}", item.id))
                    })?;
                for _ in 0..item.count {
                    env.add_name(&ingredient.key);
                    for (tag, value) in &ingredient.tags {
                        env.add_tag(tag, *value);
                    }
                }
            }
            let indices = if example.cooker == "portablecookpot" {
                let mut all = cookpot_indices.to_vec();
                all.extend_from_slice(warly_only_indices);
                all
            } else {
                cookpot_indices.to_vec()
            };
            let (_, top) = top_recipes(&indices, recipes, &env)?;
            if !top.contains(&idx) {
                return Err(Error::ParseError(format!(
                    "example_combo 验证失败：`{}` 不在其 cooker 的 top 集合中",
                    recipes[idx].name
                )));
            }
        }
    }

    Ok(examples)
}

fn example_from(ids: &[u32], cooker: &str) -> BundleExample {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    for id in ids {
        *counts.entry(*id).or_insert(0) += 1;
    }
    BundleExample {
        cooker: cooker.to_string(),
        ingredients: counts
            .into_iter()
            .map(|(id, count)| BundleExampleItem { id, count })
            .collect(),
    }
}

fn top_recipes(
    indices: &[usize],
    recipes: &[BundleRecipe],
    env: &EvalEnv,
) -> Result<(f64, Vec<usize>)> {
    let mut max = f64::NEG_INFINITY;
    let mut top = Vec::new();
    for &idx in indices {
        let recipe = &recipes[idx];
        if eval_truthy(&recipe.test, env)
            .map_err(|e| Error::ParseError(format!("食谱 `{}` AST 求值失败：{}", recipe.name, e)))?
        {
            if recipe.priority > max {
                max = recipe.priority;
                top.clear();
            }
            if recipe.priority == max {
                top.push(idx);
            }
        }
    }
    Ok((max, top))
}

// ---------------------------------------------------------------------------
// output helpers
// ---------------------------------------------------------------------------

fn zip_path_for(dir: &Path) -> PathBuf {
    let mut path = dir.to_path_buf();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cooking-game-bundle")
        .to_string();
    path.set_file_name(format!("{}.zip", name));
    path
}

fn walk_relative_files(root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    fn visit(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(base, &path, out)?;
            } else {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|e| Error::InvalidPath(e.to_string()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(rel);
            }
        }
        Ok(())
    }
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn write_zip(src_dir: &Path, zip_path: &Path) -> Result<()> {
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for rel in walk_relative_files(src_dir)? {
        let path = src_dir.join(&rel);
        let name = rel.replace('\\', "/");
        zip.start_file(name, options)?;
        let bytes = std::fs::read(&path)?;
        zip.write_all(&bytes)?;
    }
    zip.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_from_counts_repeats() {
        let example = example_from(&[3, 3, 1, 0], "cookpot");
        assert_eq!(example.cooker, "cookpot");
        assert_eq!(
            example.ingredients,
            vec![
                BundleExampleItem { id: 0, count: 1 },
                BundleExampleItem { id: 1, count: 1 },
                BundleExampleItem { id: 3, count: 2 },
            ]
        );
    }

    #[test]
    fn zip_path_has_bundle_name() {
        assert_eq!(
            zip_path_for(Path::new("output/cooking-game-bundle")),
            PathBuf::from("output/cooking-game-bundle.zip")
        );
    }

    #[test]
    fn default_config_parses_as_toml() {
        let _: toml::Value = toml::from_str(DEFAULT_CONFIG_TOML).unwrap();
    }

    #[test]
    fn manifest_hash_bytes_is_stable() {
        assert_eq!(
            crate::scripts_sync::anim::history::hash_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn portrait_keys_skip_placeholders_and_non_png() {
        assert_eq!(portrait_key("wilson.png"), Some("wilson"));
        assert_eq!(portrait_key("wathgrithr.png"), Some("wathgrithr"));
        assert_eq!(portrait_key("wx78.png"), Some("wx78"));
        for placeholder in PORTRAIT_PLACEHOLDERS {
            assert_eq!(portrait_key(&format!("{}.png", placeholder)), None);
        }
        assert_eq!(portrait_key("wilson.tex"), None);
        assert_eq!(portrait_key(".png"), None);
        assert_eq!(portrait_key("Wilson.png"), None);
        assert_eq!(portrait_key("wilson 2.png"), None);
    }

    #[test]
    fn bundle_characters_resolve_po_names() {
        let portraits = vec![
            (
                "warly".to_string(),
                PathBuf::from("/split/saveslot_portraits/warly.png"),
            ),
            (
                "wonkey".to_string(),
                PathBuf::from("/split/saveslot_portraits/wonkey.png"),
            ),
        ];
        let names = CharacterNames::from([(
            "warly".to_string(),
            (Some("Warly".to_string()), Some("沃利".to_string())),
        )]);
        let (characters, unnamed) = bundle_characters(&portraits, &names);
        assert_eq!(characters.len(), 2);
        assert_eq!(characters[0].key, "warly");
        assert_eq!(characters[0].name_zh.as_deref(), Some("沃利"));
        assert_eq!(characters[0].portrait, "ui/characters/warly.png");
        // Not in the PO: exported with the path only, reported to the caller.
        assert_eq!(characters[1].name_zh, None);
        assert_eq!(characters[1].portrait, "ui/characters/wonkey.png");
        assert_eq!(unnamed, vec!["wonkey".to_string()]);
    }

    #[test]
    fn bundle_recipe_serializes_food_types() {
        let recipe = BundleRecipe {
            id: 7,
            name: "baconeggs".into(),
            priority: 10.0,
            weight: 1.0,
            test: serde_json::json!([true]),
            desc_attrs: String::new(),
            desc_special: String::new(),
            stats: CookingStats {
                health: 20.0,
                hunger: 37.5,
                sanity: 5.0,
            },
            temperature: None,
            foodtype: Some("MEAT".into()),
            foodtype_zh: Some("肉".into()),
            secondary_foodtype: None,
            secondary_foodtype_zh: None,
            name_en: None,
            name_zh: None,
            icon: None,
            examples: Vec::new(),
        };
        let value = serde_json::to_value(&recipe).unwrap();
        assert_eq!(value["foodtype"], "MEAT");
        assert_eq!(value["foodtype_zh"], "肉");
        // Absent secondary type stays out of the payload entirely.
        assert!(value.get("secondary_foodtype").is_none());
        assert!(value.get("secondary_foodtype_zh").is_none());
        assert_eq!(value["stats"]["health"], 20.0);
        assert_eq!(value["stats"]["hunger"], 37.5);
        assert_eq!(value["stats"]["sanity"], 5.0);
        // No temperature: the key stays out; `no_chill` is dropped when false.
        assert!(value.get("temperature").is_none());

        let warm = BundleRecipe {
            temperature: Some(CookingTemperature {
                delta: 40.0,
                duration: 5.0,
                no_chill: false,
            }),
            ..recipe
        };
        let value = serde_json::to_value(&warm).unwrap();
        assert_eq!(value["temperature"]["delta"], 40.0);
        assert_eq!(value["temperature"]["duration"], 5.0);
        assert!(value["temperature"].get("no_chill").is_none());
    }
}
