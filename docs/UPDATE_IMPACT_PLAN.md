# 游戏更新影响评估与自动修订方案（Update Impact Assessment）

**状态**：v3.4（实施中）
**修订记录**：
- v1（初稿）：三层流水线总体框架、静态规则注册表、LLM 输出契约
- v2：根据对实体页面内容分层的深入取证重新聚焦——静态数据管线已被既有工具与其他维护脚本覆盖，方案重心收敛到**手写内容的实体事实管线**（详见 §2.5 取证与 §4.2 重设计）
- **v3（本版）**：补全**实体关联链深度追溯**设计——以 prefabs/ 下 prefab 定义为起点，经 `AddComponent`/`SetStateGraph`/`SetBrain` 追溯到 components/、stategraphs/、brains/ 的关联文件（§2.6 取证、§4.2.1 关联链索引行、§4.2.2 关联链模型与追溯规则、§4.2.4 编辑取舍模型）；影响传播从"单文件归属"扩展为"链式传播"
- **v3.1**：决策落定——①基础设施预构建方向确认：代码侧符号级关联索引 + wiki 大面积扫描产出 CodeTextAtlas 与 per-file/per-fn 说明文本（入仓库，作为 LLM 提示词资产；fn 级标注排期 M4）；②相关性标签是先验非闸门：old_literal 回查命中可推翻任何低相关标签；③§5 多数开放问题结案（见该节标记）；④页面全量语料抓取方案独立成文：[WIKI_CORPUS_PLAN.md](WIKI_CORPUS_PLAN.md)
- **v3.2**：§4.2.4 增补**章节级 LLM 介入边界表**（基于全语料结构分析 [CORPUS_PAGE_PATTERNS.md](CORPUS_PAGE_PATTERNS.md) + 语料实证抽样；含无标题导语区边界、料理配方≠制作配方的溯源区分、制作章新增的纯代码检验点）
- **v3.3**：§4.2 各小节标注可执行状态；**§4.2.6 页面匹配修订**——语料侧注册表（pages_by_prefab.json）取代"标题候选猜测+API 往返"，匹配变为离线确定性查表；PageSegmenter 补信息框参数级细分以支撑 F1 字节级锚定
- **v3.4（2026-08-26 实施记录）**：M2/M3a/M4 前段落地——F1 loot、F2 stats、F3 行为常量子集、fn 标注 MVP、BrainEdge 动态解析验证、atlas 全覆盖关联渲染、F4 SpawnPrefab 反向生成首版、create-check 清单正式化、state.json 缓存断点续跑、Page→Symbol 标注 P0/P1/P2/P3/P4 + `symbol-annotate` CLI runner + LLM API 配置层（SymbolPageAnnotation + 受影响页/证据组装 + 高引用 symbol 证据包 + Prompt/输出契约 + 跨页一致性报告；未配置 LLM 时跳过，配置后请求失败即报错）；join 阶段大小写归一（case_only 不再进人工纠错）、vault_crawler create_check 关闭；§4.2.2 反向传播收窄已实施（组件变更仅传播本地覆写者，超阈值聚合）。

**关联文档**：[MAINTENANCE_TOOL_AUDIT.md](MAINTENANCE_TOOL_AUDIT.md)（现有工具评估，§6 改进项已落地）、[CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md)、[WIKI_CORPUS_PLAN.md](WIKI_CORPUS_PLAN.md)（语料底座）

---

## 0. TL;DR

每次游戏更新后，一条命令完成：**结构化 diff → 受影响实体页面清单（精确到"页面的哪一句因哪个 hunk 过期"）→ 静态管线照常触发 → 手写内容由确定性建议或 LLM 起草修改 → 人审后推送 → 报告与审计**。

v2 的核心认知修正：

1. **静态部分不重复建设**。recipes/tabx/常量模块等由本项目既有管线与其他维护脚本负责；且这些 Data 页被 wiki 模块运行时直查，更新后全站引用处自动重渲染，无需逐页修改。新流程只做"触发 + 核对"（Tier0）。
2. **实体提取器的真正靶心是页面中"手写但代码强溯源"的内容**：infobox 的手写参数（掉落 Pic 串、生成来源等）与正文机制描述中的数值事实——它们转写自 `prefabs/<x>.lua` 及其关联的 components/brains/stategraphs。
3. **影响映射的基本单位是 FactChange**：`(prefab, 字段, 旧字面量, 新字面量, 证据)`。用旧值在页面手写区域做精确回查命中，而不是整页相关性猜测。
4. **前置改造已完成**：WikiClient 节流/重试/basetimestamp/批量 API/错误细分均已落地（见审计报告 §6 落地状态）。
5. **实体不是孤立的一个 fn，而是一条关联链**（v3）：prefab 定义经 `AddComponent`/`SetStateGraph`/`SetBrain` 挂接 components/、stategraphs/、brains/ 的代码——页面"行为"章节的事实大量出自这些**关联文件**（如猎犬"找 30 单位内食物"的锚点在 `brains/houndbrain.lua` 而非 hound.lua）。关联链索引是一级基础设施：既决定事实提取的搜索范围，也决定 diff 落在关联文件时的反向影响传播。

---

## 1. 目标与非目标

### 1.1 背景

实测更新节奏约每 1~3 周一次（databundles 下 2025-03 至 2026-05 共 40 个手工快照）。更新后维基工作分两类：

- **数据同步**：tabx/常量模块/Prefab JSON 数据页——已有明确维护渠道（本工具静态管线、其他维护脚本、鲁鲁bot），且模块运行时直查使内容页展示自动更新；
- **手写内容修订**：编辑们把代码里的机制与数值人工转写成页面正文和 infobox 参数，版本更新后逐句过期——目前完全靠人肉翻 diff。

现状缺口是后者没有系统性工具支撑。页面上甚至留有编辑者的等待注释（如猎犬页 `<!--lulu：实体数据更新后再来删掉这个-->`），说明需求真实存在。

### 1.2 目标

| # | 目标 | 验收标准 |
|---|------|----------|
| G1 | 更新后产出《影响评估报告》：受影响实体页面清单，每条精确到"哪个字段/哪句话 × 哪个 hunk 证据" | 对下次真实更新人工盘点，覆盖率 ≥90% |
| G2 | 静态管线自动触发并核对，报告标注"已自动处理"避免重复劳动 | Tier0 零人工干预 |
| G3 | 强溯源手写内容（掉落表、数值替换）给出确定性或 LLM 起草的修改建议，校验后人审推送 | llm/auto 建议采纳率 ≥80%；错误编辑 0 起 |
| G4 | 新内容检查清单（缺页实体、新增配方/字符串） | 清单完整率可核对 |
| G5 | 全程幂等、可审计、可回滚 | state.json 断点续跑；编辑记录 oldrevid/newrevid |

### 1.3 非目标

- 不重复实现静态数据管线（只调度既有 maintain-* 与核对结果）；
- 不接管 `Data:DST Prefab/*.json`（鲁鲁bot）与 `模块:DST Strings CN/EN`（疑似 bot）——仅一致性核对通报；
- 不处理弱溯源内容（攻略/花絮/多跳推导机制描述）的自动改写——只提示；
- 不做 wikitext 通用语法重构。

---

## 2. 事实基础（调研结论）

### 2.1 游戏侧

**scripts.zip 关键文件**（解压后 4010 个 .lua，272MB）：

| 文件 | 规模 | 说明 |
|------|------|------|
| `recipes.lua` | ~910 个 Recipe/Recipe2 | 静态管线已覆盖 |
| `languages/chinese_s.po` | 429,073 行 | 静态管线已覆盖 |
| `tuning.lua` | 9,534 行 | **数值事实的最终来源**；prefabs 下有 8,216 处 `TUNING.*` 引用 |
| `prefabs/*.lua` | 1,582 个文件 | **手写内容的主要代码对应物**（见 §2.5） |
| `components/*.lua` | 815 个文件 | 组件默认值；prefab 通常在 AddComponent 后覆写。prefabs 中 1238 文件共 8434 条组件边（§2.6） |
| brains/ 191、stategraphs/ 260、behaviours/ 29 | — | 行为逻辑（正文"行为"章节的主要素材，v3 起纳入关联链索引，见 §2.6） |
| `speech_*.lua` ×17 | 角色台词 | 台词模板自动消费 |

**版本识别**：游戏根 `version.txt` build 号；`DstContext` 已读取并支持快照目录。

**现有人工流程**：更新时旧目录改名 `scripts_<yyyymmddhhmm>` + 解压新版 + 手工 diff（如 `0529.diff`，302 文件）。databundles 下已有 40 个历史快照 = 免费的离线测试金标准。

### 2.2 维基侧数据架构

```
游戏代码 ──(本工具静态管线)──> Data:ItemTable.tabx / Data:DSTRecipes.tabx / 模块:Constants/* ──┐
游戏代码 ──(鲁鲁bot)──────> Data:DST Prefab/<prefab>.json ×3869                               │ 运行时直查
游戏代码 ──(疑似 bot)─────> 模块:DST Strings CN/EN                                            ├─> 模块:AutoInfobox 等
                                                                                              ▼
                                                            内容页 {{实体信息框/自动|dst|<prefab>|手写参数…}} + 手写正文
```

关键机制（v2 补充实证）：

1. **AutoInfobox 的数值来自 `mw.huiji.loadJson('DST_Prefab/'..prefab)`**（模块源码第 56 行），即生命/伤害/饥饿/燃料等展示全部自动。配方栏走 DSTRecipes.tabx 直查，台词走 Strings 模块。
2. **手写参数 > 自动数据**：infobox 同名参数被页面显式赋值时覆盖自动值（如火腿棒页 `|装备/伤害 = 59.5~29.5` 是公式化手写）。这些覆盖参数正是会过期的手工数据点。
3. **Data 页运行时直查 ⇒ 静态管线更新即全站生效**：改一个 tabx 无需回填任何内容页。⇒ 静态部分在新流程中降级为"触发+核对"。

### 2.3 他方 bot 观察

`Data:DST Prefab/*` 由鲁鲁bot 在更新后 1~3 天内批量更新。我们的报告将其列为核对项（窗口期内提示"Prefab JSON 尚未同步，以下自动展示可能滞后"）。

### 2.4 API 行为观察（工程约束）

| 观察 | 工程结论 |
|------|----------|
| 连续快速请求间歇 403（WAF） | ✅ 已落地节流+退避重试（WikiClient RateLimitCfg） |
| 搜索后端不稳 | 用 ItemTable 缓存 + titles 批量查询定位页面 |
| allpages ≤500 需续传；titles 批量 ≤50 | ✅ list_all_pages / get_pages_meta 已封装 |
| MediaWiki 1.38.4 formatversion=1 BC 布尔 | 解析需兼容 `"missing": ""` 形态 |

### 2.5 实体页面内容分层取证（v2 核心依据）

以"猎犬"页全文比对 `prefabs/hound.lua` + `tuning.lua` 得到的三层模型：

| 层 | 内容 | 来源 | 处置 |
|----|------|------|------|
| **A 自动层** | infobox 数值（生命/伤害/攻速/食物值）、配方栏、台词、名称图片 | `loadJson('DST_Prefab/')`、tabx、Strings | 不碰；Tier0 核对 |
| **B 手写·强溯源层** | 见下表 | prefabs/*.lua 及关联组件 | **本功能靶心** |
| **C 手写·弱溯源层** | 提示/花絮/攻略、多跳推导事实 | 行为链推理 | 仅提示 |

B 层实例（页面原文 ↔ 代码锚点一一对照）：

```
|掉落 = {{Pic|32|怪物肉}}×1<br>{{Pic|32|犬牙}}×1（12.5%） ← SetSharedLootTable('hound',{{'monstermeat',1.0},{'houndstooth',0.125}})
正文："死亡后生成 3 团猎犬火焰"                            ← NUM_HOUND_FIRE = 3（firehound OnDeath 循环）
正文："有猎犬丘的猎犬仇恨范围为 20 单位"                    ← FindEntity(inst, TUNING.HOUND_TARGET_DIST=20)
正文："寒冰猎犬对 4 单位内施加 2 层冰冻"                    ← icehound 死亡回调
```

反例（C 层）：野狗"仇恨范围 100 单位"在 hound.lua 无直接出处（多跳，源自袭击系统）——此类必须标记为不可自动改写。

**决定设计的四个代码侧事实**：

1. **一文件多 prefab**：hound.lua 定义 8 个变体（hound/firehound/icehound/moonhound/clayhound/mutatedhound/hedgehound/houndfire），各 fn 内独立设属性。统计确认此为主流组织方式 ⇒ 事实归属必须做**函数作用域分析**（可复用 prefab_override parser 的局部函数追踪技术）。
2. **数值引用形态混合**：`SetMaxHealth(TUNING.X)` 为主、少量字面量（spiderden 200）。prefabs 下模式出现次数：SetMaxHealth×264、SetDefaultDamage×245、SetRange×203、SetLoot×227、SetAttackPeriod×169、SetPerishTime×127、SetRetargetFunction×141。
3. **掉落表结构高度规整**：`SetSharedLootTable('name', { {'prefab', chance}, ... })` 与页面 Pic 串几乎一一对应（12.5% ↔ 0.125）——最理想的确定性提取目标。
4. **TUNING 反向索引必需**：改 tuning.lua 一个 key 影响所有引用它的 prefab 对应页面。

### 2.6 实体关联链取证（v3 核心依据）

以 hound 为例逐行核对，一个 prefab 的完整关联链：

```lua
-- prefabs/hound.lua
local brain = require("brains/houndbrain")          -- :55 文件级 require → 局部变量
local moonbrain = require("brains/moonbeastbrain")  -- :56
local function fncommon(bank, build, morphlist, custombrain, tag, data)  -- :459 brain 作为参数流入
    inst:AddComponent("spawnfader")                 -- :504 字符串字面量 → components/spawnfader.lua
    inst:SetStateGraph("SGhound")                   -- :520 名称串 → stategraphs/SGhound.lua（运行时按名加载，无 require）
    inst:SetBrain(custombrain or brain)             -- :538 动态表达式：默认 houndbrain，moonhound 传入 moonbrain
end
fncommon("hound", "hound_ocean", nil, moonbrain, "moonbeast", false)     -- :730 参数实参
return Prefab("hound", fndefault, ...), Prefab("firehound", fnfire, ...), ...  -- :920-928 一文件 8 变体
```

**三类关联边的规模与形态**（当前版本全量统计）：

| 关联边 | API | 文件数 | 调用数 | 形态分布 |
|--------|-----|-------|--------|---------|
| 组件边 | `inst:AddComponent(name)` | 1238 | 8434 | **100% 字符串字面量，零动态形态** ⇒ 可确定性建边 |
| 状态图边 | `inst:SetStateGraph("SGx")` | 235 | 269 | 261 处 SG 名字面量；~8 处动态（`isghost and "A" or "B"`、`data.stategraph or "SGboat"`）|
| 大脑边 | `inst:SetBrain(expr)` | 172 | 208 | 多数为局部变量名（需 require 追踪 + 共享 fn 参数流）；少量表达式（`enable and brain or nil`、`SetBrain(nil)`）|

被引用方规模：components/ 815 个文件、stategraphs/ 260 个（prefabs 引用其中 224 个不同 SG）、brains/ 191 个（prefabs 中 require 出边 108 条）。behaviours/ 仅 1 条来自 prefabs 的直接出边——行为叶子节点由 brains 间接引用，属二跳关联。

**反向影响面（关联文件变更波及多少实体页）**：combat 被 227 个 prefab 添加、health 199、locomotor 198、lootdropper 482；inspectable 高达 1122 但无页面事实。⇒ 关联文件 diff 的反向传播必须配"收窄策略"（§4.2.2），否则报告即噪音。

**关联文件确实是事实富矿**（TUNING 引用密度：prefabs 9229 / components 1529 / stategraphs 834 / brains 338——关联目录合计约占全库 1/4），且部分页面事实**只**存在于关联文件：

| 页面原文（猎犬页） | 锚点位置 | 说明 |
|--------------------|----------|------|
| "主动寻找 30 距离单位内的肉类食物和可怕食物" | `brains/houndbrain.lua:16` `local SEE_DIST = 30` | 事实在**大脑文件**的文件级常量里，不在 prefab |
| "猎犬具有群体仇恨：…周围 30 单位内最多 5 只…共享仇恨" | `prefabs/hound.lua:146` `SHARE_TARGET_DIST = 30` + `ShareTarget(...)` 回调 | 在 prefab 内，但经 combat 组件 API 生效 |
| infobox/正文的理智光环数值 | `stategraphs/SGhound.lua:153` `sanityaura.aura = -TUNING.SANITYAURA_MED` | 组件属性在**状态图**里配置而非 prefab fn |
| "海象营地的寒冰猎犬会保持在主人 2~6 距离…" | walrus_camp.lua 等第三方 prefab 的生成配置 | **同一 prefab 因生成语境不同而行为不同** |

反例（取舍对照）：`spawnfader`（淡入淡出网络同步）这类技术组件共 8000+ 条组件边中的大多数**永远不会出现在任何页面上**——关联链的提取价值高度依赖编辑取舍规则（§4.2.4）。

---

## 3. 总体架构（v3）

```
┌──────────────── Layer A 快照与 Diff（不变）───────────────────┐
│ UpdateDetector → SnapshotStore → StructuredDiff(diff.json)     │
└──────────────────────────┬─────────────────────────────────────┘
                           ▼
┌──────────────── Layer B 影响映射（v2 重构）────────────────────┐
│ Tier0 静态管线：规则注册表命中 → 触发既有 maintain-*            │
│                 + 结果核对登记（"已自动处理"）                   │
│                                                                │
│ 实体事实管线（主线）：                                          │
│   diff hunk ──实体提取──> 受影响 prefab 集合                    │
│      ↓ PrefabIndex/AssociationIndex/TuningTable（每版本缓存）   │
│      ⇠ 关联文件 hunk 反向传播：components/stategraphs/brains    │
│         变更 → 关联链反查受影响 prefab 集合（收窄后入报告）       │
│   事实提取器族 F1~F4 ──> FactChange{field, old, new, evidence}  │
│      ↓                                                         │
│   页面解析(prefab→标题候选→存在性校验) + 区域分割(B层限定)       │
│      ↓ old_literal 精确回查                                     │
│   ImpactPlan[{page, region, tier, fact, evidence}]              │
└──────────────────────────┬─────────────────────────────────────┘
                           ▼
┌──────────────── Layer C 修复执行 ──────────────────────────────┐
│ Tier1 auto   : 静态管线调度（--dry-run/--yes/--report-json 已备）│
│ Tier2 llm    : 按 fact-kind 模板起草 edits → 四重校验 → 人审     │
│ Tier3 report : report.md + impact.json + applied.jsonl          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. 详细设计

### 4.1 Layer A：快照管理与结构化 Diff（与 v1 一致）

- 更新检测（version.txt/zip hash）、SnapshotStore 目录布局、`snapshot import` 兼容 databundles 快照；
- DiffEngine 产出 `FileDiff{path,status,hunks}`（similar 行级）+ changes.patch；
- 金标准（实测修正）：人工流程将更新前旧树改名为 `scripts_<更新时刻>`，故 0529.diff 实际对照 `scripts_202605291134 ↔ 当前 scripts`（在其后下一次官方更新落地前有效）；快照对 0427↔0529 仅作相邻版本回归参考。

### 4.2 Layer B：实体事实管线（v2 核心）

**可执行状态速览（v3.4，2026-08-26 更新）**：

| 小节 | 状态 | 依据 |
|------|------|------|
| 4.2.0 Tier0 触发与核对 | ✅ 已接线 | update-scan 已产出 tier0 命中；实际 maintain-* 调度仍人工确认 |
| 4.2.1 每版本基础设施 | ✅ 已完成 | PrefabIndex/AssociationIndex/TuningTable=update-index 工件；PageSegmenter=语料侧 regions.jsonl |
| 4.2.2 关联链反向传播 | ✅ 已完成首版 | 组件变更仅传播本地覆写者；超过 30 实体聚合为计数行 |
| 4.2.3 F1 掉落表 | ✅ 已完成 | loot 提取/配对/join/定级闭环；L1 人工比对结论已回填 |
| 4.2.3 F2 数值属性 | ✅ 已完成（M3a 只读留档） | stats 提取/归属/配对/定级/审阅记录；apply 写回仍冻结 |
| 4.2.3 F3/F4 | F3 常量子集 ✅；F4 首版 ✅ | F3 brains 常量提取与配对已落地；F4 SpawnPrefab 反向生成边/atlas 展示已落地 |
| 4.2.4 编辑取舍模型 | ✅ 已完成初版 | 取舍配置 TOML（TakeupConfig）已落地 |
| 4.2.5 FactChange 类型 | ✅ 已完成 | fact.rs 统一模型已落地 |
| 4.2.6 页面匹配与定级 | ✅ 已完成 | 注册表查表 + 大小写归一；vault_crawler create_check 已关闭；create-check 清单正式化为 create_check.json/.md |

#### 4.2.0 Tier0：静态管线触发与核对

规则注册表（impact-rules.toml）保留，但职责收窄：
- po/recipes/constants/debugcommands/recipes_filter 变更 → 调度既有 maintain-*（WriteMode::DryRun/AutoConfirm）；
- 结果登记为 `auto_handled`，报告中单列，不再进入实体匹配流程；
- 附带核对项：DST Strings/Prefab JSON 是否已被他方 bot 同步（窗口期提醒）。

#### 4.2.1 每版本基础设施（构建一次、缓存复用）

| 设施 | 输入 | 产出 | 用途 |
|------|------|------|------|
| `PrefabIndex` | 扫描 prefabs/*.lua（AST） | prefab → {file, fn 区间, 文件级 local 常量, 引用的 TUNING keys, loot 表名, SpawnPrefab 出边} | hunk→prefab 归属；F4 反向引用；tuning 反查 |
| `AssociationIndex`（v3） | 在 PrefabIndex 扫描中一并产出 | 正向：prefab → {component 集合+覆写行区间, stategraph, brain 集合}；反向：components//stategraphs//brains/ 各文件 → 引用它的 prefab 列表 | 关联文件 hunk 反向传播；扩展提取器作用域（见 §4.2.2） |
| `TuningTable` | tuning.lua（后续含 override 分段） | key → 字面量值 | F2 提取时解析数值；FactChange 的 new_literal |
| `PageSegmenter` | 页面 wikitext | 区域树：RichTab 子页 / 模板骨架(保护) / 参数值 / 章节 / 散文 / 注释 / 分类 | B 层限定匹配；LLM 上下文裁剪 |

注意：猎犬类页面用 `{{RichTab/信息框}}`在一页内嵌多个变体 infobox——**页面↔prefab 是双向多对多**，分割器必须支持子页结构，每个子页独立绑定 prefab code。

#### 4.2.2 实体关联链模型与追溯规则（v3 新增）

以 prefab 定义为起点、三类出边的有向图（每条边带 file+line 锚点证据）：

| 边类型 | 解析机制 | 置信度 |
|--------|---------|--------|
| **ComponentEdge** → `components/x.lua` | `inst:AddComponent("x")` 字面量直接建边——全库 8434 处**零动态形态**，可完全确定性建边 | 确定 |
| **StateGraphEdge** → `stategraphs/SGy.lua` | `"SGy"` 字面量为主；`isghost and "A" or "B"` 形态做常量折叠产出多目标边；`data.sg` 等数据驱动形态记 unresolved | 高 / 中 |
| **BrainEdge** → `brains/z.lua` | 三步解析：① 文件级 `require("brains/z")` → 局部变量绑定；② 共享 fn 参数流回溯（`fncommon(..., custombrain, ...)` 各调用点实参，复用 prefab_override parser 的跨函数变量追踪技术）；③ `custombrain or brain` 表达式展开为**多目标边**（hound → {houndbrain, moonbeastbrain}）| 高（需 AST 追踪）/ unresolved 降级 |

索引产物：
- 正向：prefab → {fn 区间, components + 各组件在 AddComponent 之后的覆写行区间, stategraph, brains, TUNING keys, loot 表名, 文件级 local 常量表}；
- 反向：components/、stategraphs/、brains/ 每个文件 → 引用它的 prefab 清单（到变体粒度，如 hound 的 8 个 Prefab 名）。

**反向传播规则**（diff 落点不在 prefabs/ 时，沿反向边找受影响实体页）：

| diff 落点 | 传播路径 | 收窄策略 |
|-----------|---------|---------|
| `stategraphs/SGx.lua` | → 引用 SGx 的全部 prefab | 天然收窄（269 边 ÷ 224 个 SG ≈ 每 SG 低个位数 prefab），直接逐页展开 |
| `brains/z.lua` | → SetBrain 解析含 z 的全部 prefab | 同上 |
| `components/c.lua` | → AddComponent(c) 的全部 prefab | **必须收窄**：combat 227 / lootdropper 482 / inspectable 1122。首期规则：仅保留"该 prefab 在自身 fn 内覆写过被改成员"的行（未覆写者才真正消费组件默认值）；收窄后仍超阈值（>30 页）则聚合为报告计数行 + 人工评估提示，不逐页展开 |
| `behaviours/*.lua` | 二跳：behaviour ← brain ← prefab | 沿 BrainEdge 反向两跳 |

**深度记账扩展**（derivation_depth 覆盖链式跳数）：事实锚点到页面的转写距离——prefab 自身 setter = 1；关联文件内常量/配置 = 2（如 houndbrain 的 `SEE_DIST = 30`、SGhound 内 sanityaura 配置）；跨实体语境配置（海象营地覆盖寒冰猎犬行为）≥ 3 ⇒ 只提示不代改。

#### 4.2.3 事实提取器族（按精度排序，分期落地）

提取器作用域 = prefab 自身 fn ∪ AssociationIndex 可达关联文件，再经编辑取舍规则过滤（§4.2.4）。

| 族 | 代码锚点 | 页面对应物 | 自动化程度 | 期次 |
|----|----------|-----------|-----------|------|
| **F1 掉落表** | `SetSharedLootTable`（规整 `{prefab,chance}`）/`SetLoot`/`SpawnLootPrefab` | infobox `\|掉落=` Pic 串（×数量、（概率%）） | ★★★ 近乎确定性生成 | **M2 首发** |
| **F2 数值属性** | health/combat/locomotor/perishable setter（见 §2.5 统计） | infobox 覆盖参数 + 正文带数字句子 | ★★☆ 数值可靠，句子定位交给 LLM | M2~M3 |
| **F3 行为机制** | retargetfn/keeptargetfn/OnDeath 等回调内距离/次数/条件；**brain 文件内 BT 参数与文件级常量（SEE_DIST 类）、SG 状态超时/事件** | "行为"章节机制描述（其锚点大量位于关联文件，见 §2.6） | ★☆☆ 多数仅提示；关联文件常量类可达 ★★☆ | M3 之后 |
| **F4 反向生成** | 全局 SpawnPrefab/spawner 引用 | `\|生成自=` Pic 列表 | ★☆☆ 依赖全局索引 | M3 之后 |

**函数作用域归属**（F1~F3 共同前提）：diff hunk 只有行号，需回答"属于哪个 prefab"。实现：PrefabIndex 记录每个文件的 `local function <fn>(...)` 区间与 `Prefab("x", fnY, ...)` 的绑定关系；hunk 行号 → 所在 fn → prefab。跨文件共享 fn（common_fn 模式，如 hound 的 fncommon）沿调用链向上归并到最终 Prefab 名。**关联文件侧同理**：hunk 落在 brains//stategraphs//components/ 时经 AssociationIndex 反向边归属到 prefab 集合。

**TUNING 解析**：提取时把 `TUNING.HOUND_DAMAGE` 解析为具体值（复用 models/recipe/context.rs 的 resolve_tuning 思路）；tuning_override 的分段/条件结构首期不支持，遇到即降级 manual 并记录。关联文件内的**文件级 local 常量**（如 houndbrain 的 `SEE_DIST = 30`）由 PrefabIndex 常量表解析，语义等同 TUNING 值。

#### 4.2.4 编辑取舍模型（v3 新增）

从猎犬页逐段比对归纳的转写规律，作为提取器的过滤与表达约束。声明式配置（TOML），随样本页积累演进：

| 规律 | 页面证据 | 对提取器/生成器的约束 |
|------|---------|---------------------|
| 只转写**玩家可感知参数**（距离/时长/概率/次数/触发条件）；BT 节点名、SG 状态名、net_var 等内部标识符从不上页面 | 全部实体页通例 | FactChange.field 面向页面语义命名；代码符号只进 evidence，不进目标文本 |
| **技术组件是噪音**：纯实现性组件（spawnfader/updatelooper 等）8434 条组件边中无一上页 | 猎犬页无任何 spawnfader 痕迹 | 组件分级清单：combat/health/lootdropper/perishable/eater/sanityaura/workable… 进提取域；net/render/sync 类整体排除 |
| 同一 prefab 的**条件语境展开为分句枚举** | "有[[猎犬丘]] 20 / 海象营地寒冰猎犬 10 单位"等分句并列；~~"完全野生仇恨 100 单位"~~ 经 40 快照历史核实系**时间误读为距离**（100 = `max_chase_time` 秒，追击时长上限），不属语境数值——定案与逐句映射见 [KNOWLEDGE_BEHAVIOUR_CHAIN.md](KNOWLEDGE_BEHAVIOUR_CHAIN.md) §1.3 | FactChange 增加 `context` 维度，各语境值独立匹配、互不误伤；语境值进入 context 维度前**必须先核实其数值语义来源常量**（野生句的反例：数值真实存在于代码但语义是时长，与另外两句的距离类数值不可并列），时间/距离类混淆以 §1.3 教训为准 |
| **跨实体机制用链接不复制** | "每隔一段时间成群袭击玩家，详见[[猎犬袭击]]" | 被链接实体变更时仅提示"核对引用表述"，不代改 |
| 数值带单位与解释模板 | "4 {{解释\|距离单位}}"、"燃烧 6 ~ 12 秒"、"×1（12.5%）" | 确定性重生成与 LLM 模板必须复刻单位/解释/区间/Pic 写法（F1 特权路径同源） |

**章节级介入边界**（v3.2 增补；全语料实证见 [CORPUS_PAGE_PATTERNS.md](CORPUS_PAGE_PATTERNS.md) §7）：

| 章节区 | 内容来源与形态 | LLM 边界 |
|--------|--------------|---------|
| **导语区**（无标题：模板栈后至首个 h2） | 页面主体。身份公式句 + 核心机制概述段——B 层事实最密集带（死亡效果/特殊互动/生成条件），常含数字与跨链接；可含变体子章节（`===火焰猎犬===`） | 数值/字面量句 **draft**（old_literal 命中）；无字面量的单跳事实句（"死亡时冒出恐怖猎犬"类）先 **flag**，待关联索引的 SpawnPrefab 类边就绪后再评估升格；身份公式句仅重命名事件驱动 |
| 信息框手写参数（掉落串/伤害/攻击间隔/耐久） | B 层结构化参数，旧值回查最可靠 | **draft**（F1 特权路径 / F2 锚定） |
| **行为**（生物页核心手写区） | 距离/仇恨/次数等数值 + 机制叙述与战术取舍混合 | 主介入章：数值句 **draft**（depth≤2）；机制叙述、需要取舍的内容 **flag** |
| 制作 | 纯 A 层模板 `{{RRBI\|X}}`，零手写内容 | 文本永不碰；**新增检验点（纯代码）**：实体首次出现在 recipes.lua 任一 `Ingredient(...)` ⇒ 页面应有制作章/RRBI 引用，缺失进 create-check 类报告 |
| 获取 / 料理烹饪（料理页） | 料理配方是**规则约束**（preparedfoods.lua 的 foodtype/priority/cooktime 等），非精确材料表 ⇒ 页面呈现为"散文规则摘要 + `{{烹饪}}` 示例枚举"；示例非唯一真值 | 先 **flag**（溯源属静态管线领域）；校对时不得把示例枚举当完整清单 |
| 自定义世界 | 数值定义于 worldsettings_overrides.lua，但具体机制解释可能跨多文件；页面为手写 wikitable | **flag**（与 Q3 决议一致）；代码追溯索引建立后另行评估结构化替换 |
| 提示 / 策略 | 代码事实 + 玩家需求的推论，主观经验；有一定代码依赖但**无代码追溯价值** | **flag**（被引用实体变更时提示核对表述），不代改 |
| 花絮 / 皮肤 / Bug / 画廊 | 外部元信息或人工维护内容 | **ignore**（花絮的重命名监听为可选低成本项） |

横切规则（优先于章节表）：①章节内部分层由 **old_literal 回查是否命中**决定，不由章节决定——命中的句子才可能进 draft；②信息框调用结构/RichTab 组织/导航模板属 segmenter 保护区，LLM 只能改参数值不能动结构，"模板平衡破坏"是硬失败；③HTML 注释、`{{待补充}}` 等协作痕迹不可触碰，列入四重校验保护清单。

维护方式：MVP 用生物类样本页（猎犬/蜘蛛/猎犬丘等 3~5 页）人工标定初版；此后每次人审驳回按"不该改 / 不该管 / 格式错"归类回填配置。

#### 4.2.5 FactChange 模型

```rust
struct FactChange {
    prefab: String,            // 归属实体
    kind: FactKind,            // Loot | Stat | Behavior | Backref
    field: String,             // 页面语义命名："找食距离" / "combat.damage" / "loot[houndstooth]"
    context: Option<String>,   // 条件语境（§4.2.4）："野生" / "有猎犬丘" / "海象营地成员"；None = 无条件
    source_file: String,       // 锚点所在文件——可能是关联文件（brains/stategraphs/components）
    old: Option<Literal>,      // None = 新增
    new: Option<Literal>,      // None = 删除
    derivation_depth: u8,      // 1=prefab 内 setter；2=关联文件常量/配置；3+=跨实体链式推导（只提示）
    evidence: Vec<EvidenceRef> // file+hunk 定位，报告与 prompt 共用
}
```

#### 4.2.6 页面匹配与定级

1. **（v3.3 修订）** prefab → 页面直查：语料侧注册表 `index/pages_by_prefab.json` 提供变体→pageid 的**确定性映射**（RichTab 多变体自然展开；join 质量实测见 [CORPUS_CODE_ATLAS_CONTRACT.md](CORPUS_CODE_ATLAS_CONTRACT.md) §6，悬空清单即校准输入）。原"ItemTable 标题候选 + API 存在性往返"设计废除——匹配完全离线；注册表未命中的变体直接进 create-check 清单。API 批量校验仅保留为可选的语料新鲜度核对（只读，不写维基）；
2. 取 wikitext，PageSegmenter 切区，**只在 B 层（参数值+散文）检索**；
3. `old.literal` 精确回查（数值按格式容差：`0.125`↔`12.5%`、距离单位写法）；
4. 定级：

| 条件 | tier |
|------|------|
| Tier0 规则命中 | auto_handled（触发管线+登记） |
| F1 命中（B 层找到旧掉落串） | llm（优先：确定性重生成建议，人审即采纳） |
| F2 命中且 depth=1 | llm（stat-update 模板） |
| B/F2 命中但 depth≥2 或语义句 | manual（附双方证据链接） |
| 关联文件变更经反向传播命中 prefab，但页面无旧值回查落点 | manual（仅报告行 + 链路证据，草稿降为"建议核查"提示） |
| 他方维护目标（Prefab JSON/Strings） | notify-only |
| 新实体无页面 | create-check 清单 |

### 4.3 Layer C：修复执行

与 v1 相同的三层结构（校验器四重门、review bundle、apply 授权分级、basetimestamp 保护均保留），两点调整：

1. **LLM 任务模板按 FactKind 细化**：`loot-sync`（多数情况退化为确定性模板填充，不经 LLM）、`stat-update`、`text-sync`、`section-note`（manual 类的"建议核查"批注草稿）；
2. **F1 特权路径**：掉落表新旧结构都规整时，直接由代码生成目标 Pic 串（`{{Pic|32|X}}×N（P%）`），作为"确定性建议"进 review bundle——零幻觉风险，预期占 llm 类的大头。

其余（Provider 抽象、上下文包预算、成本预估、报告章节）沿用 v1 设计。

### 4.4 CLI（不变）

```
update-scan [--from] [--to] [--full]   # A+B → impact.json + report.md（只读）
update-fix  [--plan] [--only-tier] [--apply none|llm-auto|llm-all] [--yes]
snapshot import <dir>
update-report <pair-dir>
```

### 4.5 模块布局（v2 调整）

```
src/update/
├── mod.rs / snapshot.rs / diffdata.rs / rules.rs     # 与 v1 一致
├── index/
│   ├── prefab_index.rs    # AST 扫描：fn 区间/TUNING 引用/loot 表/SpawnPrefab 出边/local 常量
│   ├── association.rs     # v3：三类关联边建边（字面量/require 追踪/参数流）+ 正反向索引
│   └── tuning_table.rs    # key→值（复用 resolve_tuning 思路）
├── segment.rs             # PageSegmenter：RichTab/模板骨架/参数值/章节/散文
├── facts/
│   ├── mod.rs             # FactChange/FactKind 模型
│   ├── loot.rs            # F1（首发）
│   ├── stats.rs           # F2
│   ├── behavior.rs        # F3
│   └── backref.rs         # F4
├── matchmod.rs            # FactChange×页面 B 层回查与定级
├── pageresolve.rs / assess.rs / execute/ / report.rs   # 与 v1 一致
```

依赖方向不变：update → {parser, copyclip, wiki, mapping, models}。

### 4.6 前置改造（✅ 已完成，2026-08）

审计报告 §6 所列 P0/P1/P2 已全部落地并随 `7cfbafb` 入库：节流重试、basetimestamp/assert、错误细分、get_pages_meta/page_exists/list_all_pages、WriteMode/--yes/--dry-run/--report-json、凭据收敛、run_id 日志 span。217 测试通过，真实 API 冒烟通过。

### 4.7 测试策略

- 金标准 e2e：0427→0529 快照对（对应人工 0529.diff）；
- **猎犬案例固定为 F1/F2 回归 fixture**：断言能从 hound.lua 提取 8 个变体的 loot/health/damage 事实、正确归属函数作用域、`HOUND_TARGET_DIST` 变更时命中文页"20 单位"句子；
- **关联链 fixture（v3）**：① hound → SGhound 边与 hound → {houndbrain, moonbeastbrain} 多目标边解析正确（含 fncommon 参数流）；② `brains/houndbrain.lua` 的 `SEE_DIST` 变更命中页面"寻找 30 距离单位内的肉类食物"句；③ `components/combat.lua` 变更时收窄策略生效——覆写过 combat 成员的 prefab 保留、未覆写者按阈值聚合；④ spawnfader 类技术组件变更不产生任何页面行；
- 提取器正/漏/误例集；segmenter 对 RichTab/嵌套模板/HTML 注释的结构化断言；
- validate.rs 拒绝路径全覆盖；LLM 录制/回放，CI 不打真 API。

### 4.8 里程碑（v3 调整）

| 阶段 | 内容 | 出口条件 |
|------|------|----------|
| M1 A+Tier0 | snapshot/diff/rules + 静态管线调度与核对报告 + **AssociationIndex 字面量边（component/SG）与反向传播计数** | ✅ 基本完成 |
| M2 F1 MVP | PrefabIndex(loot 部分)+TuningTable+loot.rs+segmenter(RichTab)+匹配+确定性建议 | ✅ 已完成并闭环 |
| M3 F2 | stats.rs+stat-update LLM 模板+apply 流程实战 | 🟨 F2+审阅记录完成，apply 写回按用户要求冻结 |
| M4 F3/F4+打磨 | behavior/backref、**BrainEdge 动态解析（require 追踪/参数流/常量折叠）**、取舍配置回填、create-check、缓存断点续跑 | 🟨 F3/BrainEdge/atlas/F4 首版/create-check/state.json 完成；待下一次真实版本端到端验收 |

### 4.9 风险与对策（v2/v3 增补）

| 风险 | 对策 |
|------|------|
| 组件默认值变更（diff 在 components/ 而非 prefabs/）影响面难归属 | **v3 收窄策略**：经 AssociationIndex 反查后，仅保留在自身 fn 内覆写过被改成员的 prefab；仍超阈值（>30 页）聚合为计数行 |
| 共享组件爆炸半径（combat×227、lootdropper×482） | 同上收窄策略兜底；inspectable 等无页面事实组件直接进黑名单 |
| 动态关联形态解析失败（`data.sg` 数据驱动等） | 记 unresolved 并降级 manual；全库动态样例为个位数（SG 边 ~8 处），长尾可控 |
| 多跳推导事实（如野狗仇恨 100 来自袭击系统） | derivation_depth≥2 一律 manual，绝不代改；跨实体语境配置 ≥3 只提示 |
| 一页多实体/RichTab/消歧义命名长尾 | segmenter 子页绑定 + page-index 缓存积累 + 解析失败进 manual |
| LLM 幻觉 | F1 确定性特权路径绕开 LLM；输出契约+四重校验+人审兜底 |
| PO/tabx 巨大体量 | 静态管线已有全量重建能力，Tier0 只做调度 |
| WAF 限流 | 已落地节流重试；夜间低峰 + 断点续跑 |

---

## 5. 开放问题（v3.1 结案标记）

1. ~~`模块:DST Strings CN/EN` 与 `Data:DST Prefab/*` 的维护者协调渠道？~~ **已关闭**：不介入他方自动化数据管理，只做项目内自动数据更新 + diff 驱动的人工文本修订；
2. F2 的正文数值句定位：**搁置实时定位**，转化为 CodeTextAtlas 增量维护问题——一次性建库 → 数据修改驱动文本修订 → 文本修改反推重建关联 → 定时扫描新增；建库依赖语料抓取（[WIKI_CORPUS_PLAN.md](WIKI_CORPUS_PLAN.md)）；
3. ~~TuningTable 是否处理 tuning_override.lua 的世界选项分段？~~ **已关闭**：不处理；
4. 生物类之外的页面结构差异：**由语料大面积扫描解决**——提炼通用范式与特殊结构写成提示词供 LLM 参考；
5. ~~LLM 选型与预算额度？~~ **暂缓**：先不考虑 F2 消耗的预算；
6. ~~报告是否投递 wiki 用户页？~~ **暂缓**：先不考虑投递；
7. （v3）组件收窄的"覆写过被改成员"判定粒度与聚合阈值——随实现定，暂记 fn 级粗粒度 + >30 页聚合为缺省；
8. （v3）FactChange 的 context 取值清单能否半自动枚举——并入语料大面积扫描阶段讨论。

---

## 6. 当前实施状态与下一步计划（2026-08-27）

### 6.1 已完成（截至 2026-08-27）

| 模块/能力 | 状态 |
|-----------|------|
| M1 快照/diff/Tier0/反向传播 | ✅ 基本完成 |
| M2 F1 掉落表 | ✅ 完成并闭环 |
| M3a F2 数值提取+审阅记录 | ✅ 完成（apply 写回暂冻结） |
| F3 行为常量子集 | ✅ 完成 |
| BrainEdge 动态解析 | ✅ 验证完成 |
| atlas fn 归属 / 发散关联渲染 | ✅ 完成 |
| F4 SpawnPrefab 反向生成 | ✅ 首版完成 |
| create-check 清单化 | ✅ 完成 |
| state.json 缓存断点续跑 | ✅ 完成 |
| Page→Symbol 标注 P0/P1/P2/P3/P4 | ✅ 完成 |
| `symbol-annotate` CLI runner | ✅ 完成 |
| LLM API 配置层 | ✅ 完成（未配置跳过，配置后失败报错） |

### 6.2 当前 CLI

```bash
# 生成高引用 symbol 证据包 + Prompt
dst-huiji-wiki symbol-annotate <scripts-root> \
  --corpus wikis/dontstarve.huijiwiki.com \
  --limit 20

# 使用已有 LLM/人工 verdicts 生成覆盖报告
dst-huiji-wiki symbol-annotate <scripts-root> \
  --corpus wikis/dontstarve.huijiwiki.com \
  --verdicts verdicts.json

# 配置 LLM 后直接调用大模型标注
dst-huiji-wiki symbol-annotate <scripts-root> \
  --corpus wikis/dontstarve.huijiwiki.com \
  --llm
```

### 6.3 下一步计划

1. **Page→Symbol 实跑**：用真实 LLM 或人工标注跑一轮高引用 symbol，生成实际 `symbol_coverage`，验证 missing / inconsistent 质量；
2. **四档 region tier 落地**：把 draft/flag/ignore/Tier0-only 写入 region/证据包，接入 `grade` 与 Prompt；
3. **Code→Page 方向**：基于 Page→Symbol 标注结果实现“代码变更 → 页面生成/修订”；
4. **recentchanges 增量抓取**：按 `WIKI_CORPUS_PLAN.md` §12 实施；
5. **真实版本端到端验收**：下次游戏更新后跑 `update-scan --corpus` + `symbol-annotate`，形成完整人工比对包。
