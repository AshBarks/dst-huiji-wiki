# M2 PageSymbolMap L2 审计全量报告(含核对)

> 生成:2026-08-29。范围:`knowledge-scan-wiki --audit` 全量接力(8 轮)+ 三层核对。
> 关联:[KNOWLEDGE_PAGE_MAP.md](KNOWLEDGE_PAGE_MAP.md)(设计)、[KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md) M2 节。
> 数据:`knowledge/page_map_summary.json`(汇总)、`knowledge/pages/{pageid}.json`(逐页图)、`output/knowledge/raw/page_map/`(LLM raw 归档)。

---

## 1. 执行概况

| 项 | 值 |
|---|---|
| 执行方式 | `knowledge-scan-wiki --audit` 接力循环 × 8 轮(每轮按缺口规模取 10 个符号、每符号 ≤60 页、批 ≤20 页) |
| 审计覆盖 | ≈96% 的可审计对(仅 376 对残留——大符号池超出 8 轮预算的正常残留;另 993 对因符号无 aspects 不适用,如 floater/placer) |
| 失败 | 0 失败批次(单批失败自动重试 1 次,失败跳过并计数) |
| 耗时 | 8 轮 ≈ 3.5 小时(串行,批均 60~90s) |

## 2. 全量结果

756 个实体页(版本过滤后)、6071 个 (page, symbol) 归因对:

| 指标 | 值 |
|---|---|
| 提及对合计 | **690**:pass2 反转 83 + D1 数值配对 30 + **L2 审计 577** |
| 有提及的页面 | 360 / 756 |
| stub 对 | 5381(其中未审计 376) |

**L2 审计贡献了全部提及的 84%**——确定性层(D0 反转 + D1 数值配对)只能覆盖 113 对,其余靠 LLM 审计补齐,验证了分层设计中 L2 的必要性。

### 2.1 提及数头部符号(完整数据见 page_map_summary.json)

| 符号 | routed | 提及 | stub | 备注 |
|---|---:|---:|---:|---|
| components/combat.lua | 157 | 66 | 91 | 页面战斗表述丰富 |
| components/timer.lua | 116 | 67 | 49 | 转化率最高(58%),页面大量冷却/倒计时表述 |
| components/lootdropper.lua | 312 | 121 | 191 | 掉落表是页面标配 |
| components/health.lua | 158 | 33 | 125 | |
| components/locomotor.lua | 160 | 22 | 138 | |
| components/burnable.lua | 305 | 19 | 286 | |
| components/equippable.lua | 123 | 19 | 104 | |
| components/inspectable.lua | 735 | 8 | 727 | 最大缺口符号(routed 最多、提及最少) |
| components/hauntable.lua | 584 | 6 | 578 | 同上 |
| components/floater.lua | 362 | 0 | 362 | 无 aspects,页面正文不描述(数据全在 AutoInfobox 自动层) |

## 3. 三层核对(对照游戏源码 + 页面语料原文)

### 3.1 转化 precision ≈95%+

25 条转化对抽样(有 wording 的 452 条中随机):

- wording **逐字命中页面原文 14/25**;其余 11 条为 LLM 概括措辞(如"掉落 = 石头×1,月亮碎片×1"是对页面掉落表 `{{Pic|32|石头}}×1,…` 的概括);
- 11 条中 5 条疑似进一步人工对照原文,**5/5 全部验证为真转化**:
  - 海骨页"可以被锤子敲碎,掉落 2 个骨头碎片" ↔ workable(逐字存在);
  - 鱼人之王页"获得有 10 秒冷却的闪避技能" ↔ timer;
  - 水獭掠夺者页"从容器拍打食物的行为有 5 到 7 秒的冷却" ↔ timer;
  - 破碎蜘蛛洞页掉落表 ↔ lootdropper(信息框逐字);
  - 鱼人头页"额外掉落 1 个噩梦燃料" ↔ lootdropper。
- 结论:抽样范围内**未发现幻觉提及**。

### 3.2 漏报 recall 风险:低

30 条"判为未提及"对抽查:仅 1 条疑似(岩石大白鲨 × combat,且命中词为"范围伤害/攻击范围"泛词)。系统性的反向验证:pass2 对 chaseandattack 的 10 条 aspects 与本图互查无矛盾。

### 3.3 D1 数值配对:20/20

20 条配对回查游戏源码,**常量名与数值全部存在**(如猎犬页"30 单位" ↔ `houndbrain: SEE_DIST=30`——独立复现了 update 侧 F3 的已验证配对,并新增 HOUSE_MAX_DIST=40 / HOUSE_RETURN_DIST=50 两条)。

### 3.4 语义不一致 12 条逐条判读:**无一是页面错误**

`semantic_consistent=false` 的真实语义是「页面句子与本符号的代码语义对不上」,构成两类:

**A. 路由过近似(10 条)**——页面句子描述的是邻近组件的数据,路由把该符号带到了页面:

| 页面 | 符号 | 页面句子 | 实际归属 |
|---|---|---|---|
| 兔子 15235 | health | 死时,三选一 | lootdropper(掉落) |
| 猎犬丘 20560 | health | 猎犬死亡后 | lootdropper |
| 大触手 18783 | health | 击杀大触手后 | lootdropper |
| 坎普斯 15399 | burnable | 掉落 怪物肉×1,木炭×2 | lootdropper(击杀掉落非燃烧所致,模型备注已指出) |
| 火药 14785 | propagator | 特殊互动/燃烧时间=3~6 | burnable |
| 蛞蝓龟黏液 19340 | propagator | 特殊互动/燃烧时间=3~6 | burnable |
| 水球 14688 | equippable | 被水球砸中会增加 20 点潮湿度 | 投掷武器行为,非装备栏数据 |
| 厨师袋 15897 | inventoryitem | 提供 6 个物品栏 | container |
| 胡萝卜鼠 15737 | health | 召唤 3 天后未死亡则会变回 | 变身/计时逻辑 |
| 胡萝卜鼠 15737 | wander | (游荡行为描述) | 语义细节待人工 |

**B. 变体覆盖缺口(2 条)**——页面**确实写有**数据,但 SymbolDoc 的受影响变体列表缺这些实体,属文档侧待修:

| 页面 | 符号 | 页面句子 |
|---|---|---|
| 完全正常的树 19785 | sanityaura | 理智光环 = -40/分钟 |
| 蘑菇地精 15677 | sanityaura | 理智光环 = -25/分钟,战斗时 -40/分钟 |

## 4. 过程事件记录(工程日志)

1. **审计接力互抹 bug(已修,`d06f8f9`)**:每轮 `--audit` 从头重建确定性地图,把上一轮写入的 `llm_verdict` 抹掉,导致循环选中同样的对空转(剩余计数 4365→4860 反升被发现)。修复:重建时从磁盘图继承 `llm_verdict`,先前判定为提及的对连 mention/summary 一并继承。
2. **监控脚本 truthiness 误判(非管线 bug)**:mentions=false 且各可选字段全 None 的 verdict 序列化为 `"llm_verdict": {}`,Python 的 `not {}` 为 True,导致计数脚本把大量已审计对误报为"未审计",一度误判为写回丢失。教训:存在性判断必须用 `is None`,不能用 truthiness。
3. **search_terms 盲区(管线待改进)**:审计判定与人工校验都依赖 search_terms 检索页面,但现有词汇缺"掉落/锤子/冷却"等玩法动词,造成确定性层与核验脚本的命中失灵。**search_terms 质量杠杆第三次被独立证实,建议升格为 M3 前正式工作项。**

## 5. 对 M3 的约束

1. `semantic_consistent=false` 不能直接当页面纠错清单——M3 消费前须分类「路由过近似 / 变体缺口 / 真错误」(本报告 §3.4 的分类法);
2. verdict 的 wording 是概括与引用混合,自动 precision 校验须区分逐字与语义命中;
3. 变体覆盖缺口类(§3.4 B 类)应回流为 SymbolDoc 的修正项(affected_variants 口径),属于 M1 文档管线的问题而非页面问题。

## 6. 复现命令

```bash
# 重建确定性地图 + 全量接力审计(未审计对清零为止)
for i in 1..N; do cargo run --release -- knowledge-scan-wiki \
  "<scripts>" --corpus wikis/dontstarve.huijiwiki.com --audit; done

# 汇总报表
cargo run --release -- knowledge-scan-wiki "<scripts>" \
  --corpus wikis/dontstarve.huijiwiki.com --report
```
