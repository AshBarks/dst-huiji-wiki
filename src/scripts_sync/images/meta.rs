//! 物品图标元数据：`split/inventoryimages/` 与 `split/crafting_menu_icons/`
//! 文件名 → 生效的维基文件名（`title`，来自映射表或游戏内英文名）与上传状态。
//!
//! 由 `images-sync` 生成并落盘为 `history/icon_meta.json`（派生数据，可随时
//! 由 manifest + 游戏翻译表 + 映射表重建）；WebUI 与
//! [`crate::service::upload_icons`] 消费同一份产物，避免页面加载时再解析
//! PO / 查询维基。
//!
//! 约定：
//! - 物品栏图标收录文件名 stem（大写，如 `abigail_flower.png` →
//!   `ABIGAIL_FLOWER`）能在 `STRINGS.NAMES.*` 精确命中的，以及映射表
//!   [`IconTitleOverrides`] 中显式指定的（可为无英文名的皮肤/变体）；
//! - 制作栏图标（`source=crafting`）走 [`CraftingKeys`] 解析链：filter 图标
//!   经 `recipes_filter.lua` 的 `CRAFTING_FILTER_DEFS` 对到
//!   `STRINGS.UI.CRAFTING_FILTERS.*`，station 图标经 `recipes.lua` 的
//!   `PROTOTYPER_DEFS` 对到 `STRINGS.UI.CRAFTING_STATION_FILTERS.*`（无
//!   `filter_text` 的回退 prefab 的 `STRINGS.NAMES.*` 显示名）；
//! - 英文名以 `strings.pot` 为准（覆盖全部键），中文名取 `chinese_s.po`
//!   非空 msgstr；只有英文没有中文时照常保留英文；
//! - 生效标题优先级：映射表（按本地文件名）> 英文名自动生成；自动标题对
//!   制作栏图标追加 `Filter` / `Station Icon` 后缀（与 wiki
//!   分类:制作栏图标 的主流命名一致）。标题含 `{}` 等 MediaWiki 非法字符
//!   或 `/` 的条目标记 `uploadable = false` 并附 `note`，上传作业跳过
//!   （可用映射表或弹窗手动命名解决）；
//! - 映射表为 `config/icon_title_overrides.json`（扁平 JSON：本地文件名 →
//!   wiki 文件名，均含 `.png`），可用 `ICON__TITLE_OVERRIDES` 覆盖路径；
//! - wiki 状态按生效标题（去重后）查询，同名文件共享同一状态；标题变化时
//!   旧状态失效并重查。

use crate::error::{Error, Result};
use crate::models::PoFile;
use crate::parser::PoParser;
use crate::scripts_sync::images::history::now_ms;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 元数据在产物根目录下的相对路径。
pub const META_REL_PATH: &str = "history/icon_meta.json";

/// 映射表的默认路径（相对 CWD）；可用 `ICON__TITLE_OVERRIDES` 覆盖。
pub const OVERRIDES_DEFAULT_PATH: &str = "config/icon_title_overrides.json";

/// 本地图标文件名（含 `.png`）→ 维基实际文件名（含 `.png`）的例外映射表。
///
/// 扁平 JSON：`{ "multitool_axe_pickaxe.png": "Pick-Axe.png" }`。
/// 由 `images-sync` 应用进 [`IconMeta::icons`]；WebUI 弹窗可直接编辑。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IconTitleOverrides(pub BTreeMap<String, String>);

impl IconTitleOverrides {
    /// 查映射（返回裸文件名，含 `.png`）。
    pub fn get(&self, file: &str) -> Option<&str> {
        self.0.get(file).map(String::as_str)
    }

    /// 设置/覆盖映射。
    pub fn insert(&mut self, file: &str, title: &str) {
        self.0.insert(file.to_string(), title.to_string());
    }

    /// 删除映射，返回旧值。
    pub fn remove(&mut self, file: &str) -> Option<String> {
        self.0.remove(file)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// 映射表路径：`ICON__TITLE_OVERRIDES`（非空时）否则 [`OVERRIDES_DEFAULT_PATH`]。
pub fn overrides_path() -> PathBuf {
    crate::platform::config::icon_title_overrides_path()
}

/// 读取映射表；文件不存在时返回空表（不是错误）。
///
/// 键必须是含 `.png` 的裸文件名；值经 [`normalize_explicit_title`] 规范化，
/// 非法条目返回带上下文的 [`Error::Config`]。
pub fn load_overrides(path: &Path) -> Result<IconTitleOverrides> {
    if !path.exists() {
        return Ok(IconTitleOverrides::default());
    }
    let text = std::fs::read_to_string(path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("读取 {}: {}", path.display(), e),
        ))
    })?;
    let raw: BTreeMap<String, String> = serde_json::from_str(&text).map_err(|e| {
        Error::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("解析 {}: {}", path.display(), e),
        )))
    })?;
    let mut out = BTreeMap::new();
    for (file, title) in raw {
        if !is_icon_file_name(&file) {
            return Err(Error::Config(format!(
                "映射表 {} 的键 {file:?} 不是有效的图标文件名",
                path.display()
            )));
        }
        let title = normalize_explicit_title(&title).map_err(|e| {
            Error::Config(format!(
                "映射表 {} 中 {file:?} 的标题无效: {e}",
                path.display()
            ))
        })?;
        out.insert(file, title);
    }
    Ok(IconTitleOverrides(out))
}

/// 原子写入映射表（先写临时文件再重命名）。
pub fn save_overrides(path: &Path, overrides: &IconTitleOverrides) -> Result<PathBuf> {
    crate::platform::fs::write_json_atomic(path, overrides)?;
    Ok(path.to_path_buf())
}

/// 合法的图标文件键：含 `.png` 的裸文件名（无路径分隔符/控制字符）。
pub fn is_icon_file_name(file: &str) -> bool {
    file.ends_with(".png")
        && !file.contains('/')
        && !file.contains('\\')
        && !file.contains("..")
        && !file.chars().any(|c| c.is_control())
}

/// 一个图标的维基上传状态（按生效标题查询）。

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiStatus {
    /// 文件是否存在；`None` 表示尚未查询。
    #[serde(default)]
    pub exists: Option<bool>,
    /// 查询时刻（unix 毫秒）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<u64>,
    /// 维基规范化标题（如 `File:Axe.png`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 当前版本直链（存在时）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// 生效标题的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleSource {
    /// 来自例外映射表（`config/icon_title_overrides.json`）。
    Override,
    /// 由 `STRINGS.NAMES` 英文名自动生成。
    Auto,
}

/// 制作栏图标的语义分组（由 [`CraftingKeys`] 解析链派生，随 images-sync
/// 写入元数据；WebUI 制作栏图标页按此分组，不依赖文件名前缀猜测）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CraftingGroup {
    /// 制作栏过滤器图标（对应 `STRINGS.UI.CRAFTING_FILTERS.<key>`）。
    Filter {
        /// 过滤器键；Lua 定义缺失时为 `None`。
        key: Option<String>,
    },
    /// 制作站图标（对应 `STRINGS.UI.CRAFTING_STATION_FILTERS.<key>`；
    /// 无 `filter_text` 的站为 `None`，显示名走 prefab 回退）。
    Station {
        /// 制作站过滤键。
        key: Option<String>,
    },
}

/// 单个图标的名称与上传信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconMetaEntry {
    /// 英文名（`STRINGS.NAMES.*` msgid）；无对应条目时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_en: Option<String>,
    /// 中文名；无翻译或 msgstr 为空时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_zh: Option<String>,
    /// 来源标识（[`crate::scripts_sync::images::icons::IconSource::id`]）；
    /// 旧元数据没有该字段时为 `None`（视为物品栏图标）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// 语义分组（仅制作栏图标）；旧元数据没有该字段时为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crafting: Option<CraftingGroup>,
    /// 生效的维基文件名（含 `.png`，无 `File:` 前缀）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// `title` 的来源。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_source: Option<TitleSource>,
    /// 是否有可用的维基标题（`title.is_some()`）。
    #[serde(default)]
    pub uploadable: bool,
    /// `uploadable = false` 的原因。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 维基上传状态；`None` 表示尚未查询。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiki: Option<WikiStatus>,
}

/// `history/icon_meta.json` 的完整内容。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IconMeta {
    /// 生成时的最新完整 manifest build。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// 生成时刻（unix 毫秒）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<u64>,
    /// 文件名（含 `.png`）→ 条目；未命中 `STRINGS.NAMES` 的文件不出现。
    #[serde(default)]
    pub icons: BTreeMap<String, IconMetaEntry>,
}

impl IconMeta {
    /// 更新一个文件（及其同生效标题的所有文件）的上传状态。
    pub fn set_wiki_status(
        &mut self,
        file: &str,
        exists: bool,
        title: Option<String>,
        url: Option<String>,
    ) {
        let Some(target) = self.icons.get(file).and_then(|e| e.title.clone()) else {
            return;
        };
        let at = now_ms();
        let title_owned = title.or_else(|| Some(format!("File:{target}")));
        for e in self.icons.values_mut() {
            if e.title.as_deref() == Some(target.as_str()) {
                e.wiki = Some(WikiStatus {
                    exists: Some(exists),
                    checked_at: Some(at),
                    title: title_owned.clone(),
                    url: url.clone(),
                });
            }
        }
    }
}

/// `KTOOLS__OUT_DIR` 下的元数据文件路径。
pub fn meta_path(out_dir: &Path) -> PathBuf {
    out_dir.join(META_REL_PATH)
}

/// 读取元数据；文件不存在时返回空元数据（不是错误）。
pub fn load(out_dir: &Path) -> Result<IconMeta> {
    let path = meta_path(out_dir);
    if !path.exists() {
        return Ok(IconMeta::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("读取 {}: {}", path.display(), e),
        ))
    })?;
    let mut meta: IconMeta = serde_json::from_str(&text).map_err(|e| {
        Error::Json(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("解析 {}: {}", path.display(), e),
        )))
    })?;
    // 旧版元数据没有 title 字段：按来源感知的自动标题补齐，保证重跑
    // images-sync 之前 WebUI/上传仍可正常工作（映射表在下一次 refresh 时应用）。
    let no_overrides = IconTitleOverrides::default();
    for (file, entry) in meta.icons.iter_mut() {
        if entry.title.is_some() {
            continue;
        }
        let (title, title_source) = effective_title(
            file,
            entry.name_en.as_deref(),
            &no_overrides,
            entry.source.as_deref(),
        );
        entry.title = title;
        entry.title_source = title_source;
        entry.uploadable = entry.title.is_some();
        if let (None, Some(name_en)) = (&entry.title, entry.name_en.as_deref()) {
            entry.note.get_or_insert_with(|| invalid_note(name_en));
        }
    }
    Ok(meta)
}

/// 原子写入元数据（避免与 WebUI 读并发损坏）。
pub fn save(out_dir: &Path, meta: &IconMeta) -> Result<PathBuf> {
    let path = meta_path(out_dir);
    crate::platform::fs::write_json_atomic(&path, meta)?;
    Ok(path)
}

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
    pub fn group(&self, file: &str) -> Option<CraftingGroup> {
        let stem = file_stem_key(file);
        if let Some(key) = self.filters.get(&stem) {
            return Some(CraftingGroup::Filter {
                key: Some(key.clone()),
            });
        }
        if let Some(key) = self.station_filters.get(&stem) {
            return Some(CraftingGroup::Station {
                key: Some(key.clone()),
            });
        }
        self.station_prefabs
            .contains_key(&stem)
            .then_some(CraftingGroup::Station { key: None })
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

fn read_game_text(dst_root: &Path, rel: &str) -> Result<String> {
    // 统一走 GameSource（live → scripts.zip）；snapshot 语义由调用方决定。
    crate::platform::game_source::GameSource::new(dst_root.to_path_buf(), None)?.read(rel)
}

// -- 标题与元数据构建 ---------------------------------------------------------

/// 文件名 stem → `STRINGS.NAMES` 键（`abigail_flower.png` → `ABIGAIL_FLOWER`）。
pub fn file_stem_key(file: &str) -> String {
    file.strip_suffix(".png")
        .unwrap_or(file)
        .to_ascii_uppercase()
}

/// MediaWiki 标题禁用字符（`# < > [ ] | { }` 及控制字符）。
fn invalid_title_char(c: char) -> bool {
    matches!(c, '#' | '<' | '>' | '[' | ']' | '|' | '{' | '}') || c.is_control()
}

/// 制作栏图标的自动标题后缀：`filter_*` → `Filter`，其余（`station_*`）→
/// `Station Icon`，与 wiki 分类:制作栏图标 的主流命名一致。
fn crafting_title_suffix(file: &str) -> &'static str {
    if file.starts_with("filter_") {
        "Filter"
    } else {
        "Station Icon"
    }
}

/// 英文名 → `File:<英文名>.png`（MediaWiki 归一化：首字母大写、`_`→空格）。
/// 含占位符/非法字符、`/` 或为空时返回 `None`。
///
/// `/` 会生成子页面语义的标题（如 `File:Pick/Axe.png`）且通常与站内实际
/// 文件名不符，因此自动生成一律视为不可用，需走映射表/手动命名。
pub fn wiki_title(name_en: &str) -> Option<String> {
    let name = name_en.trim();
    if name.is_empty() || name.chars().any(invalid_title_char) || name.contains('/') {
        return None;
    }
    Some(format!("{}.png", crate::wiki::file_title(name)))
}

/// 自动生成的裸文件名（含 `.png`，无 `File:` 前缀）。
pub fn auto_title(name_en: &str) -> Option<String> {
    wiki_title(name_en).map(|t| t.strip_prefix("File:").unwrap_or(&t).to_string())
}

/// 非 png 的常见图片扩展名：手动输入这些扩展名时直接报错，避免静默改名。
const NON_PNG_EXTS: &[&str] = &[
    "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "tif", "tiff",
];

/// 去 `File:`/`文件:` 前缀（ASCII `file:` 大小写不敏感）。
fn strip_file_prefix(raw: &str) -> &str {
    if let Some(rest) = raw
        .strip_prefix("File:")
        .or_else(|| raw.strip_prefix("文件:"))
    {
        return rest;
    }
    match raw.get(..5) {
        Some(head) if head.eq_ignore_ascii_case("file:") => &raw[5..],
        _ => raw,
    }
}

/// 显式指定的 wiki 文件名规范化（映射表值 / 弹窗手动输入 / `--title`）：
/// 去 `File:`/`文件:` 前缀、trim、无扩展名补 `.png`、按 MediaWiki 规则
/// 首字母大写、`_`→空格；拒绝非法字符、非 png 扩展名与空名。
///
/// 与 [`wiki_title`] 不同，这里允许 `/`（显式指定时尊重站内真实文件名）。
pub fn normalize_explicit_title(raw: &str) -> Result<String> {
    let bare = strip_file_prefix(raw.trim()).trim();
    if bare.is_empty() {
        return Err(Error::Config("wiki 文件名不能为空".to_string()));
    }
    if bare.chars().any(invalid_title_char) {
        return Err(Error::Config(format!(
            "wiki 文件名含 MediaWiki 非法字符（# < > [ ] | {{ }} 或控制字符）：{bare}"
        )));
    }
    let stem = if bare.to_ascii_lowercase().ends_with(".png") {
        &bare[..bare.len() - 4]
    } else {
        let ext = bare.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
        if ext.as_deref().is_some_and(|e| NON_PNG_EXTS.contains(&e)) {
            return Err(Error::Config(format!("只支持 .png 图标：{bare}")));
        }
        bare
    };
    if stem.is_empty() || stem.starts_with('.') {
        return Err(Error::Config(format!("wiki 文件名无效：{bare}")));
    }
    let normalized = crate::wiki::file_title(stem);
    let normalized = normalized.strip_prefix("File:").unwrap_or(&normalized);
    Ok(format!("{normalized}.png"))
}

/// 一个图标当前的生效标题：映射表（按本地文件名）优先，其次英文名自动
/// 生成——制作栏图标（`source = Some("crafting")`）追加 `Filter` /
/// `Station Icon` 后缀，其余直接用英文名。
///
/// 与自动命名相同的映射视为未指定（按 `Auto` 返回），防御手工编辑
/// 映射表引入的冗余条目。
pub fn effective_title(
    file: &str,
    name_en: Option<&str>,
    overrides: &IconTitleOverrides,
    source: Option<&str>,
) -> (Option<String>, Option<TitleSource>) {
    let auto = name_en.and_then(|name_en| match source {
        Some("crafting") => auto_title(&format!("{name_en} {}", crafting_title_suffix(file))),
        _ => auto_title(name_en),
    });
    if let Some(title) = overrides.get(file) {
        if auto.as_deref() != Some(title) {
            return (Some(title.to_string()), Some(TitleSource::Override));
        }
    }
    match auto {
        Some(title) => (Some(title), Some(TitleSource::Auto)),
        None => (None, None),
    }
}

/// 手动标题（已经 [`normalize_explicit_title`] 规范化）是否与自动命名一致。
/// 一致时视为未指定：写路径不落映射表（覆盖表只收偏离默认的真例外）。
/// 无英文名或英文名不可自动生成标题时永远返回 `false`（此时任何标题都是
/// 真例外，如皮肤/占位符名）。
pub fn title_is_default(
    file: &str,
    name_en: Option<&str>,
    source: Option<&str>,
    title: &str,
) -> bool {
    let (auto, _) = effective_title(file, name_en, &IconTitleOverrides::default(), source);
    auto.as_deref() == Some(title)
}

/// 重新计算条目的 `title`/`title_source`/`uploadable`/`note`；标题变化时
/// 清空旧上传状态。返回标题是否发生变化。
pub fn refresh_entry_title(
    entry: &mut IconMetaEntry,
    file: &str,
    overrides: &IconTitleOverrides,
) -> bool {
    let (title, source) = effective_title(
        file,
        entry.name_en.as_deref(),
        overrides,
        entry.source.as_deref(),
    );
    let changed = entry.title != title;
    if changed {
        entry.wiki = None;
    }
    entry.title = title;
    entry.title_source = source;
    entry.uploadable = entry.title.is_some();
    entry.note = match (&entry.title, entry.name_en.as_deref()) {
        (None, Some(name_en)) => Some(invalid_note(name_en)),
        _ => None,
    };
    changed
}

fn invalid_note(name_en: &str) -> String {
    if name_en.contains('{') || name_en.contains('}') {
        "名称含占位符（如 {item}），无法生成维基标题；可在弹窗手动指定文件名".to_string()
    } else if name_en.contains('/') {
        "名称含 /，维基标题会生成子页面；可在弹窗手动指定实际文件名".to_string()
    } else {
        "名称含维基标题非法字符，无法生成标题；可在弹窗手动指定文件名".to_string()
    }
}

/// 由当前图标文件清单构建元数据；`previous` 中生效标题未变的条目的 wiki
/// 状态会保留（避免每次 images-sync 重查全部标题）。
///
/// `files` 为 `(文件名, 来源标识)` 对（来源见 `images::icons::ICON_SOURCES`）；
/// `source = "crafting"` 的文件走 [`CraftingKeys::display_name`] 取名。
pub fn build_meta(
    build: &str,
    files: impl IntoIterator<Item = (String, String)>,
    names: &NameMaps,
    crafting: &CraftingKeys,
    overrides: &IconTitleOverrides,
    previous: &IconMeta,
) -> IconMeta {
    let mut icons = BTreeMap::new();
    for (file, source) in files {
        let (name_en, name_zh) = if source == "crafting" {
            crafting
                .display_name(names, &file)
                .map(|(en, zh)| (Some(en), zh))
                .unwrap_or_default()
        } else {
            let key = file_stem_key(&file);
            (
                names.names.en.get(&key).cloned(),
                names.names.zh.get(&key).cloned(),
            )
        };
        let (title, title_source) =
            effective_title(&file, name_en.as_deref(), overrides, Some(source.as_str()));
        if name_en.is_none() && title.is_none() {
            continue;
        }
        let uploadable = title.is_some();
        let note = match (&title, name_en.as_deref()) {
            (None, Some(name_en)) => Some(invalid_note(name_en)),
            _ => None,
        };
        let wiki = previous
            .icons
            .get(&file)
            .filter(|old| old.title == title)
            .and_then(|old| old.wiki.clone());
        let crafting = if source == "crafting" {
            crafting.group(&file)
        } else {
            None
        };
        icons.insert(
            file,
            IconMetaEntry {
                name_en,
                name_zh,
                source: Some(source),
                crafting,
                title,
                title_source,
                uploadable,
                note,
                wiki,
            },
        );
    }
    IconMeta {
        build: Some(build.to_string()),
        generated_at: Some(now_ms()),
        icons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZH_PO: &str = r#"msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"

msgctxt "STRINGS.NAMES.AXE"
msgid "Axe"
msgstr "斧头"

msgctxt "STRINGS.NAMES.ABIGAIL_FLOWER"
msgid "Abigail's Flower"
msgstr "阿比盖尔之花"

msgctxt "STRINGS.NAMES.EMPTY_ZH"
msgid "Empty Zh"
msgstr ""
"#;

    const EN_POT: &str = r#"msgctxt "STRINGS.NAMES.AXE"
msgid "Axe"
msgstr ""

msgctxt "STRINGS.NAMES.EMPTY_ZH"
msgid "Empty Zh"
msgstr ""

msgctxt "STRINGS.NAMES.EN_ONLY"
msgid "English Only"
msgstr ""
"#;

    #[test]
    fn test_parse_name_maps_merges_pot_and_zh() {
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        assert_eq!(maps.names.en.get("AXE").map(String::as_str), Some("Axe"));
        assert_eq!(maps.names.zh.get("AXE").map(String::as_str), Some("斧头"));
        // 只有英文、没有中文
        assert_eq!(
            maps.names.en.get("EN_ONLY").map(String::as_str),
            Some("English Only")
        );
        assert!(!maps.names.zh.contains_key("EN_ONLY"));
        // chinese_s.po 里 msgstr 为空 → 不产生中文
        assert_eq!(
            maps.names.en.get("EMPTY_ZH").map(String::as_str),
            Some("Empty Zh")
        );
        assert!(!maps.names.zh.contains_key("EMPTY_ZH"));
    }

    const ZH_PO_CRAFTING: &str = r#"msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"

msgctxt "STRINGS.UI.CRAFTING_FILTERS.TOOLS"
msgid "Tools"
msgstr "工具"

msgctxt "STRINGS.UI.CRAFTING_STATION_FILTERS.CARPENTRY"
msgid "Carpentry"
msgstr "木工"

msgctxt "STRINGS.UI.CRAFTING_STATION_FILTERS.EMPTY"
msgid "Empty"
msgstr ""
"#;

    #[test]
    fn test_parse_name_maps_crafting_namespaces() {
        let maps = parse_name_maps(ZH_PO_CRAFTING, None).unwrap();
        assert_eq!(
            maps.crafting_filters.get("TOOLS"),
            Some(("Tools".into(), Some("工具".into())))
        );
        assert_eq!(
            maps.station_filters.get("CARPENTRY"),
            Some(("Carpentry".into(), Some("木工".into())))
        );
        // 空 msgstr → 只有英文
        assert_eq!(
            maps.station_filters.get("EMPTY"),
            Some(("Empty".into(), None))
        );
        // 不影响 NAMES 命名空间
        assert!(maps.names.is_empty());
    }

    #[test]
    fn test_wiki_title_normalizes_and_rejects() {
        assert_eq!(wiki_title("Axe").as_deref(), Some("File:Axe.png"));
        assert_eq!(
            wiki_title("Abigail's Flower").as_deref(),
            Some("File:Abigail's Flower.png")
        );
        // 下划线按 MediaWiki 规则显示为空格
        assert_eq!(
            wiki_title("gold_nugget").as_deref(),
            Some("File:Gold nugget.png")
        );
        assert_eq!(wiki_title("Art?").as_deref(), Some("File:Art?.png"));
        assert!(wiki_title("{item} Blueprint").is_none());
        assert!(wiki_title("bad#name").is_none());
        assert!(wiki_title("  ").is_none());
        // `/` 自动生成会变成子页面标题，一律拒绝
        assert!(wiki_title("Pick/Axe").is_none());
        assert_eq!(auto_title("Axe").as_deref(), Some("Axe.png"));
        assert_eq!(auto_title("Pick/Axe"), None);
    }

    #[test]
    fn test_normalize_explicit_title() {
        assert_eq!(normalize_explicit_title("Axe").unwrap(), "Axe.png");
        assert_eq!(
            normalize_explicit_title("File:Pick-Axe.png").unwrap(),
            "Pick-Axe.png"
        );
        assert_eq!(
            normalize_explicit_title("文件:Pick-Axe.png").unwrap(),
            "Pick-Axe.png"
        );
        assert_eq!(
            normalize_explicit_title("file:Pick-Axe.png").unwrap(),
            "Pick-Axe.png"
        );
        // 下划线归一化与首字母大写
        assert_eq!(
            normalize_explicit_title("gold_nugget").unwrap(),
            "Gold nugget.png"
        );
        // 显式指定允许 `/`
        assert_eq!(
            normalize_explicit_title("Pick/Axe.png").unwrap(),
            "Pick/Axe.png"
        );
        // 点号名但没有扩展名 → 补 .png
        assert_eq!(
            normalize_explicit_title("T.I.N.G.L.E").unwrap(),
            "T.I.N.G.L.E.png"
        );
        assert!(normalize_explicit_title("a.jpg").is_err());
        assert!(normalize_explicit_title("bad#name.png").is_err());
        assert!(normalize_explicit_title("  ").is_err());
        assert!(normalize_explicit_title(".png").is_err());
    }

    #[test]
    fn test_overrides_roundtrip_and_validation() {
        let dir = std::env::temp_dir().join(format!("icon_overrides_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("config/icon_title_overrides.json");
        let mut ov = IconTitleOverrides::default();
        ov.insert("multitool_axe_pickaxe.png", "Pick-Axe.png");
        save_overrides(&path, &ov).unwrap();
        let loaded = load_overrides(&path).unwrap();
        assert_eq!(
            loaded.get("multitool_axe_pickaxe.png"),
            Some("Pick-Axe.png")
        );
        assert_eq!(loaded.len(), 1);

        // 值经规范化
        std::fs::write(&path, r#"{"a.png":"gold_nugget"}"#).unwrap();
        let loaded = load_overrides(&path).unwrap();
        assert_eq!(loaded.get("a.png"), Some("Gold nugget.png"));

        // 非法键（路径穿越）与非法值
        std::fs::write(&path, r#"{"../a.png":"A.png"}"#).unwrap();
        assert!(load_overrides(&path).is_err());
        std::fs::write(&path, r#"{"a.png":"bad#name.png"}"#).unwrap();
        assert!(load_overrides(&path).is_err());
        std::fs::write(&path, r#"{"a.png":"x.jpg"}"#).unwrap();
        assert!(load_overrides(&path).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_missing_overrides_is_empty() {
        let dir = std::env::temp_dir().join(format!("icon_overrides_miss_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        assert!(load_overrides(&dir.join("none.json")).unwrap().is_empty());
    }

    fn files(list: &[&str]) -> Vec<(String, String)> {
        list.iter()
            .map(|s| (s.to_string(), "inventory".to_string()))
            .collect()
    }

    fn entry(name_en: Option<&str>, title: Option<&str>) -> IconMetaEntry {
        IconMetaEntry {
            name_en: name_en.map(str::to_string),
            name_zh: None,
            source: Some("inventory".to_string()),
            crafting: None,
            title: title.map(str::to_string),
            title_source: title.map(|_| TitleSource::Auto),
            uploadable: title.is_some(),
            note: None,
            wiki: None,
        }
    }

    #[test]
    fn test_effective_title_crafting_suffix() {
        let overrides = IconTitleOverrides::default();
        // filter_* → "Filter" 后缀
        assert_eq!(
            effective_title(
                "filter_tool.png",
                Some("Tools"),
                &overrides,
                Some("crafting")
            ),
            (Some("Tools Filter.png".into()), Some(TitleSource::Auto))
        );
        // 其余（station_*）→ "Station Icon" 后缀
        assert_eq!(
            effective_title(
                "station_carpentry.png",
                Some("Sawhorse"),
                &overrides,
                Some("crafting")
            ),
            (
                Some("Sawhorse Station Icon.png".into()),
                Some(TitleSource::Auto)
            )
        );
        // 物品栏来源保持无后缀
        assert_eq!(
            effective_title("axe.png", Some("Axe"), &overrides, Some("inventory")),
            (Some("Axe.png".into()), Some(TitleSource::Auto))
        );
        assert_eq!(
            effective_title("axe.png", Some("Axe"), &overrides, None),
            (Some("Axe.png".into()), Some(TitleSource::Auto))
        );
        // 旧来源（无 source 字段）行为不变
        // 映射表优先且不受后缀影响（值与自动命名不同才是真例外）
        let mut ov = IconTitleOverrides::default();
        ov.insert("station_carpentry.png", "Carpentry Station Icon.png");
        assert_eq!(
            effective_title(
                "station_carpentry.png",
                Some("Sawhorse"),
                &ov,
                Some("crafting")
            ),
            (
                Some("Carpentry Station Icon.png".into()),
                Some(TitleSource::Override)
            )
        );
    }

    #[test]
    fn test_title_is_default() {
        // 物品栏：与英文名标题一致 → 默认
        assert!(title_is_default("axe.png", Some("Axe"), None, "Axe.png"));
        // 不同的标题 → 非默认
        assert!(!title_is_default(
            "axe.png",
            Some("Axe"),
            None,
            "Pick-Axe.png"
        ));
        // crafting：后缀参与推导
        assert!(title_is_default(
            "filter_tool.png",
            Some("Tools"),
            Some("crafting"),
            "Tools Filter.png"
        ));
        assert!(!title_is_default(
            "filter_tool.png",
            Some("Tools"),
            Some("crafting"),
            "Tools.png"
        ));
        assert!(title_is_default(
            "station_carpentry.png",
            Some("Carpentry"),
            Some("crafting"),
            "Carpentry Station Icon.png"
        ));
        // 无英文名（皮肤/变体）：自动标题不存在，任何标题都是真例外
        assert!(!title_is_default("skin.png", None, None, "Skin Icon.png"));
        // 英文名含非法字符（自动标题不可用）：手动标题是真例外
        assert!(!title_is_default(
            "multitool_axe_pickaxe.png",
            Some("Pick/Axe"),
            None,
            "Pick-Axe.png"
        ));
    }

    #[test]
    fn test_effective_title_ignores_default_override() {
        let mut ov = IconTitleOverrides::default();
        // 与自动命名相同的映射 → 按 Auto 处理，不视为特例
        ov.insert("axe.png", "Axe.png");
        assert_eq!(
            effective_title("axe.png", Some("Axe"), &ov, None),
            (Some("Axe.png".into()), Some(TitleSource::Auto))
        );
        // 不同的映射仍是真例外
        ov.insert("axe.png", "Pick-Axe.png");
        assert_eq!(
            effective_title("axe.png", Some("Axe"), &ov, None),
            (Some("Pick-Axe.png".into()), Some(TitleSource::Override))
        );
        // 无自动标题（英文名含 `/`）时映射照常生效
        ov.remove("axe.png");
        ov.insert("multitool_axe_pickaxe.png", "Pick-Axe.png");
        assert_eq!(
            effective_title("multitool_axe_pickaxe.png", Some("Pick/Axe"), &ov, None),
            (Some("Pick-Axe.png".into()), Some(TitleSource::Override))
        );
    }

    #[test]
    fn test_crafting_keys_display_name() {
        let filters = vec![
            crate::parser::crafting::FilterIconDef {
                name: "CRAFTING_STATION".into(),
                image: "filter_none".into(),
            },
            crate::parser::crafting::FilterIconDef {
                name: "EVERYTHING".into(),
                image: "filter_none".into(),
            },
            crate::parser::crafting::FilterIconDef {
                name: "TOOLS".into(),
                image: "filter_tool".into(),
            },
        ];
        let prototypers = vec![
            crate::parser::crafting::PrototyperIconDef {
                prefab: "researchlab".into(),
                image: "station_science".into(),
                filter_key: None,
            },
            crate::parser::crafting::PrototyperIconDef {
                prefab: "carpentry_station".into(),
                image: "station_carpentry".into(),
                filter_key: Some("CARPENTRY".into()),
            },
        ];
        let keys = CraftingKeys::from_defs(&filters, &prototypers);
        // 同图标多过滤器：定义序最后一个胜出（filter_none → EVERYTHING）
        let mut maps = NameMaps::default();
        maps.crafting_filters
            .en
            .insert("TOOLS".into(), "Tools".into());
        maps.crafting_filters
            .zh
            .insert("TOOLS".into(), "工具".into());
        maps.crafting_filters
            .en
            .insert("EVERYTHING".into(), "Everything".into());
        maps.station_filters
            .en
            .insert("CARPENTRY".into(), "Carpentry".into());
        maps.station_filters
            .zh
            .insert("CARPENTRY".into(), "木工".into());
        maps.names
            .en
            .insert("RESEARCHLAB".into(), "Science Machine".into());

        assert_eq!(
            keys.display_name(&maps, "filter_tool.png"),
            Some(("Tools".into(), Some("工具".into())))
        );
        assert_eq!(
            keys.display_name(&maps, "filter_none.png"),
            Some(("Everything".into(), None))
        );
        // station：优先过滤名
        assert_eq!(
            keys.display_name(&maps, "station_carpentry.png"),
            Some(("Carpentry".into(), Some("木工".into())))
        );
        // 无 filter_text：回退 prefab 名
        assert_eq!(
            keys.display_name(&maps, "station_science.png"),
            Some(("Science Machine".into(), None))
        );
        // 未收录的图标
        assert_eq!(keys.display_name(&maps, "station_unknown.png"), None);
        assert_eq!(keys.display_name(&maps, "axe.png"), None);

        // 语义分组：filter/station 按解析链，station 无过滤名 key 为 None
        assert_eq!(
            keys.group("filter_tool.png"),
            Some(CraftingGroup::Filter {
                key: Some("TOOLS".into())
            })
        );
        // 同图标多过滤器：与 display_name 同规则（定义序最后者胜）
        assert_eq!(
            keys.group("filter_none.png"),
            Some(CraftingGroup::Filter {
                key: Some("EVERYTHING".into())
            })
        );
        assert_eq!(
            keys.group("station_carpentry.png"),
            Some(CraftingGroup::Station {
                key: Some("CARPENTRY".into())
            })
        );
        assert_eq!(
            keys.group("station_science.png"),
            Some(CraftingGroup::Station { key: None })
        );
        assert_eq!(keys.group("axe.png"), None);
        // serde 形态（WebUI 消费的字段形状）
        assert_eq!(
            serde_json::to_value(CraftingGroup::Filter { key: None }).unwrap(),
            serde_json::json!({ "kind": "filter", "key": null })
        );
    }

    #[test]
    fn test_build_meta_crafting_icons() {
        let maps = parse_name_maps(ZH_PO_CRAFTING, None).unwrap();
        // prefab 回退显示名：station_science → researchlab
        let mut maps = maps;
        maps.names
            .en
            .insert("RESEARCHLAB".into(), "Science Machine".into());
        let filters = vec![crate::parser::crafting::FilterIconDef {
            name: "TOOLS".into(),
            image: "filter_tool".into(),
        }];
        let prototypers = vec![
            crate::parser::crafting::PrototyperIconDef {
                prefab: "researchlab".into(),
                image: "station_science".into(),
                filter_key: None,
            },
            crate::parser::crafting::PrototyperIconDef {
                prefab: "carpentry_station".into(),
                image: "station_carpentry".into(),
                filter_key: Some("CARPENTRY".into()),
            },
        ];
        let keys = CraftingKeys::from_defs(&filters, &prototypers);
        let crafting_files = |list: &[&str]| {
            list.iter()
                .map(|s| (s.to_string(), "crafting".to_string()))
                .collect::<Vec<_>>()
        };

        let meta = build_meta(
            "1",
            crafting_files(&[
                "filter_tool.png",
                "station_carpentry.png",
                "station_science.png",
            ]),
            &maps,
            &keys,
            &IconTitleOverrides::default(),
            &IconMeta::default(),
        );
        // filter：显示名 + "Filter" 后缀标题
        let tool = &meta.icons["filter_tool.png"];
        assert_eq!(tool.name_en.as_deref(), Some("Tools"));
        assert_eq!(tool.name_zh.as_deref(), Some("工具"));
        assert_eq!(tool.title.as_deref(), Some("Tools Filter.png"));
        assert_eq!(tool.source.as_deref(), Some("crafting"));
        assert_eq!(
            tool.crafting,
            Some(CraftingGroup::Filter {
                key: Some("TOOLS".into())
            })
        );
        assert!(tool.uploadable);
        // station 有过滤名："Station Icon" 后缀
        let carpentry = &meta.icons["station_carpentry.png"];
        assert_eq!(
            carpentry.title.as_deref(),
            Some("Carpentry Station Icon.png")
        );
        assert_eq!(carpentry.name_zh.as_deref(), Some("木工"));
        assert_eq!(
            carpentry.crafting,
            Some(CraftingGroup::Station {
                key: Some("CARPENTRY".into())
            })
        );
        // station 无过滤名：prefab 显示名 + "Station Icon" 后缀标题
        let science = &meta.icons["station_science.png"];
        assert_eq!(science.name_en.as_deref(), Some("Science Machine"));
        assert_eq!(
            science.title.as_deref(),
            Some("Science Machine Station Icon.png")
        );
        assert_eq!(science.crafting, Some(CraftingGroup::Station { key: None }));

        // 映射表优先于自动标题；无键无映射的图标不收录
        let mut ov = IconTitleOverrides::default();
        ov.insert("station_science.png", "Science Station Icon.png");
        let meta = build_meta(
            "1",
            crafting_files(&[
                "filter_tool.png",
                "station_science.png",
                "station_unknown.png",
            ]),
            &maps,
            &keys,
            &ov,
            &IconMeta::default(),
        );
        assert_eq!(
            meta.icons["station_science.png"].title.as_deref(),
            Some("Science Station Icon.png")
        );
        assert_eq!(
            meta.icons["station_science.png"].title_source,
            Some(TitleSource::Override)
        );
        assert!(!meta.icons.contains_key("station_unknown.png"));
    }

    #[test]
    fn test_build_meta_marks_placeholders_and_keeps_status() {
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        let mut previous = IconMeta::default();
        previous.icons.insert(
            "axe.png".into(),
            IconMetaEntry {
                name_zh: Some("斧头".into()),
                wiki: Some(WikiStatus {
                    exists: Some(true),
                    checked_at: Some(1),
                    title: Some("File:Axe.png".into()),
                    url: None,
                }),
                ..entry(Some("Axe"), Some("Axe.png"))
            },
        );

        let meta = build_meta(
            "100",
            files(&[
                "axe.png",
                "abigail_flower.png",
                "empty_zh.png",
                "en_only.png",
                "variant_suffix.png",
                "placeholder.png",
            ]),
            &maps,
            &CraftingKeys::default(),
            &IconTitleOverrides::default(),
            &previous,
        );

        assert_eq!(meta.build.as_deref(), Some("100"));
        assert_eq!(meta.icons.len(), 4); // variant 无名称，不收录
        let axe = &meta.icons["axe.png"];
        assert!(axe.uploadable);
        assert_eq!(axe.title.as_deref(), Some("Axe.png"));
        assert_eq!(axe.title_source, Some(TitleSource::Auto));
        // 生效标题未变 → 保留旧状态
        assert_eq!(axe.wiki.as_ref().unwrap().exists, Some(true));
        assert!(meta.icons["abigail_flower.png"].uploadable);
        assert_eq!(
            meta.icons["abigail_flower.png"].name_zh.as_deref(),
            Some("阿比盖尔之花")
        );
        // 英文有、中文无
        let en_only = &meta.icons["en_only.png"];
        assert_eq!(en_only.name_en.as_deref(), Some("English Only"));
        assert!(en_only.name_zh.is_none());
        // mock 里没有 placeholder 键，改为直接构造验证占位符逻辑
        let mut maps2 = maps;
        maps2
            .names
            .en
            .insert("PLACEHOLDER".into(), "{item} Blueprint".into());
        let meta2 = build_meta(
            "100",
            files(&["placeholder.png"]),
            &maps2,
            &CraftingKeys::default(),
            &IconTitleOverrides::default(),
            &IconMeta::default(),
        );
        let p = &meta2.icons["placeholder.png"];
        assert!(!p.uploadable);
        assert!(p.title.is_none());
        assert!(p.note.as_deref().unwrap().contains("占位符"));
        assert!(p.wiki.is_none());
    }

    #[test]
    fn test_build_meta_applies_override_and_includes_unnamed() {
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        let mut ov = IconTitleOverrides::default();
        ov.insert("axe.png", "Pick-Axe.png");
        ov.insert("skin.png", "Skin Icon.png");
        let meta = build_meta(
            "1",
            files(&["axe.png", "skin.png", "other.png"]),
            &maps,
            &CraftingKeys::default(),
            &ov,
            &IconMeta::default(),
        );
        // 映射优先于自动生成
        let axe = &meta.icons["axe.png"];
        assert_eq!(axe.title.as_deref(), Some("Pick-Axe.png"));
        assert_eq!(axe.title_source, Some(TitleSource::Override));
        assert!(axe.uploadable);
        // 无英文名但有映射 → 收录
        let skin = &meta.icons["skin.png"];
        assert_eq!(skin.name_en, None);
        assert_eq!(skin.title.as_deref(), Some("Skin Icon.png"));
        assert_eq!(skin.title_source, Some(TitleSource::Override));
        assert!(skin.uploadable && skin.note.is_none());
        // 无英文名且无映射 → 不收录
        assert!(!meta.icons.contains_key("other.png"));
    }

    #[test]
    fn test_build_meta_records_source() {
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        let mut ov = IconTitleOverrides::default();
        ov.insert("filter_tool.png", "Tools Filter.png");
        let meta = build_meta(
            "1",
            vec![
                ("axe.png".to_string(), "inventory".to_string()),
                ("filter_tool.png".to_string(), "crafting".to_string()),
            ],
            &maps,
            &CraftingKeys::default(),
            &ov,
            &IconMeta::default(),
        );
        assert_eq!(meta.icons["axe.png"].source.as_deref(), Some("inventory"));
        assert_eq!(
            meta.icons["filter_tool.png"].source.as_deref(),
            Some("crafting")
        );
        assert_eq!(
            meta.icons["filter_tool.png"].title.as_deref(),
            Some("Tools Filter.png")
        );
    }

    #[test]
    fn test_build_meta_drops_status_when_title_changed() {
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        let mut previous = IconMeta::default();
        previous.icons.insert(
            "axe.png".into(),
            IconMetaEntry {
                wiki: Some(WikiStatus {
                    exists: Some(true),
                    checked_at: Some(1),
                    title: Some("File:Old.png".into()),
                    url: None,
                }),
                ..entry(Some("Old Name"), Some("Old.png"))
            },
        );
        let meta = build_meta(
            "100",
            files(&["axe.png"]),
            &maps,
            &CraftingKeys::default(),
            &IconTitleOverrides::default(),
            &previous,
        );
        // 生效标题从 Old.png 变为 Axe.png → 旧状态失效
        assert_eq!(meta.icons["axe.png"].title.as_deref(), Some("Axe.png"));
        assert!(meta.icons["axe.png"].wiki.is_none());
    }

    #[test]
    fn test_refresh_entry_title_resets_status_on_change() {
        let mut e = IconMetaEntry {
            wiki: Some(WikiStatus {
                exists: Some(false),
                checked_at: Some(1),
                title: Some("File:Pick/Axe.png".into()),
                url: None,
            }),
            ..entry(Some("Pick/Axe"), Some("Pick/Axe.png"))
        };
        // 无映射时自动标题因 `/` 不可用
        assert!(refresh_entry_title(
            &mut e,
            "multitool_axe_pickaxe.png",
            &IconTitleOverrides::default()
        ));
        assert!(e.title.is_none() && !e.uploadable);
        assert!(e.wiki.is_none());
        assert!(e.note.as_deref().unwrap().contains('/'));
        // 加映射后恢复可上传
        let mut ov = IconTitleOverrides::default();
        ov.insert("multitool_axe_pickaxe.png", "Pick-Axe.png");
        assert!(refresh_entry_title(
            &mut e,
            "multitool_axe_pickaxe.png",
            &ov
        ));
        assert_eq!(e.title.as_deref(), Some("Pick-Axe.png"));
        assert_eq!(e.title_source, Some(TitleSource::Override));
        assert!(e.uploadable && e.note.is_none());
    }

    #[test]
    fn test_set_wiki_status_updates_all_same_title() {
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        let mut meta = build_meta(
            "1",
            files(&["axe.png", "other_axe.png"]),
            &maps,
            &CraftingKeys::default(),
            &IconTitleOverrides::default(),
            &IconMeta::default(),
        );
        // other_axe 无键，不在 meta；手工加一个同名条目验证共享状态
        let dup = meta.icons["axe.png"].clone();
        meta.icons.insert("other_axe.png".into(), dup);
        meta.set_wiki_status("axe.png", true, None, Some("http://x".into()));
        for f in ["axe.png", "other_axe.png"] {
            assert_eq!(meta.icons[f].wiki.as_ref().unwrap().exists, Some(true));
        }
    }

    #[test]
    fn test_meta_save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("icon_meta_io_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let maps = parse_name_maps(ZH_PO, Some(EN_POT)).unwrap();
        let meta = build_meta(
            "42",
            files(&["axe.png"]),
            &maps,
            &CraftingKeys::default(),
            &IconTitleOverrides::default(),
            &IconMeta::default(),
        );
        let path = save(&dir, &meta).unwrap();
        assert!(path.ends_with("history/icon_meta.json"));
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.build.as_deref(), Some("42"));
        assert_eq!(loaded.icons["axe.png"].name_en.as_deref(), Some("Axe"));
        assert_eq!(loaded.icons["axe.png"].title.as_deref(), Some("Axe.png"));
        assert_eq!(loaded.icons["axe.png"].name_zh.as_deref(), Some("斧头"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_old_schema_name_en_string() {
        let dir = std::env::temp_dir().join(format!("icon_meta_old_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("history")).unwrap();
        std::fs::write(
            dir.join(META_REL_PATH),
            r#"{"build":"1","icons":{
                "axe.png":{"name_en":"Axe","uploadable":true},
                "placeholder.png":{"name_en":"{item} Blueprint","uploadable":false}
            }}"#,
        )
        .unwrap();
        let meta = load(&dir).unwrap();
        // 旧 schema 自动补齐自动标题
        let axe = &meta.icons["axe.png"];
        assert_eq!(axe.name_en.as_deref(), Some("Axe"));
        assert_eq!(axe.title.as_deref(), Some("Axe.png"));
        assert_eq!(axe.title_source, Some(TitleSource::Auto));
        assert!(axe.uploadable);
        // 非法英文名仍然不可上传，并补 note
        let p = &meta.icons["placeholder.png"];
        assert!(p.title.is_none() && !p.uploadable);
        assert!(p.note.as_deref().unwrap().contains("占位符"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_missing_dir_is_empty() {
        let dir = std::env::temp_dir().join(format!("icon_meta_missing_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        assert!(load(&dir).unwrap().icons.is_empty());
    }

    #[test]
    fn test_read_name_maps_from_live_tree() {
        let root = std::env::temp_dir().join(format!("icon_meta_po_{}", std::process::id()));
        let lang = root.join("data/databundles/scripts/languages");
        std::fs::create_dir_all(&lang).unwrap();
        std::fs::write(lang.join("chinese_s.po"), ZH_PO).unwrap();
        std::fs::write(lang.join("strings.pot"), EN_POT).unwrap();
        let maps = read_name_maps(&root).unwrap();
        assert_eq!(
            maps.names.en.get("EN_ONLY").map(String::as_str),
            Some("English Only")
        );
        assert_eq!(maps.names.zh.get("AXE").map(String::as_str), Some("斧头"));
        std::fs::remove_dir_all(&root).ok();
    }

    /// 真实游戏数据指纹：当前 atlas 的 54 个制作栏图标全部能解析出中英文
    /// 显示名，且自动标题（含映射表 9 条例外）与 wiki 分类:制作栏图标 的
    /// 既有文件名一一对应。本地无 `DST__ROOT` 时跳过（与 wiki API 测试同
    /// 惯例，CI 不装游戏）。
    #[test]
    fn test_real_game_crafting_icons_resolve() {
        let Ok(dst_root) = std::env::var("DST__ROOT") else {
            return;
        };
        let dst_root = PathBuf::from(dst_root);
        if !dst_root.join("data/databundles").exists() {
            return;
        }
        let names = read_name_maps(&dst_root).unwrap();
        let crafting = read_crafting_keys(&dst_root).unwrap();

        // 映射表例外：wiki 命名偏离「英文名 + Filter/Station Icon」惯例的部分
        // （Station Filter 后缀变体、prefab/编辑命名，均取自分类内现有文件名）。
        let mut ov = IconTitleOverrides::default();
        ov.insert("station_arcane.png", "Magic Station Icon.png");
        ov.insert(
            "station_carnivalgame_golfprops.png",
            "Custom Course Shack Icon.png",
        );
        ov.insert("station_carpentry.png", "Sawhorse Station Icon.png");
        ov.insert(
            "station_foodprocessing.png",
            "Seasonings Station Filter.png",
        );
        ov.insert(
            "station_hermitcrab_shop.png",
            "Bottle Exchange Station Filter.png",
        );
        ov.insert("station_none.png", "Without Station Icon.png");
        ov.insert("station_orphanage.png", "Critters Station Filter.png");
        ov.insert("station_rabbitking.png", "Rabbitking Station Icon.png");
        ov.insert("station_science.png", "Science Station Icon.png");
        ov.insert(
            "station_shadow_forge.png",
            "Shadowcrafting Station Icon.png",
        );

        let auto = [
            ("filter_armour.png", "Armor Filter.png"),
            ("filter_containers.png", "Storage Solutions Filter.png"),
            ("filter_cooking.png", "Cooking Filter.png"),
            ("filter_cosmetic.png", "Decorations Filter.png"),
            ("filter_events.png", "Special Event Filter.png"),
            ("filter_favorites.png", "Favorites Filter.png"),
            ("filter_fire.png", "Light Sources Filter.png"),
            ("filter_fishing.png", "Fishing Filter.png"),
            ("filter_gardening.png", "Food & Gardening Filter.png"),
            ("filter_health.png", "Healing Filter.png"),
            ("filter_modded.png", "Modded Items Filter.png"),
            ("filter_none.png", "Everything Filter.png"),
            ("filter_rain.png", "Rain Gear Filter.png"),
            ("filter_refine.png", "Refined Materials Filter.png"),
            ("filter_riding.png", "Beefalo Riding Filter.png"),
            ("filter_sailing.png", "Seafaring Filter.png"),
            ("filter_science.png", "Prototypers & Stations Filter.png"),
            ("filter_skull.png", "Magic Filter.png"),
            ("filter_structure.png", "Structures Filter.png"),
            ("filter_summer.png", "Summer Items Filter.png"),
            ("filter_tool.png", "Tools Filter.png"),
            ("filter_warable.png", "Clothing Filter.png"),
            ("filter_weapon.png", "Weapons Filter.png"),
            ("filter_winter.png", "Winter Items Filter.png"),
            ("station_books.png", "Bookcase Station Icon.png"),
            ("station_cartography.png", "Cartography Station Icon.png"),
            ("station_celestial.png", "Celestial Station Icon.png"),
            (
                "station_crafting_table.png",
                "Ancient Pseudoscience Station Icon.png",
            ),
            (
                "station_feast_oven.png",
                "Winter's Feast Cooking Station Icon.png",
            ),
            ("station_fishing.png", "Tackle Receptacle Station Icon.png"),
            (
                "station_hermitcrab_teashop.png",
                "Tea Brewing Station Icon.png",
            ),
            ("station_host.png", "Cawnival Creation Station Icon.png"),
            ("station_lunar_forge.png", "Brightsmithy Station Icon.png"),
            ("station_madscience_lab.png", "Mad Science Station Icon.png"),
            ("station_perd_offering.png", "Offerings Station Icon.png"),
            ("station_prizebooth.png", "Trinket Trove Station Icon.png"),
            ("station_sculpt.png", "Sculptures Station Icon.png"),
            ("station_seafaring.png", "Think Tank Station Icon.png"),
            ("station_shadow.png", "Codex Umbra Station Icon.png"),
            ("station_shellweaver.png", "Combrining Station Icon.png"),
            (
                "station_turfcrafting.png",
                "Terra Firma Tamper Station Icon.png",
            ),
            (
                "station_vault_refiner.png",
                "Sanctum Smithy Station Icon.png",
            ),
            (
                "station_wagpunk_workstation.png",
                "Fabrication Station Icon.png",
            ),
            ("station_wanderingtrader.png", "Trading Station Icon.png"),
        ];
        let overridden = [
            ("station_arcane.png", "Magic Station Icon.png"),
            (
                "station_carnivalgame_golfprops.png",
                "Custom Course Shack Icon.png",
            ),
            ("station_carpentry.png", "Sawhorse Station Icon.png"),
            (
                "station_foodprocessing.png",
                "Seasonings Station Filter.png",
            ),
            (
                "station_hermitcrab_shop.png",
                "Bottle Exchange Station Filter.png",
            ),
            ("station_none.png", "Without Station Icon.png"),
            ("station_orphanage.png", "Critters Station Filter.png"),
            ("station_rabbitking.png", "Rabbitking Station Icon.png"),
            ("station_science.png", "Science Station Icon.png"),
            (
                "station_shadow_forge.png",
                "Shadowcrafting Station Icon.png",
            ),
        ];

        for (file, expected) in auto.iter().chain(overridden.iter()) {
            let (name_en, name_zh) = crafting
                .display_name(&names, file)
                .unwrap_or_else(|| panic!("{file} 应能解析出显示名（Lua 定义 + 翻译表）"));
            assert!(!name_en.is_empty(), "{file} 英文名为空");
            assert!(!name_zh.unwrap_or_default().is_empty(), "{file} 中文名为空");
            let (title, source) = effective_title(file, Some(&name_en), &ov, Some("crafting"));
            assert_eq!(
                title.as_deref(),
                Some(*expected),
                "{file}（{name_en}）的自动标题与 wiki 分类命名不符"
            );
            let expected_source = if auto.iter().any(|(f, _)| f == file) {
                TitleSource::Auto
            } else {
                TitleSource::Override
            };
            assert_eq!(source, Some(expected_source), "{file} 标题来源");
        }
        assert_eq!(auto.len() + overridden.len(), 54, "atlas 图标总数");

        // 语义分组抽检：过滤器 / 制作站（无过滤名的 key 为 None）
        assert_eq!(
            crafting.group("filter_tool.png"),
            Some(CraftingGroup::Filter {
                key: Some("TOOLS".into())
            })
        );
        assert_eq!(
            crafting.group("station_carpentry.png"),
            Some(CraftingGroup::Station {
                key: Some("CARPENTRY".into())
            })
        );
        assert_eq!(
            crafting.group("station_none.png"),
            Some(CraftingGroup::Station { key: None })
        );
        // 54 个图标全部能判定分组
        for (file, _) in auto.iter().chain(overridden.iter()) {
            assert!(crafting.group(file).is_some(), "{file} 应有语义分组");
        }
    }

    /// 映射表防污染指纹：`config/icon_title_overrides.json` 的每个条目都必须
    /// 偏离自动命名（覆盖表只收真例外，与自动命名相同的条目视为冗余）。
    /// 本地无 `DST__ROOT` 时跳过（与 wiki API 测试同惯例，CI 不装游戏）。
    #[test]
    fn test_real_overrides_are_not_default_titles() {
        let Ok(dst_root) = std::env::var("DST__ROOT") else {
            return;
        };
        let dst_root = PathBuf::from(dst_root);
        if !dst_root.join("data/databundles").exists() {
            return;
        }
        let overrides = load_overrides(Path::new(OVERRIDES_DEFAULT_PATH)).unwrap();
        assert!(!overrides.is_empty(), "映射表为空？（CWD 应为 crate 根）");
        let names = read_name_maps(&dst_root).unwrap();
        let crafting = read_crafting_keys(&dst_root).unwrap();
        for (file, title) in overrides.0.iter() {
            // 与 meta 构建同源的英文名：crafting 走解析链，物品栏查 NAMES
            let (source, name_en) = if file.starts_with("filter_") || file.starts_with("station_") {
                (
                    Some("crafting"),
                    crafting.display_name(&names, file).map(|(en, _)| en),
                )
            } else {
                (None, names.names.en.get(&file_stem_key(file)).cloned())
            };
            assert!(
                !title_is_default(file, name_en.as_deref(), source, title),
                "映射表条目 {file} → {title} 与自动命名相同，属于冗余条目"
            );
        }
    }
}
