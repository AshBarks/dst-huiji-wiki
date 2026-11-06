//! 页面 ↔ 游戏源码 关联索引（corpus-index 的第四类派生工件）。
//!
//! 从语料派生索引（prefab 注册表）+ 本地游戏脚本树构建：
//! - `index/page_code_links.jsonl`：页面 → 源文件（provenance 角色）→ 组件/方法
//!   （函数级调用行号）→ 状态图/行为树/事件/TUNING/配方声明行 → 断言级候选钩子
//! - `index/prefab_code_links.json`：prefab → status / 文件（含锚点）/ 皮肤基础 / 依赖
//! - `index/{component,stategraph,brain}_code_links.json`：函数级二级索引
//!
//! 粒度与判定规则与 Python 参考实现（`wikis/<host>/index/_generators/`）一致；
//! 审计结论合并（`audit_findings`）依赖 corpus 之外的产物，留在 Python 富化层。
//!
//! 全程确定性提取：找不到依据就留空并标注 status，绝不猜测。

use crate::corpus::prefab_index::PrefabRegistry;
use crate::corpus::store::CorpusStore;
use crate::error::Result;
use crate::platform::progress::Reporter;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

/// 事件被这么多文件 `ListenForEvent` 即视为通用事件（animover/death 一类）。
const GENERIC_EVENT_FILE_THRESHOLD: usize = 150;

/// 每页/每名保留的锚点上限（输出体积控制；与 Python 版一致）。
const MAX_PAGE_PREFABS: usize = 12;
const MAX_PAGE_FILES: usize = 14;
const MAX_PAGE_EVENTS: usize = 14;
const MAX_PAGE_GENERIC_EVENTS: usize = 8;
const MAX_PAGE_TUNING: usize = 30;
const MAX_PAGE_HOOKS: usize = 12;
const MAX_REQUIRED_PER_PREFAB: usize = 3;
const MAX_FILES_PER_PREFAB: usize = 10;

/// 机制/生物群系页 curated 种子（源自 output/wiki-page-guide/机制页.md 的
/// grep 验证映射；标题关键词 → 源文件）。确定性规则，宁缺毋滥。
const SEEDS: &[(&str, &[&str])] = &[
    (r"饥饿", &["components/hunger.lua", "components/eater.lua"]),
    (r"理智|脑残", &["components/sanity.lua"]),
    (r"生命|血量|治疗", &["components/health.lua"]),
    (r"温度|过冷|过热|体温", &["components/temperature.lua"]),
    (
        r"湿度|降雨|下雨",
        &[
            "components/moisture.lua",
            "components/weather.lua",
            "components/seasons.lua",
        ],
    ),
    (
        r"黑暗|查理|夜晚",
        &["components/grue.lua", "components/clock.lua"],
    ),
    (r"猎犬", &["components/hounded.lua"]),
    (
        r"烹饪|锅|料理",
        &["cooking.lua", "components/cooker.lua", "cookbookdata.lua"],
    ),
    (
        r"季节",
        &["components/seasons.lua", "components/weather.lua"],
    ),
    (
        r"昼夜|时钟|月相",
        &["components/clock.lua", "components/moons.lua"],
    ),
    (
        r"自定义世界|世界选项|预设",
        &["map/customise.lua", "map/levels.lua"],
    ),
    (
        r"陨石|月石",
        &[
            "components/meteorshower.lua",
            "components/meteorologist.lua",
        ],
    ),
    (
        r"农场|耕种|作物",
        &[
            "components/farmplanted.lua",
            "components/growable.lua",
            "prefabs/farm_plant_defs.lua",
        ],
    ),
    (
        r"驯牛|皮弗娄",
        &[
            "components/beardliness.lua",
            "components/rideable.lua",
            "components/mood.lua",
        ],
    ),
    (
        r"船|航行",
        &[
            "components/boatcontainer.lua",
            "components/health.lua",
            "prefabs/boat.lua",
        ],
    ),
    (
        r"群系|生物群系|地形|地皮|区块|布景",
        &[
            "map/levels.lua",
            "map/layouts.lua",
            "map/tasks/*.lua",
            "map/rooms/*.lua",
        ],
    ),
    (r"猪王|交易", &["components/trader.lua"]),
    (
        r"钓鱼",
        &["components/fishable.lua", "components/oceanfishing.lua"],
    ),
    (
        r"闪电",
        &["components/weather.lua", "components/seasons.lua"],
    ),
    (r"地震", &["components/earthquakewarning.lua"]),
    (r"冰冻|冻结", &["components/freezable.lua"]),
    (
        r"火灾|燃烧|引燃",
        &["components/burnable.lua", "components/propagator.lua"],
    ),
    (r"爆炸", &["components/explosive.lua"]),
    (r"触电|电流", &["components/electricattacks.lua"]),
    (r"滑倒|打滑", &["components/slipperyfeet.lua"]),
    (
        r"耐久",
        &["components/finiteuses.lua", "components/armor.lua"],
    ),
    (r"腐烂|保鲜|新鲜度", &["components/perishable.lua"]),
    (r"物品栏|背包", &["components/inventory.lua"]),
    (r"衣柜|换装", &["components/wardrobe.lua"]),
    (r"幸运", &["components/luckuser.lua"]),
    (r"催眠|睡眠", &["components/sleeper.lua"]),
    (
        r"伤害|攻击",
        &["components/combat.lua", "components/aoeweapon_base.lua"],
    ),
    (
        r"随从|雇佣|追随",
        &["components/follower.lua", "components/leader.lua"],
    ),
    (r"防雨|防水|潮湿免疫", &["components/moistureimmunity.lua"]),
    (
        r"酸雨",
        &[
            "components/acidbatwavemanager.lua",
            "components/worldstate.lua",
        ],
    ),
    (r"淘气|坎普斯", &["components/kramped.lua"]),
];

/// 构建汇总（随 corpus-index 返回值上报）。
#[derive(Debug, Serialize)]
pub struct CodeLinksSummary {
    pub lua_files: usize,
    pub pages: usize,
    pub pages_linked: usize,
    pub prefabs_declared: usize,
    pub prefabs_with_anchor: usize,
    pub prefabs_retired: usize,
    pub prefabs_no_anchor: usize,
    pub components: usize,
    pub stategraphs: usize,
    pub brains: usize,
    pub recipe_anchors: usize,
    pub assertion_hooks: usize,
    pub skin_mappings: usize,
}

/// 构建产物：调用方负责经 `save_lines`/`save_json` 原子落盘。
pub struct BuildOutput {
    pub summary: CodeLinksSummary,
    pub page_jsonl: String,
    pub prefab_json: serde_json::Value,
    pub component_json: serde_json::Value,
    pub stategraph_json: serde_json::Value,
    pub brain_json: serde_json::Value,
}

/// 组件锚点：组件名 → (AddComponent 声明行, API 方法 → 调用行)。
type ComponentDecl = (Vec<usize>, BTreeMap<String, Vec<usize>>);

/// 断言钩子的 facts 侧数据：(原文, 数值列表, 单位)。
type FactRef = (String, Vec<f64>, Option<String>);

/// 单个 Lua 文件的锚点解析结果（构建期缓存）。
#[derive(Debug, Default, Clone)]
struct FileAnchors {
    components: BTreeMap<String, ComponentDecl>,
    stategraphs: Vec<(String, usize)>,
    brains: Vec<(String, usize)>,
    /// 事件名（`*push:` 前缀 = 该文件推送）→ 行号
    events: BTreeMap<String, Vec<usize>>,
    tuning: BTreeMap<String, Vec<usize>>,
}

struct Regexes {
    prefab_decl: Regex,
    add_comp: Regex,
    method_call: Regex,
    set_sg: Regex,
    brain_ref: Regex,
    req_prefab: Regex,
    listen: Regex,
    push: Regex,
    tuning: Regex,
    recipe: Regex,
    food_entry: Regex,
    skin: Regex,
    sg_state: Regex,
    sg_state_name: Regex,
    handler: Regex,
    tuning_entry: Regex,
    local_int: Regex,
    table_ident: Regex,
    brain_node: Regex,
    fn_def: Regex,
    speech: Regex,
}

fn regexes() -> &'static Regexes {
    static RE: OnceLock<Regexes> = OnceLock::new();
    RE.get_or_init(|| Regexes {
        prefab_decl: Regex::new(r#"\bPrefab\(\s*"(\w+)""#).expect("valid"),
        add_comp: Regex::new(r#"AddComponent\(\s*"([a-z0-9_]+)"\s*\)"#).expect("valid"),
        method_call: Regex::new(r"inst\.components\.([a-z0-9_]+):([A-Za-z0-9_]+)\(").expect("valid"),
        set_sg: Regex::new(r#"SetStateGraph\(\s*"([^"]+)""#).expect("valid"),
        brain_ref: Regex::new(r#"SetBrain\(\s*brains\.(\w+)|require\s*\(?\s*"brains/(\w+)""#)
            .expect("valid"),
        req_prefab: Regex::new(r#"require\s*\(?\s*"prefabs/([\w]+)""#).expect("valid"),
        listen: Regex::new(r#"ListenForEvent\(\s*"([\w]+)""#).expect("valid"),
        push: Regex::new(r#"PushEvent\(\s*"([\w]+)""#).expect("valid"),
        tuning: Regex::new(r"TUNING\.([A-Z0-9_]+)").expect("valid"),
        recipe: Regex::new(r#"Recipe2?\(\s*"(\w+)""#).expect("valid"),
        food_entry: Regex::new(r"(?m)^\t(\w+)\s*=\s*\n\t?\{").expect("valid"),
        skin: Regex::new(
            r#"CreatePrefabSkin\(\s*"(\w+)"\s*,\s*\{\s*base_prefab\s*=\s*"(\w+)""#,
        )
        .expect("valid"),
        sg_state: Regex::new(r"\bState\s*\{").expect("valid"),
        sg_state_name: Regex::new(r#"name\s*=\s*"([\w]+)""#).expect("valid"),
        handler: Regex::new(r#"EventHandler\(\s*"([\w]+)""#).expect("valid"),
        tuning_entry: Regex::new(
            r"(?m)^\s*([A-Z0-9_]{3,})\s*=\s*([-\w.*+/\s()0-9]+?),?\s*(?:--.*)?$",
        )
        .expect("valid"),
        local_int: Regex::new(r"(?m)^local\s+(\w+)\s*=\s*(\d+)\s*$").expect("valid"),
        table_ident: Regex::new(r"(?m)^\s*([A-Z][A-Z0-9_]{2,})\s*=").expect("valid"),
        brain_node: Regex::new(
            r"\b(PriorityNode|SequenceNode|ParallelNode|WhileNode|IfNode|NotDecorator|ConditionWaitNode|ChaseAndAttackAndAvoid|ChaseAndAttack|AttackWall|Follow|Wander|Leash|RunAway|RunInDirection|PanicTrigger|DoAction|FindEntity|ExecuteBehaviour|EventNode|ChargeAndAttack|MinPeriod|ScanForAction|StandStill|BrainCommon\.PanicTrigger)\s*[(\{]",
        )
        .expect("valid"),
        fn_def: Regex::new(r"function\s+[\w.]+[:.]([A-Za-z_]\w*)\s*\(").expect("valid"),
        speech: Regex::new(r#"\{\{全角色台词\|(?:dst|ds)\|([^|}\n]+)"#).expect("valid"),
    })
}

/// 递归收集 scripts 树下的 .lua 相对路径（正斜杠归一、稳定排序）。
fn collect_lua(scripts_root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![scripts_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "lua") {
                let rel = path
                    .strip_prefix(scripts_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(rel);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn line_of(text: &str, byte: usize) -> usize {
    text[..byte].matches('\n').count() + 1
}

/// 单文件锚点解析。
fn parse_file(text: &str) -> FileAnchors {
    let re = regexes();
    let mut a = FileAnchors::default();
    for cap in re.add_comp.captures_iter(text) {
        let line = line_of(text, cap.get(0).unwrap().start());
        a.components
            .entry(cap[1].to_string())
            .or_default()
            .0
            .push(line);
    }
    for cap in re.method_call.captures_iter(text) {
        let line = line_of(text, cap.get(0).unwrap().start());
        a.components
            .entry(cap[1].to_string())
            .or_default()
            .1
            .entry(cap[2].to_string())
            .or_default()
            .push(line);
    }
    for cap in re.set_sg.captures_iter(text) {
        let line = line_of(text, cap.get(0).unwrap().start());
        a.stategraphs.push((cap[1].to_string(), line));
    }
    let mut seen = BTreeSet::new();
    for cap in re.brain_ref.captures_iter(text) {
        if let Some(name) = cap.get(1).or_else(|| cap.get(2)) {
            if seen.insert(name.as_str().to_string()) {
                let line = line_of(text, cap.get(0).unwrap().start());
                a.brains.push((name.as_str().to_string(), line));
            }
        }
    }
    for cap in re.listen.captures_iter(text) {
        let line = line_of(text, cap.get(0).unwrap().start());
        a.events.entry(cap[1].to_string()).or_default().push(line);
    }
    for cap in re.push.captures_iter(text) {
        let line = line_of(text, cap.get(0).unwrap().start());
        a.events
            .entry(format!("*push:{}", &cap[1]))
            .or_default()
            .push(line);
    }
    for cap in re.tuning.captures_iter(text) {
        let line = line_of(text, cap.get(0).unwrap().start());
        a.tuning.entry(cap[1].to_string()).or_default().push(line);
    }
    a
}

/// 安全算术求值：数字 / `+ - * /` / 括号 / tuning.lua 顶部 local 常量。
/// 其余表达式（函数调用、未知标识符）返回 None，绝不猜测。
fn eval_arith(expr: &str, locals: &HashMap<String, f64>) -> Option<f64> {
    #[derive(Debug, Clone, PartialEq)]
    enum Tok {
        Num(f64),
        Ident(String),
        Op(char),
        LParen,
        RParen,
    }
    fn tokenize(expr: &str) -> Option<Vec<Tok>> {
        let mut toks = Vec::new();
        let bytes = expr.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b' ' | b'\t' => i += 1,
                b'0'..=b'9' | b'.' => {
                    let start = i;
                    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                        i += 1;
                    }
                    toks.push(Tok::Num(expr[start..i].parse().ok()?));
                }
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                    let start = i;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    toks.push(Tok::Ident(expr[start..i].to_string()));
                }
                c @ (b'+' | b'-' | b'*' | b'/') => {
                    toks.push(Tok::Op(c as char));
                    i += 1;
                }
                b'(' => {
                    toks.push(Tok::LParen);
                    i += 1;
                }
                b')' => {
                    toks.push(Tok::RParen);
                    i += 1;
                }
                _ => return None,
            }
        }
        Some(toks)
    }
    struct P<'a> {
        toks: Vec<Tok>,
        pos: usize,
        locals: &'a HashMap<String, f64>,
    }
    impl P<'_> {
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            while matches!(self.toks.get(self.pos), Some(Tok::Op('+') | Tok::Op('-'))) {
                let op = match self.toks.get(self.pos) {
                    Some(Tok::Op(c)) => *c,
                    _ => break,
                };
                self.pos += 1;
                let r = self.term()?;
                v = if op == '+' { v + r } else { v - r };
            }
            Some(v)
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.factor()?;
            while matches!(self.toks.get(self.pos), Some(Tok::Op('*') | Tok::Op('/'))) {
                let op = match self.toks.get(self.pos) {
                    Some(Tok::Op(c)) => *c,
                    _ => break,
                };
                self.pos += 1;
                let r = self.factor()?;
                v = if op == '*' {
                    v * r
                } else if r != 0.0 {
                    v / r
                } else {
                    return None;
                };
            }
            Some(v)
        }
        fn factor(&mut self) -> Option<f64> {
            match self.toks.get(self.pos)? {
                Tok::Op('-') => {
                    self.pos += 1;
                    Some(-self.factor()?)
                }
                Tok::Op('+') => {
                    self.pos += 1;
                    self.factor()
                }
                Tok::LParen => {
                    self.pos += 1;
                    let v = self.expr()?;
                    if !matches!(self.toks.get(self.pos), Some(Tok::RParen)) {
                        return None;
                    }
                    self.pos += 1;
                    Some(v)
                }
                Tok::Num(n) => {
                    self.pos += 1;
                    Some(*n)
                }
                Tok::Ident(name) => {
                    self.pos += 1;
                    self.locals.get(name).copied()
                }
                _ => None,
            }
        }
    }
    let mut p = P {
        toks: tokenize(expr.trim())?,
        pos: 0,
        locals,
    };
    let v = p.expr()?;
    (p.pos == p.toks.len()).then_some(v)
}

/// 解析 tuning.lua：常量 → (首次定义行, 数值若可求值) 与顶部 local 常量。
fn parse_tuning(text: &str) -> (HashMap<String, usize>, HashMap<String, f64>) {
    let re = regexes();
    let mut locals = HashMap::new();
    for cap in re.local_int.captures_iter(text) {
        if let Ok(v) = cap[2].parse::<f64>() {
            locals.insert(cap[1].to_string(), v);
        }
    }
    let mut lines = HashMap::new();
    let mut values = HashMap::new();
    for cap in re.tuning_entry.captures_iter(text) {
        let name = cap[1].to_string();
        let line = line_of(text, cap.get(0).unwrap().start());
        lines.entry(name.clone()).or_insert(line);
        if values.contains_key(&name) {
            continue;
        }
        if let Some(v) = eval_arith(cap[2].trim(), &locals) {
            values.insert(name, v);
        }
    }
    (lines, values)
}

/// provenance 角色（rank 决定展示优先级）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileRole {
    Definition,
    Canonical,
    Required,
    Mention,
}

impl FileRole {
    fn rank(self) -> u8 {
        match self {
            FileRole::Definition => 0,
            FileRole::Canonical => 1,
            FileRole::Required => 2,
            FileRole::Mention => 3,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            FileRole::Definition => "definition",
            FileRole::Canonical => "canonical",
            FileRole::Required => "required",
            FileRole::Mention => "mention",
        }
    }
}

/// 构建 page↔code 关联。`facts_path` 为 corpus 的 facts.jsonl（断言钩子用，
/// 不存在时跳过钩子）；调用方负责把返回产物原子落盘。
pub fn build(
    store: &CorpusStore,
    registry: &PrefabRegistry,
    scripts_root: &Path,
    reporter: &dyn Reporter,
) -> Result<BuildOutput> {
    let re = regexes();
    let metas = store.load_meta()?;
    let rels = collect_lua(scripts_root)?;
    let mut sources: HashMap<String, String> = HashMap::with_capacity(rels.len());
    for rel in &rels {
        // 游戏脚本树存在非 UTF-8 文件，lossy 读取（与 Python 参考实现的
        // errors="replace" 语义一致）。
        let bytes = std::fs::read(scripts_root.join(rel))?;
        sources.insert(rel.clone(), String::from_utf8_lossy(&bytes).into_owned());
    }
    reporter.log(format!("载入 {} 个 Lua 文件", sources.len()));

    // ---- 声明点 / 皮肤 / strings 键 / tuning / 配方 / 食物 ----
    let mut declarations: HashMap<String, Vec<String>> = HashMap::new();
    let mut skin2base: BTreeMap<String, String> = BTreeMap::new();
    let mut strings_keys: HashSet<String> = HashSet::new();
    let mut tuning_lines: HashMap<String, usize> = HashMap::new();
    let mut tuning_values: HashMap<String, f64> = HashMap::new();
    let mut recipe_lines: HashMap<String, (String, usize)> = HashMap::new();
    let mut food_lines: HashMap<String, (String, usize)> = HashMap::new();
    for (rel, text) in &sources {
        for cap in re.prefab_decl.captures_iter(text) {
            declarations
                .entry(cap[1].to_string())
                .or_default()
                .push(rel.clone());
        }
        for cap in re.skin.captures_iter(text) {
            skin2base
                .entry(cap[1].to_string())
                .or_insert_with(|| cap[2].to_string());
        }
        if rel == "strings.lua" {
            for cap in re.table_ident.captures_iter(text) {
                strings_keys.insert(cap[1].to_lowercase());
            }
        }
        if rel == "tuning.lua" {
            let (lines, values) = parse_tuning(text);
            for (k, v) in lines {
                tuning_lines.entry(k).or_insert(v);
            }
            for (k, v) in values {
                tuning_values.insert(k, v);
            }
        }
        for cap in re.recipe.captures_iter(text) {
            recipe_lines
                .entry(cap[1].to_string())
                .or_insert((rel.clone(), line_of(text, cap.get(0).unwrap().start())));
        }
        for cap in re.food_entry.captures_iter(text) {
            food_lines
                .entry(cap[1].to_string())
                .or_insert((rel.clone(), line_of(text, cap.get(0).unwrap().start())));
        }
    }

    // ---- 二级索引：components / stategraphs / brains ----
    let mut comp_index = BTreeMap::new();
    let mut sg_index = BTreeMap::new();
    let mut brain_index = BTreeMap::new();
    for (rel, text) in &sources {
        if let Some(name) = rel
            .strip_prefix("components/")
            .map(|s| s.trim_end_matches(".lua"))
        {
            let fns: BTreeSet<String> = re
                .fn_def
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .collect();
            let push: BTreeSet<String> = re
                .push
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .collect();
            let listen: BTreeSet<String> = re
                .listen
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .collect();
            let tuning: Vec<String> = re
                .tuning
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .take(40)
                .collect();
            comp_index.insert(
                name.to_string(),
                serde_json::json!({
                    "file": rel,
                    "functions": fns.into_iter().take(60).collect::<Vec<_>>(),
                    "push_events": push,
                    "listen_events": listen,
                    "tuning": tuning,
                }),
            );
        } else if let Some(name) = rel
            .strip_prefix("stategraphs/")
            .map(|s| s.trim_end_matches(".lua"))
        {
            let mut states = Vec::new();
            for m in re.sg_state.find_iter(text) {
                let line = line_of(text, m.start());
                let window_end = (m.start() + 200).min(text.len());
                if let Some(nm) = re.sg_state_name.captures(&text[m.start()..window_end]) {
                    states.push(serde_json::json!({"name": nm[1], "line": line}));
                }
            }
            let handlers: BTreeSet<String> = re
                .handler
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .collect();
            let tuning: Vec<String> = re
                .tuning
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .take(40)
                .collect();
            sg_index.insert(
                name.to_string(),
                serde_json::json!({
                    "file": rel,
                    "states": states.into_iter().take(40).collect::<Vec<_>>(),
                    "handlers": handlers,
                    "tuning": tuning,
                }),
            );
        } else if let Some(name) = rel
            .strip_prefix("brains/")
            .map(|s| s.trim_end_matches(".lua"))
        {
            let nodes: Vec<String> = re
                .brain_node
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .take(60)
                .collect();
            let tuning: Vec<String> = re
                .tuning
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .take(40)
                .collect();
            brain_index.insert(
                name.to_string(),
                serde_json::json!({
                    "file": rel,
                    "nodes": nodes,
                    "tuning": tuning,
                }),
            );
        }
    }

    // ---- 通用事件（按 ListenForEvent 文件数阈值） ----
    let is_aux_dir = |rel: &str| {
        [
            "components/",
            "stategraphs/",
            "brains/",
            "map/",
            "widgets/",
            "screens/",
        ]
        .iter()
        .any(|p| rel.starts_with(p))
    };
    let mut listen_counts: HashMap<String, usize> = HashMap::new();
    for (rel, text) in &sources {
        if is_aux_dir(rel) {
            continue;
        }
        for cap in re.listen.captures_iter(text) {
            *listen_counts.entry(cap[1].to_string()).or_default() += 1;
        }
    }
    let generic_events: HashSet<String> = listen_counts
        .into_iter()
        .filter(|(_, c)| *c > GENERIC_EVENT_FILE_THRESHOLD)
        .map(|(e, _)| e)
        .collect();

    // ---- prefab → 文件（role 分级）+ 依赖追踪 + status ----
    let names: Vec<String> = registry.prefabs.keys().map(|k| k.to_lowercase()).collect();
    let mut prefab_status: HashMap<String, String> = HashMap::new();
    let mut prefab_files: HashMap<String, Vec<(FileRole, String)>> = HashMap::new();
    let mut required_extra: HashMap<String, Vec<String>> = HashMap::new();
    let mut declared = 0usize;
    let mut retired = 0usize;
    let mut no_anchor = 0usize;
    for name in &names {
        let mut entries: Vec<(FileRole, String)> = Vec::new();
        if let Some(defs) = declarations.get(name) {
            for d in defs {
                entries.push((FileRole::Definition, d.clone()));
            }
        }
        if entries.is_empty() {
            let canon = format!("prefabs/{name}.lua");
            if sources.contains_key(&canon) {
                entries.push((FileRole::Canonical, canon));
            }
        }
        if entries.is_empty() {
            for (rel, text) in &sources {
                if text.contains(name.as_str()) {
                    entries.push((FileRole::Mention, rel.clone()));
                }
            }
        }
        let status = if !entries.is_empty() {
            if declarations.contains_key(name) {
                declared += 1;
            }
            "ok"
        } else if strings_keys.contains(name) {
            retired += 1;
            "retired_or_wiki_only"
        } else {
            no_anchor += 1;
            "no_lua_anchor"
        };
        let core: Vec<String> = entries
            .iter()
            .filter(|(r, _)| matches!(r, FileRole::Definition | FileRole::Canonical))
            .map(|(_, f)| f.clone())
            .collect();
        let mut extra: BTreeSet<String> = BTreeSet::new();
        for f in &core {
            if let Some(text) = sources.get(f) {
                for cap in re.req_prefab.captures_iter(text) {
                    let cand = format!("prefabs/{}.lua", &cap[1]);
                    if sources.contains_key(&cand) && !core.contains(&cand) {
                        extra.insert(cand);
                    }
                }
            }
        }
        if !extra.is_empty() {
            required_extra.insert(
                name.clone(),
                extra.into_iter().take(MAX_REQUIRED_PER_PREFAB).collect(),
            );
        }
        entries.sort_by(|a, b| a.0.rank().cmp(&b.0.rank()).then(a.1.cmp(&b.1)));
        entries.truncate(MAX_FILES_PER_PREFAB);
        prefab_status.insert(name.clone(), status.to_string());
        prefab_files.insert(name.clone(), entries);
    }

    // ---- 逐文件锚点缓存 ----
    let mut anchor_cache: HashMap<String, FileAnchors> = HashMap::new();
    fn anchors_of<'a>(
        rel: &str,
        sources: &HashMap<String, String>,
        cache: &'a mut HashMap<String, FileAnchors>,
    ) -> &'a FileAnchors {
        cache
            .entry(rel.to_string())
            .or_insert_with(|| parse_file(sources.get(rel).map(String::as_str).unwrap_or("")))
    }

    // ---- 页面 join ----
    let mut page2prefabs: HashMap<i64, Vec<String>> = HashMap::new();
    for (prefab, pageids) in &registry.prefabs {
        for pid in pageids {
            page2prefabs
                .entry(*pid)
                .or_default()
                .push(prefab.to_lowercase());
        }
    }
    // facts（断言钩子用）：pageid → [(raw, values, unit)]
    let mut facts_by_page: HashMap<i64, Vec<FactRef>> = HashMap::new();
    let facts_path = store.root().join("index/facts.jsonl");
    if let Ok(body) = std::fs::read_to_string(&facts_path) {
        for line in body.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(pid) = v.get("pageid").and_then(serde_json::Value::as_i64) else {
                continue;
            };
            let raw = v
                .get("raw")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            let values: Vec<f64> = v
                .get("values")
                .and_then(serde_json::Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                .unwrap_or_default();
            let unit = v
                .get("unit")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            if !values.is_empty() {
                facts_by_page
                    .entry(pid)
                    .or_default()
                    .push((raw, values, unit));
            }
        }
    }

    let mut seed_patterns: Vec<(Regex, Vec<String>)> = Vec::new();
    for (pat, files) in SEEDS {
        if let Ok(r) = Regex::new(pat) {
            seed_patterns.push((r, files.iter().map(|s| (*s).to_string()).collect()));
        }
    }

    let mut page_jsonl = String::new();
    let mut pages = 0usize;
    let mut pages_linked = 0usize;
    let mut hook_total = 0usize;
    for (pid, meta) in metas.iter() {
        if meta.redirect {
            continue;
        }
        pages += 1;
        let mut prefabs = page2prefabs.remove(pid).unwrap_or_default();
        prefabs.sort();
        prefabs.truncate(MAX_PAGE_PREFABS);
        // 页面侧抽取：注册表只收 dst 信息框变体，ds 变体与台词码在这里补齐
        let page_text = store.read_page(*pid)?;
        let speech_codes: Vec<String> = page_text
            .as_deref()
            .map(|text| {
                re.speech
                    .captures_iter(text)
                    .filter_map(|c| {
                        let s = c[1].trim().trim_end_matches('}').trim();
                        (!s.is_empty()).then(|| s.to_string())
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .take(6)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(text) = page_text.as_deref() {
            // 复用注册表的信息框参数提取（含换行参数形态），覆盖 ds 变体页
            for (arg, _) in crate::corpus::prefab_index::extract_prefabs(text) {
                let arg = arg.to_lowercase();
                if !arg.is_empty() && !prefabs.contains(&arg) {
                    prefabs.push(arg);
                }
            }
        }
        prefabs.truncate(MAX_PAGE_PREFABS);

        let mut file_roles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut components: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> =
            BTreeMap::new();
        let mut sgs: BTreeSet<String> = BTreeSet::new();
        let mut brains: BTreeSet<String> = BTreeSet::new();
        let mut events: Vec<String> = Vec::new();
        let mut generic: BTreeSet<String> = BTreeSet::new();
        let mut tuning: BTreeSet<String> = BTreeSet::new();
        let mut skin_bases: BTreeSet<String> = BTreeSet::new();
        for pf in &prefabs {
            if let Some(base) = skin2base.get(pf) {
                skin_bases.insert(base.clone());
            }
            let mut files = prefab_files.get(pf).cloned().unwrap_or_default();
            if let Some(req) = required_extra.get(pf) {
                files.extend(req.iter().map(|f| (FileRole::Required, f.clone())));
            }
            let base_key = skin2base.get(pf).cloned();
            if let Some(base) = &base_key {
                if let Some(bfs) = prefab_files.get(base.as_str()) {
                    files.extend(bfs.iter().take(4).cloned());
                }
            }
            for (role, f) in &files {
                file_roles
                    .entry(f.clone())
                    .or_default()
                    .insert(format!("{pf}:{}", role.as_str()));
            }
            for (_, f) in &files {
                let a = anchors_of(f, &sources, &mut anchor_cache);
                for (c, (_, methods)) in &a.components {
                    let slot = components
                        .entry(c.clone())
                        .or_insert_with(|| (BTreeSet::new(), BTreeSet::new()));
                    slot.0.insert(f.clone());
                    slot.1.extend(methods.keys().cloned());
                }
                sgs.extend(a.stategraphs.iter().map(|(n, _)| n.clone()));
                brains.extend(a.brains.iter().map(|(n, _)| n.clone()));
                for e in a.events.keys() {
                    let semantic = e.strip_prefix("*push:").unwrap_or(e);
                    if generic_events.contains(semantic) {
                        generic.insert(e.clone());
                    } else if !events.contains(e) {
                        events.push(e.clone());
                    }
                }
                tuning.extend(a.tuning.keys().cloned());
            }
        }
        let curated: Vec<String> = seed_patterns
            .iter()
            .filter(|(r, _)| r.is_match(&meta.title))
            .flat_map(|(_, files)| files.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(8)
            .collect();

        // anchored_files：diff 匹配用的全量锚点文件集 = prefab 侧文件 ∪
        // 组件/状态图/行为树**实现文件**。版本 diff 只需与这一个集合求交
        // （验证：组件文件 join 使 recall 32.5%→51.2%，故升为一级锚点）。
        let mut anchored: BTreeSet<String> = file_roles.keys().cloned().collect();
        for c in components.keys() {
            if let Some(f) = comp_index
                .get(c)
                .and_then(|v| v.get("file"))
                .and_then(serde_json::Value::as_str)
            {
                anchored.insert(f.to_string());
            }
        }
        for s in &sgs {
            if let Some(f) = sg_index
                .get(s)
                .and_then(|v| v.get("file"))
                .and_then(serde_json::Value::as_str)
            {
                anchored.insert(f.to_string());
            }
        }
        for b in &brains {
            if let Some(f) = brain_index
                .get(b)
                .and_then(|v| v.get("file"))
                .and_then(serde_json::Value::as_str)
            {
                anchored.insert(f.to_string());
            }
        }
        for f in &curated {
            anchored.insert(f.clone());
        }
        let anchored_files: Vec<String> = anchored.into_iter().take(80).collect();

        let mut recipe_line = None;
        let mut food_file = None;
        for pf in &prefabs {
            if recipe_line.is_none() {
                if let Some((rel, line)) = recipe_lines.get(pf) {
                    recipe_line = Some(format!("{rel}:{line}"));
                }
            }
            if food_file.is_none() {
                if let Some((rel, line)) = food_lines.get(pf) {
                    food_file = Some(format!("{rel}:{line}"));
                }
            }
        }

        // 断言级候选：facts 数值 × 本页 TUNING 常量值
        let mut hooks = Vec::new();
        let consts: Vec<(&String, f64, usize)> = tuning
            .iter()
            .filter_map(|t| {
                tuning_values
                    .get(t)
                    .zip(tuning_lines.get(t))
                    .map(|(v, l)| (t, *v, *l))
            })
            .collect();
        if !consts.is_empty() {
            if let Some(facts) = facts_by_page.get(pid) {
                for (raw, values, unit) in facts.iter().take(60) {
                    for v in values {
                        let hits: Vec<serde_json::Value> = consts
                            .iter()
                            .filter(|(_, cv, _)| (*cv - v).abs() <= 0.05f64.max(v.abs() * 0.001))
                            .take(3)
                            .map(
                                |(n, cv, l)| serde_json::json!({"name": n, "line": l, "value": cv}),
                            )
                            .collect();
                        if !hits.is_empty() {
                            hooks.push(serde_json::json!({
                                "fact": raw, "value": v, "unit": unit, "tuning": hits
                            }));
                            break;
                        }
                    }
                    if hooks.len() >= MAX_PAGE_HOOKS {
                        break;
                    }
                }
            }
        }
        hook_total += hooks.len();
        if !components.is_empty() || !curated.is_empty() {
            pages_linked += 1;
        }

        let status_of = |pf: &str| {
            prefab_status
                .get(pf)
                .cloned()
                .unwrap_or_else(|| "unregistered".to_string())
        };
        let record = serde_json::json!({
            "pageid": pid,
            "title": meta.title,
            "game_class": meta.game_class,
            "prefabs": prefabs,
            "prefab_status": prefabs
                .iter()
                .map(|pf| (pf.clone(), status_of(pf)))
                .collect::<BTreeMap<_, _>>(),
            "speech_codes": speech_codes,
            "components": components
                .iter()
                .map(|(c, (files, methods))| (
                    c.clone(),
                    serde_json::json!({
                        "files": files.iter().take(4).collect::<Vec<_>>(),
                        "methods": methods.iter().collect::<Vec<_>>(),
                    })
                ))
                .collect::<BTreeMap<_, _>>(),
            "stategraphs": sgs.into_iter().take(8).collect::<Vec<_>>(),
            "brains": brains.into_iter().take(6).collect::<Vec<_>>(),
            "events": events.into_iter().take(MAX_PAGE_EVENTS).collect::<Vec<_>>(),
            "events_generic": generic
                .into_iter()
                .take(MAX_PAGE_GENERIC_EVENTS)
                .collect::<Vec<_>>(),
            "tuning": tuning.into_iter().take(MAX_PAGE_TUNING).collect::<Vec<_>>(),
            "file_roles": file_roles
                .iter()
                .take(MAX_PAGE_FILES)
                .map(|(f, rs)| (f.clone(), rs.iter().collect::<Vec<_>>()))
                .collect::<BTreeMap<_, _>>(),
            "skin_bases": skin_bases.into_iter().collect::<Vec<_>>(),
            "curated_links": curated,
            "anchored_files": anchored_files,
            "recipe_line": recipe_line,
            "food_file": food_file,
            "assertion_hooks": hooks,
        });
        use std::fmt::Write as _;
        let _ = writeln!(page_jsonl, "{record}");
    }

    // ---- prefab 级工件（含每文件锚点） ----
    let mut prefab_json = serde_json::Map::new();
    for name in &names {
        let files: Vec<serde_json::Value> = prefab_files
            .get(name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|(role, f)| {
                let a = anchors_of(&f, &sources, &mut anchor_cache);
                serde_json::json!({
                    "role": role.as_str(),
                    "file": f,
                    "components": a.components.iter().map(|(c, (decls, methods))| (
                        c.clone(),
                        serde_json::json!({
                            "decl_lines": decls,
                            "methods": methods.iter().map(|(m, ls)| (
                                m.clone(), ls.iter().take(8).collect::<Vec<_>>()
                            )).collect::<BTreeMap<_, _>>(),
                        })
                    )).collect::<BTreeMap<_, _>>(),
                    "stategraphs": a.stategraphs.iter().take(6).map(|(n, l)|
                        serde_json::json!({"name": n, "line": l})
                    ).collect::<Vec<_>>(),
                    "brains": a.brains.iter().take(4).map(|(n, l)|
                        serde_json::json!({"name": n, "line": l})
                    ).collect::<Vec<_>>(),
                    "events": a.events.iter().map(|(e, ls)| (
                        e.clone(), ls.iter().take(6).collect::<Vec<_>>()
                    )).collect::<BTreeMap<_, _>>(),
                    "tuning": a.tuning.iter().map(|(t, ls)| (
                        t.clone(), ls.iter().take(6).collect::<Vec<_>>()
                    )).collect::<BTreeMap<_, _>>(),
                })
            })
            .collect();
        prefab_json.insert(
            name.clone(),
            serde_json::json!({
                "status": prefab_status.get(name).cloned().unwrap_or_else(|| "ok".into()),
                "files": files,
                "skin_base": skin2base.get(name),
                "factory_required": required_extra.get(name),
            }),
        );
    }

    let summary = CodeLinksSummary {
        lua_files: sources.len(),
        pages,
        pages_linked,
        prefabs_declared: declared,
        prefabs_with_anchor: names
            .iter()
            .filter(|n| prefab_status.get(*n).is_some_and(|s| s == "ok"))
            .count(),
        prefabs_retired: retired,
        prefabs_no_anchor: no_anchor,
        components: comp_index.len(),
        stategraphs: sg_index.len(),
        brains: brain_index.len(),
        recipe_anchors: recipe_lines.len(),
        assertion_hooks: hook_total,
        skin_mappings: skin2base.len(),
    };
    Ok(BuildOutput {
        summary,
        page_jsonl,
        prefab_json: serde_json::Value::Object(prefab_json),
        component_json: serde_json::Value::Object(comp_index.into_iter().collect()),
        stategraph_json: serde_json::Value::Object(sg_index.into_iter().collect()),
        brain_json: serde_json::Value::Object(brain_index.into_iter().collect()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LUA: &str = r#"
local MakePlayerCharacter = require("prefabs/player_common")
local assets = {}
return MakePlayerCharacter("wilson", prefabs, assets, common_postinit, master_postinit)
"#;

    const SAMPLE_PREFAB: &str = r#"
local brain = require "brains/beefalobrain"
local function master(inst)
    inst:AddComponent("health")
    inst:AddComponent("combat")
    inst.components.health:SetMaxHealth(TUNING.BEEFALO_HEALTH)
    inst.components.combat:SetDefaultDamage(TUNING.BEEFALO_DAMAGE)
    inst.components.lootdropper:SetLoot({})
    inst:SetBrain(brain)
    inst:SetStateGraph("SGBeefalo")
    inst:ListenForEvent("attacked", fn)
    inst:PushEvent("entermood")
end
"#;

    #[test]
    fn parse_file_extracts_all_anchor_kinds() {
        let a = parse_file(SAMPLE_PREFAB);
        let health = a.components.get("health").unwrap();
        assert_eq!(health.0, vec![4]);
        assert!(health.1.contains_key("SetMaxHealth"));
        assert!(a.components.contains_key("lootdropper"));
        assert_eq!(a.stategraphs, vec![("SGBeefalo".to_string(), 10)]);
        assert_eq!(a.brains.len(), 1);
        assert!(a.events.contains_key("attacked"));
        assert!(a.events.contains_key("*push:entermood"));
        assert!(a.tuning.contains_key("BEEFALO_HEALTH"));
    }

    #[test]
    fn player_common_require_is_prefab_dependency() {
        let re = regexes();
        let caps: Vec<_> = re.req_prefab.captures_iter(SAMPLE_LUA).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "player_common");
        // 声明点缺失（工厂形态）→ 无字面 Prefab("
        assert!(!re.prefab_decl.is_match(SAMPLE_LUA));
    }

    #[test]
    fn eval_arith_handles_constants_and_locals() {
        let mut locals = HashMap::new();
        locals.insert("total_day_time".to_string(), 480.0);
        assert_eq!(eval_arith("500 * 2", &locals), Some(1000.0));
        assert_eq!(eval_arith("6*total_day_time", &locals), Some(2880.0));
        assert_eq!(eval_arith("10 / 4", &locals), Some(2.5));
        assert_eq!(eval_arith("GetModConfigData(\"x\")", &locals), None);
        assert_eq!(eval_arith("1 + ", &locals), None);
    }

    #[test]
    fn skin_regex_matches_create_prefab_skin() {
        let re = regexes();
        let text = r#"table.insert(prefs, CreatePrefabSkin("abigail_ancient",
{
	base_prefab = "abigail","#;
        let cap = re.skin.captures(text).unwrap();
        assert_eq!(&cap[1], "abigail_ancient");
        assert_eq!(&cap[2], "abigail");
    }

    #[test]
    fn recipe2_and_food_entry_anchored() {
        let re = regexes();
        let cap = re
            .recipe
            .captures(r#"Recipe2("axe", {Ingredient("twigs", 1)}, TECH.NONE)"#)
            .unwrap();
        assert_eq!(&cap[1], "axe");
        let food = "\tbutterflymuffin =\n\t{\n\t\ttest = function";
        let cap = re.food_entry.captures(food).unwrap();
        assert_eq!(&cap[1], "butterflymuffin");
    }

    #[test]
    fn speech_regex_extracts_codes() {
        let re = regexes();
        let text = "{{全角色台词|dst|bat_bosscorpsehat|首选=wathgrithr}}";
        let cap = re.speech.captures(text).unwrap();
        assert_eq!(&cap[1], "bat_bosscorpsehat");
    }

    #[test]
    fn seed_rules_match_mechanism_titles() {
        let mut patterns = Vec::new();
        for (pat, files) in SEEDS {
            patterns.push((Regex::new(pat).unwrap(), *files));
        }
        let hit = |title: &str| {
            patterns
                .iter()
                .filter(|(r, _)| r.is_match(title))
                .flat_map(|(_, f)| f.iter().to_owned())
                .count()
        };
        assert!(hit("黑暗") > 0);
        assert!(hit("饥饿值") > 0);
        assert!(hit("烹饪") > 0);
        // 种子按标题关键词命中；"皮弗娄"也出现在驯牛机制规则里，故实体页
        // 会附加机制文件（curated_links 与 prefab 锚点互补，非互斥）。
        assert_eq!(hit("皮弗娄牛"), 3);
    }

    #[test]
    fn parse_tuning_reads_values_and_lines() {
        let text = "local total_day_time = 480\nTUNING = {\n    BEEFALO_HEALTH = 500 * 2, -- harder\n    PERISH_FAST = 6*total_day_time,\n}";
        let (lines, values) = parse_tuning(text);
        assert_eq!(lines.get("BEEFALO_HEALTH"), Some(&3));
        assert_eq!(values.get("BEEFALO_HEALTH"), Some(&1000.0));
        assert_eq!(values.get("PERISH_FAST"), Some(&2880.0));
    }
}
