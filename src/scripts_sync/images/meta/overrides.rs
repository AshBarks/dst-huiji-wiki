//! 图标文件名 → 维基文件名的例外映射表（`config/icon_title_overrides.json`）。

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::title::normalize_explicit_title;

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
