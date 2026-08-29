# PageSymbolMap 方案(M2 scan-wiki)

> 状态:v1(2026-08-29,方案 C 拍板;**M2a 确定性骨架已实现**)。
> 定位:[KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md) M2 的细化——页面 × 符号归因地图,消费方为 M3 sync(代码变更→页面修订建议)与 page-assist(页面覆盖缺口)。
> 拍板记录(2026-08-29):①建法 = 方案 C(确定性分层优先,LLM 只做 L2 审计);②先行 M2a;③detail_level 对二跳放开(封顶 summary,页面汇总只计直连);④aspects 存快照副本(靠 inputs.sha 触发刷新);⑤detail_level 新定义三档 stub/summary/detailed。

## 1. 图回答的三个层次

| 层次 | 语义 | 来源 |
|---|---|---|
| **routed(适用)** | 该页面(实体)在代码侧关联哪些符号 | D2:atlas 反向边 + behaviour 二跳 |
| **mentioned(提及)** | 页面文本是否实际体现该符号 | D0:pass2 aspects 反转;D1:数值配对;L2:LLM 审计(M2b) |
| **detail(深度)** | 提及的深度与覆盖缺口 | aspects_covered / aspects_ignored 快照 + detail_level |

## 2. M2a 三层确定性证据(已实现)

1. **D2 路由**:atlas 反向边(prefab variant → components/brains/stategraphs 文件)+ `behaviour_calls` 二跳(brain → behaviour,仅收录有 SymbolDoc 的);只收录 doc-backed 符号——路由到但无文档的符号属于 M1 扩量范畴,不进图。
2. **D0 反转**:125 份 SymbolDoc 的 `wiki.aspects[].evidence[].pageid` 反转为 page→symbol,aspects 以**快照副本**存入图(SymbolDoc 重扫不自动回写,靠 `inputs.symbol_doc_shas` 判增刷)。
3. **D1 数值配对**:页面 `quantity` facts(单值、|v|≥3)↔ 符号源码命名数值常量(`NAME = N`,|v|≥3),记录 `raw + region_id + file:NAME=value`;percent/interval 暂不参与(碰撞面大,留给 L2)。

## 3. detail_level 分级(新定义三档)

按 (page, symbol) 对的证据信号数(aspects 覆盖数 + fact 配对数):

| 档 | 条件 |
|---|---|
| **stub** | 路由到但零证据(routed-only,即 page-assist 的最大缺口信号) |
| **summary** | 恰好 1 个信号 |
| **detailed** | ≥2 个信号 |

**二跳放宽**:behaviour(经 brain 二跳)封顶 `summary`;**页面级汇总** `page_detail_level` 只统计直连符号(任一 detailed 或 ≥3 个 summary → detailed;任一 summary → summary;全 stub → stub;无直连符号 → null)。

## 4. 落盘与增量

- 位置:`knowledge/pages/{pageid}.json`(内容不变则不写,git 零噪音)。
- 增量键:`inputs.wikitext_sha256` + `inputs.symbol_doc_shas`(本页引用到的每份 SymbolDoc 的 provenance sha);消费方据此判断刷新。
- CLI:`knowledge-scan-wiki <scripts> --corpus wikis/…`(全确定性,秒级~分钟级,无 LLM 配置要求)。

## 5. M2b / M2c(未实施)

- **M2b L2 审计**:对「routed 但 D0/D1 零证据」的缺口对,收编 symbol-annotate 的分页批处理 verdict(每 symbol 1~3 次调用覆盖其全部命中页),补 `mention_source=llm_verdict`;范围用 M2a 的缺口统计数据定。
- **M2c 汇总**:全库缺口报表(按符号聚合:哪些符号被大量页面 routed 却普遍 stub → 文档或页面侧行动项);detail_level 公式如有必要再引入 LLM borderline 判定。

## 6. 已知取舍

- D1 只做 quantity↔命名常量的精确相等匹配,松匹配(区间/百分比/单位换算)留给 L2 或 update 侧 F3;
- pass2 未采样到的页面即使真实提及了符号,D0 也探不到——这正是 M2b L2 的存在意义,M2a 的"stub"语义是「确定性方法未见证据」而非「页面一定没写」;
- `pages_without_prefab`(2566 页)与系统机制专题页不在本图范围(与 M1 路由同口径)。
