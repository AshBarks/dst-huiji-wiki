//! `Follower:FollowSymbol` 关系提取（Tier A/B：字面量、同文件工厂与实体级追踪）。
//!
//! 游戏里大量 FX 实体不是独立 prefab，而是在 prefab 的 `fn()` 里本地创建的
//! “跟随实体”：先创建实体并 `AnimState:SetBank/SetBuild/PlayAnimation`，
//! 再 `cape.Follower:FollowSymbol(inst.GUID, "cape_front_swap", ...)` 把
//! 实体贴到主动画的挂点 symbol 上。`anim-index` 用本模块的结果把“无贴图
//! 挂点”和“由哪个动画补全”连起来，供 WebUI 合成预览。
//!
//! 覆盖模式：
//!
//! - 实体级内联：`local fx = CreateEntity()` + 同函数内的 `fx.AnimState:*`；
//! - 本地工厂：`local cape = CreateCapeFx()`，工厂体内配置 `inst.AnimState`；
//! - 字段绑定：`inst.head = CreateHead()`、`inst.swapfx = SpawnPrefab("x")`；
//! - 参数化工厂：`AddFollowSymbol(inst, sym, anim)` 包装函数；
//! - 同文件 `SpawnPrefab("x")` → 同文件 `Prefab("x", fn, ...)`。
//!
//! 同时提取运行时的可静态化参数：`SetFinalOffset`、字面量
//! `SetMultColour`/`SetAddColour`、`SetFrame(math.random(...))`、
//! `FollowSymbol` 第 7 参数（推断为跟随帧同步）。
//!
//! 跨文件 `SpawnPrefab("x")` 在记录里保留 `spawn_prefab`，由 `anim-index`
//! 的第二遍全局回填（`resolve_cross_file_follows`）补全；动态拼接、
//! 动态 `SpawnPrefab(expr)`、状态图内的调用仍作为 `animation = None` 的
//! 未解析记录保留，交给前端手动补。

use crate::error::{Error, Result};
use full_moon::ast;
use full_moon::visitors::Visitor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// `anim-index` 跟随关系数据的版本；缺失该字段的旧产物视为 0。
pub const FOLLOW_INDEX_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FollowConfidence {
    #[default]
    Static,
    Resolved,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FollowFxRecord {
    pub anchor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default, skip_serializing_if = "offset_is_zero")]
    pub offset: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_begin: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_end: Option<u32>,
    /// `AnimState:SetFinalOffset`（实体级绘制前后偏移）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_offset: Option<i32>,
    /// 字面量 `AnimState:SetMultColour(r, g, b, a)`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mult_colour: Option<[f32; 4]>,
    /// 字面量 `AnimState:SetAddColour(r, g, b, a)`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_colour: Option<[f32; 4]>,
    /// `FollowSymbol` 第 7 参数（`useframe`，推断为跟随帧同步）。
    #[serde(default, skip_serializing_if = "is_false")]
    pub sync_frame: bool,
    /// 工厂内 `SetFrame(math.random(...))`：预览可用随机初始相位。
    #[serde(default, skip_serializing_if = "is_false")]
    pub random_frame: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub receiver: String,
    /// Receiver 绑定的 `SpawnPrefab("x")` 目标（未解析时用于跨文件回填）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_prefab: Option<String>,
    pub confidence: FollowConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab: Option<String>,
}

fn offset_is_zero(offset: &[f32; 2]) -> bool {
    offset[0] == 0.0 && offset[1] == 0.0
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn parse_follow_symbols(source: &str) -> Result<Vec<FollowFxRecord>> {
    parse_follow_symbols_in(source, None)
}

pub fn parse_follow_symbols_in(source: &str, prefab: Option<&str>) -> Result<Vec<FollowFxRecord>> {
    Ok(parse_follow_symbols_and_entities(source, prefab)?.0)
}

/// Same as [`parse_follow_symbols_in`], but also returns the static anim
/// defaults of every `Prefab("x", fn, ...)` defined in this file. `anim-index`
/// collects them into a global map and resolves cross-file `SpawnPrefab("x")`
/// follow targets in a second pass.
pub fn parse_follow_symbols_and_entities(
    source: &str,
    prefab: Option<&str>,
) -> Result<(Vec<FollowFxRecord>, BTreeMap<String, PrefabEntityConfig>)> {
    let ast = full_moon::parse(source).map_err(Error::LuaParse)?;
    let mut collector = FunctionCollector::default();
    collector.visit_ast(&ast);
    let entities = prefab_entities(&collector);
    let prefab_fns: BTreeSet<String> = collector.prefabs.values().cloned().collect();
    let mut resolver = FollowResolver {
        funcs: collector.funcs,
        bindings: collector.bindings,
        consts: collector.consts,
        prefabs: collector.prefabs,
        prefab_fns,
        prefab: prefab.map(str::to_string),
        records: Vec::new(),
        stack: Vec::new(),
    };
    resolver.visit_ast(&ast);
    Ok((dedup(resolver.records), entities))
}

fn prefab_entities(collector: &FunctionCollector) -> BTreeMap<String, PrefabEntityConfig> {
    collector
        .prefabs
        .iter()
        .filter_map(|(prefab_name, function)| {
            let func = collector.funcs.get(function)?;
            let entity = func.entities.get("inst").or_else(|| {
                if func.entities.len() == 1 {
                    func.entities.values().next()
                } else {
                    None
                }
            })?;
            Some((prefab_name.clone(), PrefabEntityConfig::from_entity(entity)))
        })
        .collect()
}

/// Resolves records whose `receiver` is a cross-file `SpawnPrefab("x")` by
/// copying `x`'s static bank/build/animation defaults. Only fills missing
/// fields (explicit call-site values win) and counts resolved records.
pub fn resolve_cross_file_follows(
    records: &mut [FollowFxRecord],
    entities: &BTreeMap<String, PrefabEntityConfig>,
) -> usize {
    let mut resolved = 0;
    for record in records.iter_mut() {
        if record.animation.is_some() {
            continue;
        }
        let Some(name) = record.spawn_prefab.as_deref() else {
            continue;
        };
        let Some(def) = entities.get(name) else {
            continue;
        };
        let Some(animation) = def.animation.clone() else {
            continue;
        };
        if record.bank.is_none() {
            record.bank = def.bank.clone();
        }
        if record.build.is_none() {
            record.build = def.build.clone();
        }
        record.animation = Some(animation);
        if record.final_offset.is_none() {
            record.final_offset = def.final_offset;
        }
        if record.mult_colour.is_none() {
            record.mult_colour = def.mult_colour;
        }
        if record.add_colour.is_none() {
            record.add_colour = def.add_colour;
        }
        record.random_frame |= def.random_frame;
        record.confidence = FollowConfidence::Resolved;
        resolved += 1;
    }
    resolved
}

/// Static anim defaults of one `Prefab("x", fn, ...)` definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrefabEntityConfig {
    pub bank: Option<String>,
    pub build: Option<String>,
    pub animation: Option<String>,
    pub final_offset: Option<i32>,
    pub mult_colour: Option<[f32; 4]>,
    pub add_colour: Option<[f32; 4]>,
    pub random_frame: bool,
}

impl PrefabEntityConfig {
    fn from_entity(entity: &EntityAnim) -> Self {
        Self {
            bank: entity.bank.clone(),
            build: entity.build.clone(),
            animation: entity.anim.clone(),
            final_offset: entity.final_offset,
            mult_colour: entity.mult_colour,
            add_colour: entity.add_colour,
            random_frame: entity.random_frame,
        }
    }
}

fn dedup(records: Vec<FollowFxRecord>) -> Vec<FollowFxRecord> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for record in records {
        let key = [
            record.anchor.to_lowercase(),
            record.bank.clone().unwrap_or_default().to_lowercase(),
            record.build.clone().unwrap_or_default().to_lowercase(),
            record.animation.clone().unwrap_or_default().to_lowercase(),
            format!("{:?}", record.offset.map(f32::to_bits)),
            format!("{:?}", (record.frame_begin, record.frame_end)),
            format!("{:?}", record.final_offset),
            format!("{:?}", record.mult_colour),
            format!("{:?}", record.add_colour),
            format!("{}-{}", record.sync_frame, record.random_frame),
        ]
        .join("\u{1f}");
        if seen.insert(key) {
            out.push(record);
        }
    }
    out.sort_by(|a, b| {
        a.anchor
            .to_lowercase()
            .cmp(&b.anchor.to_lowercase())
            .then_with(|| {
                a.animation
                    .clone()
                    .unwrap_or_default()
                    .to_lowercase()
                    .cmp(&b.animation.clone().unwrap_or_default().to_lowercase())
            })
    });
    out
}

#[derive(Debug, Clone, Default)]
struct EntityAnim {
    bank: Option<String>,
    build: Option<String>,
    anim: Option<String>,
    anim_param: Option<usize>,
    final_offset: Option<i32>,
    mult_colour: Option<[f32; 4]>,
    add_colour: Option<[f32; 4]>,
    random_frame: bool,
}

impl EntityAnim {
    fn merge(&mut self, other: &EntityAnim) {
        if other.bank.is_some() {
            self.bank = other.bank.clone();
        }
        if other.build.is_some() {
            self.build = other.build.clone();
        }
        if other.anim.is_some() {
            self.anim = other.anim.clone();
        }
        if other.anim_param.is_some() {
            self.anim_param = other.anim_param;
        }
        if other.final_offset.is_some() {
            self.final_offset = other.final_offset;
        }
        if other.mult_colour.is_some() {
            self.mult_colour = other.mult_colour;
        }
        if other.add_colour.is_some() {
            self.add_colour = other.add_colour;
        }
        self.random_frame |= other.random_frame;
    }
}

#[derive(Debug, Clone, Default)]
struct FuncInfo {
    params: Vec<String>,
    entities: BTreeMap<String, EntityAnim>,
    follow_symbol_param: Option<usize>,
    follow_path: Option<String>,
    follow_offset: (Option<f32>, Option<f32>),
    follow_frames: (Option<u32>, Option<u32>),
    follow_sync: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Binding {
    Factory(String),
    Spawn(String),
    Alias(String),
}

#[derive(Default)]
struct FunctionCollector {
    funcs: BTreeMap<String, FuncInfo>,
    bindings: BTreeMap<String, Binding>,
    consts: BTreeMap<String, String>,
    anim_aliases: BTreeMap<String, String>,
    prefabs: BTreeMap<String, String>,
    stack: Vec<(String, usize)>,
    pending: Vec<String>,
    body_depth: usize,
}

impl FunctionCollector {
    fn push_pending(&mut self, name: String, params: Vec<String>) {
        self.funcs.entry(name.clone()).or_default().params = params;
        self.pending.push(name);
    }

    fn visit_binding(&mut self, name: &str, expr: &ast::Expression) {
        match expr {
            ast::Expression::String(token) => {
                self.consts
                    .insert(name.to_string(), extract_string_literal(&token.to_string()));
            }
            ast::Expression::Function(func) => {
                self.push_pending(name.to_string(), param_names(func.body()));
            }
            ast::Expression::FunctionCall(call) => {
                if let Some(callee) = simple_callee(call) {
                    if callee == "SpawnPrefab" {
                        if let Some(spawn) = first_string_arg(call) {
                            self.bindings
                                .insert(name.to_string(), Binding::Spawn(spawn));
                            return;
                        }
                    }
                    self.bindings
                        .insert(name.to_string(), Binding::Factory(callee));
                }
            }
            ast::Expression::Var(var) => {
                if let Some(path) = var_path(var) {
                    if path.ends_with(".AnimState") {
                        let target = path.trim_end_matches(".AnimState").to_string();
                        self.anim_aliases.insert(name.to_string(), target);
                    } else {
                        self.bindings.insert(name.to_string(), Binding::Alias(path));
                    }
                }
            }
            // Table constructors commonly hold the FX binding:
            // `inst.frostbreath = { fx = SpawnPrefab("frostbreath") }`.
            // Record `inst.frostbreath.fx` so cross-file lookup can happen.
            ast::Expression::TableConstructor(table) => {
                for field in table.fields() {
                    let (Some(key), Some(value)) =
                        (table_field_key(field), table_field_value(field))
                    else {
                        continue;
                    };
                    let child = format!("{name}.{key}");
                    self.visit_binding(&child, value);
                }
            }
            _ => {}
        }
    }
}

impl Visitor for FunctionCollector {
    fn visit_local_function(&mut self, node: &ast::LocalFunction) {
        let name = node.name().to_string();
        let params = param_names(node.body());
        self.push_pending(name, params);
    }

    fn visit_function_declaration(&mut self, node: &ast::FunctionDeclaration) {
        let name = node.name().to_string();
        let params = param_names(node.body());
        self.push_pending(name, params);
    }

    fn visit_local_assignment(&mut self, node: &ast::LocalAssignment) {
        let names: Vec<String> = node.names().iter().map(|n| n.token().to_string()).collect();
        let exprs: Vec<&ast::Expression> = node.expressions().iter().collect();
        for (name, expr) in names.iter().zip(exprs.iter()) {
            self.visit_binding(name, expr);
        }
    }

    fn visit_assignment(&mut self, node: &ast::Assignment) {
        let vars: Vec<String> = node.variables().iter().filter_map(var_path).collect();
        let exprs: Vec<&ast::Expression> = node.expressions().iter().collect();
        for (name, expr) in vars.iter().zip(exprs.iter()) {
            self.visit_binding(name, expr);
        }
    }

    fn visit_function_body(&mut self, _node: &ast::FunctionBody) {
        self.body_depth += 1;
        if let Some(name) = self.pending.pop() {
            self.stack.push((name, self.body_depth));
        }
    }

    fn visit_function_body_end(&mut self, _node: &ast::FunctionBody) {
        if self
            .stack
            .last()
            .is_some_and(|(_, depth)| *depth == self.body_depth)
        {
            self.stack.pop();
        }
        self.body_depth = self.body_depth.saturating_sub(1);
    }

    fn visit_function_call(&mut self, call: &ast::FunctionCall) {
        if let Some(callee) = simple_callee(call) {
            if callee == "Prefab" {
                self.collect_prefab(call);
            }
        }
        let Some(name) = self.stack.last().map(|(name, _)| name.clone()) else {
            return;
        };
        let consts = &self.consts;
        let aliases = &self.anim_aliases;
        let Some(func) = self.funcs.get_mut(&name) else {
            return;
        };
        if let Some((entity_path, method_name, args)) = anim_state_call(call, aliases) {
            let anim_param = if method_name == "PlayAnimation" {
                args.first().and_then(|arg| param_index(func, arg))
            } else {
                None
            };
            let entity = func.entities.entry(entity_path).or_default();
            apply_anim_state_method(entity, &method_name, &args, consts, anim_param);
            return;
        }
        if let Some((path, method)) = follow_symbol_call(call) {
            let args = method_args(method);
            if let Some(path) = path {
                func.follow_path = Some(path);
            }
            collect_follow_param(func, &args);
        }
    }
}

impl FunctionCollector {
    fn collect_prefab(&mut self, call: &ast::FunctionCall) {
        let args = call_args(call);
        let Some(ast::Expression::String(name)) = args.first() else {
            return;
        };
        let Some(function) = args.get(1).and_then(|expr| match expr {
            ast::Expression::Var(ast::Var::Name(name)) => Some(name.token().to_string()),
            _ => None,
        }) else {
            return;
        };
        self.prefabs
            .insert(extract_string_literal(&name.to_string()), function);
    }
}

struct FollowResolver {
    funcs: BTreeMap<String, FuncInfo>,
    bindings: BTreeMap<String, Binding>,
    consts: BTreeMap<String, String>,
    prefabs: BTreeMap<String, String>,
    prefab_fns: BTreeSet<String>,
    prefab: Option<String>,
    records: Vec<FollowFxRecord>,
    stack: Vec<String>,
}

struct RecordInput<'a> {
    receiver: &'a str,
    anchor: &'a str,
    entity: EntityAnim,
    animation: Option<String>,
    offset: [f32; 2],
    frame_begin: Option<u32>,
    frame_end: Option<u32>,
    sync_frame: bool,
    confidence: FollowConfidence,
}

impl FollowResolver {
    fn current_func(&self) -> Option<&FuncInfo> {
        self.stack.last().and_then(|name| self.funcs.get(name))
    }

    fn function_entity(&self, callee: &str, path_hint: Option<&str>) -> Option<EntityAnim> {
        let func = self.funcs.get(callee)?;
        if let Some(path) = path_hint {
            if let Some(entity) = func.entities.get(path) {
                return Some(entity.clone());
            }
        }
        if let Some(entity) = func.entities.get("inst") {
            return Some(entity.clone());
        }
        if func.entities.len() == 1 {
            return func.entities.values().next().cloned();
        }
        None
    }

    fn single_prefab_entity(&self) -> Option<EntityAnim> {
        let mut found: Option<EntityAnim> = None;
        for name in &self.prefab_fns {
            if let Some(entity) = self.funcs.get(name).and_then(|f| f.entities.get("inst")) {
                if found.is_some() {
                    return None;
                }
                found = Some(entity.clone());
            }
        }
        found
    }

    fn spawn_prefab_for(&self, path: &str) -> Option<String> {
        match self.bindings.get(path) {
            Some(Binding::Spawn(name)) => Some(name.clone()),
            Some(Binding::Alias(target)) => self.spawn_prefab_for(target),
            _ => None,
        }
    }

    fn entity_for(&self, path: &str) -> Option<EntityAnim> {
        self.entity_for_depth(path, 0)
    }

    fn entity_for_depth(&self, path: &str, depth: usize) -> Option<EntityAnim> {
        if depth > 8 {
            return None;
        }
        let current = self
            .current_func()
            .and_then(|func| func.entities.get(path))
            .cloned();
        let bound = match self.bindings.get(path) {
            Some(Binding::Factory(callee)) => self.function_entity(callee, None),
            Some(Binding::Spawn(name)) => self
                .prefabs
                .get(name)
                .and_then(|function| self.function_entity(function, None)),
            Some(Binding::Alias(target)) => self.entity_for_depth(target, depth + 1),
            None => None,
        };
        let mut merged = bound;
        if let Some(entity) = current {
            match &mut merged {
                Some(base) => base.merge(&entity),
                None => merged = Some(entity),
            }
        }
        if merged.is_none() && matches!(path, "inst" | "self" | "self.inst") {
            if let Some(entity) = self.current_func().and_then(|f| f.entities.get("inst")) {
                merged = Some(entity.clone());
            } else {
                merged = self.single_prefab_entity();
            }
        }
        merged
    }

    fn push_record(&mut self, input: RecordInput<'_>) {
        let RecordInput {
            receiver,
            anchor,
            entity,
            animation,
            offset,
            frame_begin,
            frame_end,
            sync_frame,
            confidence,
        } = input;
        let confidence = if animation.is_none() {
            FollowConfidence::Unresolved
        } else {
            confidence
        };
        let spawn_prefab = if animation.is_none() {
            self.spawn_prefab_for(receiver)
        } else {
            None
        };
        self.records.push(FollowFxRecord {
            anchor: anchor.to_string(),
            bank: entity.bank,
            build: entity.build,
            animation,
            offset,
            frame_begin,
            frame_end,
            final_offset: entity.final_offset,
            mult_colour: entity.mult_colour,
            add_colour: entity.add_colour,
            sync_frame,
            random_frame: entity.random_frame,
            receiver: receiver.to_string(),
            spawn_prefab,
            confidence,
            prefab: self.prefab.clone(),
        });
    }

    fn handle_follow_symbol(&mut self, path: Option<String>, method: &ast::MethodCall) {
        let args = method_args(method);
        if args.len() < 2 {
            return;
        }
        let Some((anchor, tracked)) = resolve_string(args[1], &self.consts) else {
            return;
        };
        if anchor.is_empty() {
            return;
        }
        if let Some(func) = self.current_func() {
            if param_index(func, args[1]).is_some() {
                return;
            }
        }
        let path = path.unwrap_or_default();
        let direct = self
            .current_func()
            .is_some_and(|func| func.entities.contains_key(&path));
        let entity = self.entity_for(&path);
        let animation = entity.as_ref().and_then(|entity| entity.anim.clone());
        let mut confidence = if animation.is_some() {
            if direct {
                FollowConfidence::Static
            } else {
                FollowConfidence::Resolved
            }
        } else {
            FollowConfidence::Unresolved
        };
        if tracked && confidence == FollowConfidence::Static {
            confidence = FollowConfidence::Resolved;
        }
        let offset = [
            literal_number(args.get(2).copied()).unwrap_or(0.0),
            literal_number(args.get(3).copied()).unwrap_or(0.0),
        ];
        self.push_record(RecordInput {
            receiver: &path,
            anchor: &anchor,
            entity: entity.unwrap_or_default(),
            animation,
            offset,
            frame_begin: literal_u32(args.get(7).copied()),
            frame_end: literal_u32(args.get(8).copied()),
            sync_frame: literal_bool(args.get(6).copied()).unwrap_or(false),
            confidence,
        });
    }

    fn handle_factory_call(&mut self, call: &ast::FunctionCall, callee: &str) {
        let Some(func) = self.funcs.get(callee).cloned() else {
            return;
        };
        let Some(symbol_index) = func.follow_symbol_param else {
            return;
        };
        let args = call_args(call);
        let Some((anchor, _)) = args
            .get(symbol_index)
            .and_then(|expr| resolve_string(expr, &self.consts))
        else {
            return;
        };
        if anchor.is_empty() {
            return;
        }
        let path = func.follow_path.clone().unwrap_or_default();
        let mut entity = func
            .entities
            .get(&path)
            .cloned()
            .or_else(|| self.function_entity(callee, Some(&path)))
            .unwrap_or_default();
        let animation = match entity.anim_param {
            Some(index) => args
                .get(index)
                .and_then(|expr| resolve_string(expr, &self.consts))
                .map(|(value, _)| value),
            None => entity.anim.clone(),
        };
        if animation.is_some() {
            entity.anim = animation.clone();
        }
        let offset = [
            func.follow_offset.0.unwrap_or(0.0),
            func.follow_offset.1.unwrap_or(0.0),
        ];
        self.push_record(RecordInput {
            receiver: callee,
            anchor: &anchor,
            entity,
            animation,
            offset,
            frame_begin: func.follow_frames.0,
            frame_end: func.follow_frames.1,
            sync_frame: func.follow_sync,
            confidence: FollowConfidence::Resolved,
        });
    }
}

impl Visitor for FollowResolver {
    fn visit_local_function(&mut self, node: &ast::LocalFunction) {
        self.stack.push(node.name().to_string());
    }

    fn visit_local_function_end(&mut self, _node: &ast::LocalFunction) {
        self.stack.pop();
    }

    fn visit_function_declaration(&mut self, node: &ast::FunctionDeclaration) {
        self.stack.push(node.name().to_string());
    }

    fn visit_function_declaration_end(&mut self, _node: &ast::FunctionDeclaration) {
        self.stack.pop();
    }

    fn visit_function_call(&mut self, call: &ast::FunctionCall) {
        let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
        if let Some(ast::Suffix::Call(ast::Call::MethodCall(method))) = suffixes.last().copied() {
            if method.name().token().to_string() == "FollowSymbol" {
                self.handle_follow_symbol(follow_receiver_path(call), method);
                return;
            }
        }
        if let Some(callee) = simple_callee(call) {
            self.handle_factory_call(call, &callee);
        }
    }
}

fn param_names(body: &ast::FunctionBody) -> Vec<String> {
    body.parameters()
        .iter()
        .filter_map(|param| match param {
            ast::Parameter::Name(name) => Some(name.token().to_string()),
            _ => None,
        })
        .collect()
}

fn param_index(func: &FuncInfo, expr: &ast::Expression) -> Option<usize> {
    let ast::Expression::Var(ast::Var::Name(name)) = expr else {
        return None;
    };
    let name = name.token().to_string();
    func.params.iter().position(|param| param == &name)
}

fn var_path(var: &ast::Var) -> Option<String> {
    match var {
        ast::Var::Name(name) => Some(name.token().to_string()),
        ast::Var::Expression(vex) => {
            let suffixes: Vec<&ast::Suffix> = vex.suffixes().collect();
            path_from(vex.prefix(), &suffixes)
        }
        _ => None,
    }
}

fn path_from(prefix: &ast::Prefix, suffixes: &[&ast::Suffix]) -> Option<String> {
    let ast::Prefix::Name(name) = prefix else {
        return None;
    };
    let mut out = name.token().to_string();
    for suffix in suffixes {
        match suffix {
            ast::Suffix::Index(ast::Index::Dot { name, .. }) => {
                out.push('.');
                out.push_str(&name.token().to_string());
            }
            ast::Suffix::Index(ast::Index::Brackets { expression, .. }) => {
                let value = match expression {
                    ast::Expression::String(token) => extract_string_literal(&token.to_string()),
                    _ => return None,
                };
                out.push('.');
                out.push_str(&value);
            }
            _ => return None,
        }
    }
    Some(out)
}

fn simple_callee(call: &ast::FunctionCall) -> Option<String> {
    let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
    if suffixes.len() != 1 {
        return None;
    }
    if !matches!(suffixes[0], ast::Suffix::Call(ast::Call::AnonymousCall(_))) {
        return None;
    }
    path_from(call.prefix(), &[])
}

fn call_args(call: &ast::FunctionCall) -> Vec<&ast::Expression> {
    let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
    match suffixes.last() {
        Some(ast::Suffix::Call(ast::Call::AnonymousCall(ast::FunctionArgs::Parentheses {
            arguments,
            ..
        }))) => arguments.iter().collect(),
        _ => Vec::new(),
    }
}

fn method_args(method: &ast::MethodCall) -> Vec<&ast::Expression> {
    match method.args() {
        ast::FunctionArgs::Parentheses { arguments, .. } => arguments.iter().collect(),
        _ => Vec::new(),
    }
}

fn first_string_arg(call: &ast::FunctionCall) -> Option<String> {
    call_args(call).first().and_then(|expr| match expr {
        ast::Expression::String(token) => Some(extract_string_literal(&token.to_string())),
        _ => None,
    })
}

fn follow_receiver_path(call: &ast::FunctionCall) -> Option<String> {
    let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
    let len = suffixes.len();
    if len < 2 {
        return None;
    }
    let Some(ast::Suffix::Index(ast::Index::Dot { name, .. })) = suffixes.get(len - 2) else {
        return None;
    };
    if name.token().to_string() != "Follower" {
        return None;
    }
    path_from(call.prefix(), &suffixes[..len - 2])
}

fn follow_symbol_call(call: &ast::FunctionCall) -> Option<(Option<String>, &ast::MethodCall)> {
    let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
    let ast::Suffix::Call(ast::Call::MethodCall(method)) = suffixes.last().copied()? else {
        return None;
    };
    if method.name().token().to_string() != "FollowSymbol" {
        return None;
    }
    Some((follow_receiver_path(call), method))
}

fn anim_state_call<'a>(
    call: &'a ast::FunctionCall,
    aliases: &BTreeMap<String, String>,
) -> Option<(String, String, Vec<&'a ast::Expression>)> {
    let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
    let ast::Suffix::Call(ast::Call::MethodCall(method)) = suffixes.last().copied()? else {
        return None;
    };
    let method_name = method.name().token().to_string();
    let entity_path = if suffixes.len() >= 2 {
        let anim_state = match suffixes.get(suffixes.len() - 2) {
            Some(ast::Suffix::Index(ast::Index::Dot { name, .. })) => {
                name.token().to_string() == "AnimState"
            }
            Some(ast::Suffix::Index(ast::Index::Brackets {
                expression: ast::Expression::String(token),
                ..
            })) => extract_string_literal(&token.to_string()) == "AnimState",
            _ => false,
        };
        if !anim_state {
            return None;
        }
        path_from(call.prefix(), &suffixes[..suffixes.len() - 2])
    } else {
        match call.prefix() {
            ast::Prefix::Name(name) => aliases.get(&name.token().to_string()).cloned(),
            _ => None,
        }
    }?;
    Some((entity_path, method_name, method_args(method)))
}

fn apply_anim_state_method(
    entity: &mut EntityAnim,
    method: &str,
    args: &[&ast::Expression],
    consts: &BTreeMap<String, String>,
    anim_param: Option<usize>,
) {
    match method {
        "SetBank" => {
            if let Some((value, _)) = args.first().and_then(|arg| resolve_string(arg, consts)) {
                entity.bank = Some(value);
            }
        }
        "SetBuild" => {
            if let Some((value, _)) = args.first().and_then(|arg| resolve_string(arg, consts)) {
                entity.build = Some(value);
            }
        }
        "PlayAnimation" => {
            let Some(first) = args.first() else {
                return;
            };
            if anim_param.is_some() {
                entity.anim_param = anim_param;
            } else if let Some((value, _)) = resolve_string(first, consts) {
                entity.anim = Some(value);
            }
        }
        "SetFinalOffset" => {
            entity.final_offset = literal_number(args.first().copied()).map(|value| value as i32);
        }
        "SetMultColour" => {
            entity.mult_colour = colour_from_args(args).filter(|c| *c != [1.0, 1.0, 1.0, 1.0]);
        }
        "SetAddColour" => {
            entity.add_colour =
                colour_from_args(args).filter(|c| c[0] != 0.0 || c[1] != 0.0 || c[2] != 0.0);
        }
        "SetFrame" if args.first().is_some_and(|arg| contains_math_random(arg)) => {
            entity.random_frame = true;
        }
        _ => {}
    }
}

fn contains_math_random(expr: &ast::Expression) -> bool {
    match expr {
        ast::Expression::FunctionCall(call) => {
            let suffixes: Vec<&ast::Suffix> = call.suffixes().collect();
            if suffixes.len() < 2 {
                return false;
            }
            path_from(call.prefix(), &suffixes[..suffixes.len() - 1]).as_deref()
                == Some("math.random")
        }
        ast::Expression::BinaryOperator { lhs, rhs, .. } => {
            contains_math_random(lhs) || contains_math_random(rhs)
        }
        ast::Expression::Parentheses { expression, .. } => contains_math_random(expression),
        ast::Expression::UnaryOperator { expression, .. } => contains_math_random(expression),
        _ => false,
    }
}

fn collect_follow_param(func: &mut FuncInfo, args: &[&ast::Expression]) {
    if let Some(index) = args.get(1).and_then(|arg| param_index(func, arg)) {
        func.follow_symbol_param = Some(index);
        func.follow_offset = (
            literal_number(args.get(2).copied()),
            literal_number(args.get(3).copied()),
        );
        func.follow_frames = (
            literal_u32(args.get(7).copied()),
            literal_u32(args.get(8).copied()),
        );
        func.follow_sync = literal_bool(args.get(6).copied()).unwrap_or(false);
    }
}

fn colour_from_args(args: &[&ast::Expression]) -> Option<[f32; 4]> {
    let r = literal_number(args.first().copied())?;
    let g = literal_number(args.get(1).copied())?;
    let b = literal_number(args.get(2).copied())?;
    let a = literal_number(args.get(3).copied()).unwrap_or(1.0);
    Some([r, g, b, a])
}

fn resolve_string(
    expr: &ast::Expression,
    consts: &BTreeMap<String, String>,
) -> Option<(String, bool)> {
    match expr {
        ast::Expression::String(token) => Some((extract_string_literal(&token.to_string()), false)),
        ast::Expression::Parentheses { expression, .. } => resolve_string(expression, consts),
        ast::Expression::Var(ast::Var::Name(name)) => consts
            .get(&name.token().to_string())
            .map(|value| (value.clone(), true)),
        ast::Expression::BinaryOperator { lhs, binop, rhs } => {
            if binop.to_string().trim() != ".." {
                return None;
            }
            let (left, lt) = resolve_string(lhs, consts)?;
            let (right, rt) = resolve_string(rhs, consts)?;
            Some((format!("{left}{right}"), lt || rt))
        }
        _ => None,
    }
}

fn literal_number(expr: Option<&ast::Expression>) -> Option<f32> {
    match expr? {
        ast::Expression::Number(token) => token.to_string().parse().ok(),
        ast::Expression::UnaryOperator { unop, expression } if unop.to_string().trim() == "-" => {
            literal_number(Some(expression)).map(|value| -value)
        }
        _ => None,
    }
}

fn literal_u32(expr: Option<&ast::Expression>) -> Option<u32> {
    literal_number(expr).map(|value| value.max(0.0) as u32)
}

fn literal_bool(expr: Option<&ast::Expression>) -> Option<bool> {
    match expr? {
        ast::Expression::Symbol(token) => match token.token().to_string().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn extract_string_literal(raw: &str) -> String {
    let s = raw.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s[1..s.len() - 1].to_string()
    } else if let Some(inner) = s.strip_prefix("[[").and_then(|v| v.strip_suffix("]]")) {
        inner.to_string()
    } else {
        s.to_string()
    }
}

fn table_field_key(field: &ast::Field) -> Option<String> {
    match field {
        ast::Field::NameKey { key, .. } => Some(key.token().to_string()),
        ast::Field::ExpressionKey {
            key: ast::Expression::String(raw),
            ..
        } => Some(extract_string_literal(&raw.to_string())),
        _ => None,
    }
}

fn table_field_value(field: &ast::Field) -> Option<&ast::Expression> {
    match field {
        ast::Field::NameKey { value, .. }
        | ast::Field::ExpressionKey { value, .. }
        | ast::Field::NoKey(value) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_local_factory_follow() {
        let source = r#"
local function CreateCapeFx()
    local inst = CreateEntity()
    inst.entity:AddTransform()
    inst.entity:AddAnimState()
    inst.entity:AddFollower()
    inst.AnimState:SetBank("shadow_thrall_wings")
    inst.AnimState:SetBuild("shadow_thrall_wings")
    inst.AnimState:PlayAnimation("fx_cape_front", true)
    return inst
end

local function fn()
    local inst = CreateEntity()
    local cape = CreateCapeFx()
    cape.entity:SetParent(inst.entity)
    cape.Follower:FollowSymbol(inst.GUID, "cape_front_swap", nil, nil, nil, true)
    return inst
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.anchor, "cape_front_swap");
        assert_eq!(record.bank.as_deref(), Some("shadow_thrall_wings"));
        assert_eq!(record.build.as_deref(), Some("shadow_thrall_wings"));
        assert_eq!(record.animation.as_deref(), Some("fx_cape_front"));
        assert_eq!(record.receiver, "cape");
    }

    #[test]
    fn extracts_parameterized_factory_call() {
        let source = r#"
local function AddFollowSymbol(inst, sym, anim)
    local fx = CreateEntity()
    fx.entity:AddAnimState()
    fx.AnimState:SetBank("charlie_boss")
    fx.AnimState:SetBuild("charlie_boss_basic")
    fx.AnimState:PlayAnimation(anim, true)
    fx.entity:SetParent(inst.entity)
    fx.Follower:FollowSymbol(inst.GUID, sym, 0, 0, 0, true)
    return fx
end

local function fn()
    local inst = CreateEntity()
    AddFollowSymbol(inst, "cb_hair_follow", "hair_loop")
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.anchor, "cb_hair_follow");
        assert_eq!(record.bank.as_deref(), Some("charlie_boss"));
        assert_eq!(record.build.as_deref(), Some("charlie_boss_basic"));
        assert_eq!(record.animation.as_deref(), Some("hair_loop"));
        assert_eq!(record.confidence, FollowConfidence::Resolved);
    }

    #[test]
    fn resolves_inline_entity_in_same_function() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    local fx = CreateEntity()
    fx.entity:AddAnimState()
    fx.AnimState:SetBank("shadow_insanity_player")
    fx.AnimState:SetBuild("shadow_insanity_player")
    fx.AnimState:PlayAnimation("flame_hair_fx", true)
    fx.Follower:FollowSymbol(inst.GUID, "follow", 0, 0, 0, true)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].animation.as_deref(), Some("flame_hair_fx"));
        assert_eq!(records[0].confidence, FollowConfidence::Static);
        assert_eq!(records[0].receiver, "fx");
    }

    #[test]
    fn resolves_field_entity_binding() {
        let source = r#"
local function CreateHead()
    local inst = CreateEntity()
    inst.AnimState:SetBank("daywalker_head")
    inst.AnimState:SetBuild("daywalker_head")
    inst.AnimState:PlayAnimation("idle", true)
    return inst
end

local function fn()
    local inst = CreateEntity()
    inst.head = CreateHead()
    inst.head.Follower:FollowSymbol(inst.GUID, "HEAD_follow", nil, nil, nil, true, true)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].receiver, "inst.head");
        assert_eq!(records[0].animation.as_deref(), Some("idle"));
        assert!(records[0].sync_frame);
    }

    #[test]
    fn resolves_same_file_spawn_prefab() {
        let source = r#"
local function fxfn()
    local inst = CreateEntity()
    inst.AnimState:SetBank("winona_telebrella")
    inst.AnimState:SetBuild("winona_telebrella")
    inst.AnimState:PlayAnimation("swap1")
    return inst
end

local function fn()
    local inst = CreateEntity()
    inst.swapfx = SpawnPrefab("winona_telebrella_swap_fx")
    inst.swapfx.Follower:FollowSymbol(owner.GUID, "swap_object", nil, nil, nil, true, nil, 0, 19)
    inst.swapfx2 = SpawnPrefab("winona_telebrella_swap_fx")
    inst.swapfx2.Follower:FollowSymbol(owner.GUID, "swap_object", nil, nil, nil, true, nil, 19)
    inst.swapfx2.AnimState:PlayAnimation("swap20")
end

return Prefab("winona_telebrella", fn, {}), Prefab("winona_telebrella_swap_fx", fxfn, {})
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 2);
        let by_anim: std::collections::BTreeMap<&str, &FollowFxRecord> = records
            .iter()
            .map(|record| (record.animation.as_deref().unwrap_or(""), record))
            .collect();
        assert!(by_anim.contains_key("swap1"));
        assert!(by_anim.contains_key("swap20"));
        assert_eq!(by_anim["swap1"].frame_begin, Some(0));
        assert_eq!(by_anim["swap1"].frame_end, Some(19));
        assert_eq!(by_anim["swap20"].frame_begin, Some(19));
        assert_eq!(by_anim["swap1"].bank.as_deref(), Some("winona_telebrella"));
    }

    #[test]
    fn captures_runtime_metadata() {
        let source = r#"
local function CreateFx()
    local inst = CreateEntity()
    inst.AnimState:SetBank("b")
    inst.AnimState:SetBuild("b")
    inst.AnimState:PlayAnimation("fx")
    inst.AnimState:SetFinalOffset(1)
    inst.AnimState:SetMultColour(1, 1, 1, 0.5)
    inst.AnimState:SetAddColour(0.1, 0.2, 0.3, 1)
    inst.AnimState:SetFrame(math.random(inst.AnimState:GetCurrentAnimationNumFrames()))
    return inst
end

local function fn()
    local inst = CreateEntity()
    local fx = CreateFx()
    fx.Follower:FollowSymbol(inst.GUID, "anchor", nil, nil, nil, true, true)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.final_offset, Some(1));
        assert_eq!(record.mult_colour, Some([1.0, 1.0, 1.0, 0.5]));
        assert_eq!(record.add_colour, Some([0.1, 0.2, 0.3, 1.0]));
        assert!(record.random_frame);
        assert!(record.sync_frame);
    }

    #[test]
    fn falls_back_to_single_prefab_fn_for_inst_receiver() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst.AnimState:SetBank("tophat_fx")
    inst.AnimState:SetBuild("tophat_fx")
    inst.AnimState:PlayAnimation("swirl_front", true)
end

local function attach(inst, owner)
    inst.Follower:FollowSymbol(owner.GUID, "swap_tophat_swirl_front", 0, 0, 0, true)
end

return Prefab("tophat_swirl_fx", fn, {})
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].animation.as_deref(), Some("swirl_front"));
        assert_eq!(records[0].confidence, FollowConfidence::Resolved);
    }

    #[test]
    fn marks_unresolved_when_animation_is_dynamic() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst.Follower:FollowSymbol(owner.GUID, "follow_fx_swap", nil, nil, nil, true)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].animation, None);
        assert_eq!(records[0].confidence, FollowConfidence::Unresolved);
    }

    #[test]
    fn resolves_symbol_from_string_constant() {
        let source = r#"
local SYMBOL = "fx_flame_swap"
local function CreateFx()
    local inst = CreateEntity()
    inst.AnimState:SetBank("shadow_thrall_wings")
    inst.AnimState:SetBuild("shadow_thrall_wings")
    inst.AnimState:PlayAnimation("fx_flame", true)
    return inst
end
local function fn()
    local inst = CreateEntity()
    local fx = CreateFx()
    fx.Follower:FollowSymbol(inst.GUID, SYMBOL, nil, nil, nil, true)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].anchor, "fx_flame_swap");
        assert_eq!(records[0].animation.as_deref(), Some("fx_flame"));
        assert_eq!(records[0].confidence, FollowConfidence::Resolved);
    }

    #[test]
    fn reads_offsets_and_frame_window() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    local fx = CreateFx()
    fx.Follower:FollowSymbol(inst.GUID, "swap_object", 56, -40, 0, true, nil, 3, 7)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].offset, [56.0, -40.0]);
        assert_eq!(records[0].frame_begin, Some(3));
        assert_eq!(records[0].frame_end, Some(7));
    }
    #[test]
    fn marks_cross_file_spawn_target() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst.fx = SpawnPrefab("gestalt_head")
    inst.fx.Follower:FollowSymbol(inst.GUID, "head_fx", 0, 0, 0)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].confidence, FollowConfidence::Unresolved);
        assert_eq!(records[0].spawn_prefab.as_deref(), Some("gestalt_head"));
        assert_eq!(records[0].animation, None);
    }

    #[test]
    fn binds_table_constructor_fields() {
        let source = r#"
local function fn()
    local inst = CreateEntity()
    inst.frostbreath = {
        fx = SpawnPrefab("frostbreath"),
        fx2 = SpawnPrefab("frostbreath"),
    }
    inst.frostbreath.fx.Follower:FollowSymbol(inst.GUID, "breath_left", 0, 0, 0)
end
"#;
        let records = parse_follow_symbols(source).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].receiver, "inst.frostbreath.fx");
        assert_eq!(records[0].spawn_prefab.as_deref(), Some("frostbreath"));
    }

    #[test]
    fn resolves_cross_file_spawn_from_prefab_entities() {
        let defs_source = r#"
local function other_fn()
    local inst = CreateEntity()
    inst.AnimState:SetBank("gestalt_head")
    inst.AnimState:SetBuild("gestalt_head")
    inst.AnimState:PlayAnimation("idle_loop", true)
    inst.AnimState:SetFinalOffset(1)
    return inst
end
return Prefab("gestalt_head", other_fn, {})
"#;
        let (_, entities) =
            parse_follow_symbols_and_entities(defs_source, Some("gestalt")).unwrap();
        let entity = entities.get("gestalt_head").expect("prefab entity");
        assert_eq!(entity.bank.as_deref(), Some("gestalt_head"));
        assert_eq!(entity.animation.as_deref(), Some("idle_loop"));

        let fx_source = r#"
local function fn()
    local inst = CreateEntity()
    inst.blobhead = SpawnPrefab("gestalt_head")
    inst.blobhead.Follower:FollowSymbol(inst.GUID, "head_fx", 0, 0, 0)
end
"#;
        let mut records = parse_follow_symbols(fx_source).unwrap();
        assert_eq!(records[0].animation, None);
        assert_eq!(resolve_cross_file_follows(&mut records, &entities), 1);
        assert_eq!(records[0].animation.as_deref(), Some("idle_loop"));
        assert_eq!(records[0].bank.as_deref(), Some("gestalt_head"));
        assert_eq!(records[0].build.as_deref(), Some("gestalt_head"));
        assert_eq!(records[0].final_offset, Some(1));
        assert_eq!(records[0].confidence, FollowConfidence::Resolved);
    }
}
