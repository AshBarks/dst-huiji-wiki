# Component 全量扫描报告(815/815)

> 生成:2026-08-31。范围:全部 815 个组件文件的 SymbolDoc 生成 + 质量核验。
> 关联:[KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md)、[KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md](KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md)。
> 数据:`knowledge/symbols/component__*.json`(815 份)、`output/knowledge/raw/symbols/`、汇总见 `knowledge/page_map_summary.json`。

---

## 1. 执行概况

| 项 | 值 |
|---|---|
| 覆盖 | **815/815(100%)**——含 314 个从未被 prefab 直接引用的管理器/子组件(选择器已改为目录扫描,`ecf6690`) |
| 执行方式 | 试点 52(mimo-v2.5 冒烟与试跑)+ 全量并发 4(引用优先排序)+ 清理轮 3 |
| 模型 | mimo-v2.5 @ `zen/go/v1`(764 份)+ laguna-s-2.1-free(1)+ hy3-free(50,历史保留) |
| 失败 | 试点 9 + 全量 3,全部在修复后清理轮重跑成功,零残留 |
| 耗时 | 全量轮约 6.8 小时(并发 4,~1.8 份/分钟) |

**过程中处理的模型迁移与故障**:hy3-free 被网关下线(401)→ 付费池余额耗尽 → 免费池探测(laguna/ling 可用但限流)→ 最终 `mimo-v2.5` @ `zen/go/v1`。管线为此新增三层适配:`ApiEntry` method/function 别名、`LLM__MAX_TOKENS`(推理模型输出预算)、杠杆改写恶化回退。

## 2. 全量结果

| 指标 | 值 |
|---|---|
| 覆盖分布 | well_documented **185** / partial **267** / no_wiki_mention **363** |
| search_terms 杠杆 | 触发 **393/815(48%)**,零命中率显著下降(50 份试点实测 56%→42%) |
| api_note 规则 | 无 api 且无 api_note 的文档 **0**(schema 校验全程有效) |

值得注意的空白区:363 份 no_wiki_mention 集中在管理器/副本同步/复刻组件(`*_replica`、`*_manager`、shard_* 等)——页面正文不描述它们属正常,这部分构成 PageSymbolMap 中"路由过但无内容"的主体。

## 3. 质量核验(对照游戏源码 + 页面语料)

| 检查 | 方法 | 结果 |
|---|---|---|
| api 源码存在性 | 40 份 × ≤8 条,方法名在组件源码中正则回查 | **223/223(100%)**(核对脚本修正签名后缀匹配缺陷后) |
| 引文逐字率 | 60 条抽样,引文规范化后回查页面 wikitext | **53/60(88%)**,未命中均为已知的概括/去括号模式,未发现编造 |
| api_note 规则 | 全量 815 扫描 | 0 违规 |
| search_terms 溯源 | 全量扫描 | 393 份携带 `search_terms_note` |
| 引文总量 | PageSymbolMap 页面锚点 | 1669 条(随全量增长) |

已知限制(与 M2 审计报告一致):引文存在概括/去括号/表格片段三种非逐字形态;pass2 prompt 已强化为逐字摘录(`c4eacef`),下轮重扫生效。

## 4. 过程修复记录

| 问题 | 修复 | 提交 |
|---|---|---|
| api 条目写成 `method`(mimo 字段命名偏好) | `ApiEntry` 反序列化别名 method/function | `820881d` |
| 推理模型输出为空(reasoning 耗尽输出预算) | `LLM__MAX_TOKENS` 可配(当前 16384),未设置不发送 | `820881d` |
| 杠杆改写更差仍被接受 | 改写后复检,恶化则回退原词 | `8b8d7fe` |
| pass2 引文概括/拼接 | prompt 强化逐字摘录约束 | `c4eacef` |
| 314 个无引用组件选不到 | `pick_components` 改目录扫描 | `ecf6690` |

## 5. 对下游的影响

1. **PageSymbolMap 扩容**:提及对从 701 增长(新组件的 pass2 aspects 已写入图);`--report` 一次刷新即可见;
2. **AutoInfobox 闭环补全**:waterproofer/prototyper/damagetyperesist/repairer 等"冷数据已核实但一直缺文档"的符号全部补齐;
3. **M3 sync 生效面扩大**:任何组件代码变更现在都有对应 SymbolDoc 可判脏;
4. **no_wiki_mention 363 份**是 page-assist 缺口榜与 M3 建议清单的"已知不可写"底座,避免误报。

## 6. 复现命令

```bash
# 全量(已有文档自动跳过,断点续跑)
cargo run --release -- knowledge-scan-symbols "<scripts>" \
  --category component --limit 900 \
  --corpus wikis/dontstarve.huijiwiki.com --concurrency 4
```
