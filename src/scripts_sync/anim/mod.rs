//! 动画资源历史管理与 diff（anim-sync / anim-diff）。
//!
//! - `anim-sync`：游戏更新后扫描当前 `data/anim`，归档到 `ANIM__OUT_DIR`；
//! - `anim-diff`：对比两个动画目录或历史版本，输出结构化 diff。
//!
//! 第一版支持 `.zip` 与 `dynamic/*.dyn`。`.dyn` 先 xor 解密再按 zip 解析。
//!
//! 增量与正确性规则：
//! - **解析器版本变更**（manifest.parser_version ≠ [`PARSER_VERSION`]）时
//!   等效 `--force`：全量重解析，旧解析器产出的条目级 hash 不可复用；
//! - **版本回退**（当前 label 数值低于任何已记录 label 的 manifest）时
//!   拒绝操作（即使 `--force`），报告 `rollback_skipped`，不写任何文件。

pub mod archive;
pub mod diff;
pub mod history;
pub mod index;
pub mod normalize;
pub mod preview;
pub mod remap_history;
pub mod skin_index;
pub mod snapshot;

use crate::error::Result;
use crate::platform::progress::Reporter;
use crate::scripts_sync::anim::diff::diff_directories;
use crate::scripts_sync::anim::history::{
    diff_file_maps, now_ms, AnimFileEntry, Manifest, ManifestStore, ObjectStore,
};
use crate::scripts_sync::anim::snapshot::{scan_anim_dir, AnimFileKind, FileEntry};
use crate::scripts_sync::state;
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
    let out_root = params
        .out_dir
        .clone()
        .unwrap_or_else(crate::platform::config::anim_out_dir);
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

    // 版本回退：当前 label 低于任何已记录 label 时拒绝操作（即使 --force
    // 也不放行；dry-run 同样直接返回），避免用旧版输入污染新版历史。
    if let Some(recorded) = manifests.latest_recorded_label()? {
        if state::is_rollback(&label, &recorded) {
            let msg = format!(
                "检测到版本回退：当前 {label} 低于已记录的 {recorded}，\
                 为避免污染动画历史，本次不执行任何操作。\
                 请确认 DST__ROOT 指向正确的游戏安装（或删除多余的 manifest 后重试）"
            );
            reporter.log(msg.clone());
            return Ok(serde_json::json!({
                "status": "rollback_skipped",
                "label": label,
                "recorded_label": recorded,
                "message": msg,
                "output": out_root,
            }));
        }
    }

    // 解析器版本变更（含当前 label 的旧 manifest）→ 等效 force：
    // 旧解析器产出的条目级 hash（anim/build/tex）不可与新解析器混用。
    let versioned = existing.as_ref().or(parent.as_ref());
    let parser_changed = versioned
        .map(|m| m.parser_version != PARSER_VERSION)
        .unwrap_or(false);
    let effective_force = params.force || parser_changed;
    if parser_changed {
        reporter.log(format!(
            "解析器已从 {:?} 切换到 {PARSER_VERSION}，本次全量重解析",
            versioned.map(|m| m.parser_version.as_str()).unwrap_or("")
        ));
    }

    if !effective_force && !params.dry_run {
        if let Some(existing) = existing {
            if existing.complete {
                reporter.log("当前 label 已有完整 manifest，跳过（--force 可强制）".to_string());
                return Ok(serde_json::json!({
                    "status": "up_to_date",
                    "label": label,
                    "parser_version": PARSER_VERSION,
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
        let parent_parser_ok = parent
            .as_ref()
            .map(|m| m.parser_version == PARSER_VERSION)
            .unwrap_or(false);
        let new_entries = build_entries(&files, &parent_files, parent_parser_ok)?;
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
    // parent 条目由旧解析器产出时不可复用（条目级 hash 语义已变），须重解析。
    let parent_parser_ok = parent
        .as_ref()
        .map(|m| m.parser_version == PARSER_VERSION)
        .unwrap_or(false);
    let mut entries = BTreeMap::new();
    for (rel, file) in &files {
        let ext = match file.kind {
            AnimFileKind::Zip => ".zip",
            AnimFileKind::Dyn => ".dyn",
        };
        let sha = objects.put_file(&file.path, ext)?;
        let entry = if let Some(old) = parent_files.get(rel) {
            if old.sha256 == sha && parent_parser_ok {
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
    parent_parser_ok: bool,
) -> Result<BTreeMap<String, AnimFileEntry>> {
    let mut entries = BTreeMap::new();
    for (rel, file) in files {
        if let Some(old) = parent.get(rel) {
            if old.sha256 == file.sha256 && parent_parser_ok {
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

    /// 构造最小 anim 源目录：一个 zip（内容不要求可解析为 anim/build，
    /// `parse_archive_file` 对无 anim.bin/build.bin 的 zip 返回空数据）。
    fn make_dst(ws: &Path) -> (PathBuf, PathBuf) {
        let dst = ws.join("dst");
        let anim = dst.join("data/anim");
        std::fs::create_dir_all(&anim).unwrap();
        std::fs::write(dst.join("version.txt"), b"100\n").unwrap();
        let file = std::fs::File::create(anim.join("a.zip")).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        use std::io::Write as _;
        zw.start_file("dummy.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"dummy").unwrap();
        zw.finish().unwrap();
        (dst, anim)
    }

    fn params(dst: &Path, out: &Path, force: bool, dry_run: bool) -> AnimSyncParams {
        AnimSyncParams {
            dst_root: dst.display().to_string(),
            out_dir: Some(out.to_path_buf()),
            label: None,
            force,
            dry_run,
        }
    }

    #[derive(Default)]
    struct TestReporter;

    impl Reporter for TestReporter {
        fn log(&self, _msg: String) {}
        fn stage(&self, _name: &str) {}
        fn diff(&self, _page: &str, _text: &str, _added: usize, _removed: usize) {}
        fn confirm(&self, _prompt: &str) -> bool {
            false
        }
    }

    #[test]
    fn test_sync_idempotent_then_parser_upgrade_reprocesses() {
        let ws = std::env::temp_dir().join(format!("anim_sync_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let (dst, _) = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter;

        let report = run_sync(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        // 同版本重复运行 → 幂等跳过
        let report = run_sync(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "up_to_date");

        // 模拟解析器升级：把 manifest 的 parser_version 改掉 → 应重新解析
        let manifest_path = out.join("history/manifests/100.json");
        let mut m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        m["parser_version"] = serde_json::json!("old-parser/0");
        std::fs::write(&manifest_path, serde_json::to_string(&m).unwrap()).unwrap();

        let report = run_sync(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        let m: Manifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(m.parser_version, PARSER_VERSION);
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_rollback_refuses_even_with_force() {
        let ws = std::env::temp_dir().join(format!("anim_roll_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let (dst, _) = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter;
        run_sync(&params(&dst, &out, false, false), &rep).unwrap();

        // 版本回退：version.txt 降到 50
        std::fs::write(dst.join("version.txt"), b"50\n").unwrap();
        for (force, dry_run) in [(false, false), (true, false), (false, true)] {
            let report = run_sync(&params(&dst, &out, force, dry_run), &rep).unwrap();
            assert_eq!(report["status"], "rollback_skipped", "{report}");
            assert_eq!(report["recorded_label"], "100");
        }
        // 磁盘无任何写入：无 50.json
        assert!(!out.join("history/manifests/50.json").exists());
        // 恢复 100 → 幂等 up_to_date
        std::fs::write(dst.join("version.txt"), b"100\n").unwrap();
        let report = run_sync(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "up_to_date");
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_dry_run_touches_nothing() {
        let ws = std::env::temp_dir().join(format!("anim_dry_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let (dst, _) = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter;

        let report = run_sync(&params(&dst, &out, false, true), &rep).unwrap();
        assert_eq!(report["status"], "dry_run", "{report}");
        assert!(!out.exists());
        std::fs::remove_dir_all(&ws).ok();
    }
}
