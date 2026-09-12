//! 原子文件写入与目录准备。
//!
//! 统一此前散落在 `images/meta`、`corpus/store`、`knowledge/store`、
//! `update/state`、`corpus/prefab_index`、service 输出产物里的各套
//! temp+rename 实现：service 层大量裸 `std::fs::write` 是非原子的，
//! Web 端可能读到半截 JSON。

use std::io::Write;
use std::path::Path;

use crate::error::Result;

/// 确保目标文件的父目录存在。
pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// 临时文件名：同目录下 `<file>.<ext>.tmp-<pid>`，避免并发写互相踩踏。
fn tmp_path(path: &Path) -> std::path::PathBuf {
    let pid = std::process::id();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp-{}", pid));
    path.with_file_name(name)
}

/// 原子写文本：先写同目录临时文件再 rename，父目录自动创建。
pub fn write_text_atomic(path: &Path, body: &str) -> Result<()> {
    ensure_parent(path)?;
    let tmp = tmp_path(path);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 原子写 JSON（pretty 序列化 + [`write_text_atomic`]）。
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let body = serde_json::to_string_pretty(value)?;
    write_text_atomic(path, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_text_atomic_creates_parent_and_roundtrips() {
        let dir = std::env::temp_dir().join(format!("dst_platfs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("a/b/c.txt");
        write_text_atomic(&path, "hello").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        // 覆盖写也原子。
        write_text_atomic(&path, "world2").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "world2");
        // 无临时文件残留。
        let leftovers: Vec<_> = std::fs::read_dir(dir.join("a/b"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_atomic_pretty_prints() {
        let dir = std::env::temp_dir().join(format!("dst_platjs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("x.json");
        write_json_atomic(&path, &serde_json::json!({"a": 1})).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\n  \"a\": 1"), "应为 pretty 格式：{raw}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
