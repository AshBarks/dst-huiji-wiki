# 更新后任务整合与执行顺序

**状态**：已实施（`update-local` / `update-data`；只做调度，原作业保留）
**范围**：`JOB_SPECS` 全部 34 个作业中与「游戏版本更新后」相关的部分
**关联**：[UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md)（终局是一条命令）、[FLOWS.md](FLOWS.md)（单作业流程）、[MAINTENANCE_TOOL_AUDIT.md](MAINTENANCE_TOOL_AUDIT.md)

---

## 0. 结论

1. **不是所有任务每次都要跑**。按数据源分 4 层，更新后每次都跑的核心只有 8 个作业；其余是按需的核对、索引与人工手术。
2. **可整合为 3 个编排作业 + 1 个只读核对作业**（原作业全部保留，编排层只做调度）：
   - `update-local`：`scripts-sync` → `images-sync` → `anim-sync`
   - `update-data`：`maintain-copy-clip` → `maintain-dst-recipes` → `maintain-item-table` → `maintain-strings`
   - `update-assets`：`upload-icons` → `skilltree-wiki`
   - `update-check`（只读）：`maintain-template-check` + `prefab-overrides-audit`
3. **顺序必须排**，但硬约束只有 6 条（§4），集中在「摄取 → 消费」和「模块常量 → 数据页」两处。同层内多数作业彼此独立，可以并行；当前 `resources` 里的 `out` 太粗，WebUI 实际仍会串行。
4. **不建议**把 LLM 类、单页手术类、调试类作业并进来（§5）。

---

## 1. 任务分层

| 层 | 作业 | 读 | 产物 / 写 | 维基 | 资源键 |
|----|------|----|-----------|------|--------|
| L0 摄取 | `scripts-sync` | `<DST>/data/databundles/scripts.zip` | live `scripts/` + 旧树归档 `scripts_<ts>/` + `dst_version.txt` | 无 | `scripts_tree` |
| L0 摄取 | `images-sync` | `images.zip` + `data/images/` + live `scripts/`（PO 名称表） | `output/ktools/`（split、CAS、manifest、`icon_meta.json`） | 读（上传状态） | `ktools_out` |
| L0 摄取 | `anim-sync` | `data/anim/` | `$ANIM__OUT_DIR/history/` | 无 | `anim_out` |
| L1 数据页 | `maintain-copy-clip` | 新 scripts 树 | `模块:Constants/*`（4 类） | **写** | `wiki`,`out` |
| L1 数据页 | `maintain-dst-recipes` | 新 scripts 树 | `Data:DSTRecipes.tabx` | **写** | `wiki`,`out` |
| L1 数据页 | `maintain-item-table` | 新 scripts 树 | `Data:ItemTable.tabx` | **写** | `wiki`,`out` |
| L1 数据页 | `maintain-strings` | 新 scripts 树 | `模块:<V> Strings *` + 索引 | **写** | `wiki`,`out` |
| L2 素材 | `upload-icons` | `icon_meta.json` + manifests | wiki `File:` | **写** | `wiki` |
| L2 素材 | `skilltree-wiki` | 新 scripts 树 + 站内图片 | `模块:Skilltree/<Char>` | **写** | `wiki`,`out` |
| L2 素材 | `cooking-game-export` | split 切片 + manifests + scripts | 本地 bundle | 无 | `out` |
| L3 核对/分析 | `maintain-template-check` | scripts + `icon_meta.json` + 线上模板/模块 | 报告/片段 | 读 | 无 |
| L3 核对/分析 | `prefab-overrides-audit` | scripts + 线上页面 | 审计 JSON | 读 | 无 |
| L3 核对/分析 | `maintain-prefab-overrides` | scripts + 线上页面 | 本地页面 + diff | 读 | `out` |
| L3 核对/分析 | `update-scan` | `scripts_<old>` vs `current`（+ corpus） | `impact.json` / patch | 无 | `out` |
| L3 核对/分析 | `update-index` | scripts | 关联索引 | 无 | `out` |
| L3 核对/分析 | `anim-index` / `skin-index` | scripts + `data/anim` | 索引 JSON | 无 | `anim_out` / `out` |
| L3 核对/分析 | `corpus-fetch` → `corpus-index` | 维基主命名空间 | `wikis/` + 派生索引 | 读 / 无 | `corpus` |
| L3 核对/分析 | `symbol-annotate` / `knowledge-*` | scripts + corpus + knowledge | 证据包/文档 | 无 | `out` / `knowledge` |
| 人工 | `maintain-wikitext`、`create-redirect`、`upload-image` | 指定页面/文件 | 逐页编辑 | **写** | `wiki` |
| 调试 | `parse-po`、`map-names`、`map-recipes`、`prefab-overrides(-dir)`、`anim-diff` | 单文件/单目录 | 本地 JSON | 无 | `out` |

> `images-sync` 的 `wiki_access` 现已标为 `Read`（默认查一次上传状态，`--skip-wiki-status` 可关），资源键仍只有 `ktools_out`、不持 `wiki` 写锁。它与写站任务并发时会共享 QPS 配额（默认 1）。

---

## 2. 依赖图

```
<DST>/data/databundles/
  ├─ scripts.zip ──[scripts-sync]──> scripts/ (live)  +  scripts_<旧时间戳>/ (归档)
  │                                    ├──> maintain-copy-clip ──┐
  │                                    ├──> maintain-dst-recipes │──> wiki Data: / 模块:
  │                                    ├──> maintain-item-table  │
  │                                    ├──> maintain-strings     ┘
  │                                    ├──> skilltree-wiki ──┐
  │                                    ├──> cooking-game-export
  │                                    ├──> update-scan (old vs current)
  │                                    ├──> update-index
  │                                    └──> anim-index / skin-index (还需 data/anim)
  ├─ images.zip + data/images ──[images-sync]──> output/ktools/
  │                                    ├──> upload-icons ──> wiki File:
  │                                    ├──> skilltree-wiki (icon_url 回填)
  │                                    ├──> cooking-game-export
  │                                    └──> maintain-template-check（图标状态）
  └─ data/anim ──[anim-sync]──> ANIM__OUT_DIR/history/
                                       └──> anim-index / skin-index

[wiki 主命名空间] ──[corpus-fetch]──> wikis/ ──[corpus-index]──> 派生索引 ──> symbol-annotate / knowledge-scan-symbols(pass2)
```

---

## 3. 可整合的分组

### 3.1 `update-local`：本地摄取（无维基写）

| 顺序 | 步骤 | 为什么在这个位置 |
|------|------|------------------|
| 1 | `scripts-sync` | 把 live `scripts/` 换成新版、旧树归档为 `scripts_<ts>`。下游所有「缺省读 live 树」的作业都依赖它先完成 |
| 2 | `images-sync` | 生成 `icon_meta.json` 与 manifest；其名称映射读 **live** `scripts/languages/*.po`（`src/scripts_sync/images/meta.rs:429`），跑在 sync 之前就会用旧版名称 |
| 3 | `anim-sync` | 只碰 `data/anim` 与 `ANIM__OUT_DIR`，与 1/2 无依赖；放最后是为了让"版本号 + 历史"在同一步收尾 |

- **可并行性**：`images-sync`（`ktools_out`）与 `anim-sync`（`anim_out`）互不相干，可并行；`scripts-sync` 必须最先。串行实现更简单，先按串行。
- **失败语义**：`scripts-sync` 失败（staging/换树回滚）必须中止；`images-sync` 失败则 `upload-icons`/`cooking-game-export`/`template-check` 全部不可信，也应中止（fail-fast）。
- **资源并集**：`scripts_tree` + `ktools_out` + `anim_out`。

### 3.2 `update-data`：数据页同步（全部写站）

| 顺序 | 步骤 | 为什么在这个位置 |
|------|------|------------------|
| 1 | `maintain-copy-clip` | 默认依次跑 rbtl → tech → filters → names（`src/service/maintain_jobs.rs:187`）；模块常量是数据页的"字典" |
| 2 | `maintain-dst-recipes` | 新配方若引入新制作站键，wiki 端 `模块:DSTRecipe.get_recipe_filter_cn` 会 assert，必须等 CraftingNames 先覆盖（`src/service/maintain_jobs.rs:441` 注释）；Tech 同一批常量，顺带先跑成本为零 |
| 3 | `maintain-item-table` | 只读 PO，与 1/2 无依赖；放这里是因为同样吃 `wiki` 锁，顺序无所谓 |
| 4 | `maintain-strings` | 只读 PO + 现有桶页，与 1/2/3 无依赖；它自己保证"索引最后写" |

- **可并行性**：3 与 4 在数据上独立，但三者都持 `wiki` 锁（QPS 1），实际串行；真正必须的只有 1 → 2。
- **失败语义**：写站作业半途失败会留下"部分已写"。每个子作业自身幂等（先拉历史再 diff），重跑即可；报告要区分「已写/未写/跳过（无变更）」。
- **确认粒度**：一次 `--yes` 覆盖 4 个写站作业，风险高于单作业。建议保留 `--dry-run` 产报告 → 人看 → `--yes` 两步（与现有 `WriteMode` 兼容）。
- **资源并集**：`wiki` + `out`。

### 3.3 `update-assets`：图标与技能树

| 顺序 | 步骤 | 为什么在这个位置 |
|------|------|------------------|
| 1 | `upload-icons` | 需要 `images-sync` 的 `icon_meta.json`/manifest（`src/service/upload_icons.rs:4`）。按 `source` 依次：inventory → crafting → skilltree |
| 2 | `skilltree-wiki` | `icon_url` 靠站内图片回填（`src/service/skilltree_wiki.rs:427`），必须等图标已上传；否则该节点留空并进 `missing_icon_urls` |

- **前置**：`update-local` 的 `images-sync` 完成。
- **可并行性**：无（同持 `wiki` 锁，且 2 依赖 1）。
- **注意**：`skilltree-wiki` 还会产出本地 `Skilltree.js`，上传 `零件:Skilltree.js` 仍是人工步骤，不进编排。

### 3.4 `update-check`：只读核对

| 步骤 | 前置 | 说明 |
|------|------|------|
| `maintain-template-check` | `maintain-copy-clip`、`upload-icons` | 否则会把「别名未写入 / 图标未上传」报成 error（`docs/FLOWS.md:105`） |
| `prefab-overrides-audit` | `scripts-sync` | 只读线上页面 + 本地推导 |

- 两者 `resources` 都是空数组，没有锁冲突，可以随时跑；合并成一个任务的收益是"一次拿到两份核对结论"。
- 这是唯一建议 **continue-on-error** 的组合：一项失败不影响另一项的结论。

### 3.5 保持独立（不并入编排）

| 作业 | 原因 |
|------|------|
| `update-scan`（+ `--corpus`） | 耗时长、产报告供人读、有 `state.json` 断点续跑；每次参数（old 快照）都不同 |
| `knowledge-sync` / `symbol-annotate` / `knowledge-scan-symbols` | 调 LLM、有成本、结果需人审，必须单独确认 |
| `maintain-wikitext` / `create-redirect` / `upload-image` | 单页手术，需要逐页 diff/人工判断 |
| `corpus-fetch` / `corpus-index` | 与游戏侧更新无因果关系（语料是维基侧），可随时/定时跑 |
| `anim-diff` | 临时对比，参数是任意两个目录 |
| `parse-po` / `map-names` / `map-recipes` / `prefab-overrides*` | 调试与一次性迁移；正式链路已被 maintain-* 覆盖 |

---

## 4. 顺序约束清单

| # | 约束 | 强度 | 依据 | 违反后果 |
|---|------|------|------|----------|
| H1 | `scripts-sync` → 一切读 live scripts 的作业 | 硬 | `GameSource` 的 live→zip 语义；`update-scan` 的 `new=current` 也解析 live 树 | 用旧数据生成"新"产物，且旧树已被归档，事后无法自动对齐 |
| H2 | `maintain-copy-clip`（tech/names）→ `maintain-dst-recipes` | 硬 | `模块:DSTRecipe.get_recipe_filter_cn` 对新制作站键 assert（`maintain_jobs.rs:441`） | 新配方在 wiki 端渲染报错/空值，直到 copyclip 补跑 |
| H3 | `images-sync` → `upload-icons` | 硬 | `icon_meta.json`/manifest 是 upload-icons 的唯一标题来源（`upload_icons.rs:4`） | 无标题可传 / 用旧标题重传 |
| H4 | `upload-icons`（skilltree）→ `skilltree-wiki` | 软（结果质量） | `skilltree_wiki.rs:427` 只回填站内已存在的图 | `icon_url` 留空，需下次运行才回填 |
| H5 | `images-sync` → `cooking-game-export` | 硬 | 图标取 `current/split/`，角色头像同源（`cooking_export.rs:247`） | 缺图标直接报错（除非 `--allow-missing-icons`） |
| H6 | `corpus-fetch` → `corpus-index` → `symbol-annotate` / pass2 | 硬 | 语料树是索引/证据的输入 | 索引基于旧语料；pass2 归因为空 |
| H7 | `maintain-copy-clip` + `upload-icons` → `maintain-template-check` | 软（假阳性） | `FLOWS.md:105` 的别名/图标检查读线上现状 | 报告一堆"缺口"，实际是还没写 |
| H8 | `scripts-sync` → `update-scan`（old = 上一快照，new = current） | 硬 | `SnapshotStore` 只认 `scripts_<12位时间戳>`（`src/update/snapshot.rs:49`） | `old/new` 解析失败或对比错版本 |

**不需要排序**的：`maintain-item-table` ↔ `maintain-strings` ↔ `maintain-copy-clip`；`anim-sync` ↔ `images-sync`；`upload-icons` 的三个 source 之间（但同持 `wiki` 锁）。

---

## 5. 落地形态

已落地两个编排变体，内部按固定序调用现有 `run_*`（`src/service/pipelines.rs`），**不复制任何逻辑**，原作业一字不动：

```rust
UpdateLocal { force: bool, dry_run: bool, skip_wiki_status: bool }
UpdateData  { snapshot: Option<String> }
```

- **参数**：只暴露跨步骤有意义的（`force`/`dry_run`/`skip_wiki_status`、`snapshot`）；步骤内部参数沿用缺省，避免把 36 个作业的参数拼成一张大表。`update-data` 不暴露各步 `output`（路径语义各步不同），需要产物时单独跑原作业。
- **资源**：取各步并集——`update-local` = `scripts_tree` + `ktools_out` + `anim_out`；`update-data` = `wiki` + `out`。简单、保守；代价是长任务期间挡住同键的独立作业。
- **失败语义**：fail-fast，错误消息带步骤名（`<step> 步骤失败：…`）；已完成步骤的产物保留，子作业幂等，重跑即可。
- **报告**：沿用 `CaptureReporter`，返回 `{ status, steps: [{ step, result }] }`，`--report-json` 直接落盘。
- **契约**：`job_spec.rs`（行）、`kind.rs`（变体）、`commands/mod.rs`（`CLI_EXT` + 构造臂）、前端 `JOB_DEFS` 四处已对齐，契约测试覆盖。
- **可选增强**：`--from <step>` 支持断点重跑（对齐 `update-scan` 的 `state.json` 先例）；`update-assets`/`update-check` 尚未实施。

**为什么不直接做一个通用 DAG 执行器**：参数 schema、互斥锁、前端表单都会爆炸；更新流程本身是固定的 4 条链，preset 更可测。

---

## 6. 风险与注意

1. **粗粒度 `out`**：`update-scan`、`update-index`、`cooking-game-export`、各 maintain-* 都声明 `out`，WebUI 里天然互斥。若要真并行，先把 `out` 拆成 `out/scan`、`out/ktools` 等子键——这是独立改造，不要混在编排任务里做。
2. **长事务的部分写入**：`update-data` 中途失败会留下"已写 2/4"。子作业幂等，但报告必须明确边界；必要时先 `--dry-run` 全量跑一遍。
3. **`images-sync` 的隐式维基读**：它默认查上传状态（spec 已标 `Read`），但不持 `wiki` 锁。与写站任务并发时共享 QPS 配额（默认 1），长任务会让写站排队；需要时用 `--skip-wiki-status`。
4. **bot 窗口**：`Data:DST Prefab/*` 由他方 bot 在更新后 1~3 天同步，`update-check`/`update-scan` 的结论要标注窗口，避免把"尚未同步"当成缺口。
5. **确认粒度**：`update-data` 一次确认 4 个写站作业。每步的 `wiki_edit` span（page/oldrevid/newrevid）已存在，审计链路可保留；但人审成本上升，建议默认 `--dry-run` 出报告。

---

## 7. 一次更新的实操顺序

```
更新当天
  1) update-local                     （scripts-sync → images-sync → anim-sync）
  2) update-data --dry-run            （看 4 份 diff）
  3) update-data --yes                （copyclip → recipes → item-table → strings）
  4) update-assets                    （upload-icons → skilltree-wiki）
  5) update-check                     （template-check + prefab-overrides-audit，只读）
  6) update-scan --corpus             （人工看影响报告；可分批）
按需
  7) corpus-fetch → corpus-index      （维基侧，与更新无因果关系）
  8) cooking-game-export              （要发 bundle 时）
  9) maintain-wikitext / create-redirect / upload-image  （逐个例外处理）
```
