//! 物品图标元数据：`split/inventoryimages/` 文件名 → 生效的维基文件名
//! （`title`，来自映射表或 `STRINGS.NAMES` 英文名）与上传状态。
//!
//! 由 `images-sync` 生成并落盘为 `history/icon_meta.json`（派生数据，可随时
//! 由 manifest + 游戏翻译表 + 映射表重建）；WebUI 与
//! [`crate::service::upload_icons`] 消费同一份产物，避免页面加载时再解析
//! PO / 查询维基。
//!
//! 约定：
//! - 收录两类图标：文件名 stem（大写，如 `abigail_flower.png` →
//!   `ABIGAIL_FLOWER`）能在 `STRINGS.NAMES.*` 精确命中的；以及映射表
//!   [`IconTitleOverrides`] 中显式指定的（可为无英文名的皮肤/变体）；
//! - 英文名以 `strings.pot` 为准（覆盖全部键），中文名取 `chinese_s.po`
//!   非空 msgstr；只有英文没有中文时照常保留英文；
//! - 生效标题优先级：映射表（按本地文件名）> 英文名自动生成；标题含 `{}`
//!   等 MediaWiki 非法字符或 `/` 的条目标记 `uploadable = false` 并附
//!   `note`，上传作业跳过（可用映射表或弹窗手动命名解决）；
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
use std::io::{BufReader, Read};
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
    std::env::var("ICON__TITLE_OVERRIDES")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(OVERRIDES_DEFAULT_PATH))
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
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(overrides)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
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

/// 单个图标的名称与上传信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconMetaEntry {
    /// 英文名（`STRINGS.NAMES.*` msgid）；无对应条目时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_en: Option<String>,
    /// 中文名；无翻译或 msgstr 为空时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_zh: Option<String>,
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
    // 旧版元数据没有 title 字段：按英文名自动标题补齐，保证重跑 images-sync
    // 之前 WebUI/上传仍可正常工作（映射表在下一次 refresh 时应用）。
    for entry in meta.icons.values_mut() {
        if entry.title.is_none() {
            entry.title = entry.name_en.as_deref().and_then(auto_title);
            entry.title_source = entry.title.as_ref().map(|_| TitleSource::Auto);
            entry.uploadable = entry.title.is_some();
            if let (None, Some(name_en)) = (&entry.title, entry.name_en.as_deref()) {
                entry.note.get_or_insert_with(|| invalid_note(name_en));
            }
        }
    }
    Ok(meta)
}

/// 原子写入元数据（先写临时文件再重命名，避免与 WebUI 读并发损坏）。
pub fn save(out_dir: &Path, meta: &IconMeta) -> Result<PathBuf> {
    let path = meta_path(out_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(meta)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

// -- 名称解析 ----------------------------------------------------------------

/// `STRINGS.NAMES.<KEY>` 的中英文名表（键为大写）。
#[derive(Debug, Clone, Default)]
pub struct NameMaps {
    /// KEY → 英文 msgid。
    pub en: BTreeMap<String, String>,
    /// KEY → 中文 msgstr（仅非空翻译）。
    pub zh: BTreeMap<String, String>,
}

impl NameMaps {
    pub fn is_empty(&self) -> bool {
        self.en.is_empty() && self.zh.is_empty()
    }
}

/// 解析 `STRINGS.NAMES.*` 条目。`english_pot` 提供英文全集（可补 chinese
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

/// `english_wins = true` 时覆盖已有英文名（strings.pot 为准）。
fn collect_names(file: &PoFile, maps: &mut NameMaps, english_wins: bool) {
    for e in &file.entries {
        let Some(key) = e
            .msgctxt
            .as_deref()
            .and_then(|c| c.strip_prefix("STRINGS.NAMES."))
        else {
            continue;
        };
        let key = key.to_ascii_uppercase();
        let msgid = e.msgid.trim();
        if !msgid.is_empty() && (english_wins || !maps.en.contains_key(&key)) {
            maps.en.insert(key.clone(), msgid.to_string());
        }
        if !english_wins {
            let msgstr = e.msgstr.trim();
            if !msgstr.is_empty() {
                maps.zh.insert(key, msgstr.to_string());
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

fn read_game_text(dst_root: &Path, rel: &str) -> Result<String> {
    let live = dst_root.join("data/databundles/scripts").join(rel);
    if live.exists() {
        return std::fs::read_to_string(&live).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("读取 {}: {}", live.display(), e),
            ))
        });
    }

    let zip_path = dst_root.join("data/databundles/scripts.zip");
    let file = std::fs::File::open(&zip_path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("打开 {}: {}", zip_path.display(), e),
        ))
    })?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
    let entry = format!("scripts/{rel}");
    let mut f = archive
        .by_name(&entry)
        .map_err(|e| Error::ArchiveFileNotFound(format!("{entry}: {e}")))?;
    let mut content = String::new();
    f.read_to_string(&mut content)?;
    Ok(content)
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

/// 一个文件当前的生效标题：映射表（按本地文件名）优先，其次英文名自动生成。
pub fn effective_title(
    file: &str,
    name_en: Option<&str>,
    overrides: &IconTitleOverrides,
) -> (Option<String>, Option<TitleSource>) {
    if let Some(title) = overrides.get(file) {
        return (Some(title.to_string()), Some(TitleSource::Override));
    }
    match name_en.and_then(auto_title) {
        Some(title) => (Some(title), Some(TitleSource::Auto)),
        None => (None, None),
    }
}

/// 重新计算条目的 `title`/`title_source`/`uploadable`/`note`；标题变化时
/// 清空旧上传状态。返回标题是否发生变化。
pub fn refresh_entry_title(
    entry: &mut IconMetaEntry,
    file: &str,
    overrides: &IconTitleOverrides,
) -> bool {
    let (title, source) = effective_title(file, entry.name_en.as_deref(), overrides);
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
pub fn build_meta(
    build: &str,
    files: impl IntoIterator<Item = String>,
    names: &NameMaps,
    overrides: &IconTitleOverrides,
    previous: &IconMeta,
) -> IconMeta {
    let mut icons = BTreeMap::new();
    for file in files {
        let key = file_stem_key(&file);
        let name_en = names.en.get(&key).cloned();
        let (title, title_source) = effective_title(&file, name_en.as_deref(), overrides);
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
        icons.insert(
            file,
            IconMetaEntry {
                name_en,
                name_zh: names.zh.get(&key).cloned(),
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
        assert_eq!(maps.en.get("AXE").map(String::as_str), Some("Axe"));
        assert_eq!(maps.zh.get("AXE").map(String::as_str), Some("斧头"));
        // 只有英文、没有中文
        assert_eq!(
            maps.en.get("EN_ONLY").map(String::as_str),
            Some("English Only")
        );
        assert!(!maps.zh.contains_key("EN_ONLY"));
        // chinese_s.po 里 msgstr 为空 → 不产生中文
        assert_eq!(
            maps.en.get("EMPTY_ZH").map(String::as_str),
            Some("Empty Zh")
        );
        assert!(!maps.zh.contains_key("EMPTY_ZH"));
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

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn entry(name_en: Option<&str>, title: Option<&str>) -> IconMetaEntry {
        IconMetaEntry {
            name_en: name_en.map(str::to_string),
            name_zh: None,
            title: title.map(str::to_string),
            title_source: title.map(|_| TitleSource::Auto),
            uploadable: title.is_some(),
            note: None,
            wiki: None,
        }
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
            .en
            .insert("PLACEHOLDER".into(), "{item} Blueprint".into());
        let meta2 = build_meta(
            "100",
            files(&["placeholder.png"]),
            &maps2,
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
            maps.en.get("EN_ONLY").map(String::as_str),
            Some("English Only")
        );
        assert_eq!(maps.zh.get("AXE").map(String::as_str), Some("斧头"));
        std::fs::remove_dir_all(&root).ok();
    }
}
