# 页面全量语料抓取方案（Wiki Corpus Harvesting）

**状态**：v1.2（初版已实现：`corpus-fetch` 命令 + `service::JobKind::CorpusSync`；recentchanges 增量通道见 §12）
**修订记录**：
- v1（本版）：主命名空间全量抓取方案——范围界定、存储布局、DST/单机版分类器、增量同步与验收标准
- v1.1：落地修正——①本站 MediaWiki 1.38 的 `list=allpages` 不返回 `touched/len/redirect`，枚举改用 `generator=allpages&prop=info`（长度字段名为 `length`）；②版本信号补充「/单机版」子页后缀（577 页）；③首轮回填结果见 §11
- v1.2：新增 §12 **recentchanges 增量通道设计**——低频增量 + 作者/动作归因事件流（CodeTextAtlas 对账的供料层），基于 2026-08-26 本站接口实测；RC 实施暂缓
- v1.3：语料首轮结构/习惯分析产出独立成文：[CORPUS_PAGE_PATTERNS.md](CORPUS_PAGE_PATTERNS.md)（页面大类地图、实体页通用骨架、分簇特有结构、书写习惯、不规范观察）

**关联文档**：[UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md)（v3.1，本方案为其 CodeTextAtlas / 类别范式分析的数据底座）

---

## 1. 目标与非目标

### 1.1 目标

在本地 `wikis/` 目录（不入仓库）建立 dontstarve.huijiwiki.com **主命名空间全量原始 wikitext 语料库**：

| # | 目标 | 验收标准 |
|---|------|----------|
| C1 | 全量抓取：主命名空间全部页面（含重定向）的当前版本 wikitext | 枚举数 == 落盘数，无缺漏 |
| C2 | 元数据完备：每页带 pageid/touched/分类/版本类别标注 | meta 覆盖率 100% |
| C3 | 版本分类：联机版/单机版/双版本聚合/消歧义等可区分，不确定项进人工分诊而非误判 | unknown 占比 ≤5% 且清单可见 |
| C4 | 可增量：基于 touched 时间戳的增量同步 + 删除/移动检测 | 二次运行仅拉取变更页 |
| C5 | 幂等可审计：manifest 记录每轮快照参数与统计 | 重复运行无副作用 |

### 1.2 非目标

- 不抓非主命名空间（`Data:*`、`模块:*`、`属性:`、`Form:`、`Rule:` 等——siteinfo 实测清单见 §2.2）；
- 不抓渲染 HTML（Atlas 以原始 wikitext 为唯一文本面，规避数据层刷新干扰）；
- 不做页面历史版本回溯（惯例研究需要时按页后补）；
- 本方案不涉及 wikitext 解析与图谱构建（属 CodeTextAtlas 方案）。

---

## 2. 取证结论（2026-08 实测，方案的事实基础）

### 2.1 规模

| 项 | 数值 |
|----|------|
| 主命名空间非重定向页 | 3,736 |
| 主命名空间重定向页 | 3,153 |
| 合计需抓取 | **≈6,889** |
| 批量内容接口（≤50 titles/次）估算请求 | ≈140 次 ⇒ 1 QPS 下 **<5 分钟** |

全站 statistics：pages 29,473 / articles 3,690——主命名空间外体量巨大但明确排除。

### 2.2 命名空间边界（siteinfo 实测自定义命名空间）

`属性`(102)、`Form`(106)、`概念`(108)、`smw/schema`(112)、`Rule`(114)、`Html`(274)、`博客`(500)、`模块`(828)、`零件`(2300)、`零件定义`(2302)、`Data`(3500)、`SMW`(3502) 及各自讨论页——**均不抓取**。

### 2.3 版本信号实测（重要：纠正「单机板/」前缀假设）

- **标题前缀 `单机版/` 与 `单机板/` 实际不存在（均为 0 页）**，不能作为识别依据；
- 真实信号一：**信息框模板的游戏参数**。单机版实体页写作 `{{实体信息框/自动|ds|tigershark}}`（配套 `{{DSPic}}`、`{{全角色台词|ds|…}}`），联机版为 `|dst|`（`{{Pic}}`、`{{DST}}`）——**页面自我声明版本**，是最强逐页信号；
- 真实信号二：**版本分类体系**。`分类:联机版` ×1779、`分类:单机版` ×1633，交集 81 页（多为"交易者""催眠"这类跨版本机制页）；DLC 另有子标签（如虎鲨 ∈ `分类:海难`）；
- 单机版分类内混有非实体页（更新日志"2019年生活质量更新"、技术页 `DSRoom/*`）⇒ 版本分类 ≠ 实体分类，两者正交；
- 消歧义页存在且使用 `{{消歧义…}}` 模板（insource 实测命中"代码""书""潮湿度"等页）。

---

## 3. 抓取范围界定

1. **仅主命名空间**，含重定向页（重定向正文极小但承载"别名→规范页"引导信息，按原样保留并额外产出映射索引）；
2. **抓取层不做有损过滤**：单机版页面照抓，版本归属写入元数据由下游（Atlas/segmenter）按类消费。理由：无损可回溯；聚合页/消歧义页本身是实体名→规范页链接结构的组成部分，正是 Atlas 需要的素材；
3. 消歧义页、跨版本聚合页作为**独立类别**参与结构范式分析，不并入任何单一版本类。

---

## 4. 存储布局与元数据

```
wikis/                          # 整目录加入 .gitignore
└── dontstarve.huijiwiki.com/
    ├── manifest.json           # 每轮快照：时间戳、API 统计、计数、工具版本、参数
    ├── meta.jsonl              # 每页一行 JSON（权威元数据，可重建一切派生索引）
    └── pages/
        └── <pageid>.wikitext   # 以 pageid 为文件名——标题含 / : 中文与特殊符号，不适合做路径
```

`meta.jsonl` 行结构：

```json
{"pageid":123,"title":"猎犬","ns":0,"touched":"...","len":2805,
 "new":false,"redirect":false,"rev_sha1":"...",
 "categories":["分类:联机版","分类:怪物","分类:猎犬族","..."],
 "game_class":"dst_entity","class_signals":["tpl_param:dst","cat:联机版"],
 "class_confidence":"high"}
```

设计要点：
- **pageid 寻址**：页面改名时 pageid 不变，增量同步天然跟踪移动；标题→文件映射查 meta；
- 派生索引（`redirects.json`：redirect→target 映射；`classes_summary.json`：各类别计数与清单）由脚本从 meta 再生，不手工维护；
- `wikis/` 入 `.gitignore`，随 `5a35735` 已有的本地产物忽略模式管理。

---

## 5. 版本/类型分类器（多信号，先验排序）

类别枚举：`dst_entity`｜`dst_content`（机制/系统页）｜`ds_entity`｜`ds_content`｜`mixed`（双版本聚合）｜`disambig`｜`redirect`｜`non_entity`（帮助/更新日志/沙盒等）｜`unknown`。

| 优先级 | 信号 | 判定 |
|--------|------|------|
| S0 | 枚举 redirect 标志 | redirect |
| S1 | 正文含 `{{消歧义` 模板 | disambig |
| S2 | 信息框/台词模板参数：全文出现 `\|dst\|` 且无 `\|ds\|` → dst_*；反之 ds_*；**两者皆有 → mixed** | 逐页最强信号 |
| S3 | 版本分类成员（联机版/单机版/海难/巨人国/猪镇…） | 与 S2 交叉验证；冲突记 low confidence |
| S4 | 标题/杂讯特征（"更新"后缀、`DSRoom/` 等） | 辅助 non_entity 判定 |

规则落 TOML 配置（信号模板名/分类白名单可增补）；S2/S3 均无法判定的进 `unknown` + 人工分诊队列，**不允许静默猜测**。首轮抓取完成后执行**校准步骤**：分层抽样 ~30 页人工核对类别，据此修订信号规则再全量重算（纯本地操作，秒级）。

---

## 6. 抓取流程

```
1 enumerate     list=allpages apnamespace=0 aplimit=500（redirects/nonredirects 各一遍）
                → 全量 {pageid,title,touched,len,new} 清单（~15 请求）
2 reconcile     与 meta.jsonl 比对：新增 / touched 变化 / 消失（疑似删除或移动）
3 fetch         action=query&prop=revisions|categories&rvprop=content&rvslots=main
                &titles=<≤50 个/批>（~140 请求）；记录 rev_sha1 与分类成员
4 write         pages/<pageid>.wikitext + meta.jsonl 追改；消失页标记 removed（文件归档 _removed/ 一轮）
5 classify      本地跑 §5 分类器，回填 game_class 字段
6 validate      计数对账（枚举==落盘）、len 对账、sha1 抽验、unknown 清单输出
7 manifest      写本轮统计；输出简报（新增/变更/删除/分类分布）
```

工程约束：
- 节流与重试直接复用 `WikiClient` 的 `RateLimitCfg`（默认 1 QPS、WAF 403/429 退避）——这正是此前客户端加固的直接受益场景；
- 匿名读即可完成（公共 API），配置了 `HUIJI__X_AUTHKEY` 则附带以提升稳定性；UA 标识 `dst-huiji-wiki-corpus/<version>`；
- 预计初轮墙钟时间 **<10 分钟**，可在任意时段执行，夜间低峰更稳。

---

## 7. 增量同步

- **变更检测**：每轮 enumerate 后仅抓 `touched` 晚于上轮 manifest 的页（通常个位数请求）；
- **删除/移动**：枚举中消失的 pageid → 查 `action=query&titles=` 复核，确认删除则归档本地文件；移动则沿用同一 pageid 更新标题；
- **窗口期纪律**（承接对账原则）：游戏更新后 1~3 天 bot 同步窗口内照常抓取快照（快照本身无害），但该轮产生的语料**不用于**编辑惯例归纳与关联重建的因果推断，仅在 manifest 中标记 `in_bot_window=true`；
- 全量一致性校验（sha1 全量比对）每月一次或手动触发。

---

## 8. 实现载体

- 服务层 `JobKind::CorpusSync`（WebUI 免费获得），CLI 子命令 **`corpus-fetch`**：`corpus-fetch [--full] [--dir <path>] [--dry-run]`；
  - 默认增量：枚举后仅抓 `touched` 变化页与本地缺文件页（自愈断点）；`--full` 全量重抓；
  - `--dry-run` 只枚举+对账出报告，不写任何本地文件；语料作业永不写维基；
- 核心逻辑在库内 `src/corpus/`：`mod.rs`（sync 编排+对账纯函数）、`model.rs`(PageMeta/GameClass/Manifest)、`classify.rs`（信号规则，常量表待校准后外置 TOML）、`store.rs`（布局与派生索引）；
- 批量 API 在 `src/wiki/client.rs`：`enumerate_namespace()`（generator=allpages+prop=info）、`get_pages_wikitext()`（≤50 titles/批，含 sha1 与分类成员），共享节流重试；
- Web UI 进度展示为可选后续项（JobManager SSE 现成）。

---

## 9. 验收清单

1. 枚举 6,889 页 == meta 行数 == pages 文件数（removed 归档除外）；
2. `len` 字段与实际字节数偏差 0（容差：行尾规范化前后一致即可）；
3. 分类覆盖：unknown ≤5%，其完整清单进入分诊报告；
4. 幂等：连续两轮增量运行，第二轮网络请求为 0；
5. 断点续跑：中途 kill 后重启，已完成页不重复抓取；
6. 抽样质检：随机 20 页 wikitext 与 API 现取结果逐字节一致。

---

## 10. 开放问题

1. 消歧义模板的具体变体（`{{消歧义}}`/`{{消歧义重定向}}`…）全集需在校准步确认；
2. 未来 Tier0 需要 `Data:DST Prefab/*.json` 快照做他方同步核对——是否在本布局下扩展第二命名空间抓取通道，届时另立小节，当前明确排除；
3. ~~recentchanges 增量通道~~ → 已定稿为 §12 设计，待实施。

已决：~~`分类:图鉴收录` 能否作为 entity/content 区分信号~~——**不采用**（官方图鉴分类本身混乱，2026-08 拍板）；entity 判定以信息框模板存在性为准。

---

## 11. 落地记录（2026-08 首轮回填）

| 项 | 结果 |
|----|------|
| 全量抓取 | 枚举 6,889 == 落盘 6,889，缺失 0，长度不符 0 |
| 语料体量 | 9,257,811 字节 wikitext / 6889 文件 |
| 分类分布 | redirect 3153 / dst 1685 / ds 1607 / mixed 93 / disambig 223 / **unknown 128（1.9%，达标 ≤5%）** |
| 幂等验证 | 第二轮增量：待抓取 0、未变化 6889、零网络写请求之外的开销 |
| 抽样质检 | 猎犬页本地 wikitext 与 API 现取逐字节一致 |

工程教训（已固化到实现）：
1. MediaWiki 1.38 的 `list=allpages` 输出**不含** `touched/len/new/redirect` 字段——增量同步若基于它会静默失效；必须用 `generator=allpages&prop=info`，且其长度字段名为 `length` 而非 `len`；
2. 「单机版」命名惯例的真实形态是**子页后缀** `<实体>/单机版`（577 页），非标题前缀；
3. unknown 残余 128 页以版本中立内容为主（首页、机制总览、版本历史等），符合预期，清单见 `index/classes_summary.json`。

---

## 12. recentchanges 增量通道设计（v1.2，**已实施，2026-08-31**）

> 实施记录:`corpus-fetch --rc` 落地,含检查点播种逻辑(回落枚举对账完成后
> 以当下时间播种 rc_state,消除"永远回落"死角)。实测:播种 → 增量拉取
> (10 分钟重叠窗)→ 空窗零副作用 → dry-run 事件预览 全部通过。与设计的
> 两处实现细节:①move 新标题取 `logparams.target_title`(未遇真实 move
> 事件,字段名以单测固化,如实测不符调整一处即可);②§12.7-1 真实编辑
> 捕获验证待 wiki 自然编辑发生后补验。

### 12.1 定位

| 通道 | 请求成本 | 回答的问题 | 角色 |
|------|---------|-----------|------|
| touched 枚举对账（已实现） | ~15 请求/轮，与间隔无关 | 何页变更 | 兜底 + 自愈 + 周期校准 |
| **recentchanges（本节）** | 通常 1~2 请求/天 | 何页变更 × **谁改的、什么动作、何时** | 日常增量；Atlas 对账的供料层 |

RC 通道不替代枚举对账：它更快、更省，且事件流（作者/动作/时间）正是 §7 窗口纪律"分诊—对账—静止期"所需的原始素材。两层共用同一存储布局；RC 通道只追加 `events.jsonl` 事件日志。

### 12.2 接口实测事实（2026-08-26 本站验证）

| 项 | 结论 |
|----|------|
| 参数形态 | `list=recentchanges` + `rcnamespace=0` + `rctype=edit\|new\|log`（categorize 单列类型，当前稀疏，过滤） |
| rcprop 可用集 | title/timestamp/ids/sizes/sha1/user/userid/flags/comment/parsedcomment/loginfo；**patrolled → permissiondenied**（本站不可用，剔除） |
| 日志事件字段 | 扁平化在 rc 行上：`logid`/`logtype`/`logaction`/`logparams`（非嵌套 loginfo 对象） |
| 分页上限 | rclimit=max = **500 条/次**；续传键 `rccontinue`（格式 `"ts\|rcid"`） |
| 无内容变化检测 | 每行带修订 `sha1` ⇒ 与 meta.rev_sha1 比对可跳过内容重抓 |
| 归因可行性 | user 字段实名可见：实测 **Mr鲁鲁 直接编辑主命名空间页面**（含 delete 日志动作）、数字账号（如 1007369644）、普通人类账号并存 |

### 12.3 检查点与拉取算法

状态文件 `wikis/<host>/rc_state.json`：`{last_ts, last_rcid, updated_at_ms}`。

```
1 start   = checkpoint - OVERLAP(10min)        # 重叠窗自愈时钟边界
2 loop:    rcdir=newer, rcstart=start, rctype=edit|new|log, rcnamespace=0,
           rcprop=title|timestamp|ids|sizes|sha1|user|flags|comment|loginfo
3 dedup    by rcid（重叠窗会重复投递）；同页多事件合并为一次重抓
4 apply    （§12.5 规则），events 追加写入 events.jsonl
5 advance  checkpoint = 本轮观察到的最大 (timestamp, rcid)，
           仅在语料+events 全部落盘成功后推进（崩溃即重放，幂等兜底）
6 fallback 若 checkpoint 距今 > 60 天（保守值 < $wgRCMaxAge 默认 90 天）：
           放弃 RC 通道，自动回落全量枚举对账并在报告标注原因
```

### 12.4 事件模型（`wikis/<host>/events.jsonl`，追加式）

```json
{"rcid":259702,"ts":"...","pageid":73480,"title":"群系连接",
 "actor":"1007369644","actor_class":"human","action":"edit",
 "revid":201372,"old_revid":201361,"sha1":"a42f…","oldlen":0,"newlen":0,
 "comment":"/* 森林世界（主陆地） */","bot_flag":false,
 "in_bot_window":false,"applied":"refetched"}
```

`actor_class` 判定（配置外置 TOML）：`HUIJI__USERNAME` 匹配 → `self`；用户名白名单（初版含 `Mr鲁鲁`）→ `bot_lulu`；rc flags 含 `B` → `bot_flagged`；其余 `human`。

### 12.5 事件应用规则

| 事件 | 本地动作 |
|------|---------|
| edit/create 且 sha1 ≠ 本地 | 批量重抓该页（≤50/批复用 get_pages_wikitext），更新 meta + 重分类 |
| edit/create 且 sha1 == 本地 rev_sha1 | 仅刷新 touched——零内容请求 |
| logtype=move | 用 logparams 新标题更新 meta.title（pageid 文件名不动），重抓确认 |
| logtype=delete | 页面文件归档 `_removed/<tag>/`，meta 移除 |
| logtype=restore | 按 edit 重抓 |
| 其他 logtype | 仅记 events，不动语料 |

窗口纪律延续：`in_bot_window=true` 时照常更新语料与 events，但下游 Atlas 不得据其做关联重建归因（标记随行传递）。

### 12.6 CLI 形态

`corpus-fetch --rc`：在现有命令上加增量策略开关（默认仍为枚举对账）。语义矩阵：

| 调用 | 行为 |
|------|------|
| `corpus-fetch` | 枚举对账（现状，兜底/校准） |
| `corpus-fetch --rc` | RC 增量拉取；checkpoint 缺失或超保留期时自动回落枚举模式并提示 |
| `corpus-fetch --full` | 全量重抓 |
| 任一 + `--dry-run` | 只产出报告与（RC 模式的）事件预览，不落盘不推进 checkpoint |

运行节奏建议：RC 模式可挂 cron 每小时；每 7 天跑一次枚举对账校准漂移。

### 12.7 验收标准

1. 事件捕获：真实编辑后运行 `--rc`，语料更新、events.jsonl 记录正确；
2. 幂等：checkpoint 不推进的重复运行零副作用、events 无重复行；
3. 崩溃安全：任意时刻 kill 后重启，重叠窗口自愈，无丢事件；
4. 归因抽样：Mr鲁鲁 的编辑正确标为 bot_lulu；self 编辑正确识别；
5. 回退：把 checkpoint 人为拨老于 60 天，自动回落枚举模式且报告说明。

### 12.8 开放问题

1. `$wgRCMaxAge` 本站实际值未知（按保守 60 天自限）；若未来发现 RC 表更短，缩短 checkpoint 容忍度即可；
2. categorize 事件是否需要跟踪（分类成员变化对 segmenter 有价值）？暂不，预留 rctype 参数化；
3. 是否扩展监听 `Data:`/`模块:` 命名空间（Tier0 他方同步核对）？通道天然支持 rcnamespace 参数化，默认仍仅主空间。
