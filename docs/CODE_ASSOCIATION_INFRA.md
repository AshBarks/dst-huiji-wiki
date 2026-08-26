# 基础设施 A：代码侧符号级关联索引（Code Association Infra）

**状态**：设计定稿（待实施）
**日期**：2026-08-26
**关联文档**：[UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md)（主方案 v3，§2.6 关联链取证、§4.2.2 关联链模型为本文前置；本文是"基础设施 A"讨论的固化，独立维护以免与主方案修订互相阻塞）

---

## 0. TL;DR

以 prefabs/ 下 prefab 定义为起点，用 AST 扫描建立**符号级**的代码关联索引：prefab ↔ components/stategraphs/brains/behaviours 的六类关联边（到 prefab 变体、组件方法、行为调用点实参粒度），并产出通用性分型报告与文件级说明文本骨架。它是主方案 Layer B 的前置基础设施：既决定事实提取器的搜索作用域，也决定 diff 落在关联文件时的反向影响传播。

核心设计纪律：**不支持的动态形态显式记 unresolved 进报告，不做猜测性建边**。

## 1. 目标与产物

| # | 产物 | 内容 | 服务对象 |
|---|------|------|---------|
| P1 | **SymbolIndex** | 文件 → 符号树：components 的 class/方法、stategraphs 的 State/EventHandler、brains 的 BT 结构、behaviours 的 ctor 签名，各带行区间 | hunk→符号定位；收窄判定；F3 锚点 |
| P2 | **AssociationIndex** | 六类边的正向 + 反向索引（反向到 prefab **变体**粒度） | 关联文件 hunk 反向传播；提取器作用域 |
| P3 | **GenericityReport** | 使用度分层 + 无页面事实黑名单候选 + 惰性画像清单 | 说明文本排产；降噪 |
| P4 | **说明文本骨架** | `docs/atlas/<dir>/<file>.md` 文件级（L0/L1），带证据链接、可再生成 | LLM 提示词资产 |

## 2. 事实基础（2026-08-26 当前版本全量实测）

### 2.1 规模总览

| 目录 | 文件数 | 被引用 | 备注 |
|------|-------|--------|------|
| prefabs/ | 1582 | — | 索引起点 |
| components/ | 815 | 739 个被 AddComponent 引用 | 冒号方法共 7975 个 |
| stategraphs/ | 260 | 224 个被 SetStateGraph 引用 | State{} ×3405、EventHandler ×3924 |
| brains/ | 191 | 187 个 Class(Brain；prefabs 中 require 出边 108 条 | BrainCommon 被 96/191 引用 |
| behaviours/ | 29 | 经 brains 间接引用（prefabs 直接 require 仅 1 条） | 总计约 2000 行 |
| 助手定义 | 2 | standardcomponents.lua（2067 行/105 fn/**内部 AddComponent×50**）、prefabutil.lua（171 行/2 fn/×6） | 隐藏通道的定义面 |

### 2.2 六类边与形态清单

| 边 | API/通道 | 规模 | 形态分布 | 解析策略 |
|----|---------|------|---------|---------|
| E1 组件边·直连 | `inst:AddComponent("x")` | 8434 次 / 1238 文件 | **100% 字面量，零动态** | 直接建边 |
| E2 组件边·助手展开 | `MakeInventoryPhysics` ×593、`MakeInventoryFloatable` ×485、`MakeHauntableLaunch` ×299、`MakeSmallBurnable` ×226 等 | 数千调用点 | 定义集中于 2 个文件（§2.1） | 对每个 Make* 做一次按版本静态画像（内部加了哪些组件、设了哪些默认值），调用点展开为虚拟边，锚点=调用行 |
| E3 状态图边 | `inst:SetStateGraph("SGx")` | 269 次 / 235 文件 | 261 字面量；~8 处动态（`a and "A" or "B"`）；`"SG"..` 拼接全库仅 2 处 | 字面量直建；and/or 折叠多目标边；不可解记 unresolved |
| E4 大脑边 | `inst:SetBrain(expr)` | 208 次 / 172 文件 | 多数为局部变量名；少量表达式（`custombrain or brain`、`enable and brain or nil`、`SetBrain(nil)`） | 三步：①文件级 `require("brains/z")`→局部变量绑定；②共享 fn 参数流回溯（如 hound.lua 的 `fncommon(..., custombrain, ...)` 各调用点实参）；③or 表达式折叠为**多目标边** |
| E5 Prefab deps | `Prefab(name, fn, assets, deps)` 第 4 参（`local prefabs = {}` 声明见 850 文件） | SpawnPrefab 声明图 | 表内字面量 | 直接建边；F4「生成自」基础 |
| E6 行为边（复合） | brain 内 behaviour ctor 调用 | wander×27、leash×14、faceentity×13、chaseandattack×12 个 brain 引用 | 见 §2.4 | **边 + 调用点实参快照**（不是纯边） |

二跳上下文：stategraphs 引用 CommonStates（186/260 个 SG，commonstates.lua 2974 行）会向 states 表注入状态——按 E2 同款助手画像处理；brains 引用 BrainCommon（96/191）作为 brain 内符号上下文收录（其仅直接 require 一个 behaviour，不构成隐藏通道）。

### 2.3 覆写判定的坑：inst 别名分析

prefab 内组件方法调用点共 21048 处，但前缀分布：`inst.` 仅 ~82%（17310），其余为回调里操作**其他实体**（`target.`×416、`v.`×402、`owner.`×302、`TheWorld`×184、`item.`×130…）。**只有构造中实体自身的调用才计入该 prefab 的覆写区间**。收集器须在 fn 作用域内做别名绑定分析（`local x = inst` 也算 inst）。

### 2.4 behaviours/ 的独特形态：边 + 实参快照

- behaviours 内部 TUNING 引用为 **0**——参数全部由 brain 调用点实参注入：

```lua
-- brains/houndbrain.lua
Wander(self.inst, GetHomePos, 8),          -- :179 有家猎犬游荡半径 8
Wander(self.inst, GetWanderPoint, 20),     -- :180 野生猎犬游荡半径 20
ChaseAndAttack(self.inst, 10),             -- :152 追击时长 10
```

- ctor 签名的参数名即语义：`Wander = Class(BehaviourNode, function(self, inst, homelocation, max_dist, times, ...))`。
- ⇒ E6 必须索引成「边 + BehaviourCall 记录」复合结构：`{prefab_variant, brain_file, call_site, 按 ctor 签名对齐的实参解析结果}`。实参解析复用 resolve 三件套（字面量直读 / TUNING 表 / brain 文件级常量表）；`GetHomePos` 类闭包标记 location-provider 不解析。只建 brain→behaviour 纯边价值有限，事实在实参里。

### 2.5 多 prefab 共享是常态

- hound.lua 一文件 8 变体（函数作用域归属问题，主方案已覆盖）；
- SGhound.lua 头部注释自述"也被 warglet 使用"——一 SG 多 prefab 是常态 ⇒ 反向索引必须落到 prefab 变体粒度。

## 3. 架构设计

### 3.1 模块布局

```
src/update/index/
├── scan.rs       # Pass 1 Collect：full_moon AST 逐文件独立收集
│                 #   requires / local 赋值 / fn 区间 / Prefab 注册 /
│                 #   调用点(E1~E6, components.*:method) / State{} 计数
├── resolve.rs    # Pass 2 Resolve：全库跨文件解析
│                 #   字面量直连 → require 绑定 → 同文件参数流(fn-call graph)
│                 #   → and/or 折叠 → Make*/CommonStates 助手展开
├── symbols.rs    # SymbolIndex 数据模型（P1）
├── edges.rs      # AssociationIndex 数据模型，正反向邻接（P2）
├── genericity.rs # 分型统计与黑名单（P3）
└── atlasmd.rs    # docs/atlas/*.md 生成器（P4，A2 期）
```

依赖方向不变：update → {parser, wiki, models}；full_moon 为既有依赖（lua52 特性集）。

### 3.2 两阶段解析流水线

- **Pass 1 Collect**：逐文件独立、可并行、可按内容哈希增量。产出该文件的原始符号与调用点集合。
- **Pass 2 Resolve**：全库视图下解析跨文件边。顺序：字面量直连（无需上下文）→ require 变量绑定 → 同文件参数流 → or 折叠 → 助手展开（依赖助手画像表）。
- **unresolved 台账**：每类不可解析形态计数 + 样例定位进报告（E3 的 `data.sg`、E4 的数据驱动传参等）。原则：宁进报告让人看一眼，不做猜测性建边。

### 3.3 数据模型草案

```rust
/// 六类边统一模型
struct AssocEdge {
    kind: EdgeKind,            // Component | StateGraph | Brain | PrefabDep | Behaviour | HelperVirtual
    from: PrefabRef,           // (file, variant_name) —— 到变体粒度
    to: FileRef,               // 目标文件（HelperVirtual 时为调用行锚点）
    anchor: Anchor,            // file+line，报告与 prompt 共用
    confidence: Confidence,    // Resolved | Folded(多目标) | Unresolved
}

/// E6 的复合记录
struct BehaviourCall {
    edge: AssocEdge,
    brain_file: PathBuf,
    call_site: u32,
    args: Vec<ArgValue>,       // 按 ctor 签名参数名对齐
}
enum ArgValue { Literal(Literal), Tuning(String), LocalConst(String), LocationProvider, Unresolved }

/// 符号画像（P1/P4 共用）
struct SymbolProfile {
    file: PathBuf,
    symbols: Vec<Symbol>,      // class/methods/states/nodes + 行区间
    page_visibility: PageVisibility,   // none | A(auto) | B(manual-strong) | C(multi-hop)
    evidence: Vec<PageSample>, // 样例页链接（scan B 上线后回填）
}
```

### 3.4 缓存与版本化

- 产物挂 `output/atlas/<build>/`，输入为 DstContext 快照目录，每版本构建一次；
- Pass 1 按文件内容哈希增量重建；全量重建亦为秒~十秒级（4010 个 .lua 的 Rust full_moon 解析），增量只作加速不作正确性依赖；
- 幂等性测试保证增量与全量产物一致；
- 说明文本（P4）为生成物：git 版本化 + 证据链接 + 可再生成；人只维护纠错覆写层。

## 4. GenericityReport（P3）

实测分层（组件按引用 prefab 数）：通用(≥100)=**17 个**、常见(10~99)=85、小众(2~9)=221、专用(=1)=**416（56%）**。

- 深度画像只排产通用+常见 ≈ **102 个文件**；专用组件仅做文件级 stub；
- 黑名单两阶段：先名称启发式（net_/render/updater/sync 类，如 spawnfader/updatelooper——8434 条组件边中此类无一是页面事实来源），Scan B（wiki 大面积扫描）上线后以「零页面证据」修正为实证黑名单；
- behaviours/ 自成「最通用层」，29 文件全量画像优先级最高（成本约 2000 行）；
- 巨型文件（SGwilson 类玩家状态图）**惰性画像**：hunk 命中时现做，不预排产。

## 5. 测试策略

1. **hound 金标准 fixture**：8 变体 × 六类边断言，含 moonhound→moonbeastbrain 参数流（fncommon 实参）、houndfire 的 deps 边、MakeHauntableChangePrefab 虚拟边；
2. **warglet fixture**：SGhound 变更反查出 {hound 系变体, warglet}；
3. **behaviour 实参 fixture**：houndbrain 的 Wander/ChaseAndAttack 调用点实参解析（8/20/10 与 GetHomePos→LocationProvider）；
4. **unresolved 回归集**：已知动态形态样例，防止解析器升级把"不支持"静默变成"错建边"；
5. **inst 别名单测**：`target.components.combat:` 不计入本 prefab 覆写；`local x = inst; x.components...` 计入；
6. **增量等价性**：全量 vs 增量产物 byte-identical。

## 6. 分期落地

| 期 | 内容 | 支撑的主方案里程碑 |
|----|------|------------------|
| **A0** | scan.rs 全量收集 + E1/E3/E5 字面量边 + E4 的 require 绑定 + 正反向索引 + unresolved 台账 | M1 报告的"关联文件变更 → 受影响实体"清单 |
| **A1** | inst 别名分析→覆写行区间（收窄策略）、E2 助手展开（standardcomponents/prefabutil）、E6 边+BehaviourCall 收集器与字面量实参解析、GenericityReport L0/L1、behaviours 全量画像 | M2 F1/F2 的作用域与降噪；M3 收窄 |
| **A2** | 参数流/or 折叠完整化（嵌套表达式实参）、符号级画像（page_visibility 标注）、atlasmd 说明文本生成 | M4 F3 brain/SG 事实提取 |

## 7. 风险与对策

| 风险 | 对策 |
|------|------|
| Lua 动态性长尾（数据驱动、字符串拼接、运行时构造） | 已实测压至个位数~低两位数；一律 unresolved 台账显式暴露，绝不猜边 |
| Class 名冲突（如 Writeable 在多个组件文件重复出现） | 符号按文件局部作用域组织，不做全局名字合并 |
| 助手互相调用/返回值链（helper 返回 inst 后继续配置） | 助手画像保留"内部组件边 + 默认值"，锚点双向指向调用行与定义行；递归深度限 1，更深记 unresolved |
| 画像漂移（游戏更新改变代码结构） | 产物挂快照 build 号；Pass 1 增量随内容哈希自动失效 |
| 相关性标签误伤（把相关标成无关导致漏报） | 标签只是先验非闸门：更新时 old_literal 精确回查命中可推翻任何"无关"标签（安全阀，与主方案一致） |

## 8. 决策记录

1. **说明文本存放——已拍板（2026-08-26）**：落仓库 `docs/atlas/`，git 版本化；产物新鲜度的 CI 校验随 M2 一并引入；
2. **L0 黑名单人工复核——已执行（2026-08-26）**：Universal 档 24 组件逐个判定——21 个承载页面事实（lootdropper/combat/health/perishable 等）保留；新增种子 `placer`（纯放置预览 UI）、`knownlocations`（引擎位置缓存）、`timer`（通用计时管道）；既有碎片规则（fader/updater/looper/netvar 族）经实名单核对无误杀；
3. E2 助手画像范围：已在实现期解决（§9 修正记录 3、§10）。

---

## 9. 落地记录（A0+A1 已实现，2026-08-26）

代码位于 `src/update/index/`（scan/resolve/symbols/edges/genericity 五模块 + mod.rs 构建入口 `build_from_sources` / `build_from_dir`），未接 CLI（避免与并行任务冲突，接线随 M1）。

**真实游戏树冒烟**（当前版本 scripts 目录）：

| 指标 | 数值 |
|------|------|
| 扫描文件 | 2879（parse_failures = 0） |
| 关联边 | 27868（Component 含助手展开、StateGraph 268、Brain 105、PrefabDep 4160） |
| BehaviourCall | 1074 条（含实参快照） |
| unresolved | **55**：helper_unknown 49 + brain_unresolved 3 + sg_unresolved 3 |
| 反向索引抽查 | components/combat.lua ← 200+ prefab 变体；stategraphs/SGhound.lua ← hound 系变体 + warglet |
| 方案核心断言 | moonhound → {houndbrain(Direct), moonbeastbrain(Folded)} 经 fncommon 参数流正确解析 |

**实现期修正记录**：

1. **无括号 require 形态**：DST 脑文件普遍使用 `require "behaviours/wander"`（空格形式），解析器同时支持两种；
2. **inst 别名规则补充**：除参数传入外，`local inst = CreateEntity()` 构造式绑定同样入别名集（fncommon 类工厂函数的主流写法）；
3. **零组件助手画像**：MakeInventoryPhysics 等走引擎级 `entity:AddPhysics()`、不含任何 AddComponent——为所有扫描到的全局 Make* 定义建立（可能为空的）画像，调用点静默解析而非误报 unknown。

测试：8 组嵌入式 Lua fixture（hound 全链/warglet 共享 SG/behaviour 实参/unresolved 回归/inst 别名/解析失败容错等）+ DST__ROOT 门控的真实树冒烟，全部通过；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 干净。

---

## 10. 落地记录（TuningTable 与 update-index 命令，2026-08-26）

- **TuningTable**（`src/update/index/tuning.rs`）：真实 tuning.lua 为「`function Tune(overrides)` 内局部常量折叠 + 单个巨型 `TUNING = {…}` 表构造」，非逐行赋值。解析器先对数值局部变量做不动点折叠，再求值表内 NameKey 字段：标量数字/字符串入表，表值/依赖 overrides 的表达式记入 skipped 台账。full_moon 的 token Display 携带空白 trivia——数值解析前必须 trim。
- **集成**：BehaviourCall 实参中 `TUNING.X` 经 `resolve_tuning_field` 出真值（Num/Str）；真实树解析出 **4558** 个标量。
- **产物接口**：`IndexArtifact.schema_version = 1`（正式契约字段）；组合产物 `AtlasBuild { schema_version, build_id, index, tuning }` 由 `build_atlas_from_dir(root)` 构建。
- **CLI**：`update-index <scripts-root> [--out DIR]`（JobKind::UpdateIndex，纯本地无 wiki 写入），落盘 `output/atlas/<build>/index.json + tuning.json`；build 号优先取 version.txt，否则从快照目录名 `scripts_<时间戳>` 提取，兜底 "unknown"。

---

## 11. 落地记录（Page→Symbol 标注与 CLI，2026-08-27）

### 11.1 目标

将代码侧 symbol 的“页面影响面”标注出来，为后续 Code→Page 提供前置过滤，并通过高引用 symbol 的跨页扫描发现“该提没提 / 提法不一致”。

### 11.2 已实现

- `src/update/symbol_page.rs`：
  - P0：`SymbolRef` / `SymbolKind` / `SymbolPageVisibility` / `PageEvidence` / `SymbolPageAnnotation`
  - P1：`annotate_symbol()` 从代码索引解析受影响变体 → 页面 → 候选事实证据
  - P2：`top_symbols()` / `build_symbol_evidence_packs()` / `render_symbol_pack_md()`
  - P3：`render_symbol_annotation_prompt()` / `PageSymbolVerdict` / `SymbolAnnotationResponse` / `parse_symbol_annotation_response()`
  - P4：`SymbolCoverageReport` / `MissingPage` / `InconsistentPage` / `build_coverage_report()` / `build_coverage_reports()` / `render_coverage_report_md()`
- CLI `symbol-annotate`：
  - 参数：`root`、`--corpus`、`--limit`、`--out`、`--verdicts`、`--llm`
  - 输出：`symbol_packs.json`、`symbol_prompts.md`、可选 `symbol_verdicts.json`、`symbol_coverage.json/.md`
- LLM 配置层 `src/llm.rs`：
  - 环境变量：`LLM__API_KEY`、`LLM__BASE_URL`、`LLM__MODEL`
  - 未配置 `LLM__API_KEY` 时跳过 LLM 标注；配置后请求失败返回 `Error::Llm`

### 11.3 验证

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过
- 测试：命令解析、LLM 配置缺失、P0–P4 单测、全量 lib 测试（排除 real-tree smoke）
- 模拟 P4 示例报告见 `docs/symbol_coverage_example.md` / `.json`
