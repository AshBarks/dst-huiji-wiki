//! 生效标题推导：映射表优先，其次按来源自动命名；非法字符/占位符标记不可上传。

use crate::error::{Error, Result};

use super::overrides::IconTitleOverrides;
use super::{IconMetaEntry, TitleSource};

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

/// 一个图标当前的生效标题：映射表（按本地文件名）优先，其次按来源自动
/// 生成——制作栏图标追加 `Filter` / `Station Icon` 后缀（游戏英文名），技能树
/// 图标用文件 stem 归一化（wiki 端既有命名与游戏英文名无关），物品栏直接
/// 用英文名。
///
/// 与自动命名相同的映射视为未指定（按 `Auto` 返回），防御手工编辑
/// 映射表引入的冗余条目。
pub fn effective_title(
    file: &str,
    name_en: Option<&str>,
    overrides: &IconTitleOverrides,
    source: Option<&str>,
) -> (Option<String>, Option<TitleSource>) {
    let auto = match source {
        Some("skilltree") => auto_title(file.strip_suffix(".png").unwrap_or(file)),
        _ => name_en.and_then(|name_en| match source {
            Some("crafting") => auto_title(&format!("{name_en} {}", crafting_title_suffix(file))),
            _ => auto_title(name_en),
        }),
    };
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

pub(crate) fn invalid_note(name_en: &str) -> String {
    if name_en.contains('{') || name_en.contains('}') {
        "名称含占位符（如 {item}），无法生成维基标题；可在弹窗手动指定文件名".to_string()
    } else if name_en.contains('/') {
        "名称含 /，维基标题会生成子页面；可在弹窗手动指定实际文件名".to_string()
    } else {
        "名称含维基标题非法字符，无法生成标题；可在弹窗手动指定文件名".to_string()
    }
}
