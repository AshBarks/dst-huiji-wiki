//! DST `scripts` 同步:版本检测 → 归档旧树为快照 → 解压新 `scripts.zip`。
//!
//! Port of the legacy Python `update_scripts.py` scripts-only flow:
//! 1. compare `dst_root/version.txt` with the version state file
//! 2. extract `data/databundles/scripts.zip` into an `incoming_<ts>` staging
//!    directory (atomicity: the live tree is untouched until the new tree is
//!    fully staged)
//! 3. rename the live `scripts/` tree to `scripts_<ts>` — the snapshot naming
//!    convention consumed by [`crate::context::DstContext::list_snapshots`]
//! 4. move the staged tree into place (rolling back the rename on failure),
//!    then record the new version in the state file
//!
//! The image pipeline (images.zip / built-in KTEX decode / atlas splitting)
//! lives in the [`images`] submodule (`images-sync`).

pub mod images;
pub mod state;

use crate::error::{Error, Result};
use crate::service::Reporter;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Parameters for one `scripts-sync` run.
#[derive(Debug, Clone)]
pub struct SyncParams {
    /// DST install root (the `DST__ROOT` value).
    pub dst_root: String,
    /// Version state file; defaults to `./dst_version.txt`.
    pub state_path: Option<PathBuf>,
    /// Sync even when the recorded version matches.
    pub force: bool,
    /// Report the planned actions without touching any file.
    pub dry_run: bool,
}

/// Runs the sync flow and returns a machine-readable report.
///
/// Status values: `synced`, `up_to_date`, `dry_run`.
pub fn sync(params: &SyncParams, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    let dst = Path::new(&params.dst_root);
    if !dst.exists() {
        return Err(Error::DstDirNotFound(params.dst_root.clone()));
    }
    let bundles = dst.join("data/databundles");
    let scripts_zip = bundles.join("scripts.zip");
    if !scripts_zip.exists() {
        return Err(Error::DstDirNotFound(scripts_zip.display().to_string()));
    }
    let state_path = params
        .state_path
        .clone()
        .unwrap_or_else(state::default_state_path);

    reporter.stage("DST scripts 同步检查");
    let new_version = read_new_version(dst)?;
    let last = state::read_last_version(&state_path)?;
    reporter.log(format!(
        "版本对比: 记录 {} / version.txt {}",
        last.as_deref().unwrap_or("(无记录)"),
        new_version
    ));

    if !params.force && !state::needs_update(last.as_deref(), &new_version) {
        reporter.log("DST 版本已是最新,跳过同步(--force 可强制)".to_string());
        return Ok(serde_json::json!({
            "status": "up_to_date",
            "version": new_version,
            "state_file": state_path.display().to_string(),
        }));
    }

    let ts = snapshot_timestamp();
    let snapshot_name = format!("scripts_{ts}");
    let incoming = bundles.join(format!("incoming_{ts}"));
    let live = bundles.join("scripts");
    let snapshot_dir = bundles.join(&snapshot_name);

    // Timestamp collisions degrade to a clear error + hint (no auto-suffix),
    // keeping the `scripts_<yyyymmddhhmm>` naming stable for snapshot tooling.
    if incoming.exists() || snapshot_dir.exists() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "目录冲突:{} 或 {} 已存在(同一分钟内重复运行?)。请稍后重试或手动处理",
                incoming.display(),
                snapshot_dir.display()
            ),
        )));
    }

    if params.dry_run {
        reporter.stage("dry-run 计划(不修改任何文件)");
        if live.exists() {
            reporter.log(format!(
                "将归档 {} → {}",
                live.display(),
                snapshot_dir.display()
            ));
        } else {
            reporter.log("无现存 scripts 树,将直接解压".to_string());
        }
        reporter.log(format!(
            "将解压 {} → {}",
            scripts_zip.display(),
            live.display()
        ));
        reporter.log(format!("将写入版本记录 {}", state_path.display()));
        return Ok(serde_json::json!({
            "status": "dry_run",
            "old_version": last,
            "new_version": new_version,
            "planned_snapshot": snapshot_name,
        }));
    }

    // 1. Stage the new tree first so the live tree stays intact until the
    //    replacement is fully extracted.
    reporter.stage("解压新 scripts.zip");
    reporter.log(format!("暂存目录 {}", incoming.display()));
    extract_scripts_zip(&scripts_zip, &incoming)?;
    let staged_scripts = incoming.join("scripts");
    if !staged_scripts.is_dir() {
        let _ = fs_err_cleanup(&incoming);
        return Err(Error::Config(format!(
            "{} 顶层缺少 scripts/ 目录,已清理暂存目录",
            scripts_zip.display()
        )));
    }

    // 2. Archive the live tree (if any) under the snapshot name.
    let mut archived: Option<String> = None;
    if live.exists() {
        reporter.stage("归档旧 scripts 树");
        std::fs::rename(&live, &snapshot_dir)?;
        archived = Some(snapshot_name.clone());
        reporter.log(format!("已重命名 → {}", snapshot_dir.display()));
    }

    // 3. Move the staged tree into place; roll back the archive rename on
    //    failure so the live tree is never left missing.
    if let Err(e) = std::fs::rename(&staged_scripts, &live) {
        if archived.is_some() {
            let _ = std::fs::rename(&snapshot_dir, &live);
        }
        let _ = fs_err_cleanup(&incoming);
        return Err(e.into());
    }
    let _ = fs_err_cleanup(&incoming);

    // 4. Record the new version.
    reporter.stage("写入版本记录");
    state::write_version(&state_path, &new_version)?;
    reporter.log(format!("已写入 {}", state_path.display()));

    Ok(serde_json::json!({
        "status": "synced",
        "old_version": last,
        "new_version": new_version,
        "archived_snapshot": archived,
        "state_file": state_path.display().to_string(),
    }))
}

/// Reads `<dst_root>/version.txt` (written by the game updater).
pub fn read_new_version(dst_root: &Path) -> Result<String> {
    let path = dst_root.join("version.txt");
    std::fs::read_to_string(&path)
        .map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("读取 {}: {}", path.display(), e),
            ))
        })
        .map(|s| s.trim().to_string())
}

/// Extracts `scripts.zip` into `target` (which is created if missing).
fn extract_scripts_zip(zip_path: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
    Ok(archive.extract(target)?)
}

/// Best-effort removal of a staging directory.
fn fs_err_cleanup(dir: &Path) -> Result<()> {
    Ok(std::fs::remove_dir_all(dir)?)
}

/// Current timestamp (`yyyymmddhhmm`, UTC) for snapshot naming.
///
/// Matches the `%Y%m%d%H%M` convention of existing `scripts_<ts>` snapshots;
/// zero-padded digits keep lexicographic order equal to chronological order.
pub fn snapshot_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_timestamp(secs)
}

/// Formats UNIX seconds (UTC) as `yyyymmddhhmm`.
fn format_timestamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hour, min) = (rem / 3600, (rem % 3600) / 60);
    format!("{y:04}{m:02}{d:02}{hour:02}{min:02}")
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 → (y, m, d),
/// proleptic Gregorian.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    // -- test reporter ------------------------------------------------------

    #[derive(Default)]
    struct TestReporter {
        logs: Mutex<Vec<String>>,
    }

    impl Reporter for TestReporter {
        fn log(&self, msg: String) {
            self.logs.lock().unwrap().push(msg);
        }
        fn stage(&self, name: &str) {
            self.logs.lock().unwrap().push(format!("== {name} =="));
        }
        fn diff(&self, _page: &str, _text: &str, _added: usize, _removed: usize) {}
        fn confirm(&self, _prompt: &str) -> bool {
            false
        }
    }

    // -- temp workspace helper ----------------------------------------------

    static WS_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn make_workspace(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dst_huiji_scripts_sync_ws_{}_{tag}_{}",
            std::process::id(),
            WS_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// Builds a minimal `scripts.zip` containing `scripts/<name>` entries.
    fn make_scripts_zip(zip_path: &Path, files: &[(&str, &str)]) {
        std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        for (name, content) in files {
            zw.start_file(
                format!("scripts/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(content.as_bytes()).unwrap();
        }
        zw.finish().unwrap();
    }

    fn params(dst_root: &Path, state: &Path, force: bool, dry_run: bool) -> SyncParams {
        SyncParams {
            dst_root: dst_root.display().to_string(),
            state_path: Some(state.to_path_buf()),
            force,
            dry_run,
        }
    }

    // -- timestamp ----------------------------------------------------------

    #[test]
    fn test_format_timestamp_known_values() {
        assert_eq!(format_timestamp(0), "197001010000");
        // 2023-11-14 22:13:20 UTC
        assert_eq!(format_timestamp(1_700_000_000), "202311142213");
        // 2024-02-29 00:00:00 UTC (leap day)
        assert_eq!(format_timestamp(1_709_164_800), "202402290000");
    }

    #[test]
    fn test_snapshot_timestamp_shape() {
        let ts = snapshot_timestamp();
        assert_eq!(ts.len(), 12);
        assert!(ts.chars().all(|c| c.is_ascii_digit()), "{ts}");
    }

    // -- sync flows ---------------------------------------------------------

    #[test]
    fn test_sync_archives_live_tree_and_extracts_new() {
        let ws = make_workspace("full");
        let dst = ws.join("dst");
        let bundles = dst.join("data/databundles");
        write_file(&dst.join("version.txt"), "747465\n");
        write_file(&bundles.join("scripts/a.lua"), "old-a");
        make_scripts_zip(
            &bundles.join("scripts.zip"),
            &[("a.lua", "new-a"), ("b.lua", "new-b")],
        );
        let state_path = ws.join("dst_version.txt");

        let rep = TestReporter::default();
        let report = sync(&params(&dst, &state_path, false, false), &rep).unwrap();

        assert_eq!(report["status"], "synced");
        assert_eq!(report["new_version"], "747465");
        let snapshot = report["archived_snapshot"].as_str().unwrap().to_string();
        assert!(snapshot.starts_with("scripts_"));

        // Live tree now holds the new content.
        assert_eq!(
            std::fs::read_to_string(bundles.join("scripts/a.lua")).unwrap(),
            "new-a"
        );
        // Old tree preserved as a snapshot.
        assert_eq!(
            std::fs::read_to_string(bundles.join(&snapshot).join("a.lua")).unwrap(),
            "old-a"
        );
        // Staging dir removed, version recorded.
        assert!(!bundles
            .join(format!("incoming_{}", &snapshot[8..]))
            .exists());
        assert_eq!(
            state::read_last_version(&state_path).unwrap().as_deref(),
            Some("747465")
        );
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_up_to_date_skips() {
        let ws = make_workspace("uptodate");
        let dst = ws.join("dst");
        let bundles = dst.join("data/databundles");
        write_file(&dst.join("version.txt"), "747465");
        write_file(&bundles.join("scripts/a.lua"), "old-a");
        make_scripts_zip(&bundles.join("scripts.zip"), &[("a.lua", "new-a")]);
        let state_path = ws.join("state.txt");
        state::write_version(&state_path, "747465").unwrap();

        let rep = TestReporter::default();
        let report = sync(&params(&dst, &state_path, false, false), &rep).unwrap();

        assert_eq!(report["status"], "up_to_date");
        // Nothing was touched.
        assert_eq!(
            std::fs::read_to_string(bundles.join("scripts/a.lua")).unwrap(),
            "old-a"
        );
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_force_rearchives_even_when_up_to_date() {
        let ws = make_workspace("force");
        let dst = ws.join("dst");
        let bundles = dst.join("data/databundles");
        write_file(&dst.join("version.txt"), "747465");
        write_file(&bundles.join("scripts/a.lua"), "same");
        make_scripts_zip(&bundles.join("scripts.zip"), &[("a.lua", "same")]);
        let state_path = ws.join("state.txt");
        state::write_version(&state_path, "747465").unwrap();

        let rep = TestReporter::default();
        let report = sync(&params(&dst, &state_path, true, false), &rep).unwrap();

        assert_eq!(report["status"], "synced");
        assert!(report["archived_snapshot"].as_str().is_some());
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_dry_run_touches_nothing() {
        let ws = make_workspace("dryrun");
        let dst = ws.join("dst");
        let bundles = dst.join("data/databundles");
        write_file(&dst.join("version.txt"), "747466");
        write_file(&bundles.join("scripts/a.lua"), "old-a");
        make_scripts_zip(&bundles.join("scripts.zip"), &[("a.lua", "new-a")]);
        let state_path = ws.join("state.txt");
        state::write_version(&state_path, "747465").unwrap();

        let rep = TestReporter::default();
        let report = sync(&params(&dst, &state_path, false, true), &rep).unwrap();

        assert_eq!(report["status"], "dry_run");
        assert!(report["planned_snapshot"]
            .as_str()
            .unwrap()
            .starts_with("scripts_"));
        // Nothing changed on disk.
        assert!(bundles.join("scripts/a.lua").exists());
        let entries: Vec<_> = std::fs::read_dir(&bundles)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            !entries.iter().any(|n| n.starts_with("scripts_2")),
            "{entries:?}"
        );
        assert!(
            !entries.iter().any(|n| n.starts_with("incoming_")),
            "{entries:?}"
        );
        assert_eq!(
            state::read_last_version(&state_path).unwrap().as_deref(),
            Some("747465")
        );
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_first_run_without_live_tree() {
        let ws = make_workspace("first");
        let dst = ws.join("dst");
        let bundles = dst.join("data/databundles");
        write_file(&dst.join("version.txt"), "747465");
        make_scripts_zip(&bundles.join("scripts.zip"), &[("a.lua", "new-a")]);
        let state_path = ws.join("state.txt");

        let rep = TestReporter::default();
        let report = sync(&params(&dst, &state_path, false, false), &rep).unwrap();

        assert_eq!(report["status"], "synced");
        assert!(report["archived_snapshot"].is_null());
        assert_eq!(
            std::fs::read_to_string(bundles.join("scripts/a.lua")).unwrap(),
            "new-a"
        );
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_missing_version_file_errors() {
        let ws = make_workspace("nover");
        let dst = ws.join("dst");
        let bundles = dst.join("data/databundles");
        make_scripts_zip(&bundles.join("scripts.zip"), &[("a.lua", "new-a")]);
        let state_path = ws.join("state.txt");

        let rep = TestReporter::default();
        let err = sync(&params(&dst, &state_path, false, false), &rep).unwrap_err();
        assert!(err.to_string().contains("version.txt"), "{err}");
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_sync_missing_dst_root_errors() {
        let ws = make_workspace("nodst");
        let state_path = ws.join("state.txt");
        let rep = TestReporter::default();
        let params = SyncParams {
            dst_root: ws.join("no-such-dir").display().to_string(),
            state_path: Some(state_path),
            force: false,
            dry_run: false,
        };
        let err = sync(&params, &rep).unwrap_err();
        assert!(matches!(err, Error::DstDirNotFound(_)));
        std::fs::remove_dir_all(&ws).ok();
    }
}
