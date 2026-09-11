//! `upload-icons`：图标上传（单个 / 按 build / 按来源批量 / 手动指定文件名）。
//!
//! 数据来源：
//! - 文件与名称：`history/icon_meta.json`（images-sync 生成的生效标题 +
//!   上传状态）与 `current/split/<source.dir>/<file>`（当前图标内容）；
//! - 生效标题：`config/icon_title_overrides.json` 映射表（按本地文件名）
//!   优先，其次 `STRINGS.NAMES` 英文名自动生成（MediaWiki 归一化）；
//! - 手动上传：`file` + `title` 同时给出时绕过映射/自动标题，直接把输入
//!   （去 `File:` 前缀、补 `.png`、MediaWiki 归一化）写入映射表并上传；
//! - 选择：`file` 单个 > `build`（首次加入该 build 的图标）> `source`
//!   （物品栏 / 制作栏）> 全部可上传；批量默认仅上传 wiki 上不存在的
//!   （`only_missing`），显式指定的单个文件始终上传（是否覆盖自动按标题
//!   存在性决定）；
//! - 同名标题去重：多个图标命中同一标题时只上传第一个，其余记录在
//!   `title_conflicts`。
//!
//! 本模块同时负责 images-sync 之后的元数据刷新（[`refresh_icon_meta`]）：
//! 名称总是重建并应用映射表；wiki 状态只查询缺状态或强制重查的条目，凭据
//! 缺失时保留旧状态并报告 `wiki_error`。

use super::progress::Reporter;
use super::{decide_write, WriteDecision, WriteMode};
use crate::error::{Error, Result};
use crate::scripts_sync::images::history::now_ms;
use crate::scripts_sync::images::icons::{
    build_icons_index, default_source, icon_source, IconSource, ICON_SOURCES,
};
use crate::scripts_sync::images::meta::{self as icon_meta, IconMeta};
use crate::wiki::WikiClient;
use serde_json::{json, Value};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 物品栏图标的固定描述分类。
pub const ICON_CATEGORY: &str = "[[分类:物品栏图标]]";
/// 制作栏图标的固定描述分类。
pub const CRAFTING_ICON_CATEGORY: &str = "[[分类:制作栏图标]]";
/// 未显式指定时的上传注释（物品栏图标）。
pub const UPLOAD_COMMENT: &str = "物品栏图标同步（images-sync）";
/// 未显式指定时的上传注释（制作栏图标）。
pub const CRAFTING_UPLOAD_COMMENT: &str = "制作栏图标同步（images-sync）";

/// 来源对应的描述分类。
pub fn icon_category(source_id: &str) -> &'static str {
    match source_id {
        "crafting" => CRAFTING_ICON_CATEGORY,
        _ => ICON_CATEGORY,
    }
}

/// 来源对应的默认上传注释。
pub fn upload_comment(source_id: &str) -> &'static str {
    match source_id {
        "crafting" => CRAFTING_UPLOAD_COMMENT,
        _ => UPLOAD_COMMENT,
    }
}

/// `KTOOLS__OUT_DIR`（缺省 `output/ktools`）。
pub fn out_dir_from_env() -> PathBuf {
    std::env::var("KTOOLS__OUT_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/ktools"))
}

/// 一次 [`run_upload_icons`] 调用的参数。
#[derive(Debug, Clone, Default)]
pub struct UploadIconsParams {
    /// 只上传首次加入该 build 的图标（`file` 优先）。
    pub build: Option<String>,
    /// 只上传单个图标文件名（如 `axe.png`）。
    pub file: Option<String>,
    /// 手动指定上传的 wiki 文件名（需配合 `file`）；写入映射表并绕过自动标题。
    pub title: Option<String>,
    /// 只上传指定来源（`inventory` / `crafting`）；`None` = 全部来源。
    pub source: Option<String>,
    /// 批量时仅上传 wiki 上尚不存在的；对单个 `file` 不生效。
    pub only_missing: bool,
    /// 同名重传发送 `ignorewarnings=1`。
    pub ignore_warnings: bool,
    /// 上传注释。
    pub comment: Option<String>,
}

/// images-sync 之后刷新 `history/icon_meta.json`。
///
/// - 名称表来自 `dst_root` 下游戏翻译表（live 树或 `scripts.zip`）；
/// - `requery_all = true` 时重新查询全部可上传条目的 wiki 状态，否则只查询
///   尚无状态记录的条目（`check_wiki = false` 时完全不查）。
pub async fn refresh_icon_meta(
    dst_root: &Path,
    out_dir: &Path,
    requery_all: bool,
    check_wiki: bool,
    reporter: &dyn Reporter,
) -> Result<Value> {
    let index = build_icons_index(&out_dir.join("history/manifests"))?;
    let Some(build) = index.latest_build.clone() else {
        reporter.log("暂无完整 manifest，跳过图标元数据刷新".to_string());
        return Ok(json!({ "status": "no_manifest" }));
    };
    let files: Vec<(String, String)> = index
        .entries
        .iter()
        .map(|e| (e.file.clone(), e.source.clone()))
        .collect();
    let names = icon_meta::read_name_maps(dst_root)?;
    let overrides = icon_meta::load_overrides(&icon_meta::overrides_path())?;
    if names.is_empty() && overrides.is_empty() {
        reporter.log("翻译表中没有 STRINGS.NAMES 条目，跳过图标元数据刷新".to_string());
        return Ok(json!({ "status": "no_names" }));
    }

    let previous = icon_meta::load(out_dir)?;
    let mut meta = icon_meta::build_meta(&build, files, &names, &overrides, &previous);
    let entries = meta.icons.len();
    let named = meta.icons.values().filter(|e| e.name_en.is_some()).count();
    let uploadable = meta.icons.values().filter(|e| e.uploadable).count();

    let mut wiki_checked = false;
    let mut wiki_error = None;
    if check_wiki {
        let needs: Vec<String> = meta
            .icons
            .iter()
            .filter(|(_, e)| e.uploadable && (requery_all || e.wiki.is_none()))
            .map(|(f, _)| f.clone())
            .collect();
        if !needs.is_empty() {
            reporter.log(format!("查询维基上传状态：{} 个图标…", needs.len()));
            match query_wiki_status(&mut meta, &needs).await {
                Ok(n) => {
                    wiki_checked = true;
                    reporter.log(format!("维基上传状态已更新 {n} 个"));
                }
                Err(e) => {
                    reporter.log(format!("维基上传状态查询失败，保留旧状态：{e}"));
                    wiki_error = Some(e.to_string());
                }
            }
        } else {
            wiki_checked = true;
        }
    }

    let path = icon_meta::save(out_dir, &meta)?;
    let missing = meta
        .icons
        .values()
        .filter(|e| e.uploadable && e.wiki.as_ref().and_then(|w| w.exists) == Some(false))
        .count();
    let unknown = meta
        .icons
        .values()
        .filter(|e| e.uploadable && e.wiki.is_none())
        .count();
    let unuploadable = meta
        .icons
        .values()
        .filter(|e| e.name_en.is_some() && !e.uploadable)
        .count();
    let mut by_source: BTreeMap<String, usize> = BTreeMap::new();
    for entry in meta.icons.values() {
        let source = entry.source.as_deref().unwrap_or("inventory");
        *by_source.entry(source.to_string()).or_default() += 1;
    }
    reporter.log(format!(
        "图标元数据：条目 {entries}（命名 {named} / 可上传 {uploadable}）/ 无法上传 {unuploadable} / 缺失 {missing} / 未查 {unknown}，映射表 {} → {}",
        overrides.len(),
        path.display()
    ));
    Ok(json!({
        "status": "ok",
        "build": meta.build,
        "entries": entries,
        "named": named,
        "overrides": overrides.len(),
        "uploadable": uploadable,
        "placeholder": unuploadable,
        "unuploadable": unuploadable,
        "missing": missing,
        "unknown": unknown,
        "by_source": by_source,
        "wiki_checked": wiki_checked,
        "wiki_error": wiki_error,
        "path": path.display().to_string(),
    }))
}

/// 查询 `File:` 页面存在性并写回元数据；返回更新的文件数。
///
/// 按生效标题（`entry.title`）去重后按 [`crate::wiki::TITLES_PER_QUERY`]
/// 批量查询，结果应用到所有命中该标题的文件条目。
pub async fn query_wiki_status(meta: &mut IconMeta, files: &[String]) -> Result<usize> {
    let mut targets: Vec<(String, Vec<String>)> = Vec::new();
    let mut by_title: BTreeMap<String, usize> = BTreeMap::new();
    for file in files {
        let Some(entry) = meta.icons.get(file) else {
            continue;
        };
        let Some(title) = entry.title.clone() else {
            continue;
        };
        match by_title.get(&title) {
            Some(&i) => targets[i].1.push(file.clone()),
            None => {
                by_title.insert(title.clone(), targets.len());
                targets.push((title, vec![file.clone()]));
            }
        }
    }
    if targets.is_empty() {
        return Ok(0);
    }

    let client = WikiClient::from_env()?;
    let titles: Vec<&str> = targets.iter().map(|(t, _)| t.as_str()).collect();
    let infos = client.get_files_info(&titles).await?;

    let mut checked = 0usize;
    for ((_, files), info) in targets.iter().zip(infos.iter()) {
        for file in files {
            if let Some(entry) = meta.icons.get_mut(file) {
                entry.wiki = Some(icon_meta::WikiStatus {
                    exists: Some(!info.missing),
                    checked_at: Some(now_ms()),
                    title: Some(info.title.clone()),
                    url: info.url.clone(),
                });
                checked += 1;
            }
        }
    }
    Ok(checked)
}

/// 选择待上传文件（纯函数，按文件名升序）。
///
/// `file` 优先于 `build`，`source` 在最外层过滤；都为 `None` 时返回全部
/// 可上传图标。`only_missing` 的过滤发生在状态查询之后（见
/// [`run_upload_icons`]）。手动标题路径由 [`run_upload_icons`] 单独处理，
/// 不走本函数。
pub fn select_files(
    meta: &IconMeta,
    file: Option<&str>,
    build: Option<&str>,
    first_build: Option<&BTreeMap<String, String>>,
    source: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(file) = file {
        let Some(entry) = meta.icons.get(file) else {
            return Err(Error::Config(format!(
                "图标元数据中没有 {file}（可能没有 STRINGS.NAMES 英文名，或未运行 images-sync）"
            )));
        };
        if !entry.uploadable {
            return Err(Error::Config(format!(
                "{file} 无法自动生成标题：{}（可在弹窗/--title 手动指定 wiki 文件名）",
                entry.note.as_deref().unwrap_or("名称不可用")
            )));
        }
        return Ok(vec![file.to_string()]);
    }

    if let Some(build) = build {
        let Some(map) = first_build else {
            return Err(Error::Config(
                "缺少图标历史索引，无法按 build 选择".to_string(),
            ));
        };
        let out: Vec<String> = meta
            .icons
            .iter()
            .filter(|(f, e)| {
                e.uploadable
                    && map.get(*f).map(String::as_str) == Some(build)
                    && source_matches(e, source)
            })
            .map(|(f, _)| f.clone())
            .collect();
        return Ok(out);
    }

    Ok(meta
        .icons
        .iter()
        .filter(|(_, e)| e.uploadable && source_matches(e, source))
        .map(|(f, _)| f.clone())
        .collect())
}

/// 来源过滤：`None` 全部通过；旧元数据无 `source` 视为物品栏图标。
fn source_matches(entry: &icon_meta::IconMetaEntry, source: Option<&str>) -> bool {
    match source {
        None => true,
        Some(want) => entry.source.as_deref().unwrap_or("inventory") == want,
    }
}

/// 同名标题去重：保留第一个文件，其余记入 `conflicts`。
fn dedupe_by_title(meta: &IconMeta, files: Vec<String>) -> (Vec<String>, Vec<Value>) {
    let mut kept: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::new();
    let mut conflicts = Vec::new();
    for file in files {
        let Some(entry) = meta.icons.get(&file) else {
            continue;
        };
        let Some(title) = entry.title.clone() else {
            continue;
        };
        match kept.entry(title.clone()) {
            Entry::Vacant(v) => {
                v.insert(file.clone());
                out.push(file);
            }
            Entry::Occupied(o) => conflicts.push(json!({
                "file": file,
                "title": title,
                "kept": o.get(),
            })),
        }
    }
    (out, conflicts)
}

/// 校验手动上传的本地图标文件：裸文件名 + 当前 split 产物中存在。
/// 返回命中的来源（用于定位上传目录与描述分类）。
fn validate_icon_file(out_dir: &Path, file: &str) -> Result<&'static IconSource> {
    if !icon_meta::is_icon_file_name(file) {
        return Err(Error::Config(format!("无效的图标文件名：{file}")));
    }
    for source in ICON_SOURCES {
        let path = out_dir.join("current/split").join(source.dir).join(file);
        if path.exists() {
            return Ok(source);
        }
    }
    let dirs: Vec<&str> = ICON_SOURCES.iter().map(|s| s.dir).collect();
    Err(Error::Config(format!(
        "当前 images-sync 产物中没有 {file}（已查 {}），请先运行 images-sync",
        dirs.join(" / ")
    )))
}

/// 把手动标题应用到内存中的条目（不存在则新建）。
fn apply_manual_title(
    meta: &mut IconMeta,
    overrides: &icon_meta::IconTitleOverrides,
    file: &str,
    title: &str,
) {
    let entry = meta
        .icons
        .entry(file.to_string())
        .or_insert_with(|| icon_meta::IconMetaEntry {
            name_en: None,
            name_zh: None,
            source: None,
            title: None,
            title_source: None,
            uploadable: false,
            note: None,
            wiki: None,
        });
    icon_meta::refresh_entry_title(entry, file, overrides);
    debug_assert_eq!(entry.title.as_deref(), Some(title));
}

/// 执行图标上传作业。
pub async fn run_upload_icons(
    params: &UploadIconsParams,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<Value> {
    let out_dir = out_dir_from_env();
    reporter.stage("图标上传");

    let mut meta = icon_meta::load(&out_dir)?;
    if meta.icons.is_empty() {
        return Err(Error::Config(format!(
            "尚无图标元数据（{}），请先运行 images-sync",
            icon_meta::meta_path(&out_dir).display()
        )));
    }

    // 手动标题路径：规范化 + 与 file 搭配校验；映射表在真正写入时落盘。
    let manual_title = params
        .title
        .as_deref()
        .map(icon_meta::normalize_explicit_title)
        .transpose()?;
    let (manual_pending, manual_source) = if let Some(title) = manual_title.clone() {
        let Some(file) = params.file.clone() else {
            return Err(Error::Config(
                "手动标题需要同时指定 file（WebUI 弹窗或 --file + --title）".to_string(),
            ));
        };
        if params.build.is_some() {
            return Err(Error::Config("title 与 build 不能同时指定".to_string()));
        }
        let source = validate_icon_file(&out_dir, &file)?;
        ((file, title), Some(source))
    } else {
        ((String::new(), String::new()), None)
    };

    let first_build = if params.file.is_none() && params.build.is_some() {
        let index = build_icons_index(&out_dir.join("history/manifests"))?;
        Some(
            index
                .entries
                .iter()
                .map(|e| (e.file.clone(), e.first_build.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
    } else {
        None
    };
    let mut overrides = icon_meta::load_overrides(&icon_meta::overrides_path())?;
    let mut selected = if let Some(title) = manual_title.as_deref() {
        let file = manual_pending.0.clone();
        // 仅映射表中不存在同名映射时写入（Apply 阶段落盘）。
        overrides.insert(&file, title);
        apply_manual_title(&mut meta, &overrides, &file, title);
        if let Some(source) = manual_source {
            if let Some(entry) = meta.icons.get_mut(&file) {
                entry.source = Some(source.id.to_string());
            }
        }
        vec![file]
    } else {
        select_files(
            &meta,
            params.file.as_deref(),
            params.build.as_deref(),
            first_build.as_ref(),
            params.source.as_deref(),
        )?
    };
    if selected.is_empty() {
        reporter.log("没有符合条件的图标".to_string());
        return Ok(json!({ "status": "nothing_to_upload", "total": 0 }));
    }
    reporter.log(format!("选中 {} 个图标", selected.len()));

    if params.file.is_some() {
        // 单个文件：刷新其状态仅供展示/日志，上传与否不受 only_missing 影响。
        if let Err(e) = query_wiki_status(&mut meta, &selected).await {
            reporter.log(format!("维基状态查询失败（不影响上传）：{e}"));
        }
        // 手动标题在 Apply 阶段随映射表一起落盘（dry-run 不写本地配置）。
        if manual_title.is_none() {
            icon_meta::save(&out_dir, &meta)?;
        }
    } else if params.only_missing {
        let unknown: Vec<String> = selected
            .iter()
            .filter(|f| meta.icons.get(*f).map(|e| e.wiki.is_none()).unwrap_or(true))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            reporter.log(format!("维基状态未查询的 {} 个，先查询…", unknown.len()));
            query_wiki_status(&mut meta, &unknown).await?;
            icon_meta::save(&out_dir, &meta)?;
        }
        selected.retain(|f| {
            meta.icons
                .get(f)
                .and_then(|e| e.wiki.as_ref())
                .and_then(|w| w.exists)
                == Some(false)
        });
        if selected.is_empty() {
            reporter.log("所选图标在维基上均已存在，无需上传".to_string());
            return Ok(json!({ "status": "nothing_missing", "total": 0 }));
        }
        reporter.log(format!("其中维基缺失 {} 个", selected.len()));
    }

    let (selected, conflicts) = dedupe_by_title(&meta, selected);
    if !conflicts.is_empty() {
        reporter.log(format!(
            "同名标题去重：{} 个文件与先到者共用标题，跳过",
            conflicts.len()
        ));
    }

    let plans: Vec<Value> = selected
        .iter()
        .map(|f| {
            let e = &meta.icons[f];
            json!({
                "file": f,
                "name_en": e.name_en,
                "name_zh": e.name_zh,
                "title": e.title,
            })
        })
        .collect();

    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm(&format!("上传 {} 个图标到维基？", selected.len()))
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            for plan in &plans {
                reporter.log(format!(
                    "计划上传 {} → File:{}",
                    plan["file"].as_str().unwrap_or(""),
                    plan["title"].as_str().unwrap_or("")
                ));
            }
            reporter.log(format!("已跳过上传（{reason}）"));
            Ok(json!({
                "status": reason,
                "total": selected.len(),
                "planned": plans,
                "title_conflicts": conflicts,
            }))
        }
        WriteDecision::Apply => {
            // 手动标题同时写入映射表，保证后续展示/批量沿用。
            if manual_title.is_some() {
                let path = icon_meta::save_overrides(&icon_meta::overrides_path(), &overrides)?;
                reporter.log(format!(
                    "已更新映射表 {}：{} → {}",
                    path.display(),
                    manual_pending.0,
                    manual_pending.1
                ));
            }

            let mut client = WikiClient::from_env()?;
            reporter.log("登录维基".to_string());
            client.login().await?;

            let mut uploaded = Vec::new();
            let mut failed = Vec::new();
            for (i, file) in selected.iter().enumerate() {
                let entry = &meta.icons[file];
                let title = entry.title.clone().unwrap_or_default();
                if title.is_empty() {
                    failed.push(json!({ "file": file, "error": "无法生成标题" }));
                    continue;
                }
                let source = entry
                    .source
                    .as_deref()
                    .and_then(icon_source)
                    .unwrap_or_else(default_source);
                let dir = out_dir.join("current/split").join(source.dir);
                let category = icon_category(source.id);
                let comment = params
                    .comment
                    .as_deref()
                    .unwrap_or_else(|| upload_comment(source.id));
                // 已存在则自动覆盖重传（ignorewarnings=1），无需前端猜测。
                let exists = entry.wiki.as_ref().and_then(|w| w.exists) == Some(true);
                let ignore_warnings = params.ignore_warnings || exists;
                reporter.log(format!(
                    "[{}/{}] {} → File:{}",
                    i + 1,
                    selected.len(),
                    file,
                    title
                ));
                let path = dir.join(file);
                match client
                    .upload_file(
                        &path,
                        Some(&title),
                        Some(category),
                        Some(comment),
                        ignore_warnings,
                    )
                    .await
                {
                    Ok(result) => {
                        reporter.log(format!(
                            "  已上传 File:{}（{}）{}",
                            result.filename,
                            result.result,
                            result.url.as_deref().unwrap_or("")
                        ));
                        meta.set_wiki_status(
                            file,
                            true,
                            Some(format!("File:{title}")),
                            result.url.clone(),
                        );
                        uploaded.push(json!({
                            "file": file,
                            "title": title,
                            "url": result.url,
                        }));
                    }
                    Err(e) => {
                        reporter.log(format!("  上传失败 {file}: {e}"));
                        failed.push(json!({
                            "file": file,
                            "title": title,
                            "error": e.to_string(),
                        }));
                    }
                }
            }
            icon_meta::save(&out_dir, &meta)?;

            let status = if failed.is_empty() {
                "ok"
            } else if uploaded.is_empty() {
                "failed"
            } else {
                "partial"
            };
            reporter.log(format!(
                "上传完成：成功 {} / 失败 {}",
                uploaded.len(),
                failed.len()
            ));
            Ok(json!({
                "status": status,
                "total": selected.len(),
                "uploaded": uploaded,
                "failed": failed,
                "title_conflicts": conflicts,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts_sync::images::meta::IconMetaEntry;

    fn meta_with(files: &[(&str, &str, bool)]) -> IconMeta {
        let mut meta = IconMeta::default();
        for (file, name_en, uploadable) in files {
            let title = uploadable
                .then(|| icon_meta::auto_title(name_en).unwrap_or_else(|| name_en.to_string()));
            meta.icons.insert(
                (*file).to_string(),
                IconMetaEntry {
                    name_en: Some((*name_en).to_string()),
                    name_zh: None,
                    source: Some("inventory".to_string()),
                    title,
                    title_source: Some(icon_meta::TitleSource::Auto),
                    uploadable: *uploadable,
                    note: (!*uploadable).then(|| "占位符".to_string()),
                    wiki: None,
                },
            );
        }
        meta
    }

    #[test]
    fn test_validate_icon_file_and_apply_manual_title() {
        let dir = std::env::temp_dir().join(format!("icon_upload_manual_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let img = dir.join("current/split/inventoryimages");
        std::fs::create_dir_all(&img).unwrap();
        std::fs::write(img.join("skin.png"), b"png").unwrap();
        let craft = dir.join("current/split/crafting_menu_icons");
        std::fs::create_dir_all(&craft).unwrap();
        std::fs::write(craft.join("filter_tool.png"), b"png").unwrap();

        assert_eq!(
            validate_icon_file(&dir, "skin.png").unwrap().id,
            "inventory"
        );
        assert_eq!(
            validate_icon_file(&dir, "filter_tool.png").unwrap().id,
            "crafting"
        );
        assert!(validate_icon_file(&dir, "nope.png").is_err());
        assert!(validate_icon_file(&dir, "../evil.png").is_err());

        // 有英文名但 `/` 自动标题不可用；映射后恢复
        let mut meta = meta_with(&[("multitool_axe_pickaxe.png", "Pick/Axe", false)]);
        let mut ov = icon_meta::IconTitleOverrides::default();
        ov.insert("multitool_axe_pickaxe.png", "Pick-Axe.png");
        ov.insert("skin.png", "Skin Icon.png");
        apply_manual_title(&mut meta, &ov, "multitool_axe_pickaxe.png", "Pick-Axe.png");
        assert_eq!(
            meta.icons["multitool_axe_pickaxe.png"].title.as_deref(),
            Some("Pick-Axe.png")
        );
        assert!(meta.icons["multitool_axe_pickaxe.png"].uploadable);

        // 无英文名条目按需新建
        apply_manual_title(&mut meta, &ov, "skin.png", "Skin Icon.png");
        let skin = &meta.icons["skin.png"];
        assert_eq!(skin.name_en, None);
        assert_eq!(skin.title.as_deref(), Some("Skin Icon.png"));
        assert_eq!(skin.title_source, Some(icon_meta::TitleSource::Override));
        assert!(skin.uploadable);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_select_files_by_file_and_build() {
        let meta = meta_with(&[
            ("axe.png", "Axe", true),
            ("gold.png", "Gold Nugget", true),
            ("placeholder.png", "{item} Blueprint", false),
        ]);
        // 单个
        assert_eq!(
            select_files(&meta, Some("axe.png"), None, None, None).unwrap(),
            vec!["axe.png".to_string()]
        );
        // 单个不可上传 → 错误
        assert!(select_files(&meta, Some("placeholder.png"), None, None, None).is_err());
        assert!(select_files(&meta, Some("missing.png"), None, None, None).is_err());
        // 无筛选 → 全部可上传
        assert_eq!(
            select_files(&meta, None, None, None, None).unwrap(),
            vec!["axe.png".to_string(), "gold.png".to_string()]
        );
        // 按 build（first_build 映射把 gold 归到 200）
        let mut map = BTreeMap::new();
        map.insert("axe.png".to_string(), "100".to_string());
        map.insert("gold.png".to_string(), "200".to_string());
        assert_eq!(
            select_files(&meta, None, Some("200"), Some(&map), None).unwrap(),
            vec!["gold.png".to_string()]
        );
        assert_eq!(
            select_files(&meta, None, Some("999"), Some(&map), None).unwrap(),
            Vec::<String>::new()
        );
        // 缺索引 → 错误
        assert!(select_files(&meta, None, Some("200"), None, None).is_err());
    }

    #[test]
    fn test_select_files_filters_by_source() {
        let mut meta = meta_with(&[("axe.png", "Axe", true), ("filter_tool.png", "Tools", true)]);
        meta.icons.get_mut("filter_tool.png").unwrap().source = Some("crafting".to_string());
        assert_eq!(
            select_files(&meta, None, None, None, Some("crafting")).unwrap(),
            vec!["filter_tool.png".to_string()]
        );
        assert_eq!(
            select_files(&meta, None, None, None, Some("inventory")).unwrap(),
            vec!["axe.png".to_string()]
        );
    }

    #[test]
    fn test_dedupe_by_title_keeps_first() {
        let meta = meta_with(&[
            ("garlic.png", "Garlic", true),
            ("quagmire_garlic.png", "Garlic", true),
            ("axe.png", "Axe", true),
        ]);
        let (kept, conflicts) = dedupe_by_title(
            &meta,
            vec![
                "axe.png".to_string(),
                "garlic.png".to_string(),
                "quagmire_garlic.png".to_string(),
            ],
        );
        assert_eq!(kept, vec!["axe.png".to_string(), "garlic.png".to_string()]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0]["file"], "quagmire_garlic.png");
        assert_eq!(conflicts[0]["kept"], "garlic.png");
        assert_eq!(conflicts[0]["title"], "Garlic.png");
    }

    #[test]
    fn test_select_files_after_missing_filter() {
        let mut meta = meta_with(&[
            ("a.png", "A", true),
            ("b.png", "B", true),
            ("c.png", "C", true),
        ]);
        meta.icons.get_mut("a.png").unwrap().wiki = Some(icon_meta::WikiStatus {
            exists: Some(true),
            checked_at: Some(1),
            title: None,
            url: None,
        });
        meta.icons.get_mut("b.png").unwrap().wiki = Some(icon_meta::WikiStatus {
            exists: Some(false),
            checked_at: Some(1),
            title: None,
            url: None,
        });
        let mut selected = select_files(&meta, None, None, None, None).unwrap();
        selected.retain(|f| {
            meta.icons
                .get(f)
                .and_then(|e| e.wiki.as_ref())
                .and_then(|w| w.exists)
                == Some(false)
        });
        assert_eq!(selected, vec!["b.png".to_string()]);
    }
}
