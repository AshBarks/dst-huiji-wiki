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

## 5. M2b L2 审计(已实现,试点完成)

`knowledge-scan-wiki --audit [--audit-symbols a,b,…] [--audit-max-pages 60] [--audit-batch-pages 20] [--audit-batch-max-chars 24000]`:对「stub 且符号有 aspects」的对,收编 symbol-annotate 的分页批处理 verdict(每 symbol 1~3 批,按 facts 富裕度选页),mentions=true 转为 `mention_source=llm_verdict + summary`,verdict 细节(wording/semantic_consistent/note)快照进 `llm_verdict` 字段;raw 归档 `output/knowledge/raw/page_map/`。

**试点结果(2026-08-29,10 个高频组件符号,各 60 页,共 600 页 / 30 批)**:

- stub→提及转化 **64 对(约 11%)**:combat 23、health 17、lootdropper 11、locomotor 5、burnable 4…;
- 转化案例验证了 L2 的必要性:洞穴蠕虫页“玩家接近它 2~5 距离单位时…攻击”——区间表述与常量精确匹配天然不兼容,只有 LLM 能归因;
- **语义不一致 2 对**(寄居蟹隐士/雪球 × lootdropper)——这正是 M3 页面修订建议的原始信号;
- 0 失败批次;30 批 ≈ 23 分钟(串行),换算全量成本:剩余 ~4900 个有 aspects 的 stub 对 ≈ 3~4 小时。

**推开的策略(定案)**:L2 审计按符号分批推进(高频/高 aspects 符号优先);`semantic_consistent=false` 的对在 M2c 汇总中单独成表,作为 M3 修订建议清单的直接输入。

## 6. M2c 汇总(已实现)

`knowledge-scan-wiki --report`:聚合 `knowledge/pages/*.json` → `knowledge/page_map_summary.json`(按符号 routed/mentioned/stub/未审计/不一致,detail 分布,`semantic_consistent=false` 单独成表)。

## 7. 全量结果与核对报告(2026-08-29)

> 完整报告(含 12 条不一致逐条判读、过程事件、复现命令):[KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md](KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md)。

**规模**:756 页 / 6071 对;提及对 690(pass2 反转 83 + D1 配对 30 + **L2 审计 577**——L2 贡献了全部提及的 84%);有提及页面 360;审计覆盖率 ≈96%(仅 376 对残留,993 对因符号无 aspects 不适用)。

> **2026-08-31 增量**(SG 全量 250 份入库 + 审计 9 轮后):758 页 / 对 7704(直连提及 999 + 二跳 61 + stub 6644,其中未审计 stub 227);有提及页面 446;语义不一致 verdict 累计 151。三分类:确定性层+人工覆写为 class(C 类经人工核实清零:4→A、2→B、1 留 D 复看),LLM 层结果降级为 llm_suggest 建议字段(见 §8)。剩余运营项见 [REMAINING_WORK.md](REMAINING_WORK.md)。

**三层核对结论**(对照游戏源码 + 页面语料原文):

| 检查 | 方法 | 结果 |
|---|---|---|
| 转化 precision | 25 条抽样:wording 逐字比对 + 5 条 SUSPECT 人工对照原文 | 逐字命中 14/25(其余为概括措辞);5 条 SUSPECT **全部验证为真转化**(如“可以被锤子敲碎，掉落 2 个骨头碎片”↔workable);有效 precision ≈95%+ |
| 漏报 recall | 30 条未提及对:search_terms 命中页面则疑 | 1/30 疑似,且该例命中词为泛词(范围伤害/攻击范围),低风险 |
| D1 数值配对 | 20 条配对回查源码常量 | **20/20** 常量名与数值均存在 |
| 不一致复核 | 12 条逐条人工判读 | **无一是页面错误**:10 条为路由过近似(页面句子描述的是邻近组件数据,如“死时三选一”属 lootdropper 而被路由到 health),2 条为 sanityaura 变体列表缺口(完全正常的树/蘑菇地精页面确有光环值) |

**教训**:①`semantic_consistent=false` 的语义是「页面句子与本符号代码语义对不上」,主要构成是路由过近似信号与变体覆盖缺口,不应直接当页面纠错清单用——M3 消费前需按此分类;②审计 verdict 的 wording 是概括/引用混合,自动 precision 校验须区分逐字与语义命中;③搜索词盲区(掉落/锤子/冷却等)导致确定性层与人工校验的 terms 命中失灵,search_terms 质量杠杆再次被证实有价值。

## 6. 已知取舍

- D1 只做 quantity↔命名常量的精确相等匹配,松匹配(区间/百分比/单位换算)留给 L2 或 update 侧 F3;
- pass2 未采样到的页面即使真实提及了符号,D0 也探不到——这正是 M2b L2 的存在意义,M2a 的"stub"语义是「确定性方法未见证据」而非「页面一定没写」;
- `pages_without_prefab`(2566 页)与系统机制专题页不在本图范围(与 M1 路由同口径)。

## 8. 语义不一致三分类(--classify,2026-08-31)

§7 教训①的落地:对 `semantic_consistent=false` 的对做消费前分类,命令
`knowledge-scan-wiki --classify`(聚合模式,不重建地图),产物
`knowledge/inconsistent_classification.json`。

- **确定性层**:verdict wording 抽数值 → 与「本符号文件 / 同页兄弟符号文件 /
  页面 prefab 文件」常量匹配,再叠 prefab 字面量交叉(属性赋值如 aura=-40);
  兄弟引文 bigram 重叠 ≥0.5 也判路由过近似。
- **LLM 辅助层**:确定性判 D_manual 的残差批量送审(25 条/批,审计 note 为
  证据),失败保持人工。
- **四类语义**:A_routing 路由过近似(M3 剔除)/ B_variant_gap 变体或跨文件
  缺口(回流 SymbolDoc 修正)/ C_page_error 疑似页面错误(人工纠错候选,
  含 aurafn 类动态计算值,人核时优先判 B)/ D_manual 无法判定。
- **LLM 层两级演化**:初版直接改 class,实测 run 间方差大(A 57↔45,三票
  多数仍不收敛)→ 降级为 `llm_suggest` **建议字段**(不改 class),class 由
  确定性层+人工覆写(`knowledge/inconsistent_manual.json`)决定,重跑零方差;
  人工按 suggest 分诊 D_manual。
- 2026-08-31 实测:101 对初分类 + 人工核实 7 对(C 清零:4A/2B/1D 复看)
  + sanityaura 文档回流;后续审计+50 对入池,均带 llm_suggest 分诊提示。
