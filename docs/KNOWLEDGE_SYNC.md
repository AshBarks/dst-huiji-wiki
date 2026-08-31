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

## 3.1 v1.1 与 Tier2 实测(2026-08-30)

- **v1.1 已落地**:tuning.lua 新旧树对比(49 项真实变更,如 FUMAROLE 工具组)+ prefab→atlas 变体→pages_by_prefab 页面交叉(166 prefab → 124 页面);`--corpus` 参数启用;
- **Tier2 `--draft` 已落地**:每带 precise 锚点的脏文档一次 LLM 调用,以页面原文行为 grounding,产出 old/new 句对 + 理由(仅入报告,不写 wiki);
- **实测发现的重要 caveat**:D1 锚点按数值精确匹配,同值异义常量会造成错锚——模拟 SEE_DIST 30→33 的起草中,猎犬页"群体仇恨 30 单位"实际来自 `prefabs/hound.lua: SHARE_TARGET_DIST=30`(未变更),而真正的 SEE_DIST 锚点句("主动寻找 30 距离单位内的肉类食物")反而可能漏掉。**Tier2 输出必须人工审阅**;改进方向 = 起草 prompt 注入常量的使用上下文(代码引用行)与同值竞争常量提示。

## 3.2 v1.2 锚点细化 + 复核闭环(2026-08-31)

- **v1.2 已落地**(§3.1 拍板方向):起草 prompt 按 §3.1 注入(a)每个变更常量在新源码中的使用行(≤2 行)、(b)同文件内与旧值相等的**同值竞争常量**及其使用行;并追加保守指令"无法从使用上下文确证则不输出建议——宁可漏掉也不要改错"。
- **SEE_DIST 30→33 模拟实测**(sim 树仅改 `brains/houndbrain.lua:16`,LLM mimo-v2.5):
  - 第一轮(注入上下文,无保守指令):产出 2 条建议,**均为错锚**——"瞬移回家 30"(houndbrain 无对应常量)与"群体仇恨 30"(跨文件 `prefabs/hound.lua` 的 SHARE_TARGET_DIST=30);
  - 第二轮(追加保守指令):**0 条建议**,两个错锚全被抑制;
  - 结论:使用上下文 + 保守指令能抑制同文件可消歧的错锚;**跨文件同值常量(prefab 侧)仍可能漏网**——这正是人工复核兜底存在的理由。
- **复核闭环已落地**(Tier2 输出仅入报告 → 人工裁决 → 应用清单):
  1. `--draft` 收尾自动写出裁决模板 `output/knowledge/draft_review.json`(每条建议带稳定 id D001…,decision=pending);
  2. 人工编辑 decision(approve/reject)+ note;
  3. `knowledge-sync --review output/knowledge/draft_review.json` 回读既有 `sync_report.json`(不重跑 diff/起草,不需要快照参数),产出 `output/knowledge/sync_apply_list.md`:approve 项含旧/新句 + 理由 + 审阅注记,reject 项留痕(含驳回理由);
  4. wiki 实际编辑仍由人工按清单执行(项目确认策略,自动写 wiki 不在范围内)。
  - 闭环演示(同上模拟):D001/D002 双 reject,驳回理由留痕于应用清单。

## 4. 边界与取舍

- tuning.lua / prefab 变更 → 页面的交叉在 v1.1 接入(需要 tuning 表对比与 variant 路由);
- `Removed` 符号文件 → 文档标记待下线,不在 v1 自动处理;
- 文档不存在的新符号 → 归 M1 扩量范畴,报告中单列(added_symbols)。
