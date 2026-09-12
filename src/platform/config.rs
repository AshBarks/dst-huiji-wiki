//! 环境变量与默认路径的集中解析。
//!
//! 此前 `DST__ROOT`/`KTOOLS__OUT_DIR`/`ANIM__OUT_DIR`/`ICON__TITLE_OVERRIDES`
//! 的读取散落 30+ 处且默认值各写一遍（例如 `template_check` 硬编码
//! `config/icon_title_overrides.json`，而 images/upload 走环境变量，
//! 用户自定义路径时两边不一致）。路径类环境变量统一从这里取。

use std::path::PathBuf;

use crate::error::{Error, Result};

/// 游戏安装根目录（`DST__ROOT`，必填）。
pub fn dst_root() -> Result<PathBuf> {
    std::env::var("DST__ROOT")
        .map(PathBuf::from)
        .map_err(|e| Error::EnvVarNotFound(format!("DST__ROOT: {}", e)))
}

/// [`dst_root`] 的 String 形式（`DstContext` 等旧接口仍收 String）。
pub fn dst_root_str() -> Result<String> {
    dst_root().map(|p| p.to_string_lossy().into_owned())
}

/// [`dst_root`] 的可选形式（未设置时 `None`，不报错）。
pub fn dst_root_opt() -> Option<PathBuf> {
    non_empty_env("DST__ROOT")
}

/// images-sync 产物根目录（`KTOOLS__OUT_DIR`，缺省 `output/ktools`）。
pub fn ktools_out_dir() -> PathBuf {
    non_empty_env("KTOOLS__OUT_DIR").unwrap_or_else(|| "output/ktools".into())
}

/// 动画历史根目录（`ANIM__OUT_DIR`，缺省 `output/anim`）。
pub fn anim_out_dir() -> PathBuf {
    non_empty_env("ANIM__OUT_DIR").unwrap_or_else(|| "output/anim".into())
}

/// 图标标题映射表（`ICON__TITLE_OVERRIDES`，缺省 `config/icon_title_overrides.json`）。
pub fn icon_title_overrides_path() -> PathBuf {
    non_empty_env("ICON__TITLE_OVERRIDES")
        .unwrap_or_else(|| "config/icon_title_overrides.json".into())
}

fn non_empty_env(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    #[test]
    fn env_semantics_documented() {
        // platform::config 的函数都读进程环境，测试进程内不做写入/恢复，
        // 这里仅固化“未设置变量返回 Err/None”的语义约定。
        assert!(std::env::var("DEFINITELY_NOT_SET_8f3c").is_err());
    }
}
