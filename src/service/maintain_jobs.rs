//! 维基维护作业核心实现（ItemTable / DstRecipes / CopyClip）。
//!
//! 自 service/mod.rs 下沉；写站尾段统一走 [`super::output`] 与
//! [`super::wiki_write`]。

use super::make_ctx;
use super::output::{output_copyclip_result_with_update, output_json_result_with_update, WriteCtx};
use crate::error::{Error, Result};
use crate::mapping::{compare_and_report, WikiDataConverter, WikiMapper};
use crate::models::{derive_station_aliases, PoEntry, StationAliasInputs, StationAliasReport};
use crate::parser::{
    extract_field_assignment_range, parse_crafting_filter_lists, parse_prototyper_trees,
    parse_tech_constants, RecipeParser,
};
use crate::platform::progress::Reporter;
use crate::platform::progress::WriteMode;
use crate::{DstContext, TechReport};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub(super) async fn run_maintain_item_table(
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    reporter.stage("解析 chinese_s.po");
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
    let names_entries: Vec<PoEntry> = po_file
        .entries
        .iter()
        .filter(|e| {
            e.msgctxt
                .as_ref()
                .map(|ctx: &String| ctx.starts_with("STRINGS.NAMES."))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    reporter.log(format!("找到 {} 条 NAMES 条目", names_entries.len()));

    let converter = WikiDataConverter::new();

    reporter.stage("拉取维基历史数据");
    let page_title = "Data:ItemTable.tabx";
    let historical_data = match ctx.wiki().get_json_data(page_title).await {
        Ok(historical_json) => Some(WikiDataConverter::parse_wiki_json(
            &historical_json.to_string(),
        )?),
        Err(e) => {
            reporter.log(format!("警告：获取历史数据失败：{}", e));
            None
        }
    };

    let sources = ctx.sources();
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的实体代码、中英文名及图片对照表"
    }));
    let mut wiki_data = converter.convert_to_wiki_json(&names_entries, &sources, description);

    let mut merged = false;
    if let Some(ref historical) = historical_data {
        PoEntry::merge_with_history(&mut wiki_data, historical);
        reporter.log(compare_and_report(&wiki_data, historical));
        merged = true;
    }

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };
    let summary = output_json_result_with_update(&wctx, page_title, &wiki_data, output).await?;
    Ok(serde_json::json!({ "merged_with_history": merged, "summary": summary }))
}

pub(super) async fn run_maintain_dst_recipes(
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    reporter.stage("解析 recipes.lua");
    let recipes_string = ctx.read_script_file("scripts/recipes.lua")?;

    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_string, Some("scripts/recipes.lua"))?;
    reporter.log(format!("共 {} 个配方", recipes.len()));

    reporter.stage("对比维基科技数据");
    let mut tech_report = TechReport::from_recipes(&recipes);
    match ctx.wiki().get_page("模块:RenderRecsByIngre/Data").await {
        Ok(page) => {
            if let Some(content) = &page.content {
                tech_report.compare_with_wiki(content);
                reporter.log(tech_report.generate_report());
            } else {
                reporter.log("警告：维基页面无内容".to_string());
            }
        }
        Err(e) => {
            reporter.log(format!("警告：获取科技数据失败：{}", e));
        }
    }

    reporter.stage("解析 chinese_s.po（描述查询）");
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
    reporter.log(format!("载入 {} 条 PO 条目", po_file.entries.len()));

    let converter = WikiDataConverter::with_po_entries(po_file.entries.clone());

    reporter.stage("拉取维基历史数据");
    let page_title = "Data:DSTRecipes.tabx";
    let historical_data = match ctx.wiki().get_json_data(page_title).await {
        Ok(historical_json) => Some(WikiDataConverter::parse_wiki_json(
            &historical_json.to_string(),
        )?),
        Err(e) => {
            reporter.log(format!("警告：获取历史数据失败：{}", e));
            None
        }
    };

    let sources = ctx.sources();
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的合成配方列表"
    }));
    let mut wiki_data = converter.convert_recipes(&recipes, &sources, description);

    let mut merged = false;
    if let Some(ref historical) = historical_data {
        crate::models::Recipe::merge_with_history(&mut wiki_data, historical);
        reporter.log(compare_and_report(&wiki_data, historical));
        merged = true;
    }

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };
    let summary = output_json_result_with_update(&wctx, page_title, &wiki_data, output).await?;
    Ok(
        serde_json::json!({ "merged_with_history": merged, "recipe_count": recipes.len(), "summary": summary }),
    )
}

pub(super) async fn run_maintain_copyclip(
    r#type: Option<&str>,
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    let types_to_run: Vec<String> = if let Some(t) = r#type {
        vec![t.to_lowercase()]
    } else {
        vec![
            "recipe_builder_tag_lookup".to_string(),
            "tech".to_string(),
            "crafting_filters".to_string(),
            "crafting_names".to_string(),
        ]
    };

    let mut results = serde_json::Map::new();
    for t in types_to_run {
        reporter.stage(&format!("运行 {}", t));
        let summary = match t.as_str() {
            "recipe_builder_tag_lookup" | "rbtl" => {
                maintain_recipe_builder_tag_lookup(&mut ctx, output.clone(), reporter, mode).await?
            }
            "tech" => maintain_tech(&mut ctx, output.clone(), reporter, mode).await?,
            "crafting_filters" | "filters" => {
                maintain_crafting_filters(&mut ctx, output.clone(), reporter, mode).await?
            }
            "crafting_names" | "names" => {
                maintain_crafting_names(&mut ctx, output.clone(), reporter, mode).await?
            }
            _ => {
                return Err(Error::Config(format!(
                    "未知的 copyclip 类型: {}。有效类型: recipe_builder_tag_lookup (rbtl), tech, crafting_filters (filters), crafting_names (names)",
                    t
                )));
            }
        };
        results.insert(t, summary);
    }

    Ok(serde_json::Value::Object(results))
}

pub(super) async fn maintain_recipe_builder_tag_lookup(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let debugcommands_string = ctx.read_script_file("scripts/debugcommands.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/RecipeBuilderTagLookup";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 debugcommands.lua 提取 RECIPE_BUILDER_TAG_LOOKUP...".to_string());
    let result = crate::copyclip::process_copyclip(
        &debugcommands_string,
        "RECIPE_BUILDER_TAG_LOOKUP",
        &target_content,
    )?;

    reporter.log(format!(
        "CopyClip 完成！提取内容长度: {} 字节",
        result.extracted_content.len()
    ));

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &result.updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

pub(super) async fn maintain_tech(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let constants_string = ctx.read_script_file("scripts/constants.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/Tech";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 constants.lua 提取 TECH...".to_string());
    let result = crate::copyclip::process_copyclip(&constants_string, "TECH", &target_content)?;

    reporter.log(format!(
        "CopyClip 完成！提取内容长度: {} 字节",
        result.extracted_content.len()
    ));

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &result.updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

pub(super) async fn maintain_crafting_filters(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let filter_string = ctx.read_script_file("scripts/recipes_filter.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/CraftingFilters";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 recipes_filter.lua 提取 CRAFTING_FILTERS 配方列表...".to_string());
    let field_location = extract_field_assignment_range(
        &filter_string,
        "CRAFTING_FILTERS.CHARACTER.recipes",
        "CRAFTING_FILTERS.DECOR.recipes",
    )?;

    reporter.log(format!(
        "提取内容长度: {} 字节",
        field_location.content.len()
    ));

    let marker_range = crate::copyclip::CopyClipProcessor::find_marker_range(&target_content)?;
    let updated_content = crate::copyclip::CopyClipProcessor::replace_between_markers(
        &target_content,
        &marker_range,
        &field_location.content,
    );

    reporter.log("CopyClip 完成！".to_string());

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

/// 读取游戏侧四张表并派生 CraftingNames 需要补的制作站别名。
///
/// 派生链：`constants.lua TECH` → 科技树键 →（`tuning.lua`
/// `PROTOTYPER_TREES` 反查）→ 条目名（即 PO 站筛键，如
/// `CARNIVALGAME_GOLFGAME`）。任一步读文件/解析失败即返回 `Err`，
/// 由调用方决定降级。
pub(super) fn derive_crafting_station_aliases(
    ctx: &mut DstContext,
    known_station_keys: &BTreeSet<String>,
) -> Result<StationAliasReport> {
    let constants = ctx.read_script_file("scripts/constants.lua")?;
    let tuning = ctx.read_script_file("scripts/tuning.lua")?;
    let recipes_source = ctx.read_script_file("scripts/recipes.lua")?;
    let filters_source = ctx.read_script_file("scripts/recipes_filter.lua")?;

    let tech_constants = parse_tech_constants(&constants)?;
    let prototyper_trees = parse_prototyper_trees(&tuning)?;
    let filter_lists = parse_crafting_filter_lists(&filters_source)?;

    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_source, Some("scripts/recipes.lua"))?;
    let recipe_techs: BTreeMap<String, String> = recipes
        .iter()
        .map(|r| (r.name.clone(), r.tech.clone()))
        .collect();

    let station_recipes = filter_lists
        .get("CRAFTING_STATION")
        .cloned()
        .unwrap_or_default();
    let fallback = BTreeMap::new();

    Ok(derive_station_aliases(&StationAliasInputs {
        tech_constants: &tech_constants,
        prototyper_trees: &prototyper_trees,
        known_station_keys,
        station_recipes: &station_recipes,
        recipe_techs: &recipe_techs,
        fallback: &fallback,
    }))
}

pub(super) async fn maintain_crafting_names(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;

    let station_prefix = "STRINGS.UI.CRAFTING_STATION_FILTERS.";
    let filter_prefix = "STRINGS.UI.CRAFTING_FILTERS.";

    let mut crafting_stations: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut craftings: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for entry in &po_file.entries {
        if let Some(ref entry_ctx) = entry.msgctxt {
            if let Some(key) = entry_ctx.strip_prefix(station_prefix) {
                crafting_stations.insert(
                    key.to_string(),
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            } else if let Some(key) = entry_ctx.strip_prefix(filter_prefix) {
                craftings.insert(
                    key.to_string(),
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            }
        }
    }

    // D2：补制作站别名。游戏侧科技树键与 PO 站筛键不同名时（如
    // CARNIVAL_GOLFPROPS ↔ CARNIVALGAME_GOLFGAME），wiki 端
    // `模块:DSTRecipe.get_recipe_filter_cn` 会 assert，必须在
    // crafting_stations 里保留一个别名键。派生失败只警告，不阻断生成。
    let known_station_keys: BTreeSet<String> = crafting_stations.keys().cloned().collect();
    let mut alias_added = Vec::new();
    match derive_crafting_station_aliases(ctx, &known_station_keys) {
        Ok(report) => {
            for alias in &report.aliases {
                if crafting_stations.contains_key(&alias.tree_key) {
                    continue;
                }
                if let Some(target) = crafting_stations.get(&alias.filter_key).cloned() {
                    crafting_stations.insert(alias.tree_key.clone(), target);
                    alias_added.push(format!("{}→{}", alias.tree_key, alias.filter_key));
                }
            }
            if !alias_added.is_empty() {
                reporter.log(format!(
                    "已补 {} 条制作站别名：{}",
                    alias_added.len(),
                    alias_added.join(", ")
                ));
            }
            for u in &report.unresolved {
                let total = u.sample_recipes.len();
                let samples = u.sample_recipes.iter().take(5).cloned().collect::<Vec<_>>();
                let suffix = if total > samples.len() {
                    format!(" …（共 {} 条）", total)
                } else {
                    String::new()
                };
                reporter.log(format!(
                    "警告：制作站别名未决 {}（{:?}），样例配方：{}{}",
                    u.tree_key,
                    u.reason,
                    samples.join(", "),
                    suffix
                ));
            }
        }
        Err(e) => reporter.log(format!("警告：制作站别名派生失败：{e}")),
    }

    let stations_len = crafting_stations.len();
    let craftings_len = craftings.len();

    let crafting_names = serde_json::json!({
        "crafting_stations": crafting_stations,
        "craftings": craftings
    });

    let json_content = serde_json::to_string_pretty(&crafting_names)?;
    reporter.log(format!(
        "找到 {} 个制作站点和 {} 个制作分类",
        stations_len, craftings_len
    ));

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/CraftingNames";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("定位 [[ 与 ]] 标记...".to_string());
    let start_marker = "[[";
    let end_marker = "]]";

    let start_pos = target_content
        .find(start_marker)
        .ok_or_else(|| Error::ParseError("'[[' marker not found".to_string()))?;
    let end_pos = target_content
        .rfind(end_marker)
        .ok_or_else(|| Error::ParseError("']]' marker not found".to_string()))?;

    if start_pos >= end_pos {
        return Err(Error::ParseError(
            "'[[' must appear before ']]'".to_string(),
        ));
    }

    let updated_content = format!(
        "{}{}\n{}",
        &target_content[..start_pos + start_marker.len()],
        json_content,
        &target_content[end_pos..]
    );

    reporter.log("CopyClip 完成！".to_string());

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

// ---------------------------------------------------------------------------
// Shared output helpers (wiki-write confirmation flows through the reporter)
// ---------------------------------------------------------------------------
