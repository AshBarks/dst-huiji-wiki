//! `images-sync`：游戏图片资源管线（scripts_sync 的图片下半段）。
//!
//! 数据流（两源合并视图见 [`scan`]，差异历史见 [`history`]，KTEX 解码见
//! [`ktex`]）：
//!
//! ```text
//! scan    两源盘点（zip 条目直读 + loose 原位）→ 合并视图 + 输入 hash
//! unzip   images.zip → current/unzipped/（每轮清空重建，可随时再生）
//! decode  内置 KTEX 解码器（ktex-rs，ktech 等价实现）解码 mipmap 0
//! split   xml 布局 × 解码图 → current/split/<atlas>/<sprite>.png
//!         （atlas 解码+裁剪融合为一步，无中间落盘）
//! lone    无同名 xml 的 tex → current/decoded/<base>.png（最终产物）
//!         最终产物入库 history/objects（CAS），manifest 记全量清单 + diff
//! ```
//!
//! 最终产物 = split 切片 + 独立 tex 解码图；中间产物（unzipped）不入历史。
//!
//! 增量与正确性规则：
//! - 输入 hash 与 parent manifest 一致且产物在盘 → 跳过；
//! - **解码器版本变更**（manifest.decoder ≠ [`ktex::DECODER_VERSION`]）或
//!   **切割逻辑版本变更**（manifest.split_version ≠
//!   [`split::SPLIT_VERSION`]）时等效 `--force`：全量重处理，diff 相对旧
//!   基线如实反映；
//! - 失败文件不计入 manifest，partial 不作为下个 diff 基线（避免"因失败
//!   消失"被误判为 removed）；`current/` 恒等于最近一次运行的真实产物；
//! - manifest 的 products 清单**从零构建**（复用条目显式从 parent 搬运），
//!   保证源里消失的 atlas 不会残留；
//! - **build 回退**（当前 build 数值低于任何已记录 build 的 manifest）时
//!   拒绝操作（即使 `--force`），报告 `rollback_skipped`，不写任何文件。
//!
//! 输出布局（`KTOOLS__OUT_DIR`，缺省 `output/ktools`）：
//!
//! ```text
//! current/{unzipped,split,decoded}/   最新 build 工作集（下游消费入口）
//! history/objects/<h[:2]>/<h>.png     内容寻址，仅最终产物
//! history/manifests/<build>.json      全量清单 + 相邻 diff + 输入 hash
//! ```

pub mod history;
pub mod icons;
pub mod ktex;
pub mod scan;
pub mod split;

use crate::error::{Error, Result};
use crate::scripts_sync::images::history::{
    diff_final_maps, FinalMap, Manifest, ManifestStore, ObjectStore,
};
use crate::scripts_sync::images::ktex::{DecodeOptions, DECODER_VERSION};
use crate::scripts_sync::images::scan::{scan, ScanResult, UNZIP_DIR_NAME};
use crate::scripts_sync::images::split::SPLIT_VERSION;
use crate::scripts_sync::state;
use crate::service::Reporter;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// 一次 `images-sync` 运行的参数。
#[derive(Debug, Clone)]
pub struct ImagesSyncParams {
    /// DST 安装根目录（`DST__ROOT`）。
    pub dst_root: String,
    /// 产物根目录（`KTOOLS__OUT_DIR`）；缺省 `output/ktools`。
    pub out_dir: Option<PathBuf>,
    /// 忽略增量与幂等检查，全量重跑。
    pub force: bool,
    /// 只盘点与报告计划，不写任何文件。
    pub dry_run: bool,
}

/// 一次失败的记录（报告与失败清单复用）。
#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    pub stage: &'static str,
    pub item: String,
    pub error: String,
}

/// 产物根目录下的固定子布局。
struct Layout {
    root: PathBuf,
    unzipped: PathBuf,
    split: PathBuf,
    decoded: PathBuf,
    objects: PathBuf,
    manifests: PathBuf,
}

impl Layout {
    fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            unzipped: root.join("current").join(UNZIP_DIR_NAME),
            split: root.join("current/split"),
            decoded: root.join("current/decoded"),
            objects: root.join("history/objects"),
            manifests: root.join("history/manifests"),
            root,
        }
    }
}

/// 执行 images-sync，返回机器可读报告。
///
/// status：`synced`（完整成功）/ `partial`（有失败）/ `up_to_date` / `dry_run`。
pub fn run(params: &ImagesSyncParams, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    let dst = Path::new(&params.dst_root);
    if !dst.exists() {
        return Err(Error::DstDirNotFound(params.dst_root.clone()));
    }
    let bundles = dst.join("data/databundles");
    let images_zip = bundles.join("images.zip");
    if !images_zip.exists() {
        return Err(Error::DstDirNotFound(images_zip.display().to_string()));
    }
    let loose_dir = dst.join("data/images");
    let layout = Layout::new(
        params
            .out_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("output/ktools")),
    );
    let build = crate::scripts_sync::read_new_version(dst)?;
    let manifests = ManifestStore::new(&layout.manifests);
    let parent = manifests.load_parent(&build)?;
    // 当前 build 的既有 manifest（若存在）——幂等判断与解码器切换的依据。
    let existing = manifests.load(&build)?;

    reporter.stage("DST 图片资源同步检查");
    reporter.log(format!(
        "build {} / 基线 {} / 解码器 {} / 输出 {}",
        build,
        parent
            .as_ref()
            .map(|m| m.build.clone())
            .unwrap_or_else(|| "(无)".into()),
        DECODER_VERSION,
        layout.root.display()
    ));

    // build 回退：当前 build 低于任何已记录 build 时拒绝操作，避免用旧
    // 版输入覆盖新版历史（即使 --force 也不放行；dry-run 同样直接返回）。
    if let Some(recorded) = manifests.latest_recorded_build()? {
        if state::is_rollback(&build, &recorded) {
            let msg = format!(
                "检测到 build 回退：当前 {build} 低于已记录的 {recorded}，\
                 为避免污染产物历史，本次不执行任何操作。\
                 请确认 DST__ROOT 指向正确的游戏安装（或删除多余的 manifest 后重试）"
            );
            reporter.log(msg.clone());
            return Ok(serde_json::json!({
                "status": "rollback_skipped",
                "build": build,
                "recorded_build": recorded,
                "message": msg,
                "out_dir": layout.root.display().to_string(),
            }));
        }
    }

    // 解码器版本变更（含当前 build 的旧 manifest）→ 等效 force。
    let versioned = existing.as_ref().or(parent.as_ref());
    let decoder_changed = versioned
        .map(|m| m.decoder != DECODER_VERSION)
        .unwrap_or(false);
    // 切割逻辑版本变更 → 同样等效 force（否则未变的 atlas 会复用旧逻辑
    // 切出的切片，新旧产物混用两套切割规则）。
    let splitter_changed = versioned
        .map(|m| m.split_version != SPLIT_VERSION)
        .unwrap_or(false);
    let effective_force = params.force || decoder_changed || splitter_changed;
    if decoder_changed {
        reporter.log(format!(
            "解码器已从 {:?} 切换到 {DECODER_VERSION}，本次全量重处理",
            versioned.map(|m| m.decoder.as_str()).unwrap_or("")
        ));
    }
    if splitter_changed {
        reporter.log(format!(
            "切割逻辑已从 {:?} 切换到 {SPLIT_VERSION}，本次全量重切割",
            versioned.map(|m| m.split_version.as_str()).unwrap_or("")
        ));
    }

    // 幂等：当前 build 已有完整 manifest（同解码器）且未强制 → 跳过。
    if !effective_force && !params.dry_run {
        if let Some(existing) = existing {
            if existing.complete {
                reporter.log("当前 build 已有完整 manifest，跳过（--force 可强制）".to_string());
                return Ok(up_to_date_report(&build, &layout, &existing));
            }
        }
    }

    reporter.stage("两源盘点");
    let scan = scan(&images_zip, &loose_dir, &layout.unzipped)?;
    reporter.log(format!(
        "atlas {} / 独立 tex {} / 冲突 {} / xml 冲突 {} / 遗留 png {}",
        scan.atlases.len(),
        scan.lone_tex.len(),
        scan.conflicts.len(),
        scan.xml_conflicts.len(),
        scan.stale_png
    ));

    if params.dry_run {
        return dry_run_report(
            params,
            reporter,
            &layout,
            &build,
            parent.as_ref(),
            &scan,
            decoder_changed,
            splitter_changed,
        );
    }

    // -- 真实运行 -----------------------------------------------------------
    let mut failures: Vec<Failure> = Vec::new();
    let parent_inputs = parent
        .as_ref()
        .map(|m| m.inputs.clone())
        .unwrap_or_default();
    let parent_final = parent
        .as_ref()
        .map(|m| m.products.clone())
        .unwrap_or_default();
    // 新 manifest 的 products 清单从零构建；复用条目显式从 parent 搬运。
    let mut products = FinalMap::new();

    // 1. 解压（清空重建，zip 内已删除的条目不会残留）
    reporter.stage("解压 images.zip");
    if layout.unzipped.exists() {
        std::fs::remove_dir_all(&layout.unzipped)?;
    }
    extract_zip(&images_zip, &layout.unzipped)?;
    reporter.log(format!("→ {}", layout.unzipped.display()));

    // 2. 解码 + 切割（融合：atlas 解码直接进内存裁剪，无中间落盘）
    reporter.stage("解码与切割 atlas");
    let objects = ObjectStore::new(&layout.objects);
    std::fs::create_dir_all(&layout.split)?;
    std::fs::create_dir_all(&layout.decoded)?;
    let mut expected_split: BTreeSet<String> = BTreeSet::new();
    let mut expected_decoded: BTreeSet<String> = BTreeSet::new();
    let mut tex_processed = 0u64;
    let mut atlas_processed = 0u64;
    let mut atlas_reused = 0u64;
    let mut missing_tex = 0u64;
    let mut sprites_total = 0u64;
    for atlas in &scan.atlases {
        let dir_name = split::split_dir_name(&atlas.base);
        let split_dir = layout.split.join(&dir_name);
        let Some(tex) = atlas.tex.as_ref() else {
            missing_tex += 1;
            continue;
        };
        let unchanged = !effective_force
            && parent_inputs.get(&format!("xml/{}", atlas.base)) == Some(&atlas.xml_hash)
            && parent_inputs.get(&format!("tex/{}", tex.base)) == Some(&tex.hash)
            && split_dir.exists();
        if unchanged {
            // 复用 parent 中该 atlas 的全部切片。
            let prefix = format!("split/{dir_name}/");
            let mut reused_any = false;
            for path in parent_final.keys() {
                if let Some(rel) = path.strip_prefix(&prefix) {
                    products.insert(path.clone(), parent_final[path].clone());
                    expected_split.insert(rel.to_string());
                    reused_any = true;
                }
            }
            if reused_any {
                atlas_reused += 1;
                continue;
            }
            // 父清单无记录（异常状态）→ 落入重新处理。
        }
        tex_processed += 1;

        // 解码（内置 ktex-rs）。
        let image = match std::fs::read(&tex.path)
            .map_err(|e| {
                Error::Io(std::io::Error::new(
                    e.kind(),
                    format!("读取 {}: {}", tex.path.display(), e),
                ))
            })
            .and_then(|bytes| ktex::decode_mipmap0(&bytes, DecodeOptions::default()))
        {
            Ok(img) => img,
            Err(e) => {
                failures.push(Failure {
                    stage: "decode",
                    item: tex.base.clone(),
                    error: e.to_string(),
                });
                if split_dir.exists() {
                    std::fs::remove_dir_all(&split_dir).ok();
                }
                continue;
            }
        };
        let xml_text = match std::fs::read_to_string(&atlas.xml_path) {
            Ok(t) => t,
            Err(e) => {
                failures.push(Failure {
                    stage: "split",
                    item: atlas.base.clone(),
                    error: format!("读取 xml 失败: {e}"),
                });
                continue;
            }
        };
        let (w, h) = image::GenericImageView::dimensions(&image);
        let regions = match split::parse_atlas_regions(&xml_text, w, h) {
            Ok(r) => r,
            Err(e) => {
                failures.push(Failure {
                    stage: "split",
                    item: atlas.base.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };
        std::fs::create_dir_all(&split_dir)?;
        let mut ok_sprites = 0u64;
        for region in &regions {
            let dest = split_dir.join(&region.name);
            let cropped = split::crop(&image, region);
            if let Err(e) = cropped.save(&dest) {
                failures.push(Failure {
                    stage: "split",
                    item: format!("{}/{}", atlas.base, region.name),
                    error: format!("保存切片失败: {e}"),
                });
                continue;
            }
            match objects.put_file(&dest) {
                Ok(hash) => {
                    products.insert(format!("split/{dir_name}/{}", region.name), hash);
                    expected_split.insert(format!("{dir_name}/{}", region.name));
                    ok_sprites += 1;
                }
                Err(e) => failures.push(Failure {
                    stage: "split",
                    item: format!("{}/{}", atlas.base, region.name),
                    error: format!("入库失败: {e}"),
                }),
            }
        }
        sprites_total += ok_sprites;
        atlas_processed += 1;
    }
    reporter.log(format!(
        "atlas 共 {} / 新切割 {} / 复用 {} / 缺 tex {}/失败 {}",
        scan.atlases.len(),
        atlas_processed,
        atlas_reused,
        missing_tex,
        scan.atlases.len() as u64 - atlas_processed - atlas_reused - missing_tex
    ));

    // 3. 独立 tex 解码 → 最终产物
    let mut lone_processed = 0u64;
    let mut lone_reused = 0u64;
    for tex in &scan.lone_tex {
        let rel = format!("decoded/{}.png", tex.base);
        let dest = layout.decoded.join(format!("{}.png", tex.base));
        let unchanged = !effective_force
            && parent_inputs.get(&format!("tex/{}", tex.base)) == Some(&tex.hash)
            && dest.exists()
            && parent_final.contains_key(&rel);
        if unchanged {
            if let Some(hash) = parent_final.get(&rel) {
                products.insert(rel, hash.clone());
                expected_decoded.insert(format!("{}.png", tex.base));
                lone_reused += 1;
                continue;
            }
        }
        lone_processed += 1;
        let decoded = match std::fs::read(&tex.path)
            .map_err(|e| {
                Error::Io(std::io::Error::new(
                    e.kind(),
                    format!("读取 {}: {}", tex.path.display(), e),
                ))
            })
            .and_then(|bytes| ktex::decode_mipmap0(&bytes, DecodeOptions::default()))
        {
            Ok(img) => img,
            Err(e) => {
                failures.push(Failure {
                    stage: "decode",
                    item: tex.base.clone(),
                    error: e.to_string(),
                });
                if dest.exists() {
                    std::fs::remove_file(&dest).ok();
                }
                continue;
            }
        };
        if let Err(e) = decoded.save(&dest) {
            failures.push(Failure {
                stage: "decode",
                item: tex.base.clone(),
                error: format!("保存解码图失败: {e}"),
            });
            continue;
        }
        match objects.put_file(&dest) {
            Ok(hash) => {
                products.insert(rel, hash);
                expected_decoded.insert(format!("{}.png", tex.base));
            }
            Err(e) => failures.push(Failure {
                stage: "decode",
                item: tex.base.clone(),
                error: format!("入库失败: {e}"),
            }),
        }
    }
    reporter.log(format!(
        "独立 tex 共 {} / 新解码 {} / 复用 {}",
        scan.lone_tex.len(),
        lone_processed,
        lone_reused
    ));

    // 4. 遗留清理 + 对账：current/ 只保留本次保证的产物
    reporter.stage("对账清理");
    let legacy_kteched = layout.root.join("current/kteched");
    if legacy_kteched.is_dir() {
        std::fs::remove_dir_all(&legacy_kteched)?;
        reporter.log("已移除旧版 ktech 中间目录 current/kteched".to_string());
    }
    let removed_split = reconcile_dir(&layout.split, &expected_split)?;
    let removed_decoded = reconcile_dir(&layout.decoded, &expected_decoded)?;
    reporter.log(format!(
        "清理 split {removed_split} / decoded {removed_decoded} 个陈旧文件"
    ));

    // 5. manifest + diff
    reporter.stage("manifest 与差异");
    let diff = diff_final_maps(&parent_final, &products);
    let complete = failures.is_empty();
    let stats: BTreeMap<String, u64> = [
        ("tex_total", scan.all_tex.len() as u64),
        ("tex_processed", tex_processed + lone_processed),
        ("tex_reused", atlas_reused + lone_reused),
        ("atlas_total", scan.atlases.len() as u64),
        ("atlas_processed", atlas_processed),
        ("atlas_reused", atlas_reused),
        ("atlas_missing_tex", missing_tex),
        ("sprites_new", sprites_total),
        ("lone_processed", lone_processed),
        ("lone_reused", lone_reused),
        ("added", diff.added.len() as u64),
        ("removed", diff.removed.len() as u64),
        ("changed", diff.changed.len() as u64),
        ("conflicts", scan.conflicts.len() as u64),
        ("xml_conflicts", scan.xml_conflicts.len() as u64),
        ("stale_png", scan.stale_png),
        ("zip_tex_hits", scan.zip_tex_hits),
        ("removed_stale_split", removed_split as u64),
        ("removed_stale_decoded", removed_decoded as u64),
        ("decoder_switched", u64::from(decoder_changed)),
        ("splitter_switched", u64::from(splitter_changed)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let manifest = Manifest {
        build: build.clone(),
        parent_build: parent.as_ref().map(|m| m.build.clone()),
        complete,
        decoder: DECODER_VERSION.to_string(),
        split_version: SPLIT_VERSION.to_string(),
        products,
        diff,
        inputs: scan.input_hashes(),
        stats: stats.clone(),
        synced_at: Some(history::now_ms()),
    };
    let manifest_path = manifests.save(&manifest)?;
    reporter.log(format!(
        "最终产物 {} / diff +{} -{} ~{} / manifest → {}",
        manifest.products.len(),
        manifest.diff.added.len(),
        manifest.diff.removed.len(),
        manifest.diff.changed.len(),
        manifest_path.display()
    ));

    Ok(report_json(
        if complete { "synced" } else { "partial" },
        &build,
        parent.as_ref().map(|m| m.build.as_str()),
        &layout,
        &scan,
        &manifest,
        Some(&failures),
        decoder_changed,
        splitter_changed,
    ))
}

// -- helpers ----------------------------------------------------------------

/// 解压 zip 到目标目录（目录已创建）。
fn extract_zip(zip_path: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
    Ok(archive.extract(target)?)
}

/// 目录对账：删除 `keep`（相对路径，正斜杠）之外的文件，随后移除空目录。
fn reconcile_dir(root: &Path, keep: &BTreeSet<String>) -> Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let mut removed = 0usize;
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path.clone());
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !keep.contains(&rel) {
                    std::fs::remove_file(&path)?;
                    removed += 1;
                }
            }
        }
    }
    // 自底向上移除空目录（深的先删）。
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for dir in dirs {
        if std::fs::read_dir(&dir)?.next().is_none() {
            std::fs::remove_dir(&dir).ok();
        }
    }
    Ok(removed)
}

/// dry-run：盘点 + 依据 parent manifest 估算各阶段工作量，不写任何文件。
#[allow(clippy::too_many_arguments)]
fn dry_run_report(
    params: &ImagesSyncParams,
    reporter: &dyn Reporter,
    layout: &Layout,
    build: &str,
    parent: Option<&Manifest>,
    scan: &ScanResult,
    decoder_changed: bool,
    splitter_changed: bool,
) -> Result<serde_json::Value> {
    reporter.stage("dry-run 计划（不修改任何文件）");
    if decoder_changed {
        reporter.log(format!("解码器将切换到 {DECODER_VERSION}，全量重处理"));
    }
    if splitter_changed {
        reporter.log(format!("切割逻辑将切换到 {SPLIT_VERSION}，全量重切割"));
    }
    let effective_force = params.force || decoder_changed || splitter_changed;
    let inputs = parent.map(|m| &m.inputs);
    let tex_to_decode = scan
        .all_tex
        .iter()
        .filter(|t| {
            effective_force
                || inputs.and_then(|i| i.get(&format!("tex/{}", t.base))) != Some(&t.hash)
        })
        .count() as u64;
    let atlas_to_split = scan
        .atlases
        .iter()
        .filter(|a| {
            if a.tex.is_none() {
                return false; // 缺 tex 的直接跳过，不计入计划
            }
            let dir = layout.split.join(split::split_dir_name(&a.base));
            let tex_ok = a.tex.as_ref().is_some_and(|t| {
                inputs.and_then(|i| i.get(&format!("tex/{}", t.base))) == Some(&t.hash)
            });
            let xml_ok =
                inputs.and_then(|i| i.get(&format!("xml/{}", a.base))) == Some(&a.xml_hash);
            effective_force || !(xml_ok && tex_ok && dir.exists())
        })
        .count() as u64;
    let missing_tex = scan.atlases.iter().filter(|a| a.tex.is_none()).count() as u64;
    reporter.log(format!(
        "计划：解码 {tex_to_decode} 个 tex / 切割 {atlas_to_split} 个 atlas / 缺 tex 跳过 {missing_tex}"
    ));
    Ok(serde_json::json!({
        "status": "dry_run",
        "build": build,
        "parent_build": parent.map(|m| m.build.as_str()),
        "decoder": DECODER_VERSION,
        "decoder_switched": decoder_changed,
        "splitter": SPLIT_VERSION,
        "splitter_switched": splitter_changed,
        "out_dir": layout.root.display().to_string(),
        "scan": {
            "atlases": scan.atlases.len(),
            "lone_tex": scan.lone_tex.len(),
            "all_tex": scan.all_tex.len(),
            "conflicts": scan.conflicts.len(),
            "xml_conflicts": scan.xml_conflicts.len(),
            "stale_png": scan.stale_png,
            "zip_tex_hits": scan.zip_tex_hits,
            "missing_tex": missing_tex,
        },
        "plan": {
            "tex_to_decode": tex_to_decode,
            "atlas_to_split": atlas_to_split,
            "missing_tex_skipped": missing_tex,
        },
    }))
}

fn up_to_date_report(build: &str, layout: &Layout, manifest: &Manifest) -> serde_json::Value {
    serde_json::json!({
        "status": "up_to_date",
        "build": build,
        "decoder": DECODER_VERSION,
        "splitter": SPLIT_VERSION,
        "out_dir": layout.root.display().to_string(),
        "final_products": manifest.products.len(),
        "diff_summary": {
            "added": manifest.diff.added.len(),
            "removed": manifest.diff.removed.len(),
            "changed": manifest.diff.changed.len(),
        },
        "stats": manifest.stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn report_json(
    status: &str,
    build: &str,
    parent_build: Option<&str>,
    layout: &Layout,
    scan: &ScanResult,
    manifest: &Manifest,
    failures: Option<&[Failure]>,
    decoder_changed: bool,
    splitter_changed: bool,
) -> serde_json::Value {
    serde_json::json!({
        "status": status,
        "build": build,
        "parent_build": parent_build,
        "decoder": DECODER_VERSION,
        "decoder_switched": decoder_changed,
        "splitter": SPLIT_VERSION,
        "splitter_switched": splitter_changed,
        "out_dir": layout.root.display().to_string(),
        "scan": {
            "atlases": scan.atlases.len(),
            "lone_tex": scan.lone_tex.len(),
            "all_tex": scan.all_tex.len(),
            "conflicts": scan.conflicts.len(),
            "xml_conflicts": scan.xml_conflicts.len(),
            "stale_png": scan.stale_png,
            "zip_tex_hits": scan.zip_tex_hits,
        },
        "final_products": manifest.products.len(),
        "diff": {
            "added": manifest.diff.added.len(),
            "removed": manifest.diff.removed.len(),
            "changed": manifest.diff.changed.len(),
        },
        "stats": manifest.stats,
        "failures": failures.unwrap_or(&[]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts_sync::images::ktex::{KtexHeader, MipmapMeta};
    use crate::service::Reporter;
    use image::Rgba;
    use std::sync::Mutex;

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

    fn write_file(path: &Path, content: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// 构造最小 images.zip。
    fn make_zip(zip_path: &Path, files: &[(&str, &[u8])]) {
        std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        use std::io::Write as _;
        for (name, content) in files {
            zw.start_file(
                format!("images/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(content).unwrap();
        }
        zw.finish().unwrap();
    }

    /// 纯红 4x4（565 端点可精确表示 → DXT5 解码逐位还原）。
    fn solid_red() -> image::RgbaImage {
        image::RgbaImage::from_fn(4, 4, |_, _| Rgba([255, 0, 0, 255]))
    }

    /// 任意 RGBA 图 → DXT5 KTEX 容器字节。
    fn make_tex(img: &image::RgbaImage) -> Vec<u8> {
        use texpresso::Format;
        let (w, h) = img.dimensions();
        let mut comp = vec![0u8; Format::Bc3.compressed_size(w as usize, h as usize)];
        Format::Bc3.compress(
            img.as_raw(),
            w as usize,
            h as usize,
            texpresso::Params::default(),
            &mut comp,
        );
        ktex::build_tex(
            KtexHeader {
                platform: 12,
                compression: ktex::Compression::Dxt5,
                texture_type: 1,
                mipmap_count: 1,
                flags: 0,
            },
            &[MipmapMeta {
                width: w,
                height: h,
                pitch: 16 * w.div_ceil(4),
                datasz: comp.len() as u32,
            }],
            &[comp],
        )
    }

    /// RGBA 图 → 未压缩 RGB KTEX（检验 raw 路径）。
    fn make_rgb_tex(img: &image::RgbaImage) -> Vec<u8> {
        let (w, h) = img.dimensions();
        let mut data = Vec::with_capacity(3 * w as usize * h as usize);
        for px in img.pixels() {
            data.extend_from_slice(&px.0[..3]);
        }
        ktex::build_tex(
            KtexHeader {
                platform: 12,
                compression: ktex::Compression::Rgb,
                texture_type: 1,
                mipmap_count: 1,
                flags: 0,
            },
            &[MipmapMeta {
                width: w,
                height: h,
                pitch: 3 * w,
                datasz: data.len() as u32,
            }],
            &[data],
        )
    }

    const ATLAS_XML: &str = r#"<?xml version="1.0"?>
    <Atlas><Elements>
        <Element name="s1.tex" u1="0" v1="0" u2="0.5" v2="0.5"/>
        <Element name="s2.tex" u1="0.5" v1="0.5" u2="1" v2="1"/>
    </Elements></Atlas>"#;

    /// 搭建完整假 DST：zip 源（foo atlas）+ loose 源（bar atlas + baz 独立 RGB tex）。
    fn make_dst(ws: &Path) -> PathBuf {
        let dst = ws.join("dst");
        let loose = dst.join("data/images");
        let bundles = dst.join("data/databundles");
        write_file(&dst.join("version.txt"), b"100\n");
        make_zip(
            &bundles.join("images.zip"),
            &[
                ("foo.tex", &make_tex(&solid_red())),
                ("foo.xml", ATLAS_XML.as_bytes()),
            ],
        );
        write_file(&loose.join("bar.tex"), &make_tex(&solid_red()));
        write_file(&loose.join("bar.xml"), ATLAS_XML.as_bytes());
        // baz：无 xml 的独立 tex，用 RGB 未压缩路径 + 可变内容
        write_file(&loose.join("baz.tex"), &make_rgb_tex(&solid_red()));
        write_file(&loose.join("stale.png"), b"stale");
        dst
    }

    fn params(dst: &Path, out: &Path, force: bool, dry_run: bool) -> ImagesSyncParams {
        ImagesSyncParams {
            dst_root: dst.display().to_string(),
            out_dir: Some(out.to_path_buf()),
            force,
            dry_run,
        }
    }

    #[test]
    fn test_run_full_pipeline_and_manifest() {
        let ws = std::env::temp_dir().join(format!("ktool_run_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        std::fs::create_dir_all(&ws).unwrap();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();

        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        assert_eq!(report["build"], "100");
        assert_eq!(report["decoder"], DECODER_VERSION);

        // 切片落盘：两个 atlas 各 2 片；独立图 decoded/baz.png 存在
        assert!(out.join("current/split/foo/s1.png").exists());
        assert!(out.join("current/split/foo/s2.png").exists());
        assert!(out.join("current/split/bar/s1.png").exists());
        assert!(out.join("current/decoded/baz.png").exists());
        // manifest: final = 4 切片 + 1 独立图；diff（首次）全 added
        let manifest: Manifest = serde_json::from_str(
            &std::fs::read_to_string(out.join("history/manifests/100.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.products.len(), 5);
        assert_eq!(manifest.diff.added.len(), 5);
        assert!(manifest.complete);
        assert_eq!(manifest.decoder, DECODER_VERSION);
        assert!(manifest
            .diff
            .added
            .contains(&"split/foo/s1.png".to_string()));
        assert!(manifest.products.contains_key("decoded/baz.png"));
        // CAS 对象存在且 products hash 可解析
        let objects = ObjectStore::new(out.join("history/objects"));
        for hash in manifest.products.values() {
            assert!(objects.contains(hash), "{hash}");
        }
        // 遗留 png 不进入产物目录
        assert!(!out.join("current/split/stale.png").exists());
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_run_up_to_date_then_incremental() {
        let ws = std::env::temp_dir().join(format!("ktool_run2_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        std::fs::create_dir_all(&ws).unwrap();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();
        run(&params(&dst, &out, false, false), &rep).unwrap();

        // 再跑一次同 build → up_to_date
        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "up_to_date");

        // 模拟新 build：改 version + 更新 baz.tex（独立图）+ 新增 atlas qux
        write_file(&dst.join("version.txt"), b"200\n");
        let mut baz2 = solid_red();
        for px in baz2.pixels_mut() {
            *px = Rgba([0, 255, 0, 255]); // 纯绿（565 精确可表示）
        }
        write_file(&dst.join("data/images/baz.tex"), &make_rgb_tex(&baz2));
        make_zip(
            &dst.join("data/databundles/images.zip"),
            &[
                ("foo.tex", &make_tex(&solid_red())),
                ("foo.xml", ATLAS_XML.as_bytes()),
                ("qux.tex", &make_tex(&solid_red())),
                ("qux.xml", ATLAS_XML.as_bytes()),
            ],
        );

        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        let manifest: Manifest = serde_json::from_str(
            &std::fs::read_to_string(out.join("history/manifests/200.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.parent_build.as_deref(), Some("100"));
        // diff：qux 两片 added + baz 变更 changed；foo、bar 复用
        assert_eq!(manifest.diff.added.len(), 2, "{:?}", manifest.diff);
        assert_eq!(manifest.diff.changed.len(), 1);
        assert_eq!(manifest.diff.changed[0].path, "decoded/baz.png");
        // 旧 manifest 与对象仍可访问（历史不丢）
        assert!(out.join("history/manifests/100.json").exists());
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_run_dry_run_touches_nothing() {
        let ws = std::env::temp_dir().join(format!("ktool_run3_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        std::fs::create_dir_all(&ws).unwrap();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();

        let report = run(&params(&dst, &out, false, true), &rep).unwrap();
        assert_eq!(report["status"], "dry_run", "{report}");
        assert_eq!(report["plan"]["tex_to_decode"], 3); // foo/bar/baz 全新
        assert_eq!(report["plan"]["atlas_to_split"], 2);
        assert!(!out.exists()); // 完全无写入
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_run_decode_failure_marks_partial_and_next_run_recovers() {
        let ws = std::env::temp_dir().join(format!("ktool_run4_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        std::fs::create_dir_all(&ws).unwrap();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();

        // baz.tex 损坏（magic 不符）→ decode 失败 → partial
        write_file(&dst.join("data/images/baz.tex"), b"NOT A TEX");
        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "partial", "{report}");
        assert_eq!(report["failures"].as_array().unwrap().len(), 1);

        // 修复 baz.tex 重跑同 build → 相对基线完整恢复，无 removed
        write_file(
            &dst.join("data/images/baz.tex"),
            &make_rgb_tex(&solid_red()),
        );
        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        let manifest: Manifest = serde_json::from_str(
            &std::fs::read_to_string(out.join("history/manifests/100.json")).unwrap(),
        )
        .unwrap();
        assert!(manifest.complete);
        // partial 不作 diff 基线：修复后不应出现 removed
        assert_eq!(manifest.diff.removed.len(), 0, "{:?}", manifest.diff);
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_run_old_decoder_manifest_triggers_reprocess() {
        let ws = std::env::temp_dir().join(format!("ktool_run5_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        std::fs::create_dir_all(&ws).unwrap();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();
        run(&params(&dst, &out, false, false), &rep).unwrap();

        // 伪造旧解码器 manifest（decoder 字段被改掉）→ 不应 up_to_date
        let manifest_path = out.join("history/manifests/100.json");
        let mut m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        m["decoder"] = serde_json::json!("old-ktech/0");
        std::fs::write(&manifest_path, serde_json::to_string(&m).unwrap()).unwrap();

        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        assert_eq!(report["decoder_switched"], true);
        // manifest 已被新解码器覆盖
        let m: Manifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(m.decoder, DECODER_VERSION);
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_run_old_split_version_manifest_triggers_reprocess() {
        let ws = std::env::temp_dir().join(format!("ktool_run6_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();
        run(&params(&dst, &out, false, false), &rep).unwrap();

        // 伪造旧切割逻辑 manifest（split_version 字段被改掉）→ 不应 up_to_date
        let manifest_path = out.join("history/manifests/100.json");
        let mut m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        m["split_version"] = serde_json::json!("old-split/0");
        std::fs::write(&manifest_path, serde_json::to_string(&m).unwrap()).unwrap();

        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "synced", "{report}");
        assert_eq!(report["splitter_switched"], true);
        // manifest 已记录当前切割逻辑版本
        let m: Manifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(m.split_version, SPLIT_VERSION);
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_run_rollback_refuses_even_with_force() {
        let ws = std::env::temp_dir().join(format!("ktool_run7_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dst = make_dst(&ws);
        let out = ws.join("out");
        let rep = TestReporter::default();
        run(&params(&dst, &out, false, false), &rep).unwrap();

        // 模拟 build 回退：version.txt 降到 50
        write_file(&dst.join("version.txt"), b"50\n");
        for (force, dry_run) in [(false, false), (true, false), (false, true)] {
            let report = run(&params(&dst, &out, force, dry_run), &rep).unwrap();
            assert_eq!(report["status"], "rollback_skipped", "{report}");
            assert_eq!(report["recorded_build"], "100");
        }
        // 磁盘无任何写入：无 50.json，current/ 仍是 build 100 产物
        assert!(!out.join("history/manifests/50.json").exists());
        assert!(out.join("current/decoded/baz.png").exists());
        assert!(out.join("current/split/foo/s1.png").exists());
        // 恢复 build 100 → 幂等 up_to_date（回退尝试未留下脏数据）
        write_file(&dst.join("version.txt"), b"100\n");
        let report = run(&params(&dst, &out, false, false), &rep).unwrap();
        assert_eq!(report["status"], "up_to_date");
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_reconcile_removes_stale_files() {
        let ws = std::env::temp_dir().join(format!("ktool_rec_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dir = ws.join("split/foo");
        write_file(&dir.join("keep.png"), b"k");
        write_file(&dir.join("stale.png"), b"s");
        write_file(&ws.join("split/empty_dir_nested/x.png"), b"x");

        let mut keep = BTreeSet::new();
        keep.insert("foo/keep.png".to_string());
        // empty_dir_nested 整目录不在 keep → 文件被清
        let removed = reconcile_dir(&ws.join("split"), &keep).unwrap();
        assert_eq!(removed, 2);
        assert!(dir.join("keep.png").exists());
        assert!(!dir.join("stale.png").exists());
        assert!(!ws.join("split/empty_dir_nested").exists()); // 空目录被移除
        std::fs::remove_dir_all(&ws).ok();
    }
}
