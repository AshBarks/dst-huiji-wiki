# 游戏更新影响评估与自动修订方案（Update Impact Assessment）

**状态**：早期设计（评估阶段产出，待评审）
**日期**：2026-08（基于 build 747465 / 快照 202605291134 调研）
**关联文档**：[MAINTENANCE_TOOL_AUDIT.md](MAINTENANCE_TOOL_AUDIT.md)（现有工具质量评估）、[CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md)、[FLOWS.md](FLOWS.md)

---

## 0. TL;DR

每次游戏更新后，用一条命令完成：**结构化 diff → 受影响 wiki 页面清单（分级+证据）→ 确定性管线自动更新 → LLM 起草"读 diff 就知道怎么改"的页面修改 → 人审后推送 → 报告与审计**。

核心设计决策：

1. **三层流水线**：A 快照与结构化 diff → B 影响映射（静态规则注册表 + 实体级提取）→ C 分层修复（确定性 / LLM / 报告）。
2. **prefab 代码是全链路关联键**——这是对"页面↔代码文件关联"初步思路的深化：不做整文件粗粒度关联，而是从 diff **hunk 中提取实体**（配方名、prefab 名、字符串键、TUNING 键），再经本地 ItemTable 缓存解析到具体 wiki 页面，使影响清单精确到"页面 × 变更原因"。
3. **LLM 只做有界编辑**：输出契约是 `{find, replace, rationale, evidence}` 列表而非全文重写；经四重校验（唯一匹配 / 模板平衡 / 数值溯源 / 保护内容不可删）后才进入人审；默认 dry-run。
4. **前置改造是硬阻塞**：现有 WikiClient 无重试/节流/批量查询/编辑冲突保护，且实测灰机 API 有 WAF 间歇 403（见 §2.4 与审计报告），必须先修。

---

## 1. 目标与非目标

### 1.1 背景

实测更新节奏约每 1~3 周一次（databundles 下 2025-03 至 2026-05 共 40 个手工快照目录）。每次更新后维基需要：

- 同步数据页（配方表、物品表、常量模块）——现有 4 条管线已覆盖；
- 修正内容页正文中手写的数值/文本（伤害、耐久、台词、材料列表等）——**目前完全人工比对**；
- 盘点新内容（新实体缺页、新配方、新字符串）——目前靠人翻 changelog。

现状缺口：没有"这次更新影响了哪些页面"的系统性答案，也没有批量无人值守能力。

### 1.2 目标

| # | 目标 | 验收标准 |
|---|------|----------|
| G1 | 更新后一条命令产出《影响评估报告》：受影响页面分级清单，每条附 diff 证据 | 报告覆盖 ≥90% 实际需要维护的页面（以下次真实更新人工盘点为基准） |
| G2 | 命中确定性管线的页面零人工干预自动更新 | Tier1 页面成功率 100%，无错误写入 |
| G3 | "读 diff 就知道怎么改"的页面由 LLM 起草修改，校验后人审推送 | llm-auto 类建议采纳率 ≥80%；进入 wiki 的错误编辑 0 起 |
| G4 | 新内容检查清单（缺页实体、新增配方/字符串） | 清单完整率可核对 |
| G5 | 全程幂等、可审计、可回滚 | 任何一次运行可从 state.json 续跑；每次编辑记录 oldrevid/newrevid |

### 1.3 非目标

- 不做全自动无监督推送（初期始终有人审环节，llm-auto 白名单极窄）；
- 不接管 `Data:DST Prefab/*.json`（3869 页，第三方 bot「鲁鲁bot」在维护，见 §2.3）与 `模块:DST Strings CN/EN` 系列（疑似 bot 维护，见 §5 开放问题），只做一致性核对与通报；
- 不处理与游戏代码无映射关系的纯社区内容（攻略、同人等）；
- 不做 wikitext 的通用语法重构（渲染类问题不归本功能管）。

---

## 2. 事实基础（调研结论）

### 2.1 游戏侧

**scripts.zip 关键数据文件**（解压后 4010 个 .lua，272MB）：

| 文件 | 规模 | 驱动的 wiki 数据 |
|------|------|------------------|
| `recipes.lua` | 1544 行，~910 个 Recipe/Recipe2 调用 | Data:DSTRecipes.tabx；各内容页配方栏 |
| `languages/chinese_s.po` | 429,073 行（~17MB） | Data:ItemTable.tabx、模块:DST Strings CN、模块:Constants/CraftingNames |
| `tuning.lua` | 9,534 行 | 各内容页数值的最终来源之一 |
| `constants.lua` | 3,030 行（TECH 定义等） | 模块:Constants/Tech |
| `recipes_filter.lua` | 1,644 行 | 模块:Constants/CraftingFilters |
| `debugcommands.lua` | RECIPE_BUILDER_TAG_LOOKUP | 模块:Constants/RecipeBuilderTagLookup |
| `prefabs/*.lua` | 1,582 个文件 | Data:DST Prefab/<prefab>.json（bot）；实体组件行为 |
| `components/*.lua` | 815 个文件 | 同上（数值常在此） |
| `speech_*.lua` ×17 | 角色台词 | 台词相关页面/模板 |
| `preparedfoods.lua` / `cooking.lua` / `cookbookdata.lua` | 食物与锅 | 食物页、模块:Cookpot |

**版本识别**：游戏根 `version.txt` 即 build 号（当前 `747465`）。`DstContext` 已读取该文件。

**现有人工流程**（重要资产）：更新时把旧版目录改名为 `scripts_<yyyymmddhhmm>`，解压新版到 `scripts/`，再手工生成目录 diff（如 `databundles/0529.diff`，14.4 万行）。**databundles 下已有 40 个历史快照**——这既是本功能要形式化自动化的流程，也是免费的离线测试金标准（见 §4.7）。

**典型更新规模**（0529.diff，Rifts 7 大版本）：302 个变更文件 = 144 prefabs + 53 components + 20 stategraphs + 12 languages + 17 speech_* + recipes.lua / strings.lua / tuning.lua / constants.lua / actions.lua 等。平衡性小补丁通常只有个位数文件。

**diff 形态**：标准 git unified diff；新文件带 `new file mode` 且 `--- /dev/null`；本功能将自行生成统一格式（旧路径统一规范化为相对 scripts 根），不再依赖手工 diff。

### 2.2 维基侧数据架构（映射设计的事实依据）

```
游戏代码(lua/po)
   │ ①本工具 maintain-item-table / map-names
   ├──────────> Data:ItemTable.tabx ──(mw.huiji.db.find 运行时直查)──> 模块:ItemTable/Data ──┐
   │ ②鲁鲁bot(第三方)                                                                        │
   ├──────────> Data:DST Prefab/<prefab>.json ×3869 ──────────────────────────────────────┤
   │ ③本工具 maintain-dst-recipes                                                          ├──> 模块:AutoInfobox <── {{实体信息框/自动|dst|<prefab>}}
   ├──────────> Data:DSTRecipes.tabx ──(db.find 直查 + 人工 override 表)──> 模块:DSTRecipe/Data ┘        │
   │ ④疑似 bot                                                                                            ▼
   └──────────> 模块:DST Strings CN/EN 00~99+                                                    内容页正文(火腿棒等)
   │ ⑤本工具 maintain-copy-clip
   └──────────> 模块:Constants/{Tech,CraftingFilters,CraftingNames,RecipeBuilderTagLookup}      (手写参数优先于自动数据)
```

**页面分类清单**：

| 类型 | 示例 | 数据来源 | 当前维护者 | 更新敏感度 |
|------|------|----------|-----------|-----------|
| Data 表格页 | `Data:ItemTable.tabx`(188KB)、`Data:DSTRecipes.tabx` | po / recipes.lua | **本工具**（Tier1 直接复用） | 高，但已自动化 |
| 常量模块 | `模块:Constants/Tech` 等 4 个 | constants.lua 等 | **本工具**（COPYCLIP 标记替换） | 高，已自动化 |
| Prefab 数据页 | `Data:DST Prefab/hambat.json` | prefabs/*.lua + components | 鲁鲁bot（hambat.json 于更新后 3 天由其更新） | 高，他方负责 → notify-only |
| 字符串模块 | `模块:DST Strings CN 00~99`、EN 系列 | languages/*.po | 待确认（疑似 bot） | 高，待确认归属 |
| 内容页 | `火腿棒`、`荧光果`… | `{{实体信息框/自动}}` 自动拉数据 + **手写参数与正文** | 人工 | **中——本功能 LLM 层的主战场** |
| 聚合/列表页 | 食物表、科技树等 | 多由模块实时组装 | 半自动 | 低（数据页更新即生效） |

**三个关键机制**（决定了方案形态）：

1. **prefab code 是全链路关联键**。内容页 infobox 第一个匿名参数就是 prefab 代码（`{{实体信息框/自动|dst|hambat}}`），ItemTable 提供 prefab↔中文名双向索引。→ 影响映射应**以实体为中心**，而不是"文件→页面"的静态边表。
2. **AutoInfobox 参数覆盖规则：页面手写参数 > 自动数据**。结构化字段（伤害、食物值、配方栏）大多已被 Data 页自动化；真正会过期的恰是**手写部分**（正文里的数字、描述、掉落列表 Pic 串、皮肤信息）。→ 这就是 LLM 层的工作对象，也解释了为什么不能全文重写。
3. **Data 页被模块运行时直查**（`mw.huiji.db.find`），tabx 更新即全站生效，无逐页回填问题。→ Tier1 的杠杆极高：改一个 tabx 等于更新上千个内容页的展示。

### 2.3 他方 bot 观察

`Data:DST Prefab/hambat.json` 版本历史：创建于 2025-12-21、更新于 2026-06-01（0529 更新后 3 天），编辑者均为「鲁鲁bot」。说明：(a) 该数据集有专职维护渠道，我们不应重复建设；(b) 更新后存在 1~3 天窗口期，我们的报告可以把"Prefab 数据页是否已同步"列为核对项而不是执行项。

### 2.4 API 行为观察（工程约束）

| 观察 | 证据 | 工程结论 |
|------|------|----------|
| MediaWiki 1.38.4；Data 命名空间 3500、模块 828 | siteinfo | 可用的标准 MW API 面 |
| **连续快速请求触发间歇 403（WAF）** | 调研中多次复现：首个请求成功，紧接着的同型请求 403；间隔 15~30s 后恢复 | 批量功能必须内置全局节流 + 403/429 指数退避重试，否则不可用 |
| 搜索后端偶发 `cirrussearch-backend-error` | list=search 复现 | 不要依赖站内搜索做定位；用 ItemTable 缓存 + titles 批量查询 |
| allpages 上限 500，需 apcontinue 翻页 | 实测 | 枚举类操作要实现 continuation 循环 |
| prop=revisions 多标题批量 ≤50 且 rvlimit 只能为 1 | API invalidparammix 错误 | 页面批量抓取按 50/批设计 |

---

## 3. 总体架构

```
┌──────────────────── Layer A 快照与 Diff ─────────────────────┐
│ UpdateDetector(version.txt/zip hash) → SnapshotStore          │
│   → StructuredDiff: diff.json(+changes.patch 人读)            │
└───────────────────────────┬───────────────────────────────────┘
                            ▼
┌──────────────────── Layer B 影响映射 ────────────────────────┐
│ impact-rules.toml 静态管线规则      实体提取器(recipe_call/     │
│   → Tier1 候选(Data:*.tabx等)        prefab_def/po_msgctxt/    │
│                                     tuning_ref/speech_key…)   │
│              └────────────┬───────────┘                       │
│                           ▼                                   │
│ 页面解析: 本地ItemTable缓存 → 标题候选 → API批量存在性校验       │
│                           ▼                                   │
│ 相关性定级(旧值回查wikitext等信号) → ImpactPlan                │
│   [{page, tier: auto|llm|manual|notify|create-check,          │
│     evidence: [file@hunk]}]                                   │
└───────────────────────────┬───────────────────────────────────┘
                            ▼
┌──────────────────── Layer C 修复执行 ────────────────────────┐
│ Tier1 auto   : 进程内调度 maintain-* (--yes/--dry-run)         │
│ Tier2 llm    : 上下文包 → edits{find,replace} → 四重校验        │
│               → review bundle → 授权应用(basetimestamp保护)    │
│ Tier3 report : report.md + impact.json + applied.jsonl 审计    │
└───────────────────────────────────────────────────────────────┘
```

分层原则：
- A/B/C 通过 `ImpactPlan` JSON 解耦，每层可独立运行/测试/重放；
- B 层产出的每一项必须携带 evidence（文件+hunk 定位），报告和 LLM prompt 都引用同一份证据，保证可追溯；
- C 层对 wiki 的所有写入都走统一的 apply 通道（节流、冲突保护、审计集中在一处）。

---

## 4. 详细设计

### 4.1 Layer A：快照管理与结构化 Diff

**更新检测**
- 比较 `DST__ROOT/version.txt` build 号与 `updates/state.json` 记录的 last-build；
- scripts.zip 以 size+mtime+（可选）CRC 做二次确认。

**SnapshotStore 目录布局**（项目仓库内新增，gitignore 大文件）：

```
updates/
├── snapshots/<build>/          # 解压后的 scripts 树（符号链接或硬链接到 databundles 手工快照亦可）
├── <from>..<to>/               # 一次更新的工作目录
│   ├── diff.json               # 结构化差异（机器）
│   ├── changes.patch           # unified diff（人类浏览）
│   ├── impact.json             # ImpactPlan
│   ├── artifacts/              # Tier1 dry-run 产物（新 tabx 文本、copyclip 结果等）
│   ├── review/<page>.md        # LLM 建议的人审 bundle
│   └── applied.jsonl           # 已应用的编辑审计
├── cache/
│   ├── itemtable.json          # Data:ItemTable.tabx 本地镜像
│   └── page-index.json         # title → {pageid, checked_at}
└── state.json                  # 幂等状态机
```

**兼容导入**：`snapshot import <dir>` 解析 databundles 的 `scripts_YYYYMMDDHHMM` 命名，登记为快照（避免重复解压 268MB×N）。已有 40 个快照全部可导入，立刻构成回归测试集。

**DiffEngine**
- 文件级：双树遍历 + 内容哈希 → `added / deleted / modified`；相似度 >阈值 的 added+deleted 对启发式标记 `renamed`（游戏更新偶发 prefab 改名）。
- hunk 级：对 modified 文本文件用 `similar`（已在依赖中）生成行级 hunks；序列化为：

```json
{"path":"prefabs/fumaroleaxe.lua","status":"added",
 "hunks":[{"old_start":0,"old_lines":0,"new_start":1,"new_lines":37,
           "lines":[["+", "local assets = {"], ...]}]}
```

- 性能预算：~8000 文件对比应在 60s 内完成（哈希过滤后仅少数文件做行 diff）；diff.json 按 `<from>..<to>/` 缓存，重复扫描直接复用。

### 4.2 Layer B：影响映射

#### 4.2.1 映射规则注册表（静态部分）

`impact-rules.toml` 随仓库版本化管理。规则只是**候选生成器**，最终定级由相关性评估决定（§4.2.3）：

```toml
[[rule]]
id = "recipes-tabx"
when.file = "recipes.lua"
pipeline = "maintain-dst-recipes"                 # 命中即 Tier1
targets  = [{ page = "Data:DSTRecipes.tabx", action = "regenerate" }]
extract.entities = ["recipe_call"]                # 同时提取受影响配方实体

[[rule]]
id = "po-itemtable"
when.file = "languages/chinese_s.po"
when.hunk_ctx = "STRINGS.NAMES."                  # 只有 NAMES 块变化才触发重建
pipeline = "maintain-item-table"
targets  = [{ page = "Data:ItemTable.tabx", action = "regenerate" }]
extract.entities = ["po_msgctxt"]

[[rule]]
id = "copyclip-tech"
when.file = "constants.lua"
when.hunk_ctx = "TECH"
pipeline = "maintain-copy-clip:tech"
targets  = [{ page = "模块:Constants/Tech", action = "copyclip" }]

[[rule]]                                          # 泛化规则：任意 prefab 文件
id = "prefab-file"
when.file.glob = "prefabs/*.lua"
extract.entities = ["prefab_def", "component_add", "tuning_ref"]
targets = [
  { page = "Data:DST Prefab/{{entity}}.json", action = "notify", owner = "lulu-bot" },
  { page = "{{entity_cn}}",                   action = "assess" },   # 走相关性定级
]

[[rule]]
id = "speech-file"
when.file.glob = "speech_*.lua"
extract.entities = ["speech_key"]
targets = [{ page = "{{char_cn}}", action = "assess" }]
```

字段语义：`when.file`（精确/glob）、`when.hunk_ctx`（hunk 上下文行过滤，避免 po 一变就全量重建）、`pipeline`（Tier1 管线标识）、`extract.entities`（启用的提取器）、`targets[].action ∈ {regenerate, copyclip, assess, notify}`。

#### 4.2.2 实体提取器

核心思路深化：**不做"文件→页面"的静态边表，而从 hunk 提取实体，再解析实体→页面**。静态边表粒度太粗（recipes.lua 一变，几百个物品页全算候选），无法回答"哪些页面的哪句话要改"；实体级才能给出精确证据链。

| 提取器 | 输入 | 方法 | 产出 |
|--------|------|------|------|
| `recipe_call` | 含 Recipe/Recipe2 的 ± 行 | 正则 `Recipe2?\(\s*"([a-z0-9_]+)"` + Ingredient 参数扫描（沿用 parser/recipe.rs 的语义约定，但作用于单行/局部） | 配方名、产物、材料表、tech |
| `prefab_def` | prefabs/*.lua | **新增文件→full_moon 全文件 AST**（可靠）；modified→hunk 行正则 `Prefab\("([a-z0-9_]+)"` | prefab 名、AddComponent 列表、SetPrefabNameOverride |
| `po_msgctxt` | po hunk 所在块 | 向上回溯到 msgctxt/msgid/msgstr 三元组（复用 PoParser 的块切分逻辑做增量版） | 字符串键（如 STRINGS.NAMES.X）、中英文值 |
| `tuning_ref` | tuning.lua hunk | `KEY = <literal>,` | TUNING 键 + 新值 |
| `component_add` | 任意 hunk / components/x.lua 文件名 | `AddComponent("x")`、文件名反推 | 组件名 |
| `speech_key` | speech_*.lua hunk | 键路径 + 引号文本 | 角色 + 台词键 + 新英文文本 |

原则：hunk 是局部上下文，**默认用正则**；只有整个文件是新增时才值得全文件 AST。每个提取结果都附带 `evidence = {file, hunk_index, line_in_hunk}`。

#### 4.2.3 页面解析与相关性定级

**实体→页面解析链**：
1. 本地 ItemTable 缓存（首次从 wiki 拉 `Data:ItemTable.tabx` 存 `cache/itemtable.json`，之后随 Tier1 产物自更新）：提供 `prefab → 中文名/英文名`；叠加项目已有的 `output/prefab_overrides.json` 处理 override 名；
2. 标题候选生成：中文名直连页、常见消歧义变体（`X (DST)` 等）、命名空间模板（`Data:DST Prefab/<prefab>.json`）；
3. 批量存在性校验：titles≤50/批 + 节流重试，结果写 `cache/page-index.json`；
4. 解析不出页面的实体 → 进入 `create-check` 清单（新内容或缺索引）。

**相关性评分（针对候选内容页）**：
1. 取页面 wikitext（走缓存）；
2. **信号 S1（主信号）——旧值回查**：从 evidence 提取"旧字面量"（被删除行中的数值、英文字符串、材料名），在 wikitext 中查找。命中 ⇒ 强相关（页面确实引用了刚被改掉的值）；
3. 信号 S2：infobox 手写参数名与新数据字段对应（如 `装备/伤害` ↔ weapon_damage 变更）；
4. 信号 S3：页面显式依赖变更的模板/模块。

**定级规则**：

| tier | 判定条件 | 动作 |
|------|----------|------|
| `auto` | 命中带 pipeline 的规则 | Tier1 确定性管线 |
| `llm` | S1 命中，且改动类型属于机械可推导集合：数值替换 / 英文原文替换（台词）/ 材料列表增删 / 单句事实陈述 | Tier2 LLM 起草 |
| `manual` | S1/S2 相关但属评价性、攻略性内容（提示/花絮/平衡讨论） | 仅报告提示，附证据链接 |
| `notify-only` | 他方维护目标（DST Prefab bot 页、Strings 模块） | 报告核对项，不写 |
| `create-check` | 新实体无对应内容页 | 新内容清单（含 prefab、中文名、来源文件），供编辑建页参考 |

### 4.3 Layer C：修复执行

#### Tier 1：确定性管线（无 LLM）

- `maintain-dst-recipes` / `maintain-item-table` / `maintain-copy-clip:<type>` 增加库函数入口与 `--yes` / `--dry-run`（产物落 `artifacts/`）/ `--report-json`；
- ImpactPlan 中的 auto 项在同一进程内调用处理函数（不是 shell 出 CLI）；
- 交互式确认改为三态：交互（默认）/ `--yes` / dry-run——这也是审计报告指出的无人值守能力缺口（见 [MAINTENANCE_TOOL_AUDIT.md](MAINTENANCE_TOOL_AUDIT.md) §4.4）。

#### Tier 2：LLM 辅助修改

**Provider 抽象**：OpenAI 兼容 `/chat/completions`，reqwest 直连（不引 SDK）；配置 `LLM__BASE_URL` / `LLM__API_KEY` / `LLM__MODEL`（进 `.env`，不入库）；temperature=0；超时 + 退避重试；记录每次调用 token 用量到 `applied.jsonl`。

**任务模板**（prompts/*.md 入库版本化，按任务类型分）：

| 任务 | 场景示例（来自 0529.diff） |
|------|---------------------------|
| `stat-update` | 数值替换：武器伤害调整后，正文"伤害为 X"及 infobox 手写参数 |
| `text-sync` | 台词/描述同步：`ANNOUNCE_VAULT_LOBBY_EXIT` 文案变更对应的引用 |
| `list-sync` | 掉落列表 `{{Pic|32|…}}` 串增删、配方材料行更新 |
| `section-rewrite` | 少数复杂段落（如机制描述因 actions.lua 行为变化而过时） |

**上下文包组装**（控制预算，单页 ≤~8k tokens）：
- 与该页面实体相关的 evidence hunks（含实体名的优先，超限截断并注明）；
- 页面中含旧值的 section（按 `==` 标题切分选取；infobox 整体提供但不允许模型改结构）；
- 风格约束摘要 + 2 个 few-shot 样例（从历史 review bundle 中挑选同类已采纳案例）。

**输出契约与四重校验**（防幻觉的核心，宁拒勿错）：

```json
{"edits":[{"find":"伤害为 59.5","replace":"伤害为 62","rationale":"TUNING 更新","evidence":["tuning.lua@L512"]}],
 "not_editable_reason": null}
```

1. **唯一匹配**：find 必须在当前 wikitext 中恰好命中一次，否则整包拒绝；
2. **结构守恒**：替换后 `{{ }}`、`[[ ]]` 平衡计数不变；模板名白名单外的模板不得增删；分类与维护模板（如 `{{置顶导航}}`）出现在 find/replace 中则拒绝；
3. **数值溯源**：stat-update 类的 replace 中的数字必须等于提取器产出的新值（不允许模型自行推算）；
4. **尺寸限制**：单页 edits ≤ N 条、单条 replace 长度上限，超出降级为 manual。

**人审与应用**：
- 产出 `review/<页面>.md`：原 wikitext、建议 diff、证据、rationale、校验结果；
- 应用授权分级：`--apply none`（默认，只出 bundle）→ `--apply llm-auto`（仅应用通过全部校验且属 stat/text 单点替换的）→ `--apply llm-all`（交互逐条确认）→ 人工修订 bundle 后 `--apply bundle:<path>`；
- 编辑参数：summary 固定前缀 `游戏更新同步(<build>)：<规则id>`；minor=true；携带 `basetimestamp` 做编辑冲突保护（当前 client 缺失，见前置改造）；失败页记入重试队列。

**成本预估**：平衡补丁约 10~40 页 × ~3k tokens ≈ 十万级 tokens/次；Rifts 级大版本 200+ 页，靠 state.json 断点续跑分批消化。

#### Tier 3：报告

`report.md` 固定章节：概览统计（各 tier 数量、diff 规模）/ Tier1 执行结果 / LLM 待审清单（按置信度排序）/ manual 清单 / 新内容 checklist / 他方维护核对项 / API 异常与重试记录。同时产出机器可读 `impact.json`（供 CI 或后续工具消费）。

### 4.4 CLI 设计

```
update-scan [--from <build>] [--to <build>] [--full]   # Layer A+B；默认最新 vs 上一已知版本；纯只读(wiki 仅缓存拉取)，产出 impact.json + report.md
update-fix  [--plan <path>] [--only-tier auto|llm|manual]
            [--apply none|llm-auto|llm-all] [--yes]     # Layer C
snapshot import <dir>                                   # 导入 databundles 历史/未来手工快照
update-report <pair-dir>                                # 重渲染报告
```

环境变量新增：`LLM__BASE_URL / LLM__API_KEY / LLM__MODEL`、可选 `WIKI__QPS`（默认 1）。

### 4.5 模块布局

```
src/update/
├── mod.rs          # 编排与状态机
├── snapshot.rs     # 更新检测 / 解压 / 导入
├── diffdata.rs     # FileDiff/Hunk 类型 + 序列化 + similar 封装
├── rules.rs        # impact-rules.toml 加载与匹配
├── extract.rs      # 六个实体提取器
├── pageresolve.rs  # prefab→标题候选→存在性校验→索引缓存
├── assess.rs       # 相关性评分与定级 → ImpactPlan
├── execute/
│   ├── pipeline.rs # Tier1 调度（调 maintain-* 库函数）
│   ├── llm.rs      # provider + prompt 组装（不依赖 wiki，纯文本进出，便于离线测试）
│   ├── validate.rs # edits 四重校验器
│   └── apply.rs    # 统一应用通道（节流/basetimestamp/审计）
└── report.rs       # md/json 渲染
```

依赖方向：`update → {parser, copyclip, wiki, mapping, models}`，禁止反向；`commands/update.rs` 仅做 CLI 参数到 update 库的转接，**不要重蹈 commands/maintain.rs 743 行单体的覆辙**（见审计报告 §5）。

### 4.6 前置改造（硬阻塞项）

| # | 改造 | 原因 | 位置 |
|---|------|------|------|
| 1 | WikiClient 全局节流（默认 1 QPS，可配）+ 403/429/5xx 指数退避重试 | 实测 WAF 间歇 403（§2.4）；批量场景必挂 | wiki/client.rs |
| 2 | `get_pages_batch(titles≤50)` 与 `page_exists()` | 影响扫描需批量校验几十~几百个标题 | wiki/client.rs |
| 3 | edit 携带 `basetimestamp`（可选 `assert=user`） | 当前无冲突检测，自动编辑可能覆盖人工并发修改 | wiki/client.rs do_edit |
| 4 | Error 细分：从 `WikiApi(String)` 拆出 `RateLimited` / `PageNotFound` / `AuthExpired` | 自动化流程需要按错误类别分支决策 | error.rs |
| 5 | maintain-* 处理函数抽为库函数（CLI 与逻辑分离） | Tier1 进程内调度 + 补测试 | commands/maintain.rs → 相应库模块 |
| 6 | `prompt_confirm` 非交互模式（全局 `--yes/--dry-run`） | 批处理会在 stdin 卡死 | commands/maintain.rs |

### 4.7 测试策略

- **金标准端到端 fixture**：`scripts_202604271353` vs `scripts_202605291134`（对应人工 0529.diff，302 文件）——Layer A 输出的文件清单可与 0529.diff 逐文件比对；后续每次真实更新滚动补充；
- 单测：diffdata 序列化稳定性（golden file）；六个提取器各配样例 hunk（正例+漏提+误提用例）；validate.rs 拒绝路径全覆盖（多义 find、模板不平衡、数值不溯源、触碰保护内容）；
- LLM 测试：录制/回放（provider 响应存 fixture），CI 不打真 API；
- wiki client 测试沿用现有约定（`.env` 缺失则 skip）。

### 4.8 实施里程碑

| 阶段 | 内容 | 预估 | 出口条件 |
|------|------|------|----------|
| M0 前置加固 | §4.6 六项 | 1~2 天 | 对 wiki 连续 200 次请求 0 失败；模拟冲突下 edit 正确报 editconflict |
| M1 A+B 最小闭环 | snapshot/diff/rules/report（纯确定性） | 3~5 天 | 用 0427→0529 fixture 重放出 0529.diff 同等文件清单；report.md 可读可用 |
| M2 定级完善 | pageresolve + assess + create-check | 2~4 天 | 对 0529 更新产出分级清单，人工抽查 30 条准确率 ≥85% |
| M3 LLM 子系统 | provider/prompts/validate/review/apply | 3~5 天 | fixture 回放测试全绿；bundle 评审流程走通 |
| M4 实战演练 | 随下次真实更新运行 | — | 见 §1.2 验收标准 |

### 4.9 风险与对策

| 风险 | 对策 |
|------|------|
| WAF 限流导致批量中断 | M0 节流+退避；低峰运行；断点续跑；异常页入报告重试队列 |
| LLM 幻觉/误改 | 输出契约 + 四重校验 + 默认人审 + llm-auto 白名单极窄 + newrevid 记录可秒回退 |
| 页面命名/消歧义长尾（中文名≠页面名） | 标题候选 + page-index 缓存逐步积累；解析失败的进 manual 清单人工归类 |
| 大版本 diff 过大 | 先让 Tier1 消化大头（tabx/常量模块），LLM 只处理内容页；分批续跑 |
| 与他方 bot 写冲突 | notify-only 不碰其页面；报告给出"请核对"建议 |
| PO 文件巨大（43 万行） | hunk 所属块的增量解析；ItemTable 全量重建仍可行（现有管线已是全量模式，188KB tabx 可承受） |

---

## 5. 开放问题（待确认）

1. `模块:DST Strings CN/EN` 系列的实际维护者与生成方式？若是 bot 从 po 生成，是否可协调纳入 notify 清单或拿到生成脚本？
2. 是否认识/联系得上鲁鲁bot 维护者，建立 `Data:DST Prefab/*` 更新的通报渠道？
3. 内容页消歧义命名的社区惯例全集（除 `(DST)` 后缀外还有哪些模式）？
4. LLM 选型与预算额度（影响 llm-auto 白名单宽严与并发度）；是否优先接本地模型以满足"凭证不出本机"偏好？
5. 报告除了落仓库 markdown，是否需要同时投递到 wiki 用户页（如 `用户:<bot>/更新速报`）供其他编辑订阅？
6. 旧版快照清理策略：snapshots 占盘 268MB/份，保留最近 N 份还是全部？
