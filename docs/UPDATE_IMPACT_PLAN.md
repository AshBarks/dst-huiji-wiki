# 游戏更新影响评估与自动修订方案（Update Impact Assessment）

**状态**：v2 细化版（评审中）
**修订记录**：
- v1（初稿）：三层流水线总体框架、静态规则注册表、LLM 输出契约
- **v2（本版）**：根据对实体页面内容分层的深入取证重新聚焦——静态数据管线已被既有工具与其他维护脚本覆盖，方案重心收敛到**手写内容的实体事实管线**（详见 §2.5 取证与 §4.2 重设计）

**关联文档**：[MAINTENANCE_TOOL_AUDIT.md](MAINTENANCE_TOOL_AUDIT.md)（现有工具评估，§6 改进项已落地）、[CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md)

---

## 0. TL;DR

每次游戏更新后，一条命令完成：**结构化 diff → 受影响实体页面清单（精确到"页面的哪一句因哪个 hunk 过期"）→ 静态管线照常触发 → 手写内容由确定性建议或 LLM 起草修改 → 人审后推送 → 报告与审计**。

v2 的核心认知修正：

1. **静态部分不重复建设**。recipes/tabx/常量模块等由本项目既有管线与其他维护脚本负责；且这些 Data 页被 wiki 模块运行时直查，更新后全站引用处自动重渲染，无需逐页修改。新流程只做"触发 + 核对"（Tier0）。
2. **实体提取器的真正靶心是页面中"手写但代码强溯源"的内容**：infobox 的手写参数（掉落 Pic 串、生成来源等）与正文机制描述中的数值事实——它们转写自 `prefabs/<x>.lua` 及其关联的 components/brains/stategraphs。
3. **影响映射的基本单位是 FactChange**：`(prefab, 字段, 旧字面量, 新字面量, 证据)`。用旧值在页面手写区域做精确回查命中，而不是整页相关性猜测。
4. **前置改造已完成**：WikiClient 节流/重试/basetimestamp/批量 API/错误细分均已落地（见审计报告 §6 落地状态）。

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
| `components/*.lua` | 815 个文件 | 组件默认值；prefab 通常在 AddComponent 后覆写 |
| brains/stategraphs/behaviours | 各数十~数百文件 | 行为逻辑（正文"行为"章节的素材） |
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

---

## 3. 总体架构（v2）

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
│      ↓ PrefabIndex/TuningTable（每版本基础设施，缓存）          │
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
- 金标准：`scripts_202604271353 ↔ scripts_202605291134` 对照人工 0529.diff。

### 4.2 Layer B：实体事实管线（v2 核心）

#### 4.2.0 Tier0：静态管线触发与核对

规则注册表（impact-rules.toml）保留，但职责收窄：
- po/recipes/constants/debugcommands/recipes_filter 变更 → 调度既有 maintain-*（WriteMode::DryRun/AutoConfirm）；
- 结果登记为 `auto_handled`，报告中单列，不再进入实体匹配流程；
- 附带核对项：DST Strings/Prefab JSON 是否已被他方 bot 同步（窗口期提醒）。

#### 4.2.1 每版本基础设施（构建一次、缓存复用）

| 设施 | 输入 | 产出 | 用途 |
|------|------|------|------|
| `PrefabIndex` | 扫描 prefabs/*.lua（AST） | prefab → {file, fn 区间, 引用的 TUNING keys, loot 表名, SpawnPrefab 出边} | hunk→prefab 归属；F4 反向引用；tuning 反查 |
| `TuningTable` | tuning.lua（后续含 override 分段） | key → 字面量值 | F2 提取时解析数值；FactChange 的 new_literal |
| `PageSegmenter` | 页面 wikitext | 区域树：RichTab 子页 / 模板骨架(保护) / 参数值 / 章节 / 散文 / 注释 / 分类 | B 层限定匹配；LLM 上下文裁剪 |

注意：猎犬类页面用 `{{RichTab/信息框}}`在一页内嵌多个变体 infobox——**页面↔prefab 是双向多对多**，分割器必须支持子页结构，每个子页独立绑定 prefab code。

#### 4.2.2 事实提取器族（按精度排序，分期落地）

| 族 | 代码锚点 | 页面对应物 | 自动化程度 | 期次 |
|----|----------|-----------|-----------|------|
| **F1 掉落表** | `SetSharedLootTable`（规整 `{prefab,chance}`）/`SetLoot`/`SpawnLootPrefab` | infobox `\|掉落=` Pic 串（×数量、（概率%）） | ★★★ 近乎确定性生成 | **M2 首发** |
| **F2 数值属性** | health/combat/locomotor/perishable setter（见 §2.5 统计） | infobox 覆盖参数 + 正文带数字句子 | ★★☆ 数值可靠，句子定位交给 LLM | M2~M3 |
| **F3 行为机制** | retargetfn/keeptargetfn/OnDeath 等回调内距离/次数/条件 | "行为"章节机制描述 | ★☆☆ 多数仅提示 | M3 之后 |
| **F4 反向生成** | 全局 SpawnPrefab/spawner 引用 | `\|生成自=` Pic 列表 | ★☆☆ 依赖全局索引 | M3 之后 |

**函数作用域归属**（F1~F3 共同前提）：diff hunk 只有行号，需回答"属于哪个 prefab"。实现：PrefabIndex 记录每个文件的 `local function <fn>(...)` 区间与 `Prefab("x", fnY, ...)` 的绑定关系；hunk 行号 → 所在 fn → prefab。跨文件共享 fn（common_fn 模式，如 hound 的 fncommon）沿调用链向上归并到最终 Prefab 名。

**TUNING 解析**：提取时把 `TUNING.HOUND_DAMAGE` 解析为具体值（复用 models/recipe/context.rs 的 resolve_tuning 思路）；tuning_override 的分段/条件结构首期不支持，遇到即降级 manual 并记录。

#### 4.2.3 FactChange 模型

```rust
struct FactChange {
    prefab: String,            // 归属实体
    kind: FactKind,            // Loot | Stat | Behavior | Backref
    field: String,             // "combat.damage" / "loot[houndstooth]" / ...
    old: Option<Literal>,      // None = 新增
    new: Option<Literal>,      // None = 删除
    derivation_depth: u8,      // 1=直接 setter；2+=链式推导（只提示）
    evidence: Vec<EvidenceRef> // file+hunk 定位，报告与 prompt 共用
}
```

#### 4.2.4 页面匹配与定级

1. prefab → 标题候选（ItemTable 中文名缓存 + 消歧义变体 + RichTab 子页检测）→ API 批量存在性校验（get_pages_meta）；
2. 取 wikitext，PageSegmenter 切区，**只在 B 层（参数值+散文）检索**；
3. `old.literal` 精确回查（数值按格式容差：`0.125`↔`12.5%`、距离单位写法）；
4. 定级：

| 条件 | tier |
|------|------|
| Tier0 规则命中 | auto_handled（触发管线+登记） |
| F1 命中（B 层找到旧掉落串） | llm（优先：确定性重生成建议，人审即采纳） |
| F2 命中且 depth=1 | llm（stat-update 模板） |
| B/F2 命中但 depth≥2 或语义句 | manual（附双方证据链接） |
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
│   ├── prefab_index.rs    # AST 扫描：fn 区间/TUNING 引用/loot 表/SpawnPrefab 出边
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
- 提取器正/漏/误例集；segmenter 对 RichTab/嵌套模板/HTML 注释的结构化断言；
- validate.rs 拒绝路径全覆盖；LLM 录制/回放，CI 不打真 API。

### 4.8 里程碑（v2 重排）

| 阶段 | 内容 | 出口条件 |
|------|------|----------|
| M1 A+Tier0 | snapshot/diff/rules + 静态管线调度与核对报告 | 重放出 0529 文件清单；报告可用 |
| M2 F1 MVP | PrefabIndex(loot 部分)+TuningTable+loot.rs+segmenter(RichTab)+匹配+确定性建议 | 猎犬 fixture 全绿；对 0427→0529 中掉落相关变更的建议采纳率 ≥80% |
| M3 F2 | stats.rs+stat-update LLM 模板+apply 流程实战 | stat 替换零事故；人审工作量明显低于纯人工基线 |
| M4 F3/F4+打磨 | behavior/backref、create-check、缓存与断点续跑 | 覆盖下次真实大版本 |

### 4.9 风险与对策（v2 增补）

| 风险 | 对策 |
|------|------|
| 组件默认值变更（diff 在 components/ 而非 prefabs/）影响面难归属 | 首期仅当 PrefabIndex 显示该组件被目标 prefab 直接配置时才关联；否则 manual+提示 |
| 多跳推导事实（如野狗仇恨 100 来自袭击系统） | derivation_depth≥2 一律 manual，绝不代改 |
| 一页多实体/RichTab/消歧义命名长尾 | segmenter 子页绑定 + page-index 缓存积累 + 解析失败进 manual |
| LLM 幻觉 | F1 确定性特权路径绕开 LLM；输出契约+四重校验+人审兜底 |
| PO/tabx 巨大体量 | 静态管线已有全量重建能力，Tier0 只做调度 |
| WAF 限流 | 已落地节流重试；夜间低峰 + 断点续跑 |

---

## 5. 开放问题

1. `模块:DST Strings CN/EN` 与 `Data:DST Prefab/*` 的维护者协调渠道？（notify-only 的核对阈值如何定）
2. F2 的正文数值句定位：先只做 infobox 覆盖参数（结构化、低风险），散文句是否纳入 M3？
3. TuningTable 是否需要处理 tuning_override.lua 的世界选项分段？（涉及"不同世界设置下数值不同"的页面表达）
4. 生物类之外（物品/植物/建筑）的页面结构差异有多大？是否需要 per-category segmenter 配置？
5. LLM 选型与预算额度；F1 确定性路径普及后，llm 类余量是否足够覆盖 F2？
6. 报告除落仓库外是否投递 wiki 用户页供编辑订阅？
