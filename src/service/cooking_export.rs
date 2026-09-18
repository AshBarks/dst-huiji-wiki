//! Standalone cooking-game package exporter.
//!
//! Produces a self-contained directory (and optionally a zip) consumed by the
//! independent `dst-cooking-game` project. The package contains the compiled
//! recipe AST, selectable ingredients, icons, and one verified `example_combo`
//! per recipe so the game backend can guarantee every refresh is solvable.

use crate::error::{Error, Result};
use crate::platform::fs::write_json_atomic;
use crate::platform::progress::Reporter;
use crate::scripts_sync::anim::history::hash_file;
use crate::scripts_sync::images::icons::build_icons_index;
use crate::scripts_sync::images::meta as icon_meta;
use crate::service::cooking::load_cooking_data;
use crate::service::cooking_assets::{load_po_name_index, names_for, IconResolver};
use crate::service::cooking_desc::describe_recipe;
use crate::service::cooking_eval::{collect_field_refs, eval_truthy, EvalEnv};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BUNDLE_SCHEMA_VERSION: u32 = 2;

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

#[derive(Debug, Serialize)]
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

    let names = load_po_name_index(params.snapshot.as_deref()).unwrap_or_else(|e| {
        reporter.log(format!("PO 名称加载失败，将只输出 prefab 名：{}", e));
        HashMap::new()
    });

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
        recipe_name_to_idx.insert(recipe.name.clone(), idx);
        bundle_recipes.push(BundleRecipe {
            id: idx as u32,
            name: recipe.name.clone(),
            priority: recipe.priority,
            weight: recipe.weight,
            test: recipe.test.clone(),
            desc_attrs: desc.attrs,
            desc_special: desc.specials,
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

    for (rel, src) in &ui_assets {
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
    let image_count = image_files.len() + ui_assets.len();
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
}
