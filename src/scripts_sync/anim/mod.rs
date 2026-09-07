//! 动画资源历史管理与 diff（anim-sync / anim-diff）。
//!
//! - `anim-sync`：游戏更新后扫描当前 `data/anim`，归档到 `ANIM__OUT_DIR`；
//! - `anim-diff`：对比两个动画目录或历史版本，输出结构化 diff。
//!
//! 第一版支持 `.zip` 与 `dynamic/*.dyn`。`.dyn` 先 xor 解密再按 zip 解析。

pub mod archive;
pub mod diff;
pub mod history;
pub mod index;
pub mod normalize;
pub mod preview;
pub mod skin_index;
pub mod snapshot;

use crate::error::Result;
use crate::scripts_sync::anim::diff::diff_directories;
use crate::scripts_sync::anim::history::{
    diff_file_maps, now_ms, AnimFileEntry, Manifest, ManifestStore, ObjectStore,
};
use crate::scripts_sync::anim::snapshot::{scan_anim_dir, AnimFileKind, FileEntry};
use crate::service::Reporter;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 动画解析器版本；解析逻辑变化时用于触发全量重解析。
pub const PARSER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 一次 `anim-sync` 运行的参数。
#[derive(Debug, Clone)]
pub struct AnimSyncParams {
    /// DST 安装根目录（`DST__ROOT`）。
    pub dst_root: String,
    /// 历史根目录（`ANIM__OUT_DIR` 或 `--out`）；缺省 `output/anim`。
    pub out_dir: Option<PathBuf>,
    /// 版本标签，缺省取 `DST__ROOT/version.txt`。
    pub label: Option<String>,
    /// 忽略幂等检查，强制重新扫描/归档。
    pub force: bool,
    /// 只盘点并报告计划，不写文件。
    pub dry_run: bool,
}

/// 一次 `anim-diff` 运行的参数。
#[derive(Debug, Clone)]
pub struct AnimDiffParams {
    pub old_dir: PathBuf,
    pub new_dir: PathBuf,
    /// 只对比指定相对路径（如 `dynamic/abigail_ice.dyn`）。
    pub only: Option<String>,
    /// 是否也解析未变化的文件（用于 WebUI 浏览）。
    pub parse_all: bool,
}

/// 执行 `anim-sync`，返回机器可读报告。
pub fn run_sync(params: &AnimSyncParams, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    let dst = Path::new(&params.dst_root);
    if !dst.exists() {
        return Err(crate::error::Error::DstDirNotFound(params.dst_root.clone()));
    }
    let anim_dir = dst.join("data/anim");
    if !anim_dir.is_dir() {
        return Err(crate::error::Error::DstDirNotFound(
            anim_dir.display().to_string(),
        ));
    }

    let label = params.label.clone().unwrap_or_else(|| {
        crate::scripts_sync::read_new_version(dst).unwrap_or_else(|_| "unknown".to_string())
    });
    let out_root = params.out_dir.clone().unwrap_or_else(|| {
        std::env::var("ANIM__OUT_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("output/anim"))
    });
    let objects = ObjectStore::new(out_root.join("history/objects"));
    let manifests = ManifestStore::new(out_root.join("history/manifests"));
    let parent = manifests.load_parent(&label)?;
    let existing = manifests.load(&label)?;

    reporter.stage("DST 动画资源同步检查");
    reporter.log(format!(
        "label {} / 基线 {} / 输出 {}",
        label,
        parent
            .as_ref()
            .map(|m| m.label.clone())
            .unwrap_or_else(|| "(无)".into()),
        out_root.display()
    ));

    if !params.force && !params.dry_run {
        if let Some(existing) = existing {
            if existing.complete {
                reporter.log("当前 label 已有完整 manifest，跳过（--force 可强制）".to_string());
                return Ok(serde_json::json!({
                    "status": "up_to_date",
                    "label": label,
                    "output": out_root,
                    "files": existing.files.len(),
                }));
            }
        }
    }

    reporter.stage("扫描当前 data/anim");
    let files = scan_anim_dir(&anim_dir)?;
    reporter.log(format!("发现动画文件 {} 个（zip/dyn）", files.len()));

    if params.dry_run {
        let parent_files = parent.as_ref().map(|m| m.files.clone()).unwrap_or_default();
        let new_entries = build_entries(&files, &parent_files)?;
        let file_diff = diff_file_maps(&parent_files, &new_entries);
        return Ok(serde_json::json!({
            "status": "dry_run",
            "label": label,
            "output": out_root,
            "parent": parent.as_ref().map(|m| m.label.clone()),
            "files": files.len(),
            "diff": file_diff,
        }));
    }

    reporter.stage("归档到 CAS");
    let parent_files = parent.as_ref().map(|m| m.files.clone()).unwrap_or_default();
    let mut entries = BTreeMap::new();
    for (rel, file) in &files {
        let ext = match file.kind {
            AnimFileKind::Zip => ".zip",
            AnimFileKind::Dyn => ".dyn",
        };
        let sha = objects.put_file(&file.path, ext)?;
        let entry = if let Some(old) = parent_files.get(rel) {
            if old.sha256 == sha {
                old.clone()
            } else {
                build_file_entry(file)?
            }
        } else {
            build_file_entry(file)?
        };
        entries.insert(rel.clone(), entry);
    }
    let file_diff = diff_file_maps(&parent_files, &entries);

    let archive_diff = if let Some(parent) = &parent {
        let old_files = load_history_files(&out_root, &parent.label)?;
        Some(diff_directories(
            &parent.label,
            &label,
            &old_files,
            &files,
            None,
            false,
        )?)
    } else {
        None
    };

    reporter.log(format!(
        "added {} / removed {} / changed {}",
        file_diff.added.len(),
        file_diff.removed.len(),
        file_diff.changed.len()
    ));

    let manifest = Manifest {
        label: label.clone(),
        parent_label: parent.as_ref().map(|m| m.label.clone()),
        complete: true,
        parser_version: PARSER_VERSION.to_string(),
        synced_at: Some(now_ms()),
        files: entries,
        diff: file_diff,
    };
    let path = manifests.save(&manifest)?;
    reporter.log(format!("已写入 manifest {}", path.display()));

    Ok(serde_json::json!({
        "status": "synced",
        "label": label,
        "parent": manifest.parent_label,
        "output": out_root,
        "files": manifest.files.len(),
        "diff": manifest.diff,
        "archive_diff": archive_diff,
        "manifest": path,
    }))
}

/// 执行 `anim-diff`，返回机器可读报告。
pub fn run_diff(params: &AnimDiffParams, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    let old_dir = &params.old_dir;
    let new_dir = &params.new_dir;
    if !old_dir.is_dir() {
        return Err(crate::error::Error::DstDirNotFound(
            old_dir.display().to_string(),
        ));
    }
    if !new_dir.is_dir() {
        return Err(crate::error::Error::DstDirNotFound(
            new_dir.display().to_string(),
        ));
    }
    reporter.stage("动画目录对比");
    reporter.log(format!(
        "old {} / new {}",
        old_dir.display(),
        new_dir.display()
    ));

    let old_files = scan_anim_dir(old_dir)?;
    let new_files = scan_anim_dir(new_dir)?;
    reporter.log(format!(
        "old {} 个 / new {} 个",
        old_files.len(),
        new_files.len()
    ));

    let diff = diff_directories(
        &old_dir.display().to_string(),
        &new_dir.display().to_string(),
        &old_files,
        &new_files,
        params.only.as_deref(),
        params.parse_all,
    )?;
    Ok(serde_json::to_value(diff)?)
}

/// 根据历史 manifest 从 CAS 还原一个可解析的文件快照。
pub fn load_history_files(out_root: &Path, label: &str) -> Result<BTreeMap<String, FileEntry>> {
    let objects = ObjectStore::new(out_root.join("history/objects"));
    let manifests = ManifestStore::new(out_root.join("history/manifests"));
    let Some(manifest) = manifests.load(label)? else {
        return Err(crate::error::Error::Config(format!(
            "anim manifest not found: {label}"
        )));
    };
    let mut files = BTreeMap::new();
    for (rel, entry) in &manifest.files {
        let ext = if entry.kind == "dyn" { ".dyn" } else { ".zip" };
        let path = objects.path_for(&entry.sha256, ext);
        if !path.exists() {
            return Err(crate::error::Error::Config(format!(
                "CAS object missing for {rel}: {}",
                path.display()
            )));
        }
        files.insert(
            rel.clone(),
            FileEntry {
                rel_path: rel.clone(),
                path,
                kind: if entry.kind == "dyn" {
                    AnimFileKind::Dyn
                } else {
                    AnimFileKind::Zip
                },
                sha256: entry.sha256.clone(),
                size: entry.size,
            },
        );
    }
    Ok(files)
}

fn build_entries(
    files: &BTreeMap<String, FileEntry>,
    parent: &BTreeMap<String, AnimFileEntry>,
) -> Result<BTreeMap<String, AnimFileEntry>> {
    let mut entries = BTreeMap::new();
    for (rel, file) in files {
        if let Some(old) = parent.get(rel) {
            if old.sha256 == file.sha256 {
                entries.insert(rel.clone(), old.clone());
                continue;
            }
        }
        entries.insert(rel.clone(), build_file_entry(file)?);
    }
    Ok(entries)
}

fn build_file_entry(file: &FileEntry) -> Result<AnimFileEntry> {
    let data = crate::scripts_sync::anim::archive::parse_archive_file(file)?;
    Ok(AnimFileEntry {
        kind: file.kind.as_str().to_string(),
        sha256: file.sha256.clone(),
        size: file.size,
        anim_sha256: data.anim_sha256,
        build_sha256: data.build_sha256,
        tex: data.tex_hashes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_version_not_empty() {
        assert!(!PARSER_VERSION.is_empty());
    }
}
