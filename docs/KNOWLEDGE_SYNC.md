# M3 knowledge sync 设计(代码变更 → 页面锚点)

> 状态:v1(2026-08-29,四点拍板;**最小闭环已实现**)。
> 定位:[KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md) M3 的第一段——把 update 侧 Layer A 的代码变更集与 knowledge 侧的 SymbolDoc / PageSymbolMap 接起来,产出「脏文档 + 受影响页面 + 旧值锚点」清单。
> 拍板记录(2026-08-29):①命令形态 = 独立命令 `knowledge sync`(松耦合,不并进 update-scan);②重扫 = sync 内级联,`--rescan` 显式开启(LLM 成本可控);③产出分级 = 复用 update 侧 ImpactPlan tier 语义(Tier1 auto / Tier2 llm / Tier3 report),v1 只做 Tier3 报告层;④第一版范围 = 最小闭环「变更 → 脏文档清单 + 受影响页面 + 旧值锚点定位」,Tier2 表述改写起草延后。

## 1. 命令

```bash
knowledge sync --old 202604271353 [--new current] [--rescan] [--limit 20]
```

- 快照口径复用 update 侧 `SnapshotStore`(`DST__ROOT/data/databundles/scripts_*`,new 可为 `current`);
- 报告落盘 `output/knowledge/sync_report.json`(机器可读,M3 后续 tier 的输入)。

## 2. v1 链路(全确定性,零 LLM)

1. **树差异**:复用 update 侧 `TreeDiff::diff_trees`(lua 文件 added/removed/modified);
2. **符号分类**:components/brains/stategraphs/behaviours → 符号变更;其余(prefabs/tuning/…)v1 仅列出(v1.1 再接路由交叉);
3. **脏文档判定**:`sha256(当前文件) != SymbolDoc.provenance.source_sha256` → 脏(文档生成于变更后树则自动判净);
4. **常量差异**:dirty 符号文件的新旧命名数值常量逐名对比(`NAME old→new`);
5. **页面交叉**:PageSymbolMap 中提及该符号的页面 → 受影响页面;其中 `fact_matches` 数值 == 旧常量值的条目标记为 **precise 旧值锚点**(页面原文 raw 可直接定位),其余为 context 级;
6. **排序**:脏文档按(提及页数 + 常量变更×2 + stub 页/10)降序,`--limit` 控制详列数。

## 3. 级联重扫(`--rescan`)

脏文档按类别分组,调用 `knowledge-scan-symbols --category X --pick-names … --force`(并发 2,LLM);重扫后页面图的刷新由下一次 `knowledge-scan-wiki` 运行完成(两段式约定)。Tier2(按常量差异起草页面表述修订)在 Tier3 报告被人工验证后实施。

## 4. 边界与取舍

- tuning.lua / prefab 变更 → 页面的交叉在 v1.1 接入(需要 tuning 表对比与 variant 路由);
- `Removed` 符号文件 → 文档标记待下线,不在 v1 自动处理;
- 文档不存在的新符号 → 归 M1 扩量范畴,报告中单列(added_symbols)。
