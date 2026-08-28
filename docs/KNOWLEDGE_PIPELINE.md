# Knowledge Pipeline 设计(符号知识文档驱动)

> 状态:M1 扩量至 50 份 SymbolDoc(2026-08-28);pass2 已加入版本过滤/章节过滤/重试容错;待全量 force 刷新与人工 review 后进入 M2。
> 决策记录:knowledge/ 入库 git;文档正文中文;a 阶段先 component;b 暂只做实体/prefab 页(系统机制页挂起);目标 1/3 输出建议清单而非代写内容;并发策略待 pilot 后定,当前串行。
> 决策记录(2026-08-28 补):a 阶段类别扩展**方案 B** 已定案——behaviours 全量(词典层)+ brains pilot 先行,stategraph 延后;细化见 [KNOWLEDGE_BEHAVIOUR_CHAIN.md](KNOWLEDGE_BEHAVIOUR_CHAIN.md)。

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
  "api_note": null,                  // api 为空时必填说明,例如“纯数据组件,无公开方法”
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
  "auto_maintained": {               // post-action 注入,非 LLM 生成
    "source": "Module:AutoInfobox",
    "fields": ["生命值", "最大生命值"],
    "code_fields": ["health", "health.max"],
    "overridable_fields": ["生命值"],
    "note": "生命值由 AutoInfobox 自动渲染，但实体页可能手写覆盖。"
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
- 硬校验:`summary` 非空;`api` 为空时必须提供 `api_note` 说明(允许纯数据组件);失败 → raw 已归档,报错含定位。
- `auto_maintained` 由 post-action 从 `knowledge/auto_infobox.json` 冷数据注入,不参与 LLM 生成与硬校验;字段同时保留 wiki 展示名与代码侧字段名。
- envelope 字段(reference/category/summary/wiki/related_symbols/provenance 等)类别无关;类别专属载荷(behaviour 的 ctor_params、brain 的 behaviour_invocations/context_branches 等)为可选字段,component 文档不受影响——细化见 [KNOWLEDGE_BEHAVIOUR_CHAIN.md](KNOWLEDGE_BEHAVIOUR_CHAIN.md) §4。

### pass2(link-wiki)契约

- 路由:组件 →(关联索引)→ 变体 → 页面;按 fact 富裕度采样 `--sample-pages`(默认 8)。
- 版本过滤:只使用联机版(dst)/混合(mixed)页面,排除单机版/DLC、重定向、消歧义与未知页;全文缓存与 facts 同步过滤。
- 章节过滤:提交 LLM 前按章节优先级裁剪——高/中优先级章节(导语/信息框/行为/掉落/获取/提示/策略等)进入上下文,低优先级(花絮/皮肤/Bug/画廊/脚注等)完全排除;未收录章节默认 Medium 不忽略,高优先级关键词覆盖行为/战斗/掉落/获取/进食/温度/容器等常见机制章节,并对高优先级区域额外提供开头上下文行。
- 输入:组件 API 能力清单 + 每页摘录(含实体名,单页 2KB 封顶)。
- 任务性质:**检索 + 归因**(在真实页面里找能力的玩家语言表达),不是生成建议。
- 证据约束:evidence.pageid 必须 ∈ 采样集合,过滤后无证据的 aspect 整条丢弃(宁空勿造)。
- 负结果:`aspects=[] + no_evidence_reason`(必填)→ `coverage_status=no_wiki_mention`;1~2 条 partial;≥3 条 well_documented。

## 4. 选择与增量策略

- 候选排序:复用 `top_symbols`(变体引用数降序),过滤 `kind=File ∧ path 以 components/ 开头`,取 `--limit`。
- 跳过条件:已有文档且 `source_sha256` 相同且 `prompt_rev` 相同 → skip(日志可见)。`--force` 可重扫。
- 文档新鲜但缺 `wiki` 节点且提供了 `--corpus` → 仅补跑 pass2,不重读源码。
- 并发:支持 `--concurrency N`(默认 1=串行);>1 时多个组件并行调用 LLM,注意 API 限流;组件间文件写入互不冲突。

## 5. 提示词契约(p1, component)

- system:资深 DST 源码分析员;输出单个 JSON 对象;全中文;禁止编造源码中不存在的内容。
- user:`<源码全文>` + 空文档骨架说明 + 字段填写规则(api 至少覆盖全部 public 方法;若确实没有方法则 api 为空并填写 api_note;page_hint 站在维基编辑者视角)。
- 源码 >128KB 采用“头 + 尾”截断(默认保留前 96KB + 后 32KB)并在 `truncation_note` 声明;当前仅 `playercontroller.lua` 超出。

## 6. 可观测性与公平基准

沿用 symbol-annotate 的纪律:流式进度事件(FirstToken/Tick/Done)、每文档 raw 归档、失败重试 1 次、对账(symbol 侧无页面集合概念,改为 schema 校验率统计)。pilot 以「文档数 / schema 合格率 / 耗时 / 输出字节」为硬指标。

## 7. 后续里程碑

- M2 `scan-wiki`:仅 (page, component) 经索引路由的相关对;实体页优先;产出 PageSymbolMap(mentions/aspects_covered/aspects_ignored/detail_level)。
- M3 `sync`:update-scan 变更集 → 脏 SymbolDoc 重扫 → 与 PageSymbolMap 交叉 → 页面修订建议清单;新增 `page-assist`(目标1/3,仅建议清单)。

## 8. 实施进度(更新至 2026-08-28)

### 8.1 已完成

| 项 | 内容 | 验证 |
|---|---|---|
| 基础设施 | LLM 客户端迁移至 async-openai(SSE 流式),`LLM__TIMEOUT_SECS` 可配,connect_timeout 10s | 三代 pilot 全程可观测 |
| M0 | Schema v3 + 目录契约 + 提示词契约 p3 定稿(本文档) | — |
| M1 pass1 | `knowledge-scan-symbols`:component 源码 → SymbolDoc(纯代码事实通道) | pilot 3/3 文档,schema 合格率 100% |
| M1 pass2 | link-wiki 语料归因:search_terms 全文检索采样 → 带证据 aspects / 负结果声明 | 见 8.2 |
| M1 扩量 | `--limit 50` 生成 50 份 component SymbolDoc(含 tradable) | schema 合格率 49/50(首轮 tradable 因空 api 失败,补 api_note 后通过) |
| M1 pass2 健壮性 | pass2 LLM 重试 1 次;连续失败不中断整批,保留 pass1 文档待补跑 | 长跑未因单次流式错误中断 |
| M1 纯数据组件 | `api` 允许为空,但必须填 `api_note` 说明 | tradable 成功入库 |
| M1 版本过滤 | 只保留 dst/mixed 页面,排除 ds/unknown/redirect/disambig | 全文页 6891→1849;facts 页 1881→1051 |
| M1 章节过滤 | 提交 LLM 前按章节优先级过滤,低优先级完全排除;高优先级关键词扩展 + 高优先级区域增加上下文行 | 8 组件对照见 8.2 v4 |
| M1 源码上限 | `SOURCE_BYTES_CAP` 提升至 128KB,超限采用头尾截断 | 当前仅 `playercontroller.lua` 超限 |
| M1 AutoInfobox | 新增 `knowledge/auto_infobox.json` 冷数据 + post-action 注入 `auto_maintained` | 初始覆盖 health/combat/locomotor/perishable/edible/equippable/fuel/finiteuses/stackable/workable |

### 8.2 Pilot 三轮演进(关键教训)

| 版本 | 采样/产出方式 | 结果与教训 |
|---|---|---|
| v1 (p1) | 仅源码;强制产出 wiki_aspects/page_hint | **自由发挥**:inspectable 被捏造出 7 条 aspects——「无语境+必填字段」必然幻觉 |
| v2 (p2) | 拆双通道;pass2 用 路由链+fact数排序 采样 | inspectable 空结果 ✔;但 inventoryitem 也空——fact 排序是组件无关信号,生物页霸榜 |
| v3 (p3) | pass1 增产 `search_terms`;pass2 以**全文检索命中数**为主采样(6891 页 wikitext 本地缓存),路由链降为兜底 | inventoryitem 3 条 aspects 全带引文(捡起/储物/掉出),inspectable 仍为空(命中仅 2 页)——**空即结论**成立 |
| v4 (p3+过滤) | 仅使用 dst/mixed 页面;提交语料前按章节高/中优先级裁剪 | 8 组件对照:采样页全部为联机版;旧混入 ds 证据被清除;floater 回退 no_wiki_mention,inspectable/hauntable 从 0 到 1;inventoryitem 旧的“储物/捡起”证据中 ds 页被移除 |

### 8.3 数据事实(记录备查)

- `pages_by_prefab` 覆盖 1170/2268 页(339 页多实体归属,取首映射有噪声)
- inventoryitem 变体 553 个(全部物品+可拾取生物),路由可达 417 页
- fact 富裕度排序与组件相关性无关 → 必须组件感知采样(v3 的 search_terms 即为此设计)
- 3 个 pilot 文档耗时:pass1 52~81s/个,pass2 20~24s/个(hy3-free,串行)
- 全文检索/缓存的页面从 6891 收敛到 1849(dst/mixed);facts 来源页从 1881 收敛到 1051
- 旧文档中确实混入 ds 证据:如 inventoryitem 的根箱/皮弗娄牛(单机版),lootdropper 的鲸鱼尸体/远古兵器/蹊跷的事物,workable 的木炭岩石

## 9. 后续计划

### M1 收尾(当前)
1. **全量刷新主库**:用新逻辑 `--limit 50 --force` 重跑 `knowledge/symbols/`,将当前仍含旧 ds 证据的 50 份文档替换为纯联机版+章节过滤版本
2. **人工 review 重点 diff**:inventoryitem / lootdropper / workable / floater / inspectable / hauntable,确认新 aspects 更符合联机版页面
3. **search_terms 质量杠杆**(可选):每个 API 条目强制 1~2 个词;对 0 命中组件允许模型二次修正检索词重试一次
4. **二次确认**(可选):pass2 对「检索命中但判空」的组件做二次确认 prompt(仅当命中页 ≥3 且 aspects=0,防过严)

> 当前 `output/knowledge_compare/` 为 8 组件对照临时产物,不入库;确认后可用它作为全量刷新前的预期样本。

### M1.5 行为链类别扩展(方案 B,代码基础已落地,待跑 pilot)

细化方案、批次定义、schema 载荷草案、协同边界见 [KNOWLEDGE_BEHAVIOUR_CHAIN.md](KNOWLEDGE_BEHAVIOUR_CHAIN.md)。要点:

1. 批次 1a:behaviours 全量 29 份(词典层,pass1 only;抽 2 个验证 pass2 负结果路径);
2. 批次 1b:brains pilot 3 份(houndbrain / spiderbrain / beefalobrain,对应页均有"行为"章;brain pass1 注入所引用 behaviour 的参数语义);
3. stategraph 延后:per-file 截断版起步,排除 SGwilson* / *_client / commonstates;粒度与边界决策已记录,重开条件 = brains pilot 完成;
4. 与 component 线解耦:新增 category 分派,不动 pick_components 与组件 prompt;
5. 已落地:`--category` / `--pass2-names`、behaviour 目录扫描 pick、brain/behaviour schema 与 prompt、brain 注入 behaviour 参数语义、pass2 抽样控制、单元测试;1a/1b 尚未跑。

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
