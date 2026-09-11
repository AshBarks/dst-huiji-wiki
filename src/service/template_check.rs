//! `maintain-template-check`：只读检查 `模板:Tech/dst` 与 `模板:制作栏图标`
//! 对游戏数据的覆盖率，并输出可粘贴的补录片段。
//!
//! 检查项：
//! - 科技：recipes.lua 在用（error）> constants.lua 已定义（warn）> 白名单
//!   兜底（模板独有键，info）；
//! - 制作栏：PO 分类/制作站中文名（含 tuning.lua 派生的站中文名）vs
//!   模板 `filter|dst` 分支的 case 键（error），并给出片段；
//! - 图标：分支内 `{{inv|X}}` 的 File 存在性（`history/icon_meta.json`
//!   优先，可选 live `get_files_info`；`%27` 这类错误写法会被指出）；
//! - 数据层：`模块:Constants/CraftingNames` 是否缺少 tuning.lua 派生的
//!   制作站别名（由 `maintain-copy-clip --type names` 写入）。
//!
//! 本作业永不写维基。

use super::make_ctx;
use super::progress::Reporter;
use crate::error::{Error, Result};
use crate::models::{derive_station_aliases, StationAliasInputs, TechReport};
use crate::parser::{
    parse_crafting_filter_lists, parse_prototyper_trees, parse_tech_constants, RecipeParser,
};
use crate::DstContext;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 检查配置（可选；文件缺失时用默认值）。
pub const CONFIG_PATH: &str = "config/template_check.json";
/// 图标本地文件名 → 维基文件名映射（复用 images-sync 的表）。
pub const ICON_OVERRIDES_PATH: &str = "config/icon_title_overrides.json";

const TECH_TEMPLATE_TITLE: &str = "模板:Tech/dst";
const CRAFTING_TEMPLATE_TITLE: &str = "模板:制作栏图标";
const RENDERRECS_TITLE: &str = "模块:RenderRecsByIngre/Data";
const CRAFTING_NAMES_TITLE: &str = "模块:Constants/CraftingNames";
const STATION_FILTER_PREFIX: &str = "STRINGS.UI.CRAFTING_STATION_FILTERS.";

/// 一次 [`run`] 调用的参数。
#[derive(Debug, Clone, Default)]
pub struct TemplateCheckParams {
    /// 把可粘贴片段写入该文件（可选）。
    pub output: Option<PathBuf>,
    /// 游戏脚本快照目录名（可选）。
    pub snapshot: Option<String>,
    /// 跳过 live 图标存在性查询（只用 `icon_meta.json`）。
    pub skip_icon_status: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TemplateCheckConfig {
    /// 模板独有科技键白名单（裸键，如 `CITY`、`MAGIC_ONE`）。
    #[serde(default)]
    tech_whitelist: BTreeSet<String>,
    /// 不纳入检查的制作分类键（如 `FAVORITES`）。
    #[serde(default)]
    crafting_filter_ignore: BTreeSet<String>,
    /// PO 中文名 → 模板里使用的等价键（如 `所有` → `全部`）。
    #[serde(default)]
    crafting_cn_alias: BTreeMap<String, String>,
    /// 站筛键人工兜底：科技树键 → PO 站筛键。
    #[serde(default)]
    station_alias_fallback: BTreeMap<String, String>,
    /// LOST 站配方策略：`report`（默认）。
    #[serde(default = "default_lost_policy")]
    lost_station_policy: String,
}

fn default_lost_policy() -> String {
    "report".to_string()
}

fn load_config() -> Result<TemplateCheckConfig> {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Ok(TemplateCheckConfig::default());
    }
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| Error::Config(format!("解析 {CONFIG_PATH} 失败：{e}")))
}

/// 本地图标文件名（含 `.png`）→ 维基文件名。
fn load_icon_titles() -> Result<BTreeMap<String, String>> {
    let path = Path::new(ICON_OVERRIDES_PATH);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|e| Error::Config(format!("解析 {ICON_OVERRIDES_PATH} 失败：{e}")))
}

// ---------------------------------------------------------------------------
// 纯函数：wikitext 提取 / 标题归一化
// ---------------------------------------------------------------------------

/// 从模板文本提取所有 `TECH.<KEY>` 常量名（返回去掉前缀的裸键）。
fn extract_tech_keys(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(pos) = rest.find("TECH.") {
        let after = &rest[pos + "TECH.".len()..];
        let key: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if key.is_empty() {
            rest = after;
        } else {
            rest = &after[key.len()..];
            out.insert(key);
        }
    }
    out
}

/// `模板:制作栏图标` 的 `filter|dst` 分支：中文 case 键与 `{{inv|X}}` 图标名。
#[derive(Debug, Default, PartialEq, Eq)]
struct CraftingBranch {
    keys: BTreeSet<String>,
    icons: BTreeSet<String>,
}

fn parse_crafting_branch(text: &str) -> CraftingBranch {
    let Some(start) = text.find("|filter") else {
        return CraftingBranch::default();
    };
    let rest = &text[start..];
    let end = rest.find("|#default").unwrap_or(rest.len());
    let branch = &rest[..end];

    let mut out = CraftingBranch::default();
    for line in branch.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('|') {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            for part in trimmed[1..eq].split('|') {
                let key = part.trim();
                if !key.is_empty() && !key.starts_with('#') {
                    out.keys.insert(key.to_string());
                }
            }
        }
    }

    let mut rest = branch;
    while let Some(pos) = rest.find("{{inv|") {
        let after = &rest[pos + "{{inv|".len()..];
        let name: String = after
            .chars()
            .take_while(|c| *c != '|' && *c != '}')
            .collect();
        let name = name.trim();
        if name.is_empty() {
            rest = after;
        } else {
            rest = &after[name.len()..];
            out.icons.insert(name.to_string());
        }
    }
    out
}

/// 去掉 MediaWiki 文件标题前缀。
fn strip_file_prefix(s: &str) -> &str {
    s.trim()
        .trim_start_matches("File:")
        .trim_start_matches("文件:")
        .trim()
}

/// MediaWiki 标题归一化：去 `File:`/`文件:` 前缀、`_`≡空格、忽略首字母大小写。
fn normalize_title(s: &str) -> String {
    strip_file_prefix(s).replace('_', " ").trim().to_lowercase()
}

/// 最小 `%XX` 解码（用于给错误写法提示正确标题）。
fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// 报告类型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct Snippet {
    target: &'static str,
    line: String,
    reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct TechCheck {
    used: Vec<String>,
    defined: Vec<String>,
    template: Vec<String>,
    used_missing_template: Vec<String>,
    defined_only_missing_template: Vec<String>,
    template_only_whitelisted: Vec<String>,
    template_only_unknown: Vec<String>,
    renderrecs_missing_used: Vec<String>,
    snippets: Vec<Snippet>,
}

#[derive(Debug, Clone, Serialize)]
struct MissingEntry {
    key: String,
    cn: String,
    is_station: bool,
}

#[derive(Debug, Clone, Serialize)]
struct IconIssue {
    /// 模板里的原始写法（含 `.png`）。
    name: String,
    /// 修正建议（存在时）。
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct CraftingCheck {
    missing: Vec<MissingEntry>,
    icons_missing: Vec<IconIssue>,
    icons_unknown: Vec<String>,
    snippets: Vec<Snippet>,
}

#[derive(Debug, Clone, Serialize)]
struct AliasEntry {
    tree_key: String,
    filter_key: String,
    en: String,
    cn: String,
    sample_recipes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AliasCheck {
    /// 已派生但线上 CraftingNames 缺失的别名。
    missing: Vec<AliasEntry>,
    /// 派生未决项（如 TECH.LOST 多键）。
    unresolved: Vec<crate::models::UnresolvedTreeKey>,
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

pub async fn run(
    params: &TemplateCheckParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let config = load_config()?;
    let icon_titles = load_icon_titles()?;
    let mut ctx = make_ctx(&params.snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki_mut().login().await?;

    reporter.stage("读取游戏数据");
    let constants = ctx.read_script_file("scripts/constants.lua")?;
    let tuning = ctx.read_script_file("scripts/tuning.lua")?;
    let recipes_source = ctx.read_script_file("scripts/recipes.lua")?;
    let filters_source = ctx.read_script_file("scripts/recipes_filter.lua")?;
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;

    let tech_constants = parse_tech_constants(&constants)?;
    let prototyper_trees = parse_prototyper_trees(&tuning)?;
    let filter_lists = parse_crafting_filter_lists(&filters_source)?;
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_source, Some("scripts/recipes.lua"))?;
    let prototyper_defs = parser.context().prototyper_defs.clone();

    let (po_craftings, po_stations) = collect_crafting_names(&po_file);

    reporter.stage("拉取维基页面");
    let tech_template = fetch_page_required(&mut ctx, TECH_TEMPLATE_TITLE, reporter).await?;
    let crafting_template =
        fetch_page_required(&mut ctx, CRAFTING_TEMPLATE_TITLE, reporter).await?;
    let renderrecs = fetch_page_optional(&mut ctx, RENDERRECS_TITLE, reporter).await;
    let live_crafting_names = fetch_page_optional(&mut ctx, CRAFTING_NAMES_TITLE, reporter).await;

    // ---- 科技覆盖 -------------------------------------------------------
    let tech_template_keys = extract_tech_keys(&tech_template);
    let used: BTreeSet<String> = recipes
        .iter()
        .map(|r| {
            r.tech
                .strip_prefix("TECH.")
                .unwrap_or(r.tech.as_str())
                .to_string()
        })
        .collect();
    let defined: BTreeSet<String> = tech_constants.keys().cloned().collect();

    let used_missing: BTreeSet<String> = used.difference(&tech_template_keys).cloned().collect();
    let defined_only: BTreeSet<String> = defined.difference(&used).cloned().collect();
    let defined_only_missing: BTreeSet<String> = defined_only
        .difference(&tech_template_keys)
        .cloned()
        .collect();
    let template_only: BTreeSet<String> =
        tech_template_keys.difference(&defined).cloned().collect();
    let template_only_whitelisted: BTreeSet<String> = template_only
        .iter()
        .filter(|k| config.tech_whitelist.contains(*k))
        .cloned()
        .collect();
    let template_only_unknown: BTreeSet<String> = template_only
        .difference(&template_only_whitelisted)
        .cloned()
        .collect();

    let mut renderrecs_missing_used = Vec::new();
    if let Some(content) = &renderrecs {
        let mut report = TechReport::from_recipes(&recipes);
        report.compare_with_wiki(content);
        renderrecs_missing_used = report.extra_in_parsed.clone();
    }

    let station_icon_titles = station_icon_titles(&prototyper_defs, &icon_titles);
    let known_station_keys: BTreeSet<String> = po_stations.keys().cloned().collect();
    let tech_snippets = build_tech_snippets(
        &used_missing,
        &tech_constants,
        &prototyper_trees,
        &known_station_keys,
        &po_stations,
        &station_icon_titles,
    );

    let tech_check = TechCheck {
        used: used.iter().cloned().collect(),
        defined: defined.iter().cloned().collect(),
        template: tech_template_keys.iter().cloned().collect(),
        used_missing_template: used_missing.iter().cloned().collect(),
        defined_only_missing_template: defined_only_missing.iter().cloned().collect(),
        template_only_whitelisted: template_only_whitelisted.iter().cloned().collect(),
        template_only_unknown: template_only_unknown.iter().cloned().collect(),
        renderrecs_missing_used,
        snippets: tech_snippets,
    };

    // ---- 制作栏图标覆盖 --------------------------------------------------
    let branch = parse_crafting_branch(&crafting_template);
    let expected = crafting_expectations(
        &po_craftings,
        &po_stations,
        &filter_lists,
        &config.crafting_filter_ignore,
    );
    let mut missing = Vec::new();
    for (key, cn, is_station) in &expected {
        let wanted = config
            .crafting_cn_alias
            .get(cn)
            .cloned()
            .unwrap_or_else(|| cn.clone());
        if !branch.keys.contains(&wanted) {
            missing.push(MissingEntry {
                key: key.clone(),
                cn: cn.clone(),
                is_station: *is_station,
            });
        }
    }
    let crafting_snippets =
        build_crafting_snippets(&missing, &station_icon_titles, &config.crafting_cn_alias);

    let (icons_missing, icons_unknown) =
        check_icon_files(&branch.icons, &ctx, params.skip_icon_status, reporter).await?;

    let crafting_check = CraftingCheck {
        missing,
        icons_missing,
        icons_unknown,
        snippets: crafting_snippets,
    };

    // ---- CraftingNames 别名 ---------------------------------------------
    let recipe_techs: BTreeMap<String, String> = recipes
        .iter()
        .map(|r| (r.name.clone(), r.tech.clone()))
        .collect();
    let station_recipes = filter_lists
        .get("CRAFTING_STATION")
        .cloned()
        .unwrap_or_default();
    let alias_report = derive_station_aliases(&StationAliasInputs {
        tech_constants: &tech_constants,
        prototyper_trees: &prototyper_trees,
        known_station_keys: &known_station_keys,
        station_recipes: &station_recipes,
        recipe_techs: &recipe_techs,
        fallback: &config.station_alias_fallback,
    });
    let live_keys = live_crafting_names
        .as_deref()
        .map(parse_crafting_names_keys)
        .unwrap_or_default();
    let alias_missing: Vec<AliasEntry> = alias_report
        .aliases
        .iter()
        .filter(|a| !live_keys.contains(&a.tree_key))
        .map(|a| {
            let (en, cn) = po_stations
                .get(&a.filter_key)
                .cloned()
                .unwrap_or_else(|| (a.filter_key.clone(), a.filter_key.clone()));
            AliasEntry {
                tree_key: a.tree_key.clone(),
                filter_key: a.filter_key.clone(),
                en,
                cn,
                sample_recipes: a.sample_recipes.clone(),
            }
        })
        .collect();
    let alias_check = AliasCheck {
        missing: alias_missing,
        unresolved: alias_report.unresolved,
    };

    report_to_reporter(
        &tech_check,
        &crafting_check,
        &alias_check,
        &config,
        branch.keys.len(),
        branch.icons.len(),
        reporter,
    );

    let mut snippets_text = String::new();
    let all_snippets: Vec<(&'static str, &Snippet)> = tech_check
        .snippets
        .iter()
        .map(|s| ("模板:Tech/dst", s))
        .chain(
            crafting_check
                .snippets
                .iter()
                .map(|s| ("模板:制作栏图标", s)),
        )
        .collect();
    if !all_snippets.is_empty() {
        snippets_text.push_str("# maintain-template-check 建议片段\n\n");
        let mut last_target = "";
        for (target, snippet) in &all_snippets {
            if *target != last_target {
                if !last_target.is_empty() {
                    snippets_text.push('\n');
                }
                snippets_text.push_str(&format!("# {target}\n"));
                last_target = target;
            }
            snippets_text.push_str(&format!("{}\n", snippet.line));
        }
        if !alias_check.missing.is_empty() {
            snippets_text.push_str(
                "\n# 模块:Constants/CraftingNames（由 maintain-copy-clip --type names 自动写入）\n",
            );
            for a in &alias_check.missing {
                snippets_text.push_str(&format!(
                    "\"{}\": {{\"station_en\": \"{}\", \"station_cn\": \"{}\"}},\n",
                    a.tree_key, a.en, a.cn
                ));
            }
        }
    }
    if let Some(path) = &params.output {
        std::fs::write(path, &snippets_text)?;
        reporter.log(format!("片段已写入 {}", path.display()));
    }

    Ok(serde_json::json!({
        "status": "ok",
        "tech": tech_check,
        "crafting_icons": crafting_check,
        "crafting_names_aliases": alias_check,
        "lost_station_policy": config.lost_station_policy,
        "snippets_path": params.output.as_ref().map(|p| p.display().to_string()),
    }))
}

fn report_to_reporter(
    tech: &TechCheck,
    crafting: &CraftingCheck,
    aliases: &AliasCheck,
    config: &TemplateCheckConfig,
    branch_keys: usize,
    branch_icons: usize,
    reporter: &dyn Reporter,
) {
    reporter.stage("科技模板覆盖率");
    reporter.log(format!(
        "在用 {} / 已定义 {} / 模板收录 {}",
        tech.used.len(),
        tech.defined.len(),
        tech.template.len()
    ));
    if !tech.used_missing_template.is_empty() {
        reporter.log(format!(
            "[缺·在用] {} 个：{}",
            tech.used_missing_template.len(),
            tech.used_missing_template.join(", ")
        ));
    }
    if !tech.defined_only_missing_template.is_empty() {
        reporter.log(format!(
            "[缺·已定义未使用] {} 个：{}",
            tech.defined_only_missing_template.len(),
            tech.defined_only_missing_template.join(", ")
        ));
    }
    if !tech.template_only_unknown.is_empty() {
        reporter.log(format!(
            "[模板独有·非白名单] {} 个：{}",
            tech.template_only_unknown.len(),
            tech.template_only_unknown.join(", ")
        ));
    }
    if !tech.renderrecs_missing_used.is_empty() {
        reporter.log(format!(
            "[RenderRecsByIngre/Data 缺] {} 个：{}",
            tech.renderrecs_missing_used.len(),
            tech.renderrecs_missing_used.join(", ")
        ));
    }

    reporter.stage("制作栏图标覆盖率");
    reporter.log(format!(
        "模板分支 case 键 {branch_keys} / inv 图标 {branch_icons}"
    ));
    if !crafting.missing.is_empty() {
        reporter.log(format!("[缺·制作栏] {} 个：", crafting.missing.len()));
        for m in &crafting.missing {
            reporter.log(format!(
                "  - {}（{}）{}",
                m.cn,
                m.key,
                if m.is_station { "站" } else { "分类" }
            ));
        }
    }
    for issue in &crafting.icons_missing {
        match &issue.suggestion {
            Some(s) => reporter.log(format!("[图标缺失] {}（建议：{}）", issue.name, s)),
            None => reporter.log(format!("[图标缺失] {}", issue.name)),
        }
    }
    if !crafting.icons_unknown.is_empty() {
        reporter.log(format!(
            "[图标未查] {} 个：{}",
            crafting.icons_unknown.len(),
            crafting.icons_unknown.join(", ")
        ));
    }

    reporter.stage("CraftingNames 别名");
    if aliases.missing.is_empty() {
        reporter.log("已派生别名均已存在于线上模块。".to_string());
    } else {
        for a in &aliases.missing {
            reporter.log(format!(
                "[缺别名] {}→{}（{}），样例：{}",
                a.tree_key,
                a.filter_key,
                a.cn,
                a.sample_recipes
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    for u in &aliases.unresolved {
        reporter.log(format!(
            "[别名未决] {}（{:?}），样例：{}",
            u.tree_key,
            u.reason,
            u.sample_recipes
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if config.lost_station_policy == "report" {
        reporter.log("LOST 站配方：仅报告（策略 report）。".to_string());
    }
}

// ---------------------------------------------------------------------------
// 辅助
// ---------------------------------------------------------------------------

async fn fetch_page_required(
    ctx: &mut DstContext,
    title: &str,
    reporter: &dyn Reporter,
) -> Result<String> {
    reporter.log(format!("获取 {title}…"));
    let page = ctx.wiki().get_page(title).await?;
    page.content
        .ok_or_else(|| Error::WikiApi(format!("页面 {title} 无内容")))
}

async fn fetch_page_optional(
    ctx: &mut DstContext,
    title: &str,
    reporter: &dyn Reporter,
) -> Option<String> {
    match ctx.wiki().get_page(title).await {
        Ok(page) => page.content,
        Err(e) => {
            reporter.log(format!("警告：获取 {title} 失败：{e}"));
            None
        }
    }
}

/// PO 里的制作分类与制作站中英文名：key → (en, cn)。
#[allow(clippy::type_complexity)]
fn collect_crafting_names(
    po: &crate::models::PoFile,
) -> (
    BTreeMap<String, (String, String)>,
    BTreeMap<String, (String, String)>,
) {
    let mut craftings = BTreeMap::new();
    let mut stations = BTreeMap::new();
    for entry in &po.entries {
        if let Some(ctx) = entry.msgctxt.as_deref() {
            if let Some(key) = ctx.strip_prefix("STRINGS.UI.CRAFTING_FILTERS.") {
                if !entry.msgstr.is_empty() {
                    craftings.insert(key.to_string(), (entry.msgid.clone(), entry.msgstr.clone()));
                }
            } else if let Some(key) = ctx.strip_prefix(STATION_FILTER_PREFIX) {
                if !entry.msgstr.is_empty() {
                    stations.insert(key.to_string(), (entry.msgid.clone(), entry.msgstr.clone()));
                }
            }
        }
    }
    (craftings, stations)
}

/// 站筛键 → 建议图标文件名（本地 `station_*.png` → 映射表标题）。
fn station_icon_titles(
    defs: &[crate::models::PrototyperDef],
    icon_titles: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for def in defs {
        let Some(filter_key) = def
            .filter_text
            .as_deref()
            .and_then(|p| p.strip_prefix(STATION_FILTER_PREFIX))
        else {
            continue;
        };
        let Some(icon) = def.icon_image.as_deref() else {
            continue;
        };
        let local = format!("{}.png", icon.trim_end_matches(".tex"));
        let title = icon_titles
            .get(&local)
            .cloned()
            .unwrap_or_else(|| local.trim_end_matches(".png").to_string());
        out.entry(filter_key.to_string()).or_insert(title);
    }
    out
}

/// 期望在 `模板:制作栏图标` 里出现的中文键：`(key, cn, is_station)`。
fn crafting_expectations(
    po_craftings: &BTreeMap<String, (String, String)>,
    po_stations: &BTreeMap<String, (String, String)>,
    filter_lists: &BTreeMap<String, Vec<String>>,
    ignore: &BTreeSet<String>,
) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    // 标准制作分类：只取 recipes_filter.lua 里有静态配方表的分类；
    // CRAFTING_STATION 由 wiki 端按站映射，不在此列。
    for (key, recipes) in filter_lists {
        if key == "CRAFTING_STATION" || ignore.contains(key) || recipes.is_empty() {
            continue;
        }
        if let Some((_, cn)) = po_craftings.get(key) {
            out.push((key.clone(), cn.clone(), false));
        }
    }
    // 制作站：全部 PO 站筛键。
    for (key, (_, cn)) in po_stations {
        if ignore.contains(key) {
            continue;
        }
        out.push((key.clone(), cn.clone(), true));
    }
    out
}

fn build_tech_snippets(
    used_missing: &BTreeSet<String>,
    tech_constants: &BTreeMap<String, Vec<String>>,
    prototyper_trees: &BTreeMap<String, Vec<String>>,
    known_station_keys: &BTreeSet<String>,
    po_stations: &BTreeMap<String, (String, String)>,
    station_icon_titles: &BTreeMap<String, String>,
) -> Vec<Snippet> {
    let mut snippets = Vec::new();
    for key in used_missing {
        let tree_key = tech_constants.get(key).and_then(|v| v.first()).cloned();
        let station_key = tree_key.as_deref().and_then(|tree| {
            prototyper_trees
                .iter()
                .filter(|(_, keys)| keys.iter().any(|k| k == tree))
                .map(|(entry, _)| entry.clone())
                .find(|entry| known_station_keys.contains(entry))
        });
        let (alias, link, icon) = match &station_key {
            Some(station) => {
                let cn = po_stations
                    .get(station)
                    .map(|(_, cn)| cn.clone())
                    .unwrap_or_else(|| station.clone());
                let icon = station_icon_titles
                    .get(station)
                    .cloned()
                    .unwrap_or_else(|| "Placeholder.png".to_string());
                (format!("|{cn}"), cn, icon)
            }
            None => (String::new(), String::new(), "Placeholder.png".to_string()),
        };
        let comment = tree_key
            .as_deref()
            .map(|t| format!(" <!-- 科技树键：{t} -->"))
            .unwrap_or_default();
        snippets.push(Snippet {
            target: TECH_TEMPLATE_TITLE,
            line: format!("|TECH.{key}{alias}=[[文件:{icon}|32px|link={link}]]{comment}"),
            reason: "recipes-in-use",
        });
    }
    snippets
}

fn build_crafting_snippets(
    missing: &[MissingEntry],
    station_icon_titles: &BTreeMap<String, String>,
    cn_alias: &BTreeMap<String, String>,
) -> Vec<Snippet> {
    let mut snippets = Vec::new();
    for m in missing {
        // `{{inv|X}}` 由模块自行补 `.png`，这里去掉后缀。
        let icon = if m.is_station {
            station_icon_titles
                .get(&m.key)
                .map(|t| t.trim_end_matches(".png").to_string())
                .unwrap_or_else(|| "Placeholder".to_string())
        } else {
            // 分类图标无本地映射，占位待人工按「英文名 + Filter」惯例补。
            "Placeholder".to_string()
        };
        let wanted = cn_alias.get(&m.cn).cloned().unwrap_or_else(|| m.cn.clone());
        // 尾部 `{{{width|32}}}}}` 是模板原文（宽参 + inv + switch 的闭合），
        // 用普通字符串拼接避免 format! 的转义歧义。
        let line =
            format!("|{wanted}={{{{inv|{icon}|link=制作#{wanted}|width=") + "{{{width|32}}}}}";
        snippets.push(Snippet {
            target: CRAFTING_TEMPLATE_TITLE,
            line,
            reason: if m.is_station {
                "station-missing"
            } else {
                "filter-missing"
            },
        });
    }
    snippets
}

/// 解析线上 `模块:Constants/CraftingNames` 的 `crafting_stations` 键集合。
fn parse_crafting_names_keys(content: &str) -> BTreeSet<String> {
    let Some(start) = content.find("[[") else {
        return BTreeSet::new();
    };
    let Some(end) = content.rfind("]]") else {
        return BTreeSet::new();
    };
    if start >= end {
        return BTreeSet::new();
    }
    let json = &content[start + 2..end];
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return BTreeSet::new(),
    };
    value
        .get("crafting_stations")
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// 图标存在性：`icon_meta.json` 优先；未收录且允许时查 live。
///
/// 返回 `(缺失, 未查)`。模板里 `%27` 这类错误写法会以原始名报缺失，并在
/// live 查询可用时附上 `%XX` 解码后的修正建议。
async fn check_icon_files(
    icons: &BTreeSet<String>,
    ctx: &DstContext,
    skip_live: bool,
    reporter: &dyn Reporter,
) -> Result<(Vec<IconIssue>, Vec<String>)> {
    // 本地 icon_meta 索引：归一化标题 → exists。
    let out_dir = std::env::var("KTOOLS__OUT_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/ktools"));
    let mut meta_titles: BTreeMap<String, bool> = BTreeMap::new();
    if let Ok(meta) = crate::scripts_sync::images::meta::load(&out_dir) {
        for entry in meta.icons.values() {
            if let Some(title) = entry.title.as_deref() {
                let exists = entry.wiki.as_ref().and_then(|w| w.exists).unwrap_or(false);
                meta_titles.insert(normalize_title(title), exists);
            }
        }
    }

    let mut missing: Vec<IconIssue> = Vec::new();
    let mut live_needed: Vec<String> = Vec::new(); // icon_meta 未收录的原始名
    for name in icons {
        let title = format!("{name}.png");
        match meta_titles.get(&normalize_title(&title)) {
            Some(true) => {}
            Some(false) => missing.push(IconIssue {
                name: title,
                suggestion: None,
            }),
            None => live_needed.push(title),
        }
    }

    if !skip_live && !live_needed.is_empty() {
        reporter.log(format!("查询 {} 个图标的维基存在性…", live_needed.len()));
        let client = ctx.wiki();
        let names: Vec<&str> = live_needed.iter().map(String::as_str).collect();
        match client.get_files_info(&names).await {
            Ok(infos) => {
                for (raw, info) in live_needed.iter().zip(infos.iter()) {
                    if info.missing {
                        missing.push(IconIssue {
                            name: raw.clone(),
                            suggestion: None,
                        });
                    }
                }
            }
            Err(e) => reporter.log(format!("警告：图标存在性查询失败：{e}")),
        }
    }

    // 缺失项若含 `%XX`，尝试用解码标题在线验证并给出建议。
    if !skip_live {
        let probes: Vec<(usize, String)> = missing
            .iter()
            .enumerate()
            .filter_map(|(i, issue)| {
                let decoded = percent_decode(&issue.name);
                (decoded != issue.name).then_some((i, decoded))
            })
            .collect();
        if !probes.is_empty() {
            let probe_names: Vec<&str> = probes.iter().map(|(_, d)| d.as_str()).collect();
            match ctx.wiki().get_files_info(&probe_names).await {
                Ok(infos) => {
                    for ((i, _), info) in probes.iter().zip(infos.iter()) {
                        if !info.missing {
                            missing[*i].suggestion =
                                Some(strip_file_prefix(&info.title).to_string());
                        }
                    }
                }
                Err(e) => reporter.log(format!("警告：图标修正建议查询失败：{e}")),
            }
        }
    }

    missing.sort_by(|a, b| a.name.cmp(&b.name));
    let unknown = if skip_live { live_needed } else { Vec::new() };
    Ok((missing, unknown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tech_keys() {
        let text = "|TECH.NONE|无 = x\n|TECH.SCIENCE_ONE|科学1 = y\n|#default=TECH.LOST\n{{tech|TECH.MAGIC_TWO}}";
        let keys = extract_tech_keys(text);
        assert!(keys.contains("NONE"));
        assert!(keys.contains("SCIENCE_ONE"));
        assert!(keys.contains("LOST"));
        assert!(keys.contains("MAGIC_TWO"));
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn test_parse_crafting_branch() {
        let text = r#"<includeonly>{{#switch: {{{1|ds}}}
|filter
|dst ={{#switch: {{{2|工具}}}
|工具={{inv|Tools Filter|link=制作#工具|width={{{width|32}}}}}
|光源|照明={{inv|Light Sources Filter|link=制作#光源|width={{{width|32}}}}}
|#default = {{{2|}}}[[分类:未收录的制作栏图标/{{{2|}}}]]}}
|tab
|ds = {{#switch: {{{2|工具}}}
|工具 = [[File:Icon_Tools.png]]
|#default = x}}
|TECH = {{tech|{{{2|}}}}}
|#default = 请输入类型}}</includeonly>"#;
        let branch = parse_crafting_branch(text);
        assert!(branch.keys.contains("工具"));
        assert!(branch.keys.contains("光源"));
        assert!(branch.keys.contains("照明"));
        // 只解析 filter 分支，tab 分支的 Icon_Tools 不会混入
        assert!(branch.icons.contains("Tools Filter"));
        assert!(branch.icons.contains("Light Sources Filter"));
        assert_eq!(branch.icons.len(), 2);
        // TECH 分支的占位符不进入
        assert!(!branch.keys.contains("请输入类型"));
    }

    #[test]
    fn test_normalize_title_and_percent_decode() {
        assert_eq!(
            normalize_title("文件:Winter's_Fest.png"),
            "winter's fest.png"
        );
        assert_eq!(normalize_title("File:Axe.png"), "axe.png");
        assert_eq!(percent_decode("Winter%27s_Filter"), "Winter's_Filter");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn test_crafting_expectations_skip_station_and_ignore() {
        let mut craftings = BTreeMap::new();
        craftings.insert(
            "TOOLS".to_string(),
            ("Tools".to_string(), "工具".to_string()),
        );
        craftings.insert(
            "FAVORITES".to_string(),
            ("Favorites".to_string(), "收藏夹".to_string()),
        );
        let mut stations = BTreeMap::new();
        stations.insert(
            "CARPENTRY".to_string(),
            ("Carpentry".to_string(), "木工".to_string()),
        );
        let mut lists = BTreeMap::new();
        lists.insert("TOOLS".to_string(), vec!["axe".to_string()]);
        lists.insert("FAVORITES".to_string(), vec!["x".to_string()]);
        lists.insert("CRAFTING_STATION".to_string(), vec!["y".to_string()]);
        let ignore: BTreeSet<String> = ["FAVORITES".to_string()].into_iter().collect();

        let got = crafting_expectations(&craftings, &stations, &lists, &ignore);
        assert_eq!(
            got,
            vec![
                ("TOOLS".to_string(), "工具".to_string(), false),
                ("CARPENTRY".to_string(), "木工".to_string(), true),
            ]
        );
    }

    #[test]
    fn test_parse_crafting_names_keys() {
        let content = "local json = [[{\n \"crafting_stations\": {\"A\":{},\"B\":{}},\n \"craftings\": {}\n}]]\nreturn json";
        let keys = parse_crafting_names_keys(content);
        let expected: BTreeSet<String> = ["A".to_string(), "B".to_string()].into_iter().collect();
        assert_eq!(keys, expected);
    }
}
