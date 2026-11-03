//! 游戏翻译表（`strings.pot` / `chinese_s.po` / 技能树标题）→ 图标显示名。

use crate::error::{Error, Result};
use crate::models::PoFile;
use crate::parser::PoParser;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::overrides::is_icon_file_name;
use super::title::file_stem_key;
use super::IconGroup;

// -- 名称解析 ----------------------------------------------------------------

/// 一个字符串命名空间的双语表（键为大写）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bilingual {
    /// 英文（msgid）。
    pub en: BTreeMap<String, String>,
    /// 中文（msgstr，仅非空翻译）。
    pub zh: BTreeMap<String, String>,
}

impl Bilingual {
    /// 键的中英文；英文必须存在，中文可缺（返回 `None`）。
    pub fn get(&self, key: &str) -> Option<(String, Option<String>)> {
        let en = self.en.get(key)?;
        Some((en.clone(), self.zh.get(key).cloned()))
    }

    pub fn is_empty(&self) -> bool {
        self.en.is_empty() && self.zh.is_empty()
    }
}

/// 游戏翻译表中图标命名相关命名空间的中英文名表（键为大写）。
#[derive(Debug, Clone, Default)]
pub struct NameMaps {
    /// `STRINGS.NAMES.<KEY>`（物品名）。
    pub names: Bilingual,
    /// `STRINGS.UI.CRAFTING_FILTERS.<KEY>`（制作栏过滤器名）。
    pub crafting_filters: Bilingual,
    /// `STRINGS.UI.CRAFTING_STATION_FILTERS.<KEY>`（制作站过滤名）。
    pub station_filters: Bilingual,
}

impl NameMaps {
    pub fn is_empty(&self) -> bool {
        self.names.en.is_empty()
            && self.crafting_filters.en.is_empty()
            && self.station_filters.en.is_empty()
    }
}

/// 解析图标命名相关命名空间条目。`english_pot` 提供英文全集（可补 chinese
/// 缺失的键）；`chinese_po` 同时提供英文兜底与中文翻译。
pub fn parse_name_maps(chinese_po: &str, english_pot: Option<&str>) -> Result<NameMaps> {
    let zh_file = PoParser::parse(chinese_po)?;
    let mut maps = NameMaps::default();
    collect_names(&zh_file, &mut maps, false);
    if let Some(pot) = english_pot {
        let en_file = PoParser::parse(pot)?;
        collect_names(&en_file, &mut maps, true);
    }
    Ok(maps)
}

/// 按 `msgctxt` 前缀路由到对应命名空间；`english_wins = true` 时覆盖已有
/// 英文名（strings.pot 为准）。
fn collect_names(file: &PoFile, maps: &mut NameMaps, english_wins: bool) {
    for e in &file.entries {
        let Some(ctx) = e.msgctxt.as_deref() else {
            continue;
        };
        let (bilingual, key) = if let Some(k) = ctx.strip_prefix("STRINGS.NAMES.") {
            (&mut maps.names, k)
        } else if let Some(k) = ctx.strip_prefix("STRINGS.UI.CRAFTING_FILTERS.") {
            (&mut maps.crafting_filters, k)
        } else if let Some(k) = ctx.strip_prefix("STRINGS.UI.CRAFTING_STATION_FILTERS.") {
            (&mut maps.station_filters, k)
        } else {
            continue;
        };
        let key = key.to_ascii_uppercase();
        let msgid = e.msgid.trim();
        if !msgid.is_empty() && (english_wins || !bilingual.en.contains_key(&key)) {
            bilingual.en.insert(key.clone(), msgid.to_string());
        }
        if !english_wins {
            let msgstr = e.msgstr.trim();
            if !msgstr.is_empty() {
                bilingual.zh.insert(key, msgstr.to_string());
            }
        }
    }
}

/// 读取游戏翻译表：优先 live `data/databundles/scripts/languages/`，
/// 回退 `scripts.zip`。
pub fn read_name_maps(dst_root: &Path) -> Result<NameMaps> {
    let chinese = read_game_text(dst_root, "languages/chinese_s.po")?;
    let pot = read_game_text(dst_root, "languages/strings.pot").ok();
    parse_name_maps(&chinese, pot.as_deref())
}

/// 制作栏图标（`source=crafting`）的显示名解析表：由游戏 Lua 定义推导，
/// 把本地图标文件名映射到 [`NameMaps`] 中的翻译键。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CraftingKeys {
    /// filter 图标 stem 大写（如 `FILTER_TOOL`）→ `CRAFTING_FILTERS` 键。
    /// 多个过滤器共用同一图标时取定义序最后一个（`filter_none` 归
    /// `EVERYTHING` 而非 `CRAFTING_STATION`，与 wiki 既有命名一致）。
    pub filters: BTreeMap<String, String>,
    /// station 图标 stem 大写 → 定义序首个非空 `CRAFTING_STATION_FILTERS` 键。
    pub station_filters: BTreeMap<String, String>,
    /// station 图标 stem 大写 → 定义序首个 prefab 的 `NAMES` 键（无
    /// `filter_text` 的站图标回退显示名用，如 `STATION_CARPENTRY` →
    /// `CARPENTRY_STATION` = "Sawhorse"）。
    pub station_prefabs: BTreeMap<String, String>,
}

impl CraftingKeys {
    /// 三个映射均为空时视为没有可用制作栏命名数据。
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
            && self.station_filters.is_empty()
            && self.station_prefabs.is_empty()
    }

    /// 由 [`crate::parser::crafting`] 的两个 Lua 表解析产物构建。
    pub fn from_defs(
        filters: &[crate::parser::crafting::FilterIconDef],
        prototypers: &[crate::parser::crafting::PrototyperIconDef],
    ) -> CraftingKeys {
        let mut keys = CraftingKeys::default();
        for def in filters {
            keys.filters.insert(
                def.image.to_ascii_uppercase(),
                def.name.to_ascii_uppercase(),
            );
        }
        for def in prototypers {
            let stem = def.image.to_ascii_uppercase();
            if let Some(key) = &def.filter_key {
                keys.station_filters
                    .entry(stem.clone())
                    .or_insert_with(|| key.to_ascii_uppercase());
            }
            keys.station_prefabs
                .entry(stem)
                .or_insert_with(|| def.prefab.to_ascii_uppercase());
        }
        keys
    }

    /// 图标文件（如 `filter_tool.png` / `station_carpentry.png`）的显示名：
    /// filter 图标查 `CRAFTING_FILTERS`，station 图标优先
    /// `CRAFTING_STATION_FILTERS`、回退 prefab 的 `NAMES`。查不到返回 `None`。
    pub fn display_name(&self, names: &NameMaps, file: &str) -> Option<(String, Option<String>)> {
        let stem = file_stem_key(file);
        if let Some(key) = self.filters.get(&stem) {
            return names.crafting_filters.get(key);
        }
        if let Some(key) = self.station_filters.get(&stem) {
            if let Some(found) = names.station_filters.get(key) {
                return Some(found);
            }
        }
        self.station_prefabs
            .get(&stem)
            .and_then(|key| names.names.get(key))
    }

    /// 图标文件的语义分组：filter/station 按解析链命中；station 无过滤名时
    /// `key = None`（显示名走 prefab 回退）。两表都未命中的图标返回 `None`。
    pub fn group(&self, file: &str) -> Option<IconGroup> {
        let stem = file_stem_key(file);
        if let Some(key) = self.filters.get(&stem) {
            return Some(IconGroup::Crafting {
                section: "filter".to_string(),
                key: Some(key.clone()),
            });
        }
        if let Some(key) = self.station_filters.get(&stem) {
            return Some(IconGroup::Crafting {
                section: "station".to_string(),
                key: Some(key.clone()),
            });
        }
        self.station_prefabs
            .contains_key(&stem)
            .then_some(IconGroup::Crafting {
                section: "station".to_string(),
                key: None,
            })
    }
}

/// 读取制作栏图标的显示名解析表：`recipes_filter.lua` + `recipes.lua`
///（live 树或 `scripts.zip`，与 [`read_name_maps`] 同源）。
pub fn read_crafting_keys(dst_root: &Path) -> Result<CraftingKeys> {
    let source = crate::platform::game_source::GameSource::new(dst_root.to_path_buf(), None)?;
    let filters =
        crate::parser::crafting::parse_crafting_filter_defs(&source.read("recipes_filter.lua")?)?;
    let prototypers = crate::parser::crafting::parse_prototyper_defs(&source.read("recipes.lua")?)?;
    Ok(CraftingKeys::from_defs(&filters, &prototypers))
}

// -- 技能树图标命名 -----------------------------------------------------------

/// 技能树图标的名称解析表（随 images-sync 构建）：
/// 图标 → 节点内联 `title` 键（解析器提取，回退节点名约定）→
/// PO `STRINGS.SKILLTREE.<角色>.<键>_TITLE` 双语名，例外表最后兜底。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkilltreeNames {
    /// icon stem 大写 → (角色大写, TITLE 键大写)。
    pub icon_nodes: BTreeMap<String, (String, String)>,
    /// (角色大写, TITLE 键大写) → (英文 msgid, 中文 msgstr 仅非空)。
    pub titles: SkillTitleMap,
    /// 例外表（`config/skilltree_icon_names.json`）：本地文件名 → (英文, 中文)。
    pub extra: BTreeMap<String, (String, String)>,
}

impl SkilltreeNames {
    pub fn is_empty(&self) -> bool {
        self.icon_nodes.is_empty() && self.titles.is_empty() && self.extra.is_empty()
    }

    /// 图标文件的中英文名：例外表优先，其次节点表 + PO；都没有返回 `None`。
    pub fn display_name(&self, file: &str) -> Option<(String, Option<String>)> {
        if let Some((en, zh)) = self.extra.get(file) {
            return Some((en.clone(), Some(zh.clone())));
        }
        let (char_name, key) = self.icon_nodes.get(&file_stem_key(file))?;
        let (en, zh) = self.titles.get(&(char_name.clone(), key.clone()))?;
        Some((en.clone(), zh.clone()))
    }

    /// 图标所属角色（小写）：节点表命中用其角色，否则回退文件名首段前缀
    /// （角色名不含下划线，孤儿图标也总有分组）。
    pub fn character(&self, file: &str) -> String {
        if let Some((char_name, _)) = self.icon_nodes.get(&file_stem_key(file)) {
            return char_name.to_lowercase();
        }
        file.split('_').next().unwrap_or("other").to_lowercase()
    }
}

/// `STRINGS.SKILLTREE.<角色>.<键>_TITLE` → `(角色大写, 键大写)`，形态不符返回
/// `None`。
pub(crate) fn split_skilltree_title_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("STRINGS.SKILLTREE.")?;
    let key = rest.strip_suffix("_TITLE")?;
    let (char_name, key) = key.split_once('.')?;
    Some((char_name.to_ascii_uppercase(), key.to_ascii_uppercase()))
}

/// 本地别名形态的 title 引用（如 `SKILLTREESTRINGS.<键>_TITLE`）→ 键大写；
/// 取末段并剥 `_TITLE` 后缀，剥不出时用末段原样。
fn skilltree_alias_key(path: &str) -> String {
    let last = path.rsplit('.').next().unwrap_or(path);
    last.strip_suffix("_TITLE")
        .unwrap_or(last)
        .to_ascii_uppercase()
}

/// 技能树双语名表：(角色, TITLE 键) → (英文 msgid, 中文 msgstr 仅非空)。
pub type SkillTitleMap = BTreeMap<(String, String), (String, Option<String>)>;

/// 解析 PO 中 `STRINGS.SKILLTREE.<角色>.<键>_TITLE` 条目为双语名表。
pub fn parse_skilltree_titles(chinese_po: &str) -> Result<SkillTitleMap> {
    let file = PoParser::parse(chinese_po)?;
    let mut out = BTreeMap::new();
    for e in &file.entries {
        let Some((pair, en, zh)) = (|| {
            let ctx = e.msgctxt.as_deref()?;
            if !ctx.starts_with("STRINGS.SKILLTREE.") || !ctx.ends_with("_TITLE") {
                return None;
            }
            let (pair, en, zh) = (
                split_skilltree_title_path(ctx)?,
                e.msgid.trim(),
                e.msgstr.trim(),
            );
            if en.is_empty() {
                return None;
            }
            Some((
                pair,
                en.to_string(),
                (!zh.is_empty()).then(|| zh.to_string()),
            ))
        })() else {
            continue;
        };
        out.insert(pair, (en, zh));
    }
    Ok(out)
}

/// 技能树图标例外表路径。
pub fn skilltree_icon_names_path() -> PathBuf {
    crate::platform::config::skilltree_icon_names_path()
}

/// 例外表条目（`{ "en": ..., "zh": ... }`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct SkillNameEntry {
    pub en: String,
    pub zh: String,
}

/// 读取技能树图标例外表；文件不存在返回空表（不是错误）。
pub fn load_skilltree_extra(path: &Path) -> Result<BTreeMap<String, (String, String)>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text = std::fs::read_to_string(path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("读取 {}: {}", path.display(), e),
        ))
    })?;
    let raw: BTreeMap<String, SkillNameEntry> = serde_json::from_str(&text).map_err(|e| {
        Error::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("解析 {}: {}", path.display(), e),
        )))
    })?;
    let mut out = BTreeMap::new();
    for (file, entry) in raw {
        if !is_icon_file_name(&file) {
            return Err(Error::Config(format!(
                "例外表 {} 的键 {file:?} 不是有效的图标文件名",
                path.display()
            )));
        }
        let en = entry.en.trim();
        let zh = entry.zh.trim();
        if en.is_empty() || zh.is_empty() {
            return Err(Error::Config(format!(
                "例外表 {} 中 {file:?} 的 en/zh 不能为空",
                path.display()
            )));
        }
        out.insert(file, (en.to_string(), zh.to_string()));
    }
    Ok(out)
}

/// 读取技能树图标命名表：PO `SKILLTREE` 命名空间 + 解析全部
/// `prefabs/skilltree_<角色>.lua`（角色清单来自 PO TITLE 键）建节点表，
/// 合并例外表。单角色读取/解析失败降级记日志（不影响其余角色）。
pub fn read_skilltree_names(dst_root: &Path) -> Result<SkilltreeNames> {
    let chinese = read_game_text(dst_root, "languages/chinese_s.po")?;
    let titles = parse_skilltree_titles(&chinese)?;
    let chars: std::collections::BTreeSet<String> = titles.keys().map(|(c, _)| c.clone()).collect();
    let source = crate::platform::game_source::GameSource::new(dst_root.to_path_buf(), None)?;
    let mut icon_nodes = BTreeMap::new();
    for char_name in &chars {
        let rel = format!("prefabs/skilltree_{}.lua", char_name.to_lowercase());
        let lua = match source.read(&rel) {
            Ok(lua) => lua,
            Err(e) => {
                tracing::warn!("跳过技能树命名：读取 {rel} 失败：{e}");
                continue;
            }
        };
        let tree = match crate::parser::skilltree::parse_skill_tree(&lua, &char_name.to_lowercase())
        {
            Ok(tree) => tree,
            Err(e) => {
                tracing::warn!("跳过技能树命名：解析 {rel} 失败：{e}");
                continue;
            }
        };
        for node in &tree.nodes {
            let Some(icon) = &node.icon else {
                continue;
            };
            // 内联 title 引用优先（角色与键取自引用原文）；游戏动态循环
            // (`data.title = ..._STRINGS[uppercase_name.."_TITLE"]`) 的默认键
            // 即节点名，故解析器未给出内联引用时回退节点名约定。部分角色文件
            // 用本地别名（如 `SKILLTREESTRINGS.<键>_TITLE` = 角色命名空间），
            // 此时键取末段、角色用解析角色。
            let (char_key, key) = node
                .title_key
                .as_deref()
                .map(|path| {
                    split_skilltree_title_path(path)
                        .unwrap_or_else(|| (char_name.clone(), skilltree_alias_key(path)))
                })
                .unwrap_or_else(|| (char_name.clone(), node.name.to_ascii_uppercase()));
            icon_nodes.insert(icon.to_ascii_uppercase(), (char_key, key));
        }
    }
    let extra = load_skilltree_extra(&skilltree_icon_names_path())?;
    Ok(SkilltreeNames {
        icon_nodes,
        titles,
        extra,
    })
}

fn read_game_text(dst_root: &Path, rel: &str) -> Result<String> {
    // 统一走 GameSource（live → scripts.zip）；snapshot 语义由调用方决定。
    crate::platform::game_source::GameSource::new(dst_root.to_path_buf(), None)?.read(rel)
}
