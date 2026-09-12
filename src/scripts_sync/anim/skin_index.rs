//! 皮肤动画资源索引（`skin-index`）。
//!
//! 解析 `prefabs/skinprefabs.lua` 中的 `CreatePrefabSkin(...)` 条目，建立
//! `base_prefab -> skins` 映射；再根据 build 名在 `data/anim/dynamic/` 中
//! 配对 `.zip`（build 包）与 `.dyn`（贴图包），输出 `output/skin-index.json`
//! 供 WebUI 皮肤预览使用。
//!
//! 设计文档：`docs/ANIM_SKIN_PREVIEW_PLAN.md`。
//!
//! - `build` 优先取 `build_name_override`，否则直接用 skin 名；
//! - `zip` / `dyn` 按同名 stem 在 `dynamic/` 目录中查找；
//! - 一个 build 可以被多个 skin prefab 复用（如 `abigail_ice` 与
//!   `abigail_flower_ice`）。

use crate::error::{Error, Result};
use crate::platform::progress::Reporter;
use crate::scripts_sync::anim::index::{line_of, string_literal_text};
use full_moon::ast;
use full_moon::node::Node;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 一个皮肤条目解析并配对 `dynamic/` 文件后的结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkinEntry {
    /// Skin prefab 名（`CreatePrefabSkin` 第一个参数）。
    pub skin: String,
    /// `base_prefab` 字段：皮肤对应的原始 prefab。
    pub base_prefab: String,
    /// Build 名：优先 `build_name_override`，否则用 skin 名。
    pub build: String,
    /// `type` 字段（`item` / `base`），非字符串字面量时缺省。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// 配对的 build 包路径（相对 `data/anim`，如 `dynamic/abigail_ice.zip`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
    /// 配对的贴图包路径（相对 `data/anim`，如 `dynamic/abigail_ice.dyn`）。
    #[serde(rename = "dyn", skip_serializing_if = "Option::is_none")]
    pub dyn_file: Option<String>,
}

impl SkinEntry {
    /// `.zip` 与 `.dyn` 是否都已找到。
    pub fn has_pair(&self) -> bool {
        self.zip.is_some() && self.dyn_file.is_some()
    }
}

/// skin-index 统计信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkinIndexStats {
    /// 去重后的 skin 数（有效条目数）。
    pub skin_entries: usize,
    /// 带 `build_name_override` 的条目数。
    pub with_build_override: usize,
    /// 字段缺失/非法被跳过的条目数。
    pub skipped_entries: usize,
    /// 重复 skin 名被丢弃的条目数。
    pub duplicate_skins: usize,
    /// 不同 `base_prefab` 数。
    pub unique_base_prefabs: usize,
    /// 不同 build 数。
    pub unique_builds: usize,
    /// `dynamic/*.zip` 文件数。
    pub dynamic_zip_files: usize,
    /// `dynamic/*.dyn` 文件数。
    pub dynamic_dyn_files: usize,
    /// zip + dyn 均配对成功的条目数。
    pub paired: usize,
    /// 只有 zip 的条目数。
    pub zip_only: usize,
    /// 只有 dyn 的条目数。
    pub dyn_only: usize,
    /// 两者都缺的条目数。
    pub missing_files: usize,
    /// 至少有一个皮肤完成 zip+dyn 配对的 base_prefab 数。
    pub base_prefabs_with_pair: usize,
}

/// skin-index 工件（`output/skin-index.json`）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkinIndexArtifact {
    pub schema_version: u32,
    pub scripts_root: String,
    pub anim_root: Option<String>,
    pub stats: SkinIndexStats,
    /// `base_prefab -> skins` 映射（一级 key 为 base_prefab）。
    pub prefab_skins: BTreeMap<String, Vec<SkinEntry>>,
}

/// 一次 `skin-index` 运行的参数。
#[derive(Debug, Clone)]
pub struct SkinIndexParams {
    /// 游戏脚本根目录（含 `prefabs/skinprefabs.lua`）。
    pub scripts_root: PathBuf,
    /// 动画资源目录（`data/anim`）；缺省由 scripts 路径推导。
    pub anim_root: Option<PathBuf>,
    /// 输出 JSON 路径；缺省 `output/skin-index.json`。
    pub out: Option<PathBuf>,
}

/// `DST__ROOT` 下的缺省脚本根目录（存在时返回）。
pub fn default_scripts_root() -> Option<PathBuf> {
    let root = crate::platform::config::dst_root_opt()?
        .to_string_lossy()
        .into_owned();
    let dir = PathBuf::from(root.trim()).join("data/databundles/scripts");
    dir.is_dir().then_some(dir)
}

/// 运行 `skin-index`：解析 skinprefabs.lua + 扫描 dynamic/，输出皮肤索引。
/// 纯本地操作，不访问维基。
pub fn run_skin_index(
    params: &SkinIndexParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let skin_file = params.scripts_root.join("prefabs").join("skinprefabs.lua");
    if !skin_file.is_file() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("skinprefabs.lua not found: {}", skin_file.display()),
        )));
    }
    let anim_root = params.anim_root.clone().unwrap_or_else(|| {
        // scripts/<...>/databundles/scripts -> scripts/<...>/data/anim
        params
            .scripts_root
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("anim"))
            .unwrap_or_else(|| PathBuf::from("data/anim"))
    });
    reporter.log(format!("scripts: {}", params.scripts_root.display()));
    reporter.log(format!("skinprefabs: {}", skin_file.display()));
    reporter.log(format!("anim:    {}", anim_root.display()));

    reporter.stage("解析 skinprefabs.lua");
    let source = std::fs::read_to_string(&skin_file)?;
    let (mut entries, skipped) = parse_skinprefabs(&source)?;
    reporter.log(format!(
        "CreatePrefabSkin 条目 {} / 跳过 {}",
        entries.len() + skipped.len(),
        skipped.len()
    ));
    for entry in skipped.iter().take(20) {
        reporter.log(format!(
            "跳过条目 line {} skin {:?} base_prefab {:?}",
            entry.line, entry.skin, entry.base_prefab
        ));
    }

    // Dedupe repeated skin names: the first declaration wins.
    let total_before_dedupe = entries.len();
    let mut seen = BTreeSet::new();
    entries.retain(|e| seen.insert(e.skin.clone().unwrap_or_default()));
    let duplicate_skins = total_before_dedupe - entries.len();
    let with_build_override = entries
        .iter()
        .filter(|e| e.build_override.is_some())
        .count();

    reporter.stage("扫描 data/anim/dynamic");
    let dynamic = scan_dynamic_dir(&anim_root)?;
    reporter.log(format!(
        "dynamic/*.zip {} / dynamic/*.dyn {}",
        dynamic.zip_files, dynamic.dyn_files
    ));

    reporter.stage("配对 skin -> build/zip/dyn");
    let mut prefab_skins: BTreeMap<String, Vec<SkinEntry>> = BTreeMap::new();
    for raw in &entries {
        let (Some(skin), Some(base_prefab)) = (&raw.skin, &raw.base_prefab) else {
            continue;
        };
        let build = raw.build_override.clone().unwrap_or_else(|| skin.clone());
        let zip = dynamic
            .zip_stems
            .contains(&build)
            .then(|| format!("dynamic/{build}.zip"));
        let dyn_file = dynamic
            .dyn_stems
            .contains(&build)
            .then(|| format!("dynamic/{build}.dyn"));
        prefab_skins
            .entry(base_prefab.clone())
            .or_default()
            .push(SkinEntry {
                skin: skin.clone(),
                base_prefab: base_prefab.clone(),
                build,
                kind: raw.kind.clone(),
                zip,
                dyn_file,
            });
    }

    let mut unique_builds = BTreeSet::new();
    let (mut paired, mut zip_only, mut dyn_only, mut missing_files) = (0, 0, 0, 0);
    for group in prefab_skins.values() {
        for entry in group {
            unique_builds.insert(entry.build.clone());
            match (entry.zip.as_ref(), entry.dyn_file.as_ref()) {
                (Some(_), Some(_)) => paired += 1,
                (Some(_), None) => zip_only += 1,
                (None, Some(_)) => dyn_only += 1,
                (None, None) => missing_files += 1,
            }
        }
    }
    let base_prefabs_with_pair = prefab_skins
        .values()
        .filter(|group| group.iter().any(|e| e.has_pair()))
        .count();

    let stats = SkinIndexStats {
        skin_entries: entries.len(),
        with_build_override,
        skipped_entries: skipped.len(),
        duplicate_skins,
        unique_base_prefabs: prefab_skins.len(),
        unique_builds: unique_builds.len(),
        dynamic_zip_files: dynamic.zip_files,
        dynamic_dyn_files: dynamic.dyn_files,
        paired,
        zip_only,
        dyn_only,
        missing_files,
        base_prefabs_with_pair,
    };

    let artifact = SkinIndexArtifact {
        schema_version: 1,
        scripts_root: params.scripts_root.display().to_string(),
        anim_root: Some(anim_root.display().to_string()),
        stats: stats.clone(),
        prefab_skins,
    };

    let out_path = params
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("output").join("skin-index.json"));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::platform::fs::write_text_atomic(&out_path, serde_json::to_string_pretty(&artifact)?)?;

    reporter.log(format!(
        "skin {} / base_prefab {} / build {} / 配对 {} / 缺文件 {}",
        stats.skin_entries,
        stats.unique_base_prefabs,
        stats.unique_builds,
        stats.paired,
        stats.missing_files,
    ));
    reporter.log(format!("已写入 {}", out_path.display()));

    Ok(serde_json::json!({
        "schema_version": artifact.schema_version,
        "skin_entries": stats.skin_entries,
        "duplicate_skins": stats.duplicate_skins,
        "skipped_entries": stats.skipped_entries,
        "with_build_override": stats.with_build_override,
        "unique_base_prefabs": stats.unique_base_prefabs,
        "unique_builds": stats.unique_builds,
        "paired": stats.paired,
        "zip_only": stats.zip_only,
        "dyn_only": stats.dyn_only,
        "missing_files": stats.missing_files,
        "base_prefabs_with_pair": stats.base_prefabs_with_pair,
        "output": out_path,
    }))
}

/// `dynamic/` 目录的 stem 配对视图。
#[derive(Debug, Default, Clone)]
pub(crate) struct DynamicStems {
    /// 有 `<stem>.zip` 的 stem 集合。
    pub zip_stems: BTreeSet<String>,
    /// 有 `<stem>.dyn` 的 stem 集合。
    pub dyn_stems: BTreeSet<String>,
    pub zip_files: usize,
    pub dyn_files: usize,
}

/// 扫描 `<anim_root>/dynamic/`，收集按 stem 分组的 zip/dyn 文件。
pub(crate) fn scan_dynamic_dir(anim_root: &Path) -> Result<DynamicStems> {
    let dir = anim_root.join("dynamic");
    let mut stems = DynamicStems::default();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("dynamic directory not found: {}", dir.display()),
            )));
        }
        Err(e) => return Err(Error::Io(e)),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("zip") => {
                stems.zip_stems.insert(stem.to_string());
                stems.zip_files += 1;
            }
            Some(ext) if ext.eq_ignore_ascii_case("dyn") => {
                stems.dyn_stems.insert(stem.to_string());
                stems.dyn_files += 1;
            }
            _ => {}
        }
    }
    Ok(stems)
}

/// 从 `skinprefabs.lua` 解析出的原始皮肤条目。
#[derive(Debug, Clone, Default)]
pub(crate) struct RawSkinEntry {
    pub skin: Option<String>,
    pub base_prefab: Option<String>,
    pub build_override: Option<String>,
    pub kind: Option<String>,
    pub line: u32,
}

/// 解析 `skinprefabs.lua`：收集所有 `CreatePrefabSkin(name, { ... })` 调用。
/// 返回（字段完整的条目，被跳过的条目）。
pub(crate) fn parse_skinprefabs(source: &str) -> Result<(Vec<RawSkinEntry>, Vec<RawSkinEntry>)> {
    let ast = full_moon::parse(source)
        .map_err(|e| Error::ParseError(format!("failed to parse skinprefabs.lua: {e:?}")))?;
    let mut visitor = SkinVisitor {
        source,
        entries: Vec::new(),
    };
    visitor.visit_block(ast.nodes());
    let mut good = Vec::new();
    let mut skipped = Vec::new();
    for entry in visitor.entries {
        if entry.skin.as_deref().is_some_and(|s| !s.is_empty())
            && entry.base_prefab.as_deref().is_some_and(|s| !s.is_empty())
        {
            good.push(entry);
        } else {
            skipped.push(entry);
        }
    }
    Ok((good, skipped))
}

struct SkinVisitor<'s> {
    source: &'s str,
    entries: Vec<RawSkinEntry>,
}

impl<'s> SkinVisitor<'s> {
    fn visit_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call) => self.visit_call(call),
                ast::Stmt::Assignment(assign) => {
                    for expr in assign.expressions().iter() {
                        self.visit_expr(expr);
                    }
                }
                ast::Stmt::LocalAssignment(assign) => {
                    for expr in assign.expressions().iter() {
                        self.visit_expr(expr);
                    }
                }
                ast::Stmt::LocalFunction(local_fn) => self.visit_block(local_fn.body().block()),
                ast::Stmt::FunctionDeclaration(func) => self.visit_block(func.body().block()),
                ast::Stmt::If(if_stmt) => {
                    self.visit_block(if_stmt.block());
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.visit_block(else_if.block());
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.visit_block(else_block);
                    }
                }
                ast::Stmt::While(w) => self.visit_block(w.block()),
                ast::Stmt::Repeat(r) => self.visit_block(r.block()),
                ast::Stmt::NumericFor(f) => self.visit_block(f.block()),
                ast::Stmt::GenericFor(f) => self.visit_block(f.block()),
                ast::Stmt::Do(d) => self.visit_block(d.block()),
                _ => {}
            }
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns() {
                self.visit_expr(expr);
            }
        }
    }

    fn visit_call(&mut self, call: &ast::FunctionCall) {
        if is_named_call(call, "CreatePrefabSkin") {
            self.entries.push(self.parse_entry(call));
        }
        for suffix in call.suffixes() {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                self.visit_args(args);
            }
        }
    }

    fn visit_args(&mut self, args: &ast::FunctionArgs) {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                for expr in arguments.iter() {
                    self.visit_expr(expr);
                }
            }
            ast::FunctionArgs::TableConstructor(table) => self.visit_table(table),
            _ => {}
        }
    }

    fn visit_table(&mut self, table: &ast::TableConstructor) {
        for field in table.fields() {
            match field {
                ast::Field::NoKey(expression) => self.visit_expr(expression),
                ast::Field::NameKey { value, .. } => self.visit_expr(value),
                ast::Field::ExpressionKey { key, value, .. } => {
                    self.visit_expr(key);
                    self.visit_expr(value);
                }
                _ => {}
            }
        }
    }

    fn visit_expr(&mut self, expr: &ast::Expression) {
        match expr {
            ast::Expression::FunctionCall(call) => self.visit_call(call),
            ast::Expression::Function(func) => self.visit_block(func.body().block()),
            ast::Expression::TableConstructor(table) => self.visit_table(table),
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.visit_expr(lhs);
                self.visit_expr(rhs);
            }
            ast::Expression::UnaryOperator { expression, .. } => self.visit_expr(expression),
            ast::Expression::Parentheses { expression, .. } => self.visit_expr(expression),
            ast::Expression::Var(ast::Var::Expression(vex)) => {
                for suffix in vex.suffixes() {
                    if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                        self.visit_args(args);
                    }
                }
            }
            _ => {}
        }
    }

    fn parse_entry(&self, call: &ast::FunctionCall) -> RawSkinEntry {
        let mut entry = RawSkinEntry {
            line: self.line(call),
            ..RawSkinEntry::default()
        };
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = call.suffixes().next()
        else {
            return entry;
        };
        let args: Vec<_> = arguments.iter().collect();
        if let Some(ast::Expression::String(s)) = args.first() {
            entry.skin = Some(string_literal_text(&s.to_string()));
        }
        if let Some(ast::Expression::TableConstructor(table)) = args.get(1) {
            for field in table.fields() {
                let ast::Field::NameKey { key, value, .. } = field else {
                    continue;
                };
                let ast::Expression::String(s) = value else {
                    continue;
                };
                let text = string_literal_text(&s.to_string());
                match key.token().to_string().as_str() {
                    "base_prefab" => entry.base_prefab = Some(text),
                    "build_name_override" => entry.build_override = Some(text),
                    "type" => entry.kind = Some(text),
                    _ => {}
                }
            }
        }
        entry
    }

    fn line(&self, node: &impl Node) -> u32 {
        node.start_position()
            .map(|p| line_of(self.source, p.bytes()))
            .unwrap_or(0)
    }
}

fn is_named_call(call: &ast::FunctionCall, name: &str) -> bool {
    matches!(&call.prefix(), ast::Prefix::Name(prefix) if prefix.token().to_string() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestReporter;

    impl Reporter for TestReporter {
        fn log(&self, _msg: String) {}
        fn stage(&self, _name: &str) {}
        fn diff(&self, _page: &str, _text: &str, _added: usize, _removed: usize) {}
        fn confirm(&self, _prompt: &str) -> bool {
            true
        }
    }

    const SAMPLE: &str = r#"
local prefs = {}

table.insert(prefs, CreatePrefabSkin("abigail_ice",
{
	base_prefab = "abigail",
	type = "item",
	build_name_override = "abigail_ice",
}))

table.insert(prefs, CreatePrefabSkin("abigail_flower_ice",
{
	base_prefab = "abigail_flower",
	type = "item",
	build_name_override = "abigail_ice",
}))

table.insert(prefs, CreatePrefabSkin("wilson_ice",
{
	base_prefab = "wilson",
	type = "base",
}))

table.insert(prefs, CreatePrefabSkin("broken",
{
	type = "item",
}))

return unpack(prefs)
"#;

    #[test]
    fn parses_entries_and_skips_incomplete() {
        let (good, skipped) = parse_skinprefabs(SAMPLE).unwrap();
        assert_eq!(good.len(), 3);
        assert_eq!(skipped.len(), 1);
        let abigail = good
            .iter()
            .find(|e| e.skin.as_deref() == Some("abigail_ice"))
            .unwrap();
        assert_eq!(abigail.base_prefab.as_deref(), Some("abigail"));
        assert_eq!(abigail.build_override.as_deref(), Some("abigail_ice"));
        assert_eq!(abigail.kind.as_deref(), Some("item"));
        assert!(abigail.line > 0);
        let wilson = good
            .iter()
            .find(|e| e.skin.as_deref() == Some("wilson_ice"))
            .unwrap();
        assert_eq!(wilson.build_override, None);
        assert_eq!(wilson.kind.as_deref(), Some("base"));
    }

    #[test]
    fn dynamic_dir_scan_pairs_by_stem() {
        let dir = std::env::temp_dir().join(format!("skin_idx_scan_{}", std::process::id()));
        let dyn_dir = dir.join("dynamic");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dyn_dir).unwrap();
        for f in ["abigail_ice.zip", "abigail_ice.dyn", "wilson_ice.zip"] {
            std::fs::write(dyn_dir.join(f), b"").unwrap();
        }
        let stems = scan_dynamic_dir(&dir).unwrap();
        assert_eq!(stems.zip_files, 2);
        assert_eq!(stems.dyn_files, 1);
        assert!(stems.zip_stems.contains("wilson_ice"));
        assert!(!stems.dyn_stems.contains("wilson_ice"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_writes_prefab_skins_mapping() {
        let dir = std::env::temp_dir().join(format!("skin_idx_run_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts/prefabs")).unwrap();
        std::fs::create_dir_all(dir.join("anim/dynamic")).unwrap();
        std::fs::write(dir.join("scripts/prefabs/skinprefabs.lua"), SAMPLE).unwrap();
        for f in ["abigail_ice.zip", "abigail_ice.dyn", "wilson_ice.dyn"] {
            std::fs::write(dir.join("anim/dynamic").join(f), b"").unwrap();
        }
        let out = dir.join("out/skin-index.json");
        let report = run_skin_index(
            &SkinIndexParams {
                scripts_root: dir.join("scripts"),
                anim_root: Some(dir.join("anim")),
                out: Some(out.clone()),
            },
            &TestReporter,
        )
        .unwrap();

        assert_eq!(report["skin_entries"], 3);
        assert_eq!(report["unique_base_prefabs"], 3);
        assert_eq!(report["with_build_override"], 2);
        // Both `abigail_ice` and `abigail_flower_ice` pair with the same build.
        assert_eq!(report["paired"], 2);
        assert_eq!(report["zip_only"], 0);
        assert_eq!(report["dyn_only"], 1);
        assert_eq!(report["missing_files"], 0);

        let artifact: SkinIndexArtifact =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let abigail = &artifact.prefab_skins["abigail"];
        assert_eq!(abigail.len(), 1);
        assert_eq!(abigail[0].zip.as_deref(), Some("dynamic/abigail_ice.zip"));
        assert_eq!(
            abigail[0].dyn_file.as_deref(),
            Some("dynamic/abigail_ice.dyn")
        );
        // `abigail_flower_ice` reuses the same build as `abigail_ice`.
        let flower = &artifact.prefab_skins["abigail_flower"];
        assert_eq!(flower[0].build, "abigail_ice");
        assert_eq!(flower[0].zip.as_deref(), Some("dynamic/abigail_ice.zip"));
        // `wilson_ice` has no override, so build == skin name; only the dyn exists.
        let wilson = &artifact.prefab_skins["wilson"];
        assert_eq!(wilson[0].build, "wilson_ice");
        assert_eq!(wilson[0].zip, None);
        assert_eq!(
            wilson[0].dyn_file.as_deref(),
            Some("dynamic/wilson_ice.dyn")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn duplicate_skin_names_keep_first() {
        let dir = std::env::temp_dir().join(format!("skin_idx_dup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts/prefabs")).unwrap();
        std::fs::create_dir_all(dir.join("anim/dynamic")).unwrap();
        std::fs::write(
            dir.join("scripts/prefabs/skinprefabs.lua"),
            r#"
table.insert(prefs, CreatePrefabSkin("a", { base_prefab = "p" }))
table.insert(prefs, CreatePrefabSkin("a", { base_prefab = "q" }))
"#,
        )
        .unwrap();
        let report = run_skin_index(
            &SkinIndexParams {
                scripts_root: dir.join("scripts"),
                anim_root: Some(dir.join("anim")),
                out: Some(dir.join("out.json")),
            },
            &TestReporter,
        )
        .unwrap();
        assert_eq!(report["skin_entries"], 1);
        assert_eq!(report["duplicate_skins"], 1);
        assert_eq!(report["unique_base_prefabs"], 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
