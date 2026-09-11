//! Prefab → animation asset index.
//!
//! This is the first "animation asset management" milestone: scan
//! `scripts/prefabs` for `Asset("ANIM", "anim/xxx.zip")` /
//! `Asset("DYNAMIC_ANIM", "anim/dynamic/xxx.zip")` declarations, associate
//! them to prefab variants through `Prefab(name, fn, assets, deps)`, verify
//! existence against `data/anim`, and report dynamic/unresolved forms.
//!
//! Skin-related `.dyn`/`PKGREF` mapping lives in the sibling `skin_index`
//! module (`prefabs/skinprefabs.lua` → `prefab_skins` index).

use crate::error::Result;
use crate::parser::anim_override::{parse_anim_overrides_in, SymbolOverrideCall, SymbolRemapIndex};
use crate::parser::clothing_overrides::{parse_clothing_overrides, ClothingEntry};
use crate::service::Reporter;
use full_moon::ast;
use full_moon::node::Node;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One parsed `Asset("ANIM", ...)` / `Asset("DYNAMIC_ANIM", ...)` entry.
#[derive(Debug, Clone, Serialize)]
pub struct AssetEntry {
    pub kind: String,
    pub raw_path: String,
    pub line: u32,
    /// Normalized path relative to `data/anim` when statically known.
    pub normalized: Option<String>,
}

/// Template for an `Asset` entry inside an asset-factory function.
#[derive(Debug, Clone)]
struct TemplateAssetEntry {
    kind: String,
    raw: String,
    line: u32,
    path_template: Option<PathTemplate>,
}

/// A string path built from literals and factory parameters.
#[derive(Debug, Clone)]
enum PathTemplate {
    Literal(String),
    Concat(Vec<PathPart>),
}

#[derive(Debug, Clone)]
enum PathPart {
    Lit(String),
    Param(String),
}

/// A factory function that returns an asset table (`makeassetlist`, etc.).
#[derive(Debug, Clone)]
struct AssetFactory {
    params: Vec<String>,
    entries: Vec<TemplateAssetEntry>,
}

/// A factory function that returns one or more `Prefab(...)` calls.
#[derive(Debug, Clone)]
struct PrefabFactory {
    params: Vec<String>,
    templates: Vec<PrefabTemplate>,
}

#[derive(Debug, Clone)]
struct PrefabTemplate {
    name: TemplateName,
    assets: TemplateAssets,
    line: u32,
}

#[derive(Debug, Clone)]
enum TemplateName {
    Lit(String),
    Param(String),
    Other,
}

#[derive(Debug, Clone)]
enum TemplateAssets {
    /// `Prefab(name, fn, assets_var, ...)` where `assets_var` is a parameter.
    Param(String),
    /// `Prefab(name, fn, makeassetlist(arg), ...)`.
    FactoryCall {
        name: String,
        args: Vec<TemplateArg>,
    },
    /// Inline table in the Prefab call itself.
    Inline(Vec<TemplateAssetEntry>),
    Other(String),
}

#[derive(Debug, Clone)]
enum TemplateArg {
    Lit(String),
    Param(String),
    Other(String),
}

/// A concrete argument value at a factory/prefab call site.
#[derive(Debug, Clone)]
enum ResolvedArg {
    Str(String),
    Var(String),
    Table(Vec<AssetEntry>),
    Other(String),
}

/// A prefab variant and its resolved animation assets.
#[derive(Debug, Clone, Serialize)]
pub struct PrefabAnimRecord {
    pub prefab_file: String,
    pub prefab_name: Option<String>,
    pub asset_var: Option<String>,
    pub anims: Vec<AnimRef>,
    /// Related non-anim build/package files (PKGREF `.zip`) useful for rendering.
    pub related_files: Vec<String>,
    pub unresolved: Vec<UnresolvedRef>,
    pub content: AnimContent,
}

/// A resolved animation file reference attached to a prefab variant.
#[derive(Debug, Clone, Serialize)]
pub struct AnimRef {
    pub kind: String,
    pub path: String,
    pub normalized: String,
    pub exists: bool,
}

/// A dynamic/unresolved asset reference or asset-table expression.
#[derive(Debug, Clone, Serialize)]
pub struct UnresolvedRef {
    pub kind: String,
    pub raw: String,
    pub line: u32,
}

/// Unresolved entry in the global ledger, with prefab context.
#[derive(Debug, Clone, Serialize)]
pub struct GlobalUnresolvedRef {
    pub prefab_file: String,
    pub prefab_name: Option<String>,
    pub kind: String,
    pub raw: String,
    pub line: u32,
}

/// Content-level summary extracted from an animation package's `anim.bin`
/// and `build.bin`.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct AnimContent {
    pub banks: Vec<String>,
    pub animations: Vec<String>,
    pub builds: Vec<String>,
    pub symbols: Vec<String>,
    pub atlases: Vec<String>,
}

/// How often an animation file is referenced by prefab variants.
#[derive(Debug, Clone, Serialize)]
pub struct AnimFileUsage {
    pub path: String,
    pub kind: String,
    pub exists: bool,
    pub referenced_by: Vec<String>,
    pub content: AnimContent,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimIndexStats {
    pub prefab_files: usize,
    pub prefab_variants: usize,
    pub anim_refs: usize,
    pub resolved_refs: usize,
    pub unresolved_refs: usize,
    pub unique_anim_paths: usize,
    pub missing_anim_paths: usize,
    pub multi_anim_files: usize,
    pub skipped_pkgref_dyn: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimIndexArtifact {
    pub schema_version: u32,
    pub scripts_root: String,
    pub anim_root: Option<String>,
    pub stats: AnimIndexStats,
    pub prefabs: Vec<PrefabAnimRecord>,
    pub anim_files: BTreeMap<String, AnimFileUsage>,
    pub build_files: BTreeMap<String, AnimContent>,
    pub unresolved: Vec<GlobalUnresolvedRef>,
}

#[derive(Debug, Clone)]
pub struct AnimIndexParams {
    pub scripts_root: PathBuf,
    pub anim_root: Option<PathBuf>,
    pub out: Option<PathBuf>,
}

/// Run `anim-index`: scan prefab Lua and build a prefab↔animation-file JSON
/// index. Local-only, no wiki writes.
pub fn run_index(params: &AnimIndexParams, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    reporter.stage("构建 prefab × 动画资源索引");

    let prefabs_dir = params.scripts_root.join("prefabs");
    if !prefabs_dir.is_dir() {
        return Err(crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("prefabs directory not found: {}", prefabs_dir.display()),
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
    reporter.log(format!("prefabs: {}", prefabs_dir.display()));
    reporter.log(format!("anim:    {}", anim_root.display()));

    let mut prefab_records = Vec::new();
    let mut unresolved = Vec::new();
    let mut prefab_file_count = 0usize;
    let mut skipped_pkgref_dyn = 0usize;
    let mut remap_calls: Vec<SymbolOverrideCall> = Vec::new();

    let mut content_cache: BTreeMap<String, AnimContent> = BTreeMap::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_lua_files(&prefabs_dir, &mut files)?;
    files.sort();

    for path in &files {
        let rel = path
            .strip_prefix(&params.scripts_root)
            .map_err(|e| crate::error::Error::InvalidPath(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(path)?;
        let scanner = Scanner::new(&source);
        let file_index = scanner.scan(rel.clone());
        prefab_file_count += 1;
        skipped_pkgref_dyn += file_index.skipped_pkgref_dyn;

        // Tier-A/C 提取：AnimState 符号重映射调用（常量三元组 + 变量追踪）。
        // 整文件解析失败时静默跳过——Scanner 已把 PARSE_ERROR 记进 unresolved。
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if let Ok(mut calls) = parse_anim_overrides_in(&source, Some(&stem)) {
            remap_calls.append(&mut calls);
        }

        for mut record in file_index.prefabs {
            // Resolve existence now that we know anim_root.
            for anim in &mut record.anims {
                anim.exists = anim_root.join(&anim.normalized).is_file();
            }

            // Deduplicate repeated animation paths from factory expansion.
            let mut seen = BTreeSet::new();
            record
                .anims
                .retain(|a| seen.insert((a.kind.clone(), a.normalized.clone())));

            // Content-level index from anim.bin/build.bin.
            let mut content = AnimContent::default();
            for anim in &record.anims {
                if let Some(cached) = content_cache.get(&anim.normalized) {
                    merge_content(&mut content, cached);
                    continue;
                }
                if !anim.exists {
                    continue;
                }
                if let Ok(c) = load_anim_content(&anim_root, &anim.normalized) {
                    merge_content(&mut content, &c);
                    content_cache.insert(anim.normalized.clone(), c);
                }
            }
            sort_dedup_content(&mut content);
            record.content = content;

            for ur in &record.unresolved {
                unresolved.push(GlobalUnresolvedRef {
                    prefab_file: record.prefab_file.clone(),
                    prefab_name: record.prefab_name.clone(),
                    kind: ur.kind.clone(),
                    raw: ur.raw.clone(),
                    line: ur.line,
                });
            }
            if !record.anims.is_empty() || !record.unresolved.is_empty() {
                prefab_records.push(record);
            }
        }
    }

    // Tier-C 覆盖面：重映射调用也大量存在于 stategraphs（SG*.lua）与
    // components（skinner 等）。这些目录不做 prefab 结构扫描，仅提取调用。
    for dir_name in ["stategraphs", "components"] {
        let dir = params.scripts_root.join(dir_name);
        if !dir.is_dir() {
            continue;
        }
        let mut extra_files = Vec::new();
        collect_lua_files(&dir, &mut extra_files)?;
        extra_files.sort();
        for path in &extra_files {
            let source = std::fs::read_to_string(path)?;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if let Ok(mut calls) = parse_anim_overrides_in(&source, Some(&stem)) {
                remap_calls.append(&mut calls);
            }
        }
    }

    // Build reverse index: anim file -> prefab variants.
    let mut anim_files: BTreeMap<String, AnimFileUsage> = BTreeMap::new();
    for record in &prefab_records {
        let label = match &record.prefab_name {
            Some(name) => format!("{}#{}", record.prefab_file, name),
            None => format!("{}#?", record.prefab_file),
        };
        for anim in &record.anims {
            let entry = anim_files
                .entry(anim.normalized.clone())
                .or_insert_with(|| AnimFileUsage {
                    path: anim.normalized.clone(),
                    kind: anim.kind.clone(),
                    exists: anim.exists,
                    referenced_by: Vec::new(),
                    content: AnimContent::default(),
                });
            entry.kind = anim.kind.clone();
            entry.exists = anim.exists;
            if let Some(cached) = content_cache.get(&anim.normalized) {
                entry.content = cached.clone();
            }
            entry.referenced_by.push(label.clone());
        }
    }

    let unique_anim_paths = anim_files.len();
    let missing_anim_paths = anim_files.values().filter(|v| !v.exists).count();
    let multi_anim_files = prefab_records.iter().filter(|r| r.anims.len() >= 2).count();
    let anim_refs: usize = prefab_records.iter().map(|r| r.anims.len()).sum();
    let resolved_refs = anim_refs;
    let unresolved_refs = unresolved.len();
    let prefab_variants = prefab_records.len();

    let mut build_files: BTreeMap<String, AnimContent> = BTreeMap::new();
    for record in &prefab_records {
        for rel in &record.related_files {
            if build_files.contains_key(rel) {
                continue;
            }
            if let Ok(c) = load_anim_content(&anim_root, rel) {
                build_files.insert(rel.clone(), c);
            }
        }
    }

    let artifact = AnimIndexArtifact {
        schema_version: 1,
        scripts_root: params.scripts_root.display().to_string(),
        anim_root: Some(anim_root.display().to_string()),
        stats: AnimIndexStats {
            prefab_files: prefab_file_count,
            prefab_variants,
            anim_refs,
            resolved_refs,
            unresolved_refs,
            unique_anim_paths,
            missing_anim_paths,
            multi_anim_files,
            skipped_pkgref_dyn,
        },
        prefabs: prefab_records,
        anim_files,
        build_files,
        unresolved,
    };

    let out_path = params
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("output").join("anim-index.json"));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&artifact)?)?;

    // Tier-A 重映射索引产物：anim-index.json 的伴生文件，供 WebUI 的
    // override 选择器与渲染端点查询。
    let remap_index = SymbolRemapIndex::from_calls(&remap_calls);
    let remap_entries: usize = remap_index.symbols.values().map(Vec::len).sum();

    // Tier-B：clothing.lua 数据表（脚本根目录，不在 prefabs/ 下）。
    let clothing_path = params.scripts_root.join("clothing.lua");
    let clothing_index: BTreeMap<String, ClothingEntry> = if clothing_path.is_file() {
        let source = std::fs::read_to_string(&clothing_path)?;
        parse_clothing_overrides(&source)?
    } else {
        BTreeMap::new()
    };

    let remap_path = out_path.with_file_name("anim-remap-index.json");
    // label 取 DST version.txt（与 anim-sync manifest 同源）；
    // scripts_root = <dst>/data/databundles/scripts → dst 根 = 三层祖先。
    let dst_root = params
        .scripts_root
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf);
    let label = dst_root
        .as_deref()
        .and_then(|d| crate::scripts_sync::read_new_version(d).ok())
        .unwrap_or_else(|| "unknown".to_string());
    let remap_artifact = serde_json::json!({
        "schema_version": 1,
        "label": label,
        "parser_version": super::PARSER_VERSION,
        "generated_at": super::history::now_ms(),
        "scripts_root": params.scripts_root.display().to_string(),
        "stats": {
            "calls": remap_calls.len(),
            "symbols": remap_index.len(),
            "entries": remap_entries,
            "clothing": clothing_index.len(),
        },
        "symbols": remap_index.symbols,
        "clothing": clothing_index,
    });
    std::fs::write(&remap_path, serde_json::to_string_pretty(&remap_artifact)?)?;

    // 版本快照：供 remap-diff 对比（同 label 覆盖，确定性输出保证幂等）。
    let snapshot_path = super::remap_history::save_remap_snapshot(
        &super::remap_history::remap_history_dir(),
        &remap_artifact,
    )?;

    reporter.log(format!(
        "prefab 文件 {} / 变体记录 {} / 动画引用 {} / 唯一动画 {} / unresolved {}",
        artifact.stats.prefab_files,
        artifact.stats.prefab_variants,
        artifact.stats.anim_refs,
        artifact.stats.unique_anim_paths,
        artifact.stats.unresolved_refs,
    ));
    reporter.log(format!("已写入 {}", out_path.display()));
    reporter.log(format!(
        "重映射：{} symbol / {} 条目（{} 次调用），已写入 {}",
        remap_index.len(),
        remap_entries,
        remap_calls.len(),
        remap_path.display()
    ));
    reporter.log(format!(
        "重映射快照 {}（label {label}）",
        snapshot_path.display()
    ));

    Ok(serde_json::json!({
        "schema_version": artifact.schema_version,
        "prefab_files": artifact.stats.prefab_files,
        "prefab_variants": artifact.stats.prefab_variants,
        "anim_refs": artifact.stats.anim_refs,
        "resolved_refs": artifact.stats.resolved_refs,
        "unresolved_refs": artifact.stats.unresolved_refs,
        "unique_anim_paths": artifact.stats.unique_anim_paths,
        "missing_anim_paths": artifact.stats.missing_anim_paths,
        "multi_anim_files": artifact.stats.multi_anim_files,
        "skipped_pkgref_dyn": artifact.stats.skipped_pkgref_dyn,
        "symbol_overrides": {
            "calls": remap_calls.len(),
            "symbols": remap_index.len(),
            "entries": remap_entries,
            "clothing": clothing_index.len(),
            "label": label,
            "snapshot": snapshot_path,
            "output": remap_path,
        },
        "output": out_path,
    }))
}

pub(crate) fn collect_lua_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_lua_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("lua") {
            out.push(path);
        }
    }
    Ok(())
}

struct Scanner<'s> {
    source: &'s str,
    /// Local table variable name -> Asset entries collected from a literal
    /// `local x = { Asset(...), ... }` and `table.insert(x, Asset(...))`.
    asset_tables: BTreeMap<String, Vec<AssetEntry>>,
    /// Local string constants used to resolve simple variable arguments.
    consts: BTreeMap<String, String>,
    /// Asset-returning factory functions (`makeassetlist`, etc.).
    asset_factories: BTreeMap<String, AssetFactory>,
    /// Prefab-returning factory functions (`MakeAxe`, `item`, `pillar`, etc.).
    prefab_factories: BTreeMap<String, PrefabFactory>,
    /// Parsed `Prefab(...)` records and expanded factory records.
    prefabs: Vec<PrefabAnimRecord>,
    /// Count of PKGREF `.dyn` references we intentionally skip.
    skipped_pkgref_dyn: usize,
}

impl<'s> Scanner<'s> {
    fn new(source: &'s str) -> Self {
        Self {
            source,
            asset_tables: BTreeMap::new(),
            consts: BTreeMap::new(),
            asset_factories: BTreeMap::new(),
            prefab_factories: BTreeMap::new(),
            prefabs: Vec::new(),
            skipped_pkgref_dyn: 0,
        }
    }

    fn scan(mut self, prefab_file: String) -> ScannedFile {
        let ast = match full_moon::parse(self.source) {
            Ok(ast) => ast,
            Err(e) => {
                // A parse failure should not abort the whole index; report it
                // as an unresolved prefab so the file is visible in stats.
                return ScannedFile {
                    prefabs: vec![PrefabAnimRecord {
                        prefab_file,
                        prefab_name: None,
                        asset_var: None,
                        anims: Vec::new(),
                        related_files: Vec::new(),
                        unresolved: vec![UnresolvedRef {
                            kind: "PARSE_ERROR".to_string(),
                            raw: format!("{:?}", e),
                            line: 0,
                        }],
                        content: AnimContent::default(),
                    }],
                    skipped_pkgref_dyn: 0,
                };
            }
        };
        self.collect_consts_and_factories(ast.nodes());
        self.walk_block(ast.nodes());
        for record in &mut self.prefabs {
            record.prefab_file = prefab_file.clone();
        }
        ScannedFile {
            prefabs: self.prefabs,
            skipped_pkgref_dyn: self.skipped_pkgref_dyn,
        }
    }

    fn collect_consts_and_factories(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assign) => {
                    let names: Vec<_> = assign.names().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        let var = name.token().to_string();
                        if let ast::Expression::String(s) = expr {
                            self.consts
                                .insert(var.clone(), string_literal_text(&s.to_string()));
                        }
                        if let ast::Expression::Function(func) = expr {
                            self.register_factory(&var, func.body());
                            self.collect_consts_and_factories(func.body().block());
                        }
                    }
                }
                ast::Stmt::Assignment(assign) => {
                    let vars: Vec<_> = assign.variables().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (var, expr) in vars.iter().zip(exprs.iter()) {
                        if let ast::Var::Name(name) = var {
                            if let ast::Expression::Function(func) = expr {
                                let var_name = name.token().to_string();
                                self.register_factory(&var_name, func.body());
                                self.collect_consts_and_factories(func.body().block());
                            }
                        }
                    }
                }
                ast::Stmt::LocalFunction(local_fn) => {
                    let name = local_fn.name().to_string();
                    self.register_factory(&name, local_fn.body());
                    self.collect_consts_and_factories(local_fn.body().block());
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    let name = func_decl.name().to_string();
                    self.register_factory(&name, func_decl.body());
                    self.collect_consts_and_factories(func_decl.body().block());
                }
                ast::Stmt::If(if_stmt) => {
                    self.collect_consts_and_factories(if_stmt.block());
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.collect_consts_and_factories(else_if.block());
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.collect_consts_and_factories(else_block);
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.collect_consts_and_factories(while_stmt.block())
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.collect_consts_and_factories(repeat_stmt.block())
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.collect_consts_and_factories(for_stmt.block())
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    self.collect_consts_and_factories(for_stmt.block())
                }
                ast::Stmt::Do(do_stmt) => self.collect_consts_and_factories(do_stmt.block()),
                _ => {}
            }
        }
    }

    fn register_factory(&mut self, name: &str, body: &ast::FunctionBody) {
        let params: Vec<String> = body.parameters().iter().map(|p| p.to_string()).collect();

        if let Some(asset_factory) = self.collect_asset_factory(name, body, &params) {
            self.asset_factories.insert(name.to_string(), asset_factory);
        }
        if let Some(prefab_factory) = self.collect_prefab_factory(name, body, &params) {
            self.prefab_factories
                .insert(name.to_string(), prefab_factory);
        }
    }

    fn collect_asset_factory(
        &self,
        _name: &str,
        body: &ast::FunctionBody,
        params: &[String],
    ) -> Option<AssetFactory> {
        let mut entries = Vec::new();
        if let Some(ast::LastStmt::Return(ret)) = body.block().last_stmt() {
            for expr in ret.returns() {
                if let ast::Expression::TableConstructor(table) = expr {
                    let mut found = false;
                    for field in table.fields() {
                        let fexpr = match field {
                            ast::Field::NoKey(expression) => expression,
                            ast::Field::NameKey { value, .. } => value,
                            ast::Field::ExpressionKey { value, .. } => value,
                            _ => continue,
                        };
                        if let ast::Expression::FunctionCall(call) = fexpr {
                            if let Some(entry) = self.parse_template_asset_call(call, params) {
                                entries.push(entry);
                                found = true;
                            }
                        }
                    }
                    if found {
                        return Some(AssetFactory {
                            params: params.to_vec(),
                            entries,
                        });
                    }
                }
            }
        }
        None
    }

    fn collect_prefab_factory(
        &self,
        _name: &str,
        body: &ast::FunctionBody,
        params: &[String],
    ) -> Option<PrefabFactory> {
        let mut calls = Vec::new();
        self.collect_prefab_calls_in_block(body.block(), &mut calls);
        if calls.is_empty() {
            return None;
        }
        let mut templates = Vec::new();
        for call in calls {
            if let Some(template) = self.parse_prefab_template(call, params) {
                templates.push(template);
            }
        }
        if templates.is_empty() {
            None
        } else {
            Some(PrefabFactory {
                params: params.to_vec(),
                templates,
            })
        }
    }

    fn collect_prefab_calls_in_block<'a>(
        &self,
        block: &'a ast::Block,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::FunctionCall(call) => {
                    if is_named_call(call, "Prefab") {
                        out.push(call);
                    }
                    self.collect_prefab_calls_in_args(call, out);
                }
                ast::Stmt::LocalAssignment(assign) => {
                    for expr in assign.expressions().iter() {
                        self.collect_prefab_calls_in_expr(expr, out);
                    }
                }
                ast::Stmt::Assignment(assign) => {
                    for expr in assign.expressions().iter() {
                        self.collect_prefab_calls_in_expr(expr, out);
                    }
                }
                ast::Stmt::LocalFunction(_) => {}
                ast::Stmt::FunctionDeclaration(_) => {}
                ast::Stmt::If(if_stmt) => {
                    self.collect_prefab_calls_in_block(if_stmt.block(), out);
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.collect_prefab_calls_in_block(else_if.block(), out);
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.collect_prefab_calls_in_block(else_block, out);
                    }
                }
                ast::Stmt::While(while_stmt) => {
                    self.collect_prefab_calls_in_block(while_stmt.block(), out);
                }
                ast::Stmt::Repeat(repeat_stmt) => {
                    self.collect_prefab_calls_in_block(repeat_stmt.block(), out);
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.collect_prefab_calls_in_block(for_stmt.block(), out);
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    self.collect_prefab_calls_in_block(for_stmt.block(), out);
                }
                ast::Stmt::Do(do_stmt) => {
                    self.collect_prefab_calls_in_block(do_stmt.block(), out);
                }
                _ => {}
            }
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns() {
                self.collect_prefab_calls_in_expr(expr, out);
            }
        }
    }

    fn collect_prefab_calls_in_args<'a>(
        &self,
        call: &'a ast::FunctionCall,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        for suffix in call.suffixes() {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                self.collect_prefab_calls_in_args_expr(args, out);
            }
        }
    }

    fn collect_prefab_calls_in_args_expr<'a>(
        &self,
        args: &'a ast::FunctionArgs,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                for expr in arguments.iter() {
                    self.collect_prefab_calls_in_expr(expr, out);
                }
            }
            ast::FunctionArgs::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => {
                            self.collect_prefab_calls_in_expr(expression, out)
                        }
                        ast::Field::NameKey { value, .. } => {
                            self.collect_prefab_calls_in_expr(value, out)
                        }
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.collect_prefab_calls_in_expr(key, out);
                            self.collect_prefab_calls_in_expr(value, out);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_prefab_calls_in_expr<'a>(
        &self,
        expr: &'a ast::Expression,
        out: &mut Vec<&'a ast::FunctionCall>,
    ) {
        match expr {
            ast::Expression::FunctionCall(call) => {
                if is_named_call(call, "Prefab") {
                    out.push(call);
                }
                self.collect_prefab_calls_in_args(call, out);
            }
            ast::Expression::Function(_) => {}
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.collect_prefab_calls_in_expr(lhs, out);
                self.collect_prefab_calls_in_expr(rhs, out);
            }
            ast::Expression::UnaryOperator { expression, .. } => {
                self.collect_prefab_calls_in_expr(expression, out);
            }
            ast::Expression::Parentheses { expression, .. } => {
                self.collect_prefab_calls_in_expr(expression, out);
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => {
                            self.collect_prefab_calls_in_expr(expression, out)
                        }
                        ast::Field::NameKey { value, .. } => {
                            self.collect_prefab_calls_in_expr(value, out)
                        }
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.collect_prefab_calls_in_expr(key, out);
                            self.collect_prefab_calls_in_expr(value, out);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn parse_prefab_template(
        &self,
        call: &ast::FunctionCall,
        params: &[String],
    ) -> Option<PrefabTemplate> {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "Prefab" {
            return None;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return None;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 3 {
            return None;
        }
        let name = classify_template_name(args[0], params);
        let assets = self.classify_template_assets(args[2], params);
        Some(PrefabTemplate {
            name,
            assets,
            line: self.line(args[2]),
        })
    }

    fn classify_template_assets(
        &self,
        expr: &ast::Expression,
        params: &[String],
    ) -> TemplateAssets {
        match expr {
            ast::Expression::Var(ast::Var::Name(name)) => {
                let var = name.token().to_string();
                if params.iter().any(|p| p == &var) {
                    TemplateAssets::Param(var)
                } else {
                    TemplateAssets::Other(var)
                }
            }
            ast::Expression::FunctionCall(call) => {
                if let ast::Prefix::Name(prefix) = call.prefix() {
                    let factory_name = prefix.token().to_string();
                    if self.asset_factories.contains_key(&factory_name) {
                        let args = self.classify_template_args(call, params);
                        return TemplateAssets::FactoryCall {
                            name: factory_name,
                            args,
                        };
                    }
                }
                TemplateAssets::Other(expr.to_string().trim().to_string())
            }
            ast::Expression::TableConstructor(table) => {
                let entries = self.parse_template_asset_table(table, params);
                TemplateAssets::Inline(entries)
            }
            other => TemplateAssets::Other(other.to_string().trim().to_string()),
        }
    }

    fn classify_template_args(
        &self,
        call: &ast::FunctionCall,
        params: &[String],
    ) -> Vec<TemplateArg> {
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return Vec::new();
        };
        arguments
            .iter()
            .map(|expr| classify_template_arg(expr, params))
            .collect()
    }

    fn parse_template_asset_table(
        &self,
        table: &ast::TableConstructor,
        params: &[String],
    ) -> Vec<TemplateAssetEntry> {
        let mut entries = Vec::new();
        for field in table.fields() {
            let expr = match field {
                ast::Field::NoKey(expression) => expression,
                ast::Field::NameKey { value, .. } => value,
                ast::Field::ExpressionKey { value, .. } => value,
                _ => continue,
            };
            if let ast::Expression::FunctionCall(call) = expr {
                if let Some(entry) = self.parse_template_asset_call(call, params) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    fn parse_template_asset_call(
        &self,
        call: &ast::FunctionCall,
        params: &[String],
    ) -> Option<TemplateAssetEntry> {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "Asset" {
            return None;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        })) = suffixes.first()?
        else {
            return None;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 2 {
            return None;
        }
        let kind = match &args[0] {
            ast::Expression::String(s) => string_literal_text(&s.to_string()),
            _ => return None,
        };
        if kind != "ANIM" && kind != "DYNAMIC_ANIM" {
            return None;
        }
        let path_template = parse_path_template(args[1], params);
        Some(TemplateAssetEntry {
            kind,
            raw: args[1].to_string().trim().to_string(),
            line: self.line(call),
            path_template,
        })
    }

    fn walk_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            match stmt {
                ast::Stmt::LocalAssignment(assign) => {
                    let names: Vec<_> = assign.names().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (name, expr) in names.iter().zip(exprs.iter()) {
                        let var = name.token().to_string();
                        if let ast::Expression::TableConstructor(table) = expr {
                            let mut entries = self.parse_asset_table(table);
                            if let Some(existing) = self.asset_tables.get_mut(&var) {
                                existing.append(&mut entries);
                            } else {
                                self.asset_tables.insert(var.clone(), entries);
                            }
                        }
                        let is_prefab_factory_fn = matches!(expr, ast::Expression::Function(_))
                            && self.prefab_factories.contains_key(&var);
                        if !is_prefab_factory_fn {
                            self.scan_expr_for_prefabs(expr);
                        }
                    }
                }
                ast::Stmt::Assignment(assign) => {
                    let vars: Vec<_> = assign.variables().iter().collect();
                    let exprs: Vec<_> = assign.expressions().iter().collect();
                    for (var, expr) in vars.iter().zip(exprs.iter()) {
                        let name = match var {
                            ast::Var::Name(name) => name.token().to_string(),
                            _ => continue,
                        };
                        if let ast::Expression::TableConstructor(table) = expr {
                            let mut entries = self.parse_asset_table(table);
                            if let Some(existing) = self.asset_tables.get_mut(&name) {
                                existing.append(&mut entries);
                            } else {
                                self.asset_tables.insert(name.clone(), entries);
                            }
                        }
                        let is_prefab_factory_fn = matches!(expr, ast::Expression::Function(_))
                            && self.prefab_factories.contains_key(&name);
                        if !is_prefab_factory_fn {
                            self.scan_expr_for_prefabs(expr);
                        }
                    }
                }
                ast::Stmt::FunctionCall(call) => {
                    self.try_collect_table_insert(call);
                    self.scan_call_for_prefabs(call);
                }
                ast::Stmt::LocalFunction(local_fn) => {
                    let name = local_fn.name().to_string();
                    if !self.prefab_factories.contains_key(&name) {
                        self.walk_block(local_fn.body().block());
                    }
                }
                ast::Stmt::FunctionDeclaration(func_decl) => {
                    let name = func_decl.name().to_string();
                    if !self.prefab_factories.contains_key(&name) {
                        self.walk_block(func_decl.body().block());
                    }
                }
                ast::Stmt::If(if_stmt) => {
                    self.walk_block(if_stmt.block());
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.walk_block(else_if.block());
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.walk_block(else_block);
                    }
                }
                ast::Stmt::While(while_stmt) => self.walk_block(while_stmt.block()),
                ast::Stmt::Repeat(repeat_stmt) => self.walk_block(repeat_stmt.block()),
                ast::Stmt::NumericFor(for_stmt) => self.walk_block(for_stmt.block()),
                ast::Stmt::GenericFor(for_stmt) => self.walk_block(for_stmt.block()),
                ast::Stmt::Do(do_stmt) => self.walk_block(do_stmt.block()),
                _ => {}
            }
        }
        if let Some(ast::LastStmt::Return(ret)) = block.last_stmt() {
            for expr in ret.returns() {
                self.scan_expr_for_prefabs(expr);
            }
        }
    }

    fn parse_asset_table(&mut self, table: &ast::TableConstructor) -> Vec<AssetEntry> {
        let mut entries = Vec::new();
        for field in table.fields() {
            let expr = match field {
                ast::Field::NoKey(expression) => expression,
                ast::Field::NameKey { value, .. } => value,
                ast::Field::ExpressionKey { value, .. } => value,
                _ => continue,
            };
            if let ast::Expression::FunctionCall(call) = expr {
                if let Some(entry) = self.parse_asset_call(call) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    fn try_collect_table_insert(&mut self, call: &ast::FunctionCall) {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return;
        };
        if prefix.token().to_string() != "table" {
            return;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        // Find `table.insert(...)`.
        let has_insert = suffixes.iter().any(|s| {
            matches!(
                s,
                ast::Suffix::Index(ast::Index::Dot { name, .. })
                    if name.token().to_string() == "insert"
            )
        });
        if !has_insert {
            return;
        }
        for suffix in &suffixes {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
                arguments,
                ..
            })) = suffix
            {
                let args: Vec<_> = arguments.iter().collect();
                if args.len() < 2 {
                    continue;
                }
                let var = match &args[0] {
                    ast::Expression::Var(ast::Var::Name(name)) => name.token().to_string(),
                    _ => continue,
                };
                if let ast::Expression::FunctionCall(inner) = &args[1] {
                    if let Some(entry) = self.parse_asset_call(inner) {
                        self.asset_tables.entry(var).or_default().push(entry);
                    }
                }
            }
        }
    }

    fn parse_asset_call(&mut self, call: &ast::FunctionCall) -> Option<AssetEntry> {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "Asset" {
            return None;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        })) = suffixes.first()?
        else {
            return None;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 2 {
            return None;
        }
        let kind = match &args[0] {
            ast::Expression::String(s) => string_literal_text(&s.to_string()),
            _ => return None,
        };
        let line = self.line(call);
        if kind == "PKGREF" {
            // Skin `.dyn` references are intentionally out of scope for now,
            // but `.zip` build/package references are useful for rendering.
            if let ast::Expression::String(s) = &args[1] {
                let path = string_literal_text(&s.to_string());
                if path.ends_with(".dyn") {
                    self.skipped_pkgref_dyn += 1;
                    return None;
                }
                return Some(AssetEntry {
                    kind,
                    raw_path: path.clone(),
                    line,
                    normalized: Some(normalize_asset_path("PKGREF", &path)),
                });
            }
            return None;
        }
        if kind != "ANIM" && kind != "DYNAMIC_ANIM" {
            return None;
        }
        let normalized = match &args[1] {
            ast::Expression::String(s) => Some(normalize_asset_path(
                &kind,
                &string_literal_text(&s.to_string()),
            )),
            _ => None,
        };
        let raw = match &args[1] {
            ast::Expression::String(s) => string_literal_text(&s.to_string()),
            _ => args[1].to_string().trim().to_string(),
        };
        Some(AssetEntry {
            kind,
            raw_path: raw,
            line,
            normalized,
        })
    }

    fn scan_call_for_prefabs(&mut self, call: &ast::FunctionCall) {
        // Expand known prefab-returning factory calls (MakeAxe, item, pillar, ...).
        if let ast::Prefix::Name(prefix) = call.prefix() {
            let name = prefix.token().to_string();
            if self.prefab_factories.contains_key(&name) {
                self.instantiate_prefab_factory(&name, call);
                return;
            }
        }
        if self.try_parse_prefab_call(call, None) {
            return;
        }
        // Recurse into arguments for nested Prefab calls / table constructors.
        for suffix in call.suffixes() {
            if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                self.scan_args_for_prefabs(args);
            }
        }
    }

    fn instantiate_prefab_factory(&mut self, factory_name: &str, call: &ast::FunctionCall) {
        let Some(factory) = self.prefab_factories.get(factory_name).cloned() else {
            return;
        };
        let resolved_args = self.resolve_call_args(call);
        let mut bindings = BTreeMap::new();
        for (i, param) in factory.params.iter().enumerate() {
            if let Some(arg) = resolved_args.get(i) {
                bindings.insert(param.clone(), arg.clone());
            }
        }

        for template in &factory.templates {
            let prefab_name = self.resolve_template_name(&template.name, &bindings);
            let mut anims = Vec::new();
            let mut related_files = Vec::new();
            let mut unresolved = Vec::new();
            let mut asset_var = None;

            match &template.assets {
                TemplateAssets::Param(param) => {
                    if let Some(arg) = bindings.get(param) {
                        self.push_resolved_arg_as_assets(
                            arg,
                            &mut anims,
                            &mut related_files,
                            &mut unresolved,
                            &mut asset_var,
                        );
                    } else {
                        unresolved.push(UnresolvedRef {
                            kind: "ASSET_TABLE".to_string(),
                            raw: param.clone(),
                            line: template.line,
                        });
                    }
                }
                TemplateAssets::FactoryCall {
                    name: factory_name,
                    args,
                } => {
                    let mut resolved_factory_args = Vec::new();
                    for arg in args {
                        resolved_factory_args.push(self.resolve_template_arg(arg, &bindings));
                    }
                    let (entries, unrs) =
                        self.eval_asset_factory_resolved(factory_name, &resolved_factory_args);
                    anims.extend(entries.into_iter().map(|e| AnimRef {
                        kind: e.kind,
                        path: e.raw_path,
                        normalized: e.normalized.unwrap_or_default(),
                        exists: false,
                    }));
                    unresolved.extend(unrs);
                }
                TemplateAssets::Inline(entries) => {
                    let (concrete, unrs) = self.resolve_template_entries(entries, &bindings);
                    for e in concrete {
                        self.push_asset_entry(&e, &mut anims, &mut related_files, &mut unresolved);
                    }
                    unresolved.extend(unrs);
                }
                TemplateAssets::Other(raw) => {
                    unresolved.push(UnresolvedRef {
                        kind: "ASSET_TABLE".to_string(),
                        raw: raw.clone(),
                        line: template.line,
                    });
                }
            }

            self.prefabs.push(PrefabAnimRecord {
                prefab_file: String::new(),
                prefab_name,
                asset_var,
                anims,
                related_files,
                unresolved,
                content: AnimContent::default(),
            });
        }
    }

    fn resolve_call_args(&mut self, call: &ast::FunctionCall) -> Vec<ResolvedArg> {
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return Vec::new();
        };
        arguments
            .iter()
            .map(|expr| self.resolve_call_arg(expr))
            .collect()
    }

    fn resolve_call_arg(&mut self, expr: &ast::Expression) -> ResolvedArg {
        match expr {
            ast::Expression::String(s) => ResolvedArg::Str(string_literal_text(&s.to_string())),
            ast::Expression::Var(ast::Var::Name(name)) => {
                ResolvedArg::Var(name.token().to_string())
            }
            ast::Expression::TableConstructor(table) => {
                ResolvedArg::Table(self.parse_asset_table(table))
            }
            other => ResolvedArg::Other(other.to_string().trim().to_string()),
        }
    }

    fn resolve_template_name(
        &self,
        template: &TemplateName,
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> Option<String> {
        match template {
            TemplateName::Lit(s) => Some(s.clone()),
            TemplateName::Param(p) => bindings.get(p).and_then(|arg| self.resolve_arg_string(arg)),
            TemplateName::Other => None,
        }
    }

    fn resolve_template_arg(
        &self,
        arg: &TemplateArg,
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> ResolvedArg {
        match arg {
            TemplateArg::Lit(s) => ResolvedArg::Str(s.clone()),
            TemplateArg::Param(p) => bindings
                .get(p)
                .cloned()
                .unwrap_or(ResolvedArg::Other(p.clone())),
            TemplateArg::Other(raw) => ResolvedArg::Other(raw.clone()),
        }
    }

    fn resolve_arg_string(&self, arg: &ResolvedArg) -> Option<String> {
        match arg {
            ResolvedArg::Str(s) => Some(s.clone()),
            ResolvedArg::Var(v) => self.consts.get(v).cloned(),
            _ => None,
        }
    }

    fn push_resolved_arg_as_assets(
        &mut self,
        arg: &ResolvedArg,
        anims: &mut Vec<AnimRef>,
        related_files: &mut Vec<String>,
        unresolved: &mut Vec<UnresolvedRef>,
        asset_var: &mut Option<String>,
    ) {
        match arg {
            ResolvedArg::Var(v) => {
                *asset_var = Some(v.clone());
                if let Some(entries) = self.asset_tables.get(v) {
                    for entry in entries {
                        self.push_asset_entry(entry, anims, related_files, unresolved);
                    }
                } else {
                    unresolved.push(UnresolvedRef {
                        kind: "ASSET_TABLE".to_string(),
                        raw: v.clone(),
                        line: 0,
                    });
                }
            }
            ResolvedArg::Table(entries) => {
                for entry in entries {
                    self.push_asset_entry(entry, anims, related_files, unresolved);
                }
            }
            ResolvedArg::Str(s) => {
                unresolved.push(UnresolvedRef {
                    kind: "ASSET_TABLE".to_string(),
                    raw: s.clone(),
                    line: 0,
                });
            }
            ResolvedArg::Other(raw) => {
                unresolved.push(UnresolvedRef {
                    kind: "ASSET_TABLE".to_string(),
                    raw: raw.clone(),
                    line: 0,
                });
            }
        }
    }

    fn eval_asset_factory_call(
        &mut self,
        call: &ast::FunctionCall,
    ) -> (Vec<AssetEntry>, Vec<UnresolvedRef>) {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return (Vec::new(), Vec::new());
        };
        let factory_name = prefix.token().to_string();
        let args = self.resolve_call_args(call);
        self.eval_asset_factory_resolved(&factory_name, &args)
    }

    fn eval_asset_factory_resolved(
        &mut self,
        factory_name: &str,
        args: &[ResolvedArg],
    ) -> (Vec<AssetEntry>, Vec<UnresolvedRef>) {
        let Some(factory) = self.asset_factories.get(factory_name).cloned() else {
            return (
                Vec::new(),
                vec![UnresolvedRef {
                    kind: "ASSET_FACTORY".to_string(),
                    raw: factory_name.to_string(),
                    line: 0,
                }],
            );
        };
        let mut bindings = BTreeMap::new();
        for (i, param) in factory.params.iter().enumerate() {
            if let Some(arg) = args.get(i) {
                bindings.insert(param.clone(), arg.clone());
            }
        }
        self.resolve_template_entries(&factory.entries, &bindings)
    }

    fn resolve_template_entries(
        &mut self,
        entries: &[TemplateAssetEntry],
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> (Vec<AssetEntry>, Vec<UnresolvedRef>) {
        let mut anims = Vec::new();
        let mut unresolved = Vec::new();
        for entry in entries {
            let Some(path_template) = &entry.path_template else {
                unresolved.push(UnresolvedRef {
                    kind: entry.kind.clone(),
                    raw: entry.raw.clone(),
                    line: entry.line,
                });
                continue;
            };
            let Some(raw_path) = self.eval_path_template(path_template, bindings) else {
                unresolved.push(UnresolvedRef {
                    kind: entry.kind.clone(),
                    raw: entry.raw.clone(),
                    line: entry.line,
                });
                continue;
            };
            anims.push(AssetEntry {
                kind: entry.kind.clone(),
                raw_path: raw_path.clone(),
                line: entry.line,
                normalized: Some(normalize_asset_path(&entry.kind, &raw_path)),
            });
        }
        (anims, unresolved)
    }

    fn eval_path_template(
        &self,
        template: &PathTemplate,
        bindings: &BTreeMap<String, ResolvedArg>,
    ) -> Option<String> {
        match template {
            PathTemplate::Literal(s) => Some(s.clone()),
            PathTemplate::Concat(parts) => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        PathPart::Lit(s) => out.push_str(s),
                        PathPart::Param(p) => {
                            let arg = bindings.get(p)?;
                            out.push_str(&self.resolve_arg_string(arg)?);
                        }
                    }
                }
                Some(out)
            }
        }
    }

    fn scan_expr_for_prefabs(&mut self, expr: &ast::Expression) {
        match expr {
            ast::Expression::FunctionCall(call) => self.scan_call_for_prefabs(call),
            ast::Expression::Function(func) => self.walk_block(func.body().block()),
            ast::Expression::BinaryOperator { lhs, rhs, .. } => {
                self.scan_expr_for_prefabs(lhs);
                self.scan_expr_for_prefabs(rhs);
            }
            ast::Expression::UnaryOperator { expression, .. } => {
                self.scan_expr_for_prefabs(expression);
            }
            ast::Expression::Parentheses { expression, .. } => {
                self.scan_expr_for_prefabs(expression);
            }
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => self.scan_expr_for_prefabs(expression),
                        ast::Field::NameKey { value, .. } => self.scan_expr_for_prefabs(value),
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.scan_expr_for_prefabs(key);
                            self.scan_expr_for_prefabs(value);
                        }
                        _ => {}
                    }
                }
            }
            ast::Expression::Var(ast::Var::Expression(vex)) => {
                for suffix in vex.suffixes() {
                    if let ast::Suffix::Call(ast::Call::AnonymousCall(args)) = suffix {
                        self.scan_args_for_prefabs(args);
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_args_for_prefabs(&mut self, args: &ast::FunctionArgs) {
        match args {
            ast::FunctionArgs::Parentheses { arguments, .. } => {
                for expr in arguments.iter() {
                    self.scan_expr_for_prefabs(expr);
                }
            }
            ast::FunctionArgs::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        ast::Field::NoKey(expression) => self.scan_expr_for_prefabs(expression),
                        ast::Field::NameKey { value, .. } => self.scan_expr_for_prefabs(value),
                        ast::Field::ExpressionKey { key, value, .. } => {
                            self.scan_expr_for_prefabs(key);
                            self.scan_expr_for_prefabs(value);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn try_parse_prefab_call(
        &mut self,
        call: &ast::FunctionCall,
        fallback_file: Option<&str>,
    ) -> bool {
        let ast::Prefix::Name(prefix) = call.prefix() else {
            return false;
        };
        if prefix.token().to_string() != "Prefab" {
            return false;
        }
        let suffixes: Vec<_> = call.suffixes().collect();
        let Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) = suffixes.first()
        else {
            return false;
        };
        let args: Vec<_> = arguments.iter().collect();
        if args.len() < 3 {
            return false;
        }

        let prefab_name = match &args[0] {
            ast::Expression::String(s) => Some(string_literal_text(&s.to_string())),
            _ => None,
        };

        let mut anims = Vec::new();
        let mut related_files = Vec::new();
        let mut unresolved = Vec::new();
        let mut asset_var = None;

        match &args[2] {
            ast::Expression::Var(ast::Var::Name(name)) => {
                let var = name.token().to_string();
                asset_var = Some(var.clone());
                if let Some(entries) = self.asset_tables.get(&var) {
                    for entry in entries {
                        self.push_asset_entry(
                            entry,
                            &mut anims,
                            &mut related_files,
                            &mut unresolved,
                        );
                    }
                } else {
                    unresolved.push(UnresolvedRef {
                        kind: "ASSET_TABLE".to_string(),
                        raw: var,
                        line: self.line(args[2]),
                    });
                }
            }
            ast::Expression::TableConstructor(table) => {
                for entry in self.parse_asset_table(table) {
                    self.push_asset_entry(&entry, &mut anims, &mut related_files, &mut unresolved);
                }
            }
            ast::Expression::FunctionCall(call) => {
                let (entries, unrs) = self.eval_asset_factory_call(call);
                for entry in entries {
                    self.push_asset_entry(&entry, &mut anims, &mut related_files, &mut unresolved);
                }
                unresolved.extend(unrs);
            }
            other => {
                unresolved.push(UnresolvedRef {
                    kind: "ASSET_TABLE".to_string(),
                    raw: other.to_string().trim().to_string(),
                    line: self.line(args[2]),
                });
            }
        }

        self.prefabs.push(PrefabAnimRecord {
            prefab_file: fallback_file.unwrap_or_default().to_string(),
            prefab_name,
            asset_var,
            anims,
            related_files,
            unresolved,
            content: AnimContent::default(),
        });
        true
    }

    fn push_asset_entry(
        &self,
        entry: &AssetEntry,
        anims: &mut Vec<AnimRef>,
        related_files: &mut Vec<String>,
        unresolved: &mut Vec<UnresolvedRef>,
    ) {
        match &entry.normalized {
            Some(normalized) if entry.kind == "ANIM" || entry.kind == "DYNAMIC_ANIM" => {
                anims.push(AnimRef {
                    kind: entry.kind.clone(),
                    path: entry.raw_path.clone(),
                    normalized: normalized.clone(),
                    exists: false, // filled later by run_index
                });
            }
            Some(normalized) if entry.kind == "PKGREF" => {
                related_files.push(normalized.clone());
            }
            Some(_) | None => unresolved.push(UnresolvedRef {
                kind: entry.kind.clone(),
                raw: entry.raw_path.clone(),
                line: entry.line,
            }),
        }
    }

    fn line(&self, node: &impl Node) -> u32 {
        node.start_position()
            .map(|p| line_of(self.source, p.bytes()))
            .unwrap_or(0)
    }
}

struct ScannedFile {
    prefabs: Vec<PrefabAnimRecord>,
    skipped_pkgref_dyn: usize,
}

pub(crate) fn line_of(source: &str, byte: usize) -> u32 {
    source.as_bytes()[..byte.min(source.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count() as u32
        + 1
}

/// Extract the text of a Lua string literal (`"x"`, `'x'`, `[[x]]`).
pub(crate) fn string_literal_text(raw: &str) -> String {
    let t = raw.trim();
    for q in ['"', '\''] {
        if let Some(inner) = t.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner.to_string();
        }
        if let Some(inner) = t.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
            return inner.trim().to_string();
        }
    }
    t.to_string()
}

fn normalize_asset_path(kind: &str, raw: &str) -> String {
    let mut p = raw.trim().to_string();
    if let Some(stripped) = p.strip_prefix("anim/") {
        p = stripped.to_string();
    } else if let Some(stripped) = p.strip_prefix("/anim/") {
        p = stripped.to_string();
    }
    // DYNAMIC_ANIM uses `anim/dynamic/xxx.zip`; ANIM usually uses root zips,
    // but if it somehow contains dynamic/ we preserve that as a valid anim-relative path.
    let _ = kind;
    p
}

fn load_anim_content(anim_root: &Path, normalized: &str) -> Result<AnimContent> {
    let bytes = std::fs::read(anim_root.join(normalized))?;
    let data = super::archive::parse_archive_bytes(&bytes)?;
    let mut content = AnimContent::default();
    if let Some(anim) = &data.anim {
        for bank in &anim.banks {
            content.banks.push(bank.name.clone());
            for animation in &bank.animations {
                content.animations.push(animation.name.clone());
            }
        }
    }
    if let Some(build) = &data.build {
        content.builds.push(build.name.clone());
        content.atlases.extend(build.atlases.iter().cloned());
        content
            .symbols
            .extend(build.symbols.iter().map(|s| s.name.clone()));
    }
    sort_dedup_content(&mut content);
    Ok(content)
}

fn merge_content(target: &mut AnimContent, src: &AnimContent) {
    target.banks.extend(src.banks.iter().cloned());
    target.animations.extend(src.animations.iter().cloned());
    target.builds.extend(src.builds.iter().cloned());
    target.symbols.extend(src.symbols.iter().cloned());
    target.atlases.extend(src.atlases.iter().cloned());
}

fn sort_dedup_content(content: &mut AnimContent) {
    for v in [
        &mut content.banks,
        &mut content.animations,
        &mut content.builds,
        &mut content.symbols,
        &mut content.atlases,
    ] {
        v.sort();
        v.dedup();
    }
}

fn is_named_call(call: &ast::FunctionCall, name: &str) -> bool {
    matches!(&call.prefix(), ast::Prefix::Name(prefix) if prefix.token().to_string() == name)
}

fn classify_template_name(expr: &ast::Expression, params: &[String]) -> TemplateName {
    match expr {
        ast::Expression::String(s) => TemplateName::Lit(string_literal_text(&s.to_string())),
        ast::Expression::Var(ast::Var::Name(name)) => {
            let var = name.token().to_string();
            if params.iter().any(|p| p == &var) {
                TemplateName::Param(var)
            } else {
                TemplateName::Other
            }
        }
        other => {
            let _ = other;
            TemplateName::Other
        }
    }
}

fn classify_template_arg(expr: &ast::Expression, params: &[String]) -> TemplateArg {
    match expr {
        ast::Expression::String(s) => TemplateArg::Lit(string_literal_text(&s.to_string())),
        ast::Expression::Var(ast::Var::Name(name)) => {
            let var = name.token().to_string();
            if params.iter().any(|p| p == &var) {
                TemplateArg::Param(var)
            } else {
                TemplateArg::Other(var)
            }
        }
        other => TemplateArg::Other(other.to_string().trim().to_string()),
    }
}

fn parse_path_template(expr: &ast::Expression, params: &[String]) -> Option<PathTemplate> {
    match expr {
        ast::Expression::String(s) => {
            Some(PathTemplate::Literal(string_literal_text(&s.to_string())))
        }
        ast::Expression::Parentheses { expression, .. } => parse_path_template(expression, params),
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            if binop.to_string().trim() != ".." {
                return None;
            }
            let mut parts = Vec::new();
            if !flatten_path_parts(lhs, params, &mut parts) {
                return None;
            }
            if !flatten_path_parts(rhs, params, &mut parts) {
                return None;
            }
            Some(PathTemplate::Concat(parts))
        }
        _ => None,
    }
}

fn flatten_path_parts(expr: &ast::Expression, params: &[String], out: &mut Vec<PathPart>) -> bool {
    match expr {
        ast::Expression::String(s) => {
            out.push(PathPart::Lit(string_literal_text(&s.to_string())));
            true
        }
        ast::Expression::Var(ast::Var::Name(name)) => {
            let var = name.token().to_string();
            if params.iter().any(|p| p == &var) {
                out.push(PathPart::Param(var));
                true
            } else {
                false
            }
        }
        ast::Expression::Parentheses { expression, .. } => {
            flatten_path_parts(expression, params, out)
        }
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            if binop.to_string().trim() != ".." {
                return false;
            }
            flatten_path_parts(lhs, params, out) && flatten_path_parts(rhs, params, out)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_src(src: &str) -> ScannedFile {
        Scanner::new(src).scan("prefabs/test.lua".to_string())
    }

    #[test]
    fn simple_prefab_asset() {
        let src = r#"
local assets = {
    Asset("ANIM", "anim/foo.zip"),
}
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        let p = &scanned.prefabs[0];
        assert_eq!(p.prefab_name.as_deref(), Some("foo"));
        assert_eq!(p.asset_var.as_deref(), Some("assets"));
        assert_eq!(p.anims.len(), 1);
        assert_eq!(p.anims[0].normalized, "foo.zip");
        assert!(p.unresolved.is_empty());
    }

    #[test]
    fn multiple_prefabs_share_assets() {
        let src = r#"
local assets = { Asset("ANIM", "anim/shared.zip") }
local function a() end
local function b() end
return Prefab("a", a, assets), Prefab("b", b, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 2);
        assert!(scanned.prefabs.iter().all(|p| p.anims.len() == 1));
        assert!(scanned
            .prefabs
            .iter()
            .all(|p| p.anims[0].normalized == "shared.zip"));
    }

    #[test]
    fn dynamic_asset_is_unresolved() {
        let src = r#"
local assets = { Asset("ANIM", "anim/" .. build .. ".zip") }
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        let p = &scanned.prefabs[0];
        assert!(p.anims.is_empty());
        assert_eq!(p.unresolved.len(), 1);
        assert_eq!(p.unresolved[0].kind, "ANIM");
    }

    #[test]
    fn dynamic_anim_normalized_under_dynamic_dir() {
        let src = r#"
local assets = { Asset("DYNAMIC_ANIM", "anim/dynamic/body.zip") }
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        let p = &scanned.prefabs[0];
        assert_eq!(p.anims[0].normalized, "dynamic/body.zip");
    }

    #[test]
    fn inline_asset_table_in_prefab() {
        let src = r#"
local function fn() end
return Prefab("foo", fn, { Asset("ANIM", "anim/bar.zip") })
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        assert_eq!(scanned.prefabs[0].anims[0].normalized, "bar.zip");
    }

    #[test]
    fn pkgref_dyn_is_skipped() {
        let src = r#"
local assets = {
    Asset("PKGREF", "anim/dynamic/skin.dyn"),
    Asset("ANIM", "anim/base.zip"),
}
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.skipped_pkgref_dyn, 1);
        let p = &scanned.prefabs[0];
        assert_eq!(p.anims.len(), 1);
        assert_eq!(p.anims[0].normalized, "base.zip");
    }

    #[test]
    fn pkgref_zip_is_related_file() {
        let src = r#"
local assets = {
    Asset("PKGREF", "anim/base_build.zip"),
    Asset("ANIM", "anim/anim_only.zip"),
}
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        let p = &scanned.prefabs[0];
        assert_eq!(p.anims.len(), 1);
        assert_eq!(p.anims[0].normalized, "anim_only.zip");
        assert_eq!(p.related_files, vec!["base_build.zip"]);
    }

    #[test]
    fn global_asset_table_assignment() {
        let src = r#"
assets = { Asset("ANIM", "anim/global.zip") }
local function fn() end
return Prefab("foo", fn, assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        assert_eq!(scanned.prefabs[0].anims[0].normalized, "global.zip");
    }

    #[test]
    fn normalize_asset_paths() {
        assert_eq!(normalize_asset_path("ANIM", "anim/foo.zip"), "foo.zip");
        assert_eq!(
            normalize_asset_path("DYNAMIC_ANIM", "anim/dynamic/bar.zip"),
            "dynamic/bar.zip"
        );
    }

    #[test]
    fn make_axe_factory_expands_to_variants() {
        let src = r#"
local assets = { Asset("ANIM", "anim/axe.zip") }
local golden_assets = { Asset("ANIM", "anim/goldenaxe.zip") }
local function fn() end
local function MakeAxe(name, common, master, data, _assets, _prefabs)
    return Prefab(name, fn, _assets, _prefabs)
end
return MakeAxe("axe", nil, nil, nil, assets),
       MakeAxe("goldenaxe", nil, nil, nil, golden_assets)
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 2);
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("axe")
                && p.anims.iter().any(|a| a.normalized == "axe.zip")));
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("goldenaxe")
                && p.anims.iter().any(|a| a.normalized == "goldenaxe.zip")));
    }

    #[test]
    fn makeassetlist_factory_expands_dynamic_path() {
        let src = r#"
local function makeassetlist(name)
    return { Asset("ANIM", "anim/" .. name .. ".zip") }
end
local function fn() end
local function item(name)
    return Prefab(name, fn, makeassetlist(name))
end
return item("pillar_ruins"), item("pillar_algae")
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 2);
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("pillar_ruins")
                && p.anims.iter().any(|a| a.normalized == "pillar_ruins.zip")));
        assert!(scanned
            .prefabs
            .iter()
            .any(|p| p.prefab_name.as_deref() == Some("pillar_algae")
                && p.anims.iter().any(|a| a.normalized == "pillar_algae.zip")));
    }

    #[test]
    fn direct_asset_factory_call_in_prefab() {
        let src = r#"
local function makeassetlist(name)
    return { Asset("ANIM", "anim/" .. name .. ".zip") }
end
local function fn() end
return Prefab("foo", fn, makeassetlist("foo"))
"#;
        let scanned = scan_src(src);
        assert_eq!(scanned.prefabs.len(), 1);
        assert_eq!(scanned.prefabs[0].anims[0].normalized, "foo.zip");
    }

    #[test]
    fn sort_dedup_content_helper() {
        let mut c = AnimContent {
            banks: vec!["b".to_string(), "a".to_string(), "b".to_string()],
            animations: vec!["x".to_string(), "x".to_string()],
            builds: vec![],
            symbols: vec![],
            atlases: vec![],
        };
        sort_dedup_content(&mut c);
        assert_eq!(c.banks, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(c.animations, vec!["x".to_string()]);
    }
}
