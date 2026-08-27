# Knowledge Pipeline 设计(符号知识文档驱动)

> 状态:M0 设计冻结 v2(2026-08-27):按 pilot 评审拆分双通道,wiki_aspects/page_hint 移出代码事实通道。
> 决策记录:knowledge/ 入库 git;文档正文中文;a 阶段先 component;b 暂只做实体/prefab 页(系统机制页挂起);目标 1/3 输出建议清单而非代写内容;并发策略待 pilot 后定,当前串行。

## 1. 意图(与产品构想的映射)

| 构想 | 落点 |
|---|---|
| a. LLM 扫代码,标注主要游戏性 symbol | `knowledge scan-symbols` → SymbolDoc 文档库 |
| b. LLM 扫 wiki 语料,symbol 提及/忽略地图 | `knowledge scan-wiki`(M2)→ PageSymbolMap |
| c. 随代码更新增量修订 a/b | `knowledge sync`(M3),sha 驱动,复用 update-scan diff |
| 目标1 新页面组织建议 | 消费 SymbolDoc 族(相似组件/同类实体)(M3 后 `page-assist`) |
| 目标2 改码→页面影响与修订 | sync 产物:受影响页面 + 需变更的 aspects 清单 |
| 目标3 手工编辑→symbol 归因 | 编辑段落 × SymbolDoc 术语归因(M3 后) |

核心原则:**LLM 产物是版本化文档(基础数据),不是一次性报表;关联索引只做路由不做理解。**

## 2. 目录契约

```
knowledge/                    # git 入库,版本化审阅
└── symbols/
    ├── component__health.json
    └── ...
output/knowledge/raw/         # 不入库:LLM 原始响应 + 执行元数据
└── symbols/component__health__p1.json
```

- 文档文件名:`{category}__{name}.json`,name = 去掉 `components/` 前缀与 `.lua` 后缀。
- 写入原子化(tmp + rename);`provenance.source_sha256` 是增量判据。

## 3. SymbolDoc Schema(v1, prompt_rev=p1)

```jsonc
{
  "schema_version": 2,
  "prompt_rev": "p2",
  "reference": {"kind": "file", "path": "components/health.lua"},
  "category": "component",
  "display_name": "health",
  "summary": "中文 2~4 句:该组件管理什么、如何参与玩法。",
  "api": [{"name": "DoDelta", "signature": "(delta)", "effect": "…", "page_hint": "实体页应写…"}],
  "events_published": ["healthdelta"],
  "events_listened": [],
  "netvars": [],
  "tunables": [],
  "gameplay_tags": ["生存", "战斗"],
  "wiki": {                          // pass2(link-wiki)产物;pass1 后为空
    "scanned_pageids": [13857],
    "aspects": [{"aspect": "血量数值标注", "evidence": [{"pageid": 13857, "quote": "…"}]}],
    "no_evidence_reason": null,      // 空结果时必填,声明判断依据
    "coverage_status": "partial"     // no_wiki_mention | partial | well_documented
  },
  "related_symbols": [{"kind": "file", "path": "prefabs/...", "relation": "consumer"}],
  "truncation_note": null,            // 源码超长被截断时说明
  "provenance": {"source_sha256": "…", "source_bytes": 22115,
                 "build_id": "…", "model": "…", "generated_at": "…"}
}
```

- v2 双通道:`wiki_aspects`/`api.page_hint` 已从代码事实通道**移除**(pilot 证明仅凭源码必然自由发挥;inspectable 类基础设施组件在 wiki 中几乎不被提及,空即是结论)。页面组织建议改由目标 1 流程在编辑时动态合成。
- LLM 只产出 knowledge 部分;`reference/schema_version/prompt_rev/provenance` 由管线回填。
- 解析走容错链(围栏/散文剥离/尾逗号/残缺元素),复用 symbol_page 既有工具。
- 硬校验:`summary` 非空、`api` 非空(component 必有方法);失败 → raw 已归档,报错含定位。

### pass2(link-wiki)契约

- 路由:组件 →(关联索引)→ 变体 → 页面;按 fact 富裕度采样 `--sample-pages`(默认 8)。
- 输入:组件 API 能力清单 + 每页摘录(含实体名,单页 2KB 封顶)。
- 任务性质:**检索 + 归因**(在真实页面里找能力的玩家语言表达),不是生成建议。
- 证据约束:evidence.pageid 必须 ∈ 采样集合,过滤后无证据的 aspect 整条丢弃(宁空勿造)。
- 负结果:`aspects=[] + no_evidence_reason`(必填)→ `coverage_status=no_wiki_mention`;1~2 条 partial;≥3 条 well_documented。

## 4. 选择与增量策略

- 候选排序:复用 `top_symbols`(变体引用数降序),过滤 `kind=File ∧ path 以 components/ 开头`,取 `--limit`。
- 跳过条件:已有文档且 `source_sha256` 相同且 `prompt_rev` 相同 → skip(日志可见)。`--force` 可重扫。
- 文档新鲜但缺 `wiki` 节点且提供了 `--corpus` → 仅补跑 pass2,不重读源码。

## 5. 提示词契约(p1, component)

- system:资深 DST 源码分析员;输出单个 JSON 对象;全中文;禁止编造源码中不存在的内容。
- user:`<源码全文>` + 空文档骨架说明 + 字段填写规则(api 至少覆盖全部 public 方法;page_hint 站在维基编辑者视角)。
- 源码 >48KB 截断并在 `truncation_note` 声明(当前 components 最大 ~27KB,未触发)。

## 6. 可观测性与公平基准

沿用 symbol-annotate 的纪律:流式进度事件(FirstToken/Tick/Done)、每文档 raw 归档、失败重试 1 次、对账(symbol 侧无页面集合概念,改为 schema 校验率统计)。pilot 以「文档数 / schema 合格率 / 耗时 / 输出字节」为硬指标。

## 7. 后续里程碑

- M2 `scan-wiki`:仅 (page, component) 经索引路由的相关对;实体页优先;产出 PageSymbolMap(mentions/aspects_covered/aspects_ignored/detail_level)。
- M3 `sync`:update-scan 变更集 → 脏 SymbolDoc 重扫 → 与 PageSymbolMap 交叉 → 页面修订建议清单;新增 `page-assist`(目标1/3,仅建议清单)。

## 8. 实施进度(2026-08-27)

### 8.1 已完成

| 项 | 内容 | 验证 |
|---|---|---|
| 基础设施 | LLM 客户端迁移至 async-openai(SSE 流式),`LLM__TIMEOUT_SECS` 可配,connect_timeout 10s | 三代 pilot 全程可观测 |
| M0 | Schema v3 + 目录契约 + 提示词契约 p3 定稿(本文档) | — |
| M1 pass1 | `knowledge-scan-symbols`:component 源码 → SymbolDoc(纯代码事实通道) | pilot 3/3 文档,schema 合格率 100% |
| M1 pass2 | link-wiki 语料归因:search_terms 全文检索采样 → 带证据 aspects / 负结果声明 | 见 8.2 |

### 8.2 Pilot 三轮演进(关键教训)

| 版本 | 采样/产出方式 | 结果与教训 |
|---|---|---|
| v1 (p1) | 仅源码;强制产出 wiki_aspects/page_hint | **自由发挥**:inspectable 被捏造出 7 条 aspects——「无语境+必填字段」必然幻觉 |
| v2 (p2) | 拆双通道;pass2 用 路由链+fact数排序 采样 | inspectable 空结果 ✔;但 inventoryitem 也空——fact 排序是组件无关信号,生物页霸榜 |
| v3 (p3) | pass1 增产 `search_terms`;pass2 以**全文检索命中数**为主采样(6891 页 wikitext 本地缓存),路由链降为兜底 | inventoryitem 3 条 aspects 全带引文(捡起/储物/掉出),inspectable 仍为空(命中仅 2 页)——**空即结论**成立 |

### 8.3 数据事实(记录备查)

- `pages_by_prefab` 覆盖 1170/2268 页(339 页多实体归属,取首映射有噪声)
- inventoryitem 变体 553 个(全部物品+可拾取生物),路由可达 417 页
- fact 富裕度排序与组件相关性无关 → 必须组件感知采样(v3 的 search_terms 即为此设计)
- 3 个 pilot 文档耗时:pass1 52~81s/个,pass2 20~24s/个(hy3-free,串行)

## 9. 后续计划

### M1 收尾(下一步)
1. **扩量**:limit=20~50 全量 component(串行预计 1~2.5h;扩量前先跑 10 个验证 search_terms 质量的稳定性)
2. **search_terms 质量杠杆**:候选改进——每个 API 条目强制 1~2 个词;对 0 命中组件允许模型二次修正检索词重试一次
3. 可选:pass2 对「检索命中但判空」的组件做二次确认 prompt(仅当命中页 ≥3 且 aspects=0,防过严)

### M2(构想 b 完整版)
1. `knowledge scan-wiki`:对 2268 页 × 已有 SymbolDoc 的组件做 (page,symbol) 归因,产出 PageSymbolMap(aspects_covered/aspects_ignored/detail_level)
2. 旧 verdict 审计管线收编为 `--audit` 校验子模式
3. 系统机制页(蜘蛛/冬季等专题)是否纳入 → 待解禁后评估路由扩展

### M3(构想 c 增量)
1. `knowledge sync`:update-scan 变更集 → sha 驱动脏文档重扫 → 与 PageSymbolMap 交叉 → 页面修订建议清单
2. `page-assist`(目标 1/3):新页面组织建议 / 手工编辑的 symbol 归因,输出建议清单(不代写)
3. 并发策略:pilot 通过后决定(候选:2~4 并发 + 全局 QPS 上限 + 失败退避)

### 观察项
- inventoryitem 类「广谱组件」(数百变体)与「窄谱组件」的 aspects 产出率差异,指导文档粒度是否需要按实体页聚合二次加工
- hy3-free 的 reasoning_content 未被采用(Delta.content 之外字段),如需思维链可观测,后续评估 async-openai 该字段支持
