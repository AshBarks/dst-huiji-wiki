# 文档索引（状态与入口）

> 新增/废弃文档时请同步本表。状态含义：
> - **active**：现行设计或运行手册，改代码时以它为准；
> - **参考**：短篇专题说明，内容已稳定；
> - **历史**：某一时点的报告/旧计划，**结构图可能已过期**，不要据此改代码；
> - **草案**：未落地，仅记录思路。

## 先读这三篇

| 文档 | 说明 |
|---|---|
| [../AGENTS.md](../AGENTS.md) | 项目知识基线：目录图、WHERE TO LOOK、约定与反模式 |
| [STRUCTURE_OPT_PLAN.md](STRUCTURE_OPT_PLAN.md) | 结构优化**进度台账**（P0–P9），每阶段的拍板与遗留 |
| [FLOWS.md](FLOWS.md) | 命令流程：每个作业从参数到落盘的路径 |

## active：现行设计与手册

| 文档 | 主题 |
|---|---|
| [FLOWS.md](FLOWS.md) | 命令流程（作业 → 步骤 → 产物） |
| [UPDATE_TASK_GROUPS.md](UPDATE_TASK_GROUPS.md) | 更新后编排（update-*）的任务分组与顺序约束 |
| [UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md) | 更新影响评估：关联图集、Tier0 规则、符号证据包 |
| [CODE_ASSOCIATION_INFRA.md](CODE_ASSOCIATION_INFRA.md) | 代码侧符号级关联索引 |
| [CORPUS_CODE_ATLAS_CONTRACT.md](CORPUS_CODE_ATLAS_CONTRACT.md) | 语料侧 ↔ 代码侧关联索引对接契约 |
| [WIKI_CORPUS_PLAN.md](WIKI_CORPUS_PLAN.md) | 页面全量语料抓取与分类 |
| [CORPUS_PAGE_PATTERNS.md](CORPUS_PAGE_PATTERNS.md) | 语料结构与命名/文字习惯 |
| [KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md) | 符号知识文档管线总体设计 |
| [KNOWLEDGE_PAGE_MAP.md](KNOWLEDGE_PAGE_MAP.md) | PageSymbolMap（scan-wiki） |
| [KNOWLEDGE_SYNC.md](KNOWLEDGE_SYNC.md) | knowledge-sync：代码变更 → 页面锚点 |
| [KNOWLEDGE_BEHAVIOUR_CHAIN.md](KNOWLEDGE_BEHAVIOUR_CHAIN.md) | 行为链（behaviours/brains/stategraphs）知识文档 |
| [WIKITEXT_PARSER_PLAN.md](WIKITEXT_PARSER_PLAN.md) | wikitext 无损解析/编辑（`crates/dst-wikitext`） |
| [COOKING_SIMULATOR.md](COOKING_SIMULATOR.md) | 烹饪模拟器导出包布局与 schema 版本 |
| [ANIM_FOLLOW_FX.md](ANIM_FOLLOW_FX.md) | FollowSymbol 跟随 FX 合成预览 |
| [SKILLTREE_ICONS_PLAN.md](SKILLTREE_ICONS_PLAN.md) | 技能树图标管理方案 |
| [MODEL_MAPPING.md](MODEL_MAPPING.md) | 数据模型 → wiki JSON 映射框架 |
| [WEBUI_SMOKE.md](WEBUI_SMOKE.md) | WebUI 手工冒烟清单（Playwright） |

## 参考：短篇专题

| 文档 | 主题 |
|---|---|
| [COPYCLIP.md](COPYCLIP.md) | CopyClip 标记区间替换 |
| [PO.md](PO.md) | PO 文件解析要点 |
| [RECIPES.md](RECIPES.md) | Recipe 解析要点 |
| [PREFAB_NAME_OVERRIDES.md](PREFAB_NAME_OVERRIDES.md) | 预制体名称覆盖 |

## 历史：报告 / 旧计划（结构图可能过期）

| 文档 | 时点 | 说明 |
|---|---|---|
| [PLAN.md](PLAN.md) | 2026-04 | 最初的项目结构与依赖规划，已被现行结构取代 |
| [START.md](START.md) | 2026-04 | 项目初始想法 |
| [REFACTORING.md](REFACTORING.md) | 2026-04 | 早期重构记录；文中 `commands/maintain.rs` 等布局已不存在 |
| [CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md) | 2026-07 | 代码质量审计报告（一次性） |
| [REMAINING_WORK.md](REMAINING_WORK.md) | 2026-09 | 当时的剩余工作清单，多数已完成 |
| [MAINTENANCE_TOOL_AUDIT.md](MAINTENANCE_TOOL_AUDIT.md) | 2026-09 | 维护工具质量评估（一次性） |
| [COMPONENT_FULL_SCAN_REPORT.md](COMPONENT_FULL_SCAN_REPORT.md) | 2026-08 | Component 全量扫描结果 |
| [KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md](KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md) | 2026-08 | PageSymbolMap 审计结果 |
| [symbol_coverage_example.md](symbol_coverage_example.md) | 2026-08 | 示例报告（模拟数据） |

## 草案

| 文档 | 说明 |
|---|---|
| [ANIM_SKIN_PREVIEW_PLAN.md](ANIM_SKIN_PREVIEW_PLAN.md) | 动画皮肤预览集成方案，未落地 |
