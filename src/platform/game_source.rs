//! 统一的游戏数据源：snapshot → live 解压树 → scripts.zip。
//!
//! 此前四套读取逻辑各自为政（`DstContext::read_script_file`、
//! `dataset::read_game_file`（无 zip 回退）、`images/meta::read_game_text`
//! （无 snapshot 支持）、`update` 直接目录读取），同一个 `--snapshot` 在
//! 不同命令里行为不同。本模块是唯一的解析顺序定义处：
//!
//! 1. 指定 snapshot 时**严格只读** `data/databundles/<snapshot>/`；
//! 2. 未指定时先读 live 解压树 `data/databundles/scripts/`；
//! 3. 回退 `data/databundles/scripts.zip`（zip 内条目名 `scripts/<rel>`）。

use std::collections::BTreeSet;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::error::{Error, Result};

pub struct GameSource {
    dst_root: PathBuf,
    snapshot: Option<String>,
}

impl GameSource {
    /// 显式指定游戏根目录构建；snapshot 目录存在性在此校验。
    pub fn new(dst_root: impl Into<PathBuf>, snapshot: Option<String>) -> Result<Self> {
        let dst_root = dst_root.into();
        if !dst_root.exists() {
            return Err(Error::DstDirNotFound(dst_root.display().to_string()));
        }
        if let Some(name) = &snapshot {
            let dir = Self::snapshot_dir(&dst_root, name);
            if !dir.exists() {
                return Err(Error::Config(format!(
                    "Snapshot directory does not exist: {}",
                    dir.display()
                )));
            }
        }
        Ok(Self { dst_root, snapshot })
    }

    /// 从 `DST__ROOT` 环境变量构建。
    pub fn from_env(snapshot: Option<String>) -> Result<Self> {
        Self::new(crate::platform::config::dst_root()?, snapshot)
    }

    pub fn dst_root(&self) -> &Path {
        &self.dst_root
    }

    pub fn snapshot(&self) -> Option<&str> {
        self.snapshot.as_deref()
    }

    /// 读取 scripts 根下的文本文件（容忍 `scripts/` 前缀）。
    pub fn read(&self, rel: &str) -> Result<String> {
        let rel = rel.strip_prefix("scripts/").unwrap_or(rel);

        if let Some(snap) = &self.snapshot {
            let path = Self::snapshot_dir(&self.dst_root, snap).join(rel);
            return std::fs::read_to_string(&path).map_err(|e| read_error(&path, e));
        }

        let live = self.live_scripts_dir().join(rel);
        if live.exists() {
            return std::fs::read_to_string(&live).map_err(|e| read_error(&live, e));
        }

        self.read_zip(rel)
    }

    /// 列出 scripts 根下某目录的直接子项（文件与目录名，不含路径）。
    /// zip 回退时枚举 `scripts/<rel>/` 下的直接子项。
    pub fn list_dir(&self, rel: &str) -> Result<Vec<String>> {
        let rel = rel.strip_prefix("scripts/").unwrap_or(rel);

        if let Some(snap) = &self.snapshot {
            let dir = Self::snapshot_dir(&self.dst_root, snap).join(rel);
            return list_fs_dir(&dir);
        }

        let live = self.live_scripts_dir().join(rel);
        if live.exists() {
            return list_fs_dir(&live);
        }

        let prefix = format!("scripts/{}/", rel.trim_end_matches('/'));
        let mut archive = self.open_zip()?;
        let mut names = BTreeSet::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name();
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            // 只取直接子项（zip 条目可能带完整中间目录路径）。
            if rest.is_empty() || rest.contains('/') {
                continue;
            }
            names.insert(rest.to_string());
        }
        Ok(names.into_iter().collect())
    }

    fn databundles(&self) -> PathBuf {
        self.dst_root.join("data/databundles")
    }

    fn live_scripts_dir(&self) -> PathBuf {
        self.databundles().join("scripts")
    }

    fn snapshot_dir(dst_root: &Path, name: &str) -> PathBuf {
        dst_root.join("data/databundles").join(name)
    }

    fn open_zip(&self) -> Result<ZipArchive<BufReader<std::fs::File>>> {
        let scripts_zip = self.databundles().join("scripts.zip");
        if !scripts_zip.exists() {
            return Err(Error::Config(format!(
                "无法读取 {}：scripts.zip 也不存在；请先 scripts-sync 解压，或检查 --snapshot 拼写",
                self.live_scripts_dir().display()
            )));
        }
        let file = std::fs::File::open(&scripts_zip)?;
        Ok(ZipArchive::new(BufReader::new(file))?)
    }

    fn read_zip(&self, rel: &str) -> Result<String> {
        let mut archive = self.open_zip()?;
        let entry = format!("scripts/{}", rel);
        let mut f = archive
            .by_name(&entry)
            .map_err(|e| Error::ArchiveFileNotFound(format!("{}: {}", entry, e)))?;
        let mut content = String::new();
        f.read_to_string(&mut content)?;
        Ok(content)
    }
}

fn read_error(path: &Path, e: std::io::Error) -> Error {
    Error::Io(std::io::Error::other(format!(
        "read {}: {}",
        path.display(),
        e
    )))
}

fn list_fs_dir(dir: &Path) -> Result<Vec<String>> {
    let entries = std::fs::read_dir(dir).map_err(|e| read_error(dir, e))?;
    Ok(entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tree(dir: &Path, files: &[(&str, &str)]) {
        for (rel, body) in files {
            let abs = dir.join(rel);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(abs, body).unwrap();
        }
    }

    #[test]
    fn read_prefers_live_then_zip_fallback() {
        let tmp = std::env::temp_dir().join(format!("dst_gs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let root = tmp.join("game");
        write_tree(&root, &[("version.txt", "123456")]);

        // 无 live 无 zip → 报错。
        let gs = GameSource::new(&root, None).unwrap();
        assert!(gs.read("prefabs/axe.lua").is_err());

        // zip 回退可读。
        let zip_path = root.join("data/databundles/scripts.zip");
        let _ = std::fs::create_dir_all(zip_path.parent().unwrap());
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file(
            "scripts/prefabs/axe.lua",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        std::io::Write::write_all(&mut zw, b"return 'from-zip'").unwrap();
        zw.finish().unwrap();
        assert_eq!(gs.read("prefabs/axe.lua").unwrap(), "return 'from-zip'");
        assert_eq!(
            gs.read("scripts/prefabs/axe.lua").unwrap(),
            "return 'from-zip'"
        );

        // live 树优先。
        write_tree(
            &root,
            &[("data/databundles/scripts/prefabs/axe.lua", "return 'live'")],
        );
        assert_eq!(gs.read("prefabs/axe.lua").unwrap(), "return 'live'");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn snapshot_is_strict_no_fallback() {
        let tmp = std::env::temp_dir().join(format!("dst_gss_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let root = tmp.join("game");
        write_tree(
            &root,
            &[
                ("version.txt", "1"),
                ("data/databundles/scripts.zip", "not a zip"),
                ("data/databundles/scripts/prefabs/axe.lua", "return 'live'"),
                (
                    "data/databundles/scripts_1/prefabs/axe.lua",
                    "return 'snap'",
                ),
            ],
        );
        let gs = GameSource::new(&root, Some("scripts_1".into())).unwrap();
        assert_eq!(gs.snapshot(), Some("scripts_1"));
        assert_eq!(gs.read("prefabs/axe.lua").unwrap(), "return 'snap'");
        // snapshot 内缺失 → 直接报错，不回退 live/zip。
        assert!(gs.read("prefabs/missing.lua").is_err());

        // 不存在的 snapshot 名在构造期报错。
        assert!(GameSource::new(&root, Some("nope".into())).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn list_dir_covers_fs_and_zip() {
        let tmp = std::env::temp_dir().join(format!("dst_gsl_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let root = tmp.join("game");
        write_tree(&root, &[("version.txt", "1")]);
        let gs = GameSource::new(&root, None).unwrap();

        // zip 枚举直接子项。
        let zip_path = root.join("data/databundles/scripts.zip");
        let _ = std::fs::create_dir_all(zip_path.parent().unwrap());
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        for name in [
            "scripts/prefabs/skilltree_walter.lua",
            "scripts/prefabs/skilltree_willow.lua",
            "scripts/tuning.lua",
        ] {
            zw.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zw, b"return 1").unwrap();
        }
        zw.finish().unwrap();
        let mut names = gs.list_dir("prefabs").unwrap();
        names.sort();
        assert_eq!(names, vec!["skilltree_walter.lua", "skilltree_willow.lua"]);

        // live 树存在时走文件系统（含子目录名）。
        write_tree(
            &root,
            &[
                ("data/databundles/scripts/prefabs/skilltree_woodie.lua", "x"),
                ("data/databundles/scripts/prefabs/sub", ""),
            ],
        );
        let names = gs.list_dir("prefabs").unwrap();
        assert!(names.contains(&"skilltree_woodie.lua".to_string()));
        assert!(names.contains(&"sub".to_string()));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
