# 语料侧 ↔ 代码侧关联索引对接契约（草案）

**状态**：v0.1（对接契约草案，随两侧实现演进）
**日期**：2026-08-26
**关联文档**：[CODE_ASSOCIATION_INFRA.md](CODE_ASSOCIATION_INFRA.md)（代码侧基础设施 A，并行工作线维护）、[UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md) §4.2（编辑取舍模型）、[CORPUS_PAGE_PATTERNS.md](CORPUS_PAGE_PATTERNS.md) §7（区域模型）

> **决策记录**（2026-08-26）：①Atlas 构建器 MVP 由语料侧先做**离线 join 原型**验证连接键质量，再决定归属；②region_id 采用 `{pageid}:{kind}:{序号}` 方案；③语料侧按 契约(E) → 注册表(A) → 切分器(B) → 事实抽取(C) 顺序实施。

---

## 1. 连接键：prefab 变体名

两侧共享的**唯一硬连接键**是 prefab 变体名字符串。

| 侧 | 来源 | 权威性 |
|----|------|--------|
| 代码侧 | `prefabs/*.lua` 的 `CreatePrefab` 返回名 / `AssocEdge.prefab_variant` | 定义侧 |
| 语料侧 | 页面信息框模板参数 `{{实体信息框/自动\|dst\|<prefab>…}}` 的第二参数 | 引用侧 |

约定：

1. **一页多变体**：RichTab/信息框聚合页（实测 193 页）内每个 `实体信息框/自动` 实例各贡献一个变体——注册表按"收集页内全部信息框 prefab 参数"实现，天然覆盖；
2. **手写体信息框** `{{实体信息框|dst|X}}`（33 页）同样提取，标记来源为 manual；
3. **v1 明确排除**：无信息框参数的聚合页（如「暗影生物」用 `{{分类查询}}`）、机制页、生物群系页——它们进 `pages_without_prefab` 清单而非建边；
4. 匹配失败的变体名（页面写了而代码里没有，或反之）是**一等报告项**，不是静默丢弃——这正是原型验证要量的东西。

## 2. 区域切分与稳定 id（语料侧工件）

PageSegmenter 把每页切成互不重叠的字节区间：

- **kinds**：`intro`（文首至首个 h2，含模板栈）/ `infobox`（预留）/ `tab:<变体>`（RichTab 内各信息框实例）/ `section:<标题>`（一个 h2 一区，h3 折叠进父区）；
- **id** = `{pageid}:{kind}:{序号}`，序号按字节顺序在 kind 内递增；
- **稳定性规则**：id 只绑定结构位置——文本修改只更新 `hash`（FNV-1a 64，跨版本确定）；插入/删除章节会使后续序号位移，消费方须用 `(pageid, kind, title)` 三元组做跨轮次语义匹配，id 仅用于单轮内引用；
- 每区带 `{start_byte, end_byte, len, hash, title?}`，hash 相同的区域可跳过重析。

## 3. 数值句事实清单（语料侧工件）

对区域文本跑正则族（区间 / 单值带单位 / 掉落 Pic·File 串 / 百分比），产出规范化事实候选：
`{pageid, region_id, fact_kind: interval|quantity|loot|percent, raw, values[], unit?, snippet}`。

用途：update-scan 的 old_literal 回查直接在此清单上运行（F2 的页面半边）；atlas join 后补上代码侧 evidence 即成完整 FactChange 前体。**清单是候选不是断言**——语义命名（field）留给后续 LLM/规则标注。

## 4. 工件路径与版本化

| 工件 | 路径（本契约约定） | schema_version |
|------|-------------------|----------------|
| 页面↔prefab 注册表 | `wikis/<host>/index/pages_by_prefab.json` | 1 |
| 区域切分 | `wikis/<host>/index/regions.jsonl`（每页一行） | 1 |
| 数值事实候选 | `wikis/<host>/index/facts.jsonl`（每事实一行） | 1 |

所有 JSON 工件顶层带 `schema_version` 与 `generated_at_ms`；破坏性变更升大号并保留旧读取器一轮。代码侧索引工件的落盘路径由该侧自定，本契约只要求其顶层暴露 `prefab_variant` 字段名与此处一致（当前实现已满足：`AssocEdge.prefab_variant`）。

## 5. Join 原型验证计划（先行执行）

离线 join 原型的验收指标（跑全量语料 + 代码侧当前索引快照）：

1. **匹配率**：实体类页面中 ≥1 个 prefab 参数命中的比例（目标基线，无阈值）；
2. **多变体分布**：每页变体数直方图（预期 RichTab 页=多值）；
3. **悬空清单**：页面有而索引无 / 索引有而页面无的变体名双侧列表；
4. 结论回填本文档 §6。

## 6. 记录

（待原型验证后回填）
