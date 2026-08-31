# 行为链符号知识文档方案(behaviours / brains / stategraphs)

> 状态:v1.2(2026-08-28,方案 B 已拍板;**1a 已落地 29/29,1b pilot 已落地 3/3,待人工 review**)。
> 实施进度:代码基础设施 + `--pick-names` + pick_brains 目录扫描已完成;1a(behaviours 29 份词典,pass2 抽 wander/chaseandattack)与 1b(houndbrain/spiderbrain/beefalobrain)已跑完;LLM 输出形状容错(param_name 别名、default 原生类型、api 裸字符串)已加入。
> 定位:本文是 [KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md) 的类别扩展细化——把 SymbolDoc 从 component 扩展到行为链三类符号。基础管线(schema envelope、pass1/pass2 双通道、store 原子落盘、sha 增量、可观测性纪律)以该文档为准,本文只写**类别差异**。
> 决策记录(2026-08-28 拍板):
> 1. **方案 B**:behaviours 全量(词典层)+ brains pilot 联合先行;stategraph 延后;
> 2. brains pilot 样本 = houndbrain / spiderbrain / beefalobrain(对应页面均有"行为"章);
> 3. behaviours 跳过 pass2,只抽 2 个验证负结果路径;
> 4. brain pass1 注入所引用 behaviour 的参数语义;
> 5. SG 采用 per-file 截断版起步,排除 SGwilson* / *_client / commonstates;
> 6. "ChaseAndAttack 100"语义疑点单独立案(§1.3),核实结论回填本文与 UPDATE_IMPACT_PLAN。

**关联文档**:[KNOWLEDGE_PIPELINE.md](KNOWLEDGE_PIPELINE.md)(基础管线)、[UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md)(F3 行为事实提取、关联链模型)、[WIKI_CORPUS_PLAN.md](WIKI_CORPUS_PLAN.md)(语料底座)

---

## 1. 事实基础

### 1.1 代码侧规模与体量(当前版本 `data/databundles/scripts`)

| 目录 | 文件数 | 组成 | atlas 反向边 | 源码尺寸 | 页面锚点密度(实体页"行为"章) |
|---|---|---|---|---|---|
| `brains/` | 191 | 187 个 Brain 子类 + 4 个共享 helper(braincommon / beecommon / wobycommon / wx78_shadowdrone_braincommon,源码无 `Class(Brain`) | 86 个文件被 prefab 经 SetBrain 引用(头部:houndbrain 7 变体、deerbrain / deergemmedbrain / rabbitkingbrain 各 3) | 最大 cozy_bunnymanbrain 34.9KB,**全部低于 48KB 截断线** | **高**:行为章数值句的主要锚点(SEE_DIST=30、HOUSE_MAX_DIST=40、Follow 2~6、SIT_BOY_DIST=10 等) |
| `behaviours/` | 29 | 全部为 BT 叶子节点(Wander / ChaseAndAttack / RunAway…) | 经 brains 二跳引用 | ≤9.5KB | **几乎为零**:节点名从不上页面,只有实例化后的效果上页面 |
| `stategraphs/` | 260 | 实体 SG + 玩家 SG(SGwilson*、*_client)+ 共享库(commonstates 95KB) | 206 个被引用(头部:SGwilson 14 变体、SGboat 9、SGhound / SGspider 各 8) | **14 个 >48KB**:SGwilson 994KB、SGwilson_client 284KB、SGhermitcrab 183KB、SGwx78_possessedbody 154KB… | **中低**:个别点(per-state 组件配置如 sanityaura、特殊状态描述),大量为动画/音效/网络同步,页面永不写 |

### 1.2 管线侧已有资产

- `top_symbols` 已枚举三类符号:File 类含 stategraphs/brains 路径,另有 `SymbolRef::Behaviour` 构造子符号;**仅** `pick_components` 硬编码 `components/` 前缀、pass1 prompt 文案硬编码"组件"——这是类别扩展的两个改动点;
- atlas `index.json` 反向边对 brains/stategraphs 与 components **同构**(→ `prefabs/x.lua#variant` 列表),pass2 路由链可直接复用;
- atlas `behaviour_calls` 1074 条(brain_file / ctor / line / args / prefab_variants)——behaviour 的"二跳路由"(behaviour←brain←prefab)已免费存在;去重 25 个 ctor,头部:DoAction 223、Wander 186、FaceEntity 109、ChaseAndAttack 94、RunAway 80、Leash 78、Follow 73、ChattyNode 69、StandStill 64、Panic 35;
- pass2 的 search_terms 全文检索采样、dst/mixed 版本过滤、章节优先级("行为"=高优先)均**类别无关**,原样可用;
- update 侧 F3"行为常量子集"已落地:brain 文件级常量 ↔ 行为章配对已验证(猎犬 SEE_DIST 类)。SymbolDoc 与 F3 共用"**brain 文件级常量 = 页面数值事实**"的认知,可互相校准;update 侧该类事实的 `derivation_depth=2`(关联文件常量),认知对齐。

### 1.3 语义歧义案例:ChaseAndAttack 的"100"(立案,词典证据已就位,待正式核实)

- 页面(猎犬页 pageid 13857 行为章,原文三条):
  - "完全野生的猎犬仇恨范围为 100 单位。"
  - "有[[猎犬丘]]的猎犬的仇恨范围为 20 单位。"
  - "海象营地的寒冰猎犬的仇恨范围为 10 单位。超过 20 单位时则放弃追击。"
- 代码:`brains/houndbrain.lua:224` `ChaseAndAttack(self.inst, 100)`;而 ChaseAndAttack 构造子为 `(inst, max_chase_time, give_up_dist, max_attacks, …)`(`behaviours/chaseandattack.lua:1`),即 **100 是追击时长上限(秒)**,give_up_dist 为 nil;
- 索敌距离实际来自 `prefabs/hound.lua:198` retargetfn 的 `FindEntity(inst, TUNING.HOUND_TARGET_DIST=20 / HOUND_FOLLOWER_TARGET_DIST=10)`(tuning.lua:1481/1488);
- **1a 词典交叉印证(2026-08-28)**:houndbrain 文档 `behaviour_invocations` 按语境分支记录——野生无家 `ChaseAndAttack(inst, 100)` → "追击最长 100 秒";有家 `(inst, 10, 20)` → "追击最长 10 秒 + 放弃距离 20"。三条页面句与代码的映射:①"野生 100 单位" ↔ max_chase_time=100 秒(**疑似把时间误读为距离**);②"有丘 20 单位" ↔ give_up_dist=20(确是距离,但来源更可能是 give_up_dist 而非索敌距离);③"海象营地 10 单位" ↔ HOUND_FOLLOWER_TARGET_DIST=10(确是距离)。
- **历史核实结论(2026-08-30,定案)**:对全部 40 个快照(2025-03-12 → 2026-05-29)逐版本比对,`SEE_DIST=30 / SIT_BOY_DIST=10 / SHARE_TARGET_DIST=30 / HOUND_TARGET_DIST=20 / HOUND_FOLLOWER_TARGET_DIST=10` 与三处 ChaseAndAttack 调用(`10`、`10,20`、`100`)**横跨 14 个月零变化**。页面的"100 单位"不可能来自任何历史版本的代码——**定案为时间误读为距离**:野生猎犬的 100 是 `max_chase_time`(追击时长上限,秒),页面写成"仇恨范围 100 单位"系误读;其余两条(20/10)数字恰好是真实距离(索敌/放弃距离),无需修正数值但表述口径可校准。处置:①猎犬页行为章"野生 100 单位"句应改为追击时长表述——**修订建议稿已出(2026-08-31,人工已核,Tier2 格式)**:旧句"`*完全野生的猎犬仇恨范围为 100 单位。`" → 新句"`*完全野生的猎犬的索敌范围为 20 单位,追击同一目标最多持续 100 秒,超时后放弃追击。`"(索敌距离依据 `prefabs/hound.lua` retargetfn 的 `TUNING.HOUND_TARGET_DIST=20`;100 依据 `houndbrain.lua:224` `ChaseAndAttack(self.inst, 100)` 的 max_chase_time;wiki 实际编辑待执行);②UPDATE_IMPACT_PLAN §4.2.4 语境分句范例需修正;③F3 旧值回查锚点排查(该句不再适合作为距离类锚点)。
- **教训**:behaviour 词典文档(`ctor_params`)正是为此而设——本例的语义锚点即来自 1a 词典。

## 2. 四类符号的性质差异(为什么不能一套模板套四次)

| | component(已做) | behaviour | brain | stategraph(延后) |
|---|---|---|---|---|
| 本质 | **能力提供者**(api 语义) | **参数化行为原语**(词典/词法) | **每实体组合层**(BT 结构 + 常量 + 语境分支) | **执行/表现层**(状态机 + 时间线) |
| 主角字段 | api 列表 | ctor 参数语义、成功/失败条件 | BT 优先级结构、文件级常量、PushEvent | states、事件处理、per-state 组件配置 |
| 更新频率(维护成本) | 中 | **极低**(29 个原语多年稳定) | 高(数值常量常调) | 低~中 |
| pass2 预期 | 有事实 | 大面积 no_wiki_mention(空即结论) | 有事实(行为章) | 弱事实 |
| 依赖方向 | 被 prefab 消费 | 被 brain 消费 | 消费 behaviour、与 SG 事件互通 | 消费 brain 推来的事件 |

依赖链:`behaviour(叶) ← brain(组合) ← SG(执行)`。**brain 文档的语义单元就是 behaviour 调用**——不先有词典,brain 文档生成时只能逐个现猜参数含义(§1.3 即实物风险)。

## 3. 批次定义(方案 B)

| 批次 | 内容 | pass2 | 成本估算 |
|---|---|---|---|
| **1a 词典层** | behaviours 全量 29 份(`behaviour__wander` 等) | **跳过**;仅抽 Wander / ChaseAndAttack(调用频次 top 且参数最多)2 个验证负结果路径 | pass1 29 份 ≈ 25~40 分钟(串行,按 pilot 52~81s/份) |
| **1b 组合层** | brains pilot 3 份:houndbrain(猎犬页 13857 基准 + update 侧关联链 fixture)、spiderbrain(蜘蛛页 15246)、beefalobrain(皮弗娄牛页 15238) | 正常跑(路由链 + search_terms;"行为"章高优先) | pass1 3 份 + pass2 3 份 ≈ 5 分钟 |

- 顺序:**1a 先于 1b**——1b 的 prompt 注入依赖 1a 产物;同批串行时天然满足;
- 1b 的人工 review 基准:三页"行为"章逐句对照 brain 文档(猎犬页可直接复用 §1.3 的核查结论);
- SG 不在本期(§7)。

## 4. Schema 与提示词契约(草案)

### 4.1 envelope 不变

`reference / category / display_name / summary / search_terms / wiki / related_symbols / truncation_note / provenance` 全类共用;`schema_version` 不升,类别专属字段全部为**可选字段**,component 文档零影响;`prompt_rev` **按类别独立**(component 线继续 p3,behaviour / brain 各自 p4 起,互不牵制)。

- doc_id 沿用 `{category}__{name}`:`behaviour__wander`、`brain__houndbrain`;
- `events_published / events_listened / netvars / tunables / gameplay_tags` 字段名保留,按类别在 prompt 中重定义(见下)。

### 4.2 behaviour 载荷(category=behaviour)

| 字段 | 内容 | 约束 |
|---|---|---|
| `ctor_params` | `[{name, semantic, default?}]` 参数名→玩家语义+默认值 | 覆盖 inst 之外的全部位置参数;若确实没有额外参数,可空但必须 `api_note` 说明(与 component 空 api 规则同构) |
| `effects` | 对组件/SG 状态标签的作用(locomotor / combat 调用、sg 状态标签检查) | 严格取自源码 |
| `success_fail_conditions` | BT 语义:何时 SUCCESS / FAILED / RUNNING | 严格取自源码 |
| `api` | 一般为空(原语少有对外方法) | 空时必填 `api_note` |
| `tunables` | 源码内默认值(如 Wander 的 `wander_dist=12`、`times` 四项) | 严格取自源码 |

注意:参数头注释覆盖不均(仅 wander.lua 带 `-- Parameters:` 块,多数文件没有)——ctor_params 语义主要靠读实现建立,这正是词典需要专门产出的原因。

### 4.3 brain 载荷(category=brain)

| 字段 | 内容 | 约束 |
|---|---|---|
| `tunables` | **文件级 local 常量**(SEE_DIST、HOUSE_MAX_DIST 类)= 页面数值事实锚点 | 名称+字面量严格取自源码;与 F3 常量表可交叉校验 |
| `behaviour_invocations` | `[{ctor, args_semantic, context?}]` 本 brain 实际调用的 behaviour 及**实例化语义**(数值事实在此承载,如 ChaseAndAttack(inst,100) → "追击至多 100 秒") | LLM 生成;管线用 atlas behaviour_calls 预填清单做**报告级交叉核对**(结构归管线、语义归 LLM;硬失败阈值 pilot 后定) |
| `context_branches` | `[{condition, semantic}]` HasTag("clay") / lunar_aligned 等条件分支 | 对应页面"分句枚举"写法(UPDATE_IMPACT_PLAN §4.2.4 语境维度) |
| `bt_structure` | 优先级节点树的文字化摘要(意图粒度,不追求完整还原) | pilot 后按输出质量定粒度(见 §10) |
| `events_published` | brain 侧 PushEvent(reanimate / becomestatue / chomp 类,SG 侧监听) | 沿用 envelope 字段 |

prompt 注入:brain pass1 附带**本 brain 实际调用的** behaviour 构造子签名 + ctor_params 摘要(从 1a 文档读取,控制上下文预算;不注入全文)。

### 4.4 硬校验(沿用 KNOWLEDGE_PIPELINE §3 纪律)

`summary` 非空;空 api 必填 `api_note`;behaviour 的 ctor_params 空时必填 `api_note`;失败 → raw 已归档、报错含定位。

## 5. pass2(link-wiki)类别差异

| 类别 | 路由 | 预期 |
|---|---|---|
| brain | 复用组件路由链(brain → 反向边 → 变体 → pages_by_prefab)+ search_terms 全文检索;"行为"章高优先不变 | 有事实;证据约束、负结果声明、coverage 分档全部沿用 |
| behaviour | **默认跳过**;抽 Wander / ChaseAndAttack 2 个验证负结果路径 | 预期 no_wiki_mention——**空即结论**在叶子原语上成立与否,本身就是本次要验证的观察点 |
| behaviours 的价值主战场 | pass1 词典(被 brain prompt 引用)+ M3 sync 的 brain diff 解释 | — |

## 6. 选择与排序

- **behaviours**:29 份全量,无需排序(调用频次仅用于 pass2 抽样挑选);
- **brains**:候选 = atlas 反向边覆盖的 86 个;**排除 4 个 helper**(识别规则:源码无 `Class(Brain`);排序沿用变体引用数降序;pilot 手选 houndbrain / spiderbrain / beefalobrain;
- 未被 prefab 引用的 ~101 个 brain(事件驱动/内部生成实体等)暂不建文档,与 component 的 top_symbols 逻辑一致;
- **stategraphs**:延后,首期若做 = 引用变体数降序 + §7 排除规则。

## 7. stategraph 延后决策(已定)

三个未决问题中现在定两个:

1. **粒度**:per-file 起步(超 48KB 截断并在 `truncation_note` 声明);per-State 粒度**不做**(文档数爆炸、路由复杂化);
2. **边界**:排除 SGwilson / SGwilson_client 及一切 `*_client`、commonstates.lua(共享库非实体 SG);玩家/角色侧对应物属系统机制页,与 KNOWLEDGE_PIPELINE"系统机制页挂起"一致;

遗留到实施时定:SG 的 doc_id 命名(SGhound → `stategraph__SGhound` 还是去前缀)。**重开条件**:brains pilot 完成、schema 稳定后评估。

## 8. 与 component 线(另一工作线)的协同边界

- 不动 `pick_components` 与组件 prompt 现有路径:新增按 path 前缀的 category 分派(`brains/`→brain,`behaviours/`→behaviour,`stategraphs/`→stategraph[延后]),**component 行为零变化**,合并冲突面最小;
- store 命名 `{category}__{name}.json` 天然支持新类别;
- M1 收尾(50 份 component 全量 `--force` 刷新 + 人工 review)为另一条工作线,本方案不触碰其产物与配置。

## 9. 实施清单

1. ✅ `scan_symbols.rs`:类别分派(pick 泛化)+ brain / behaviour prompt 模板 + brain 注入 behaviour 参数语义;
2. ✅ `types.rs`:类别载荷可选字段(behaviour:ctor_params / effects / success_fail_conditions;brain:behaviour_invocations / context_branches;tunables 重定义说明);
3. ✅ 校验:brain 的 behaviour_invocations 与 atlas behaviour_calls 报告级交叉核对;behaviour 空参数规则;
4. ✅ 跑批次 1a(29/29 入库)→ 1b(3/3 入库);人工 review 待做(对照猎犬/蜘蛛/皮弗娄牛"行为"章);
5. ⬜ 回填 KNOWLEDGE_PIPELINE §8 进度与本文状态;
6. ⬜ (独立工作项)§1.3 "100 单位"疑点核实,结论回填本文与 UPDATE_IMPACT_PLAN(若属实)。

### 9.1 pilot 实测记录(2026-08-28)

**批次与产物**

- 1a:`--category behaviour --limit 29 --pass2-names wander,chaseandattack`,29/29 入库;`--concurrency 2` 稳定。
- 1b:`--category brain --limit 3 --pick-names houndbrain,spiderbrain,beefalobrain --corpus …`,3/3 入库。

**质量观察**

- **词典层可用性达标**:`max_chase_time` 被标注为"最长追逐时间(秒)",Wander/RunAway/Leash 等参数语义合理;§1.3 疑点获得词典锚点——houndbrain 文档 `behaviour_invocations` 已按语境分支区分 `ChaseAndAttack(inst, 100)`(非 clay 非宠物无家)→"追击最长 100 秒"、`(inst, 10, 20)`(有家)→"追击最长 10 秒 + 放弃距离 20"。
- **pass2 对 behaviour 的价值高于预期**:chaseandattack = well_documented(10 aspects,含 pageid 15386"超过最大追逐时间"表述)、wander = partial(2)。"节点名不上页面"成立,但节点*效果*有大量页面表达——1a 抽样只跑 2 个的决策仍有效,但后续全量 behaviour pass2 值得重估。
- **brain pass2 偏保守**:houndbrain 采样 8 页(含猎犬相关页)返回 no_wiki_mention,而猎犬页"行为"章确有数值事实;疑因 caps 清单只有 tunables 名与裸 ctor 名(如 `ChaseAndAttack`),页面语言无法对上。候选改进:brain pass2 的语义清单改用 behaviour_invocations 的 args_semantic 文本(需先有 1a 词典,现已具备)。
- **spiderbrain/beefalobrain 路由弱**:spiderbrain 路由 0 页(负结果),beefalobrain 仅 1 页——这两个 brain 的 SetBrain 不走 atlas 可追踪路径,pass2 依赖 search_terms 全文检索兜底,符合设计但印证 §1.2"二跳路由免费"只对被 atlas 记录的 brain 成立。

### 9.2 第二批与 SG 推广(2026-08-29)

- **SG 批次 1**(20 份,引用数排序,含 SGhound):states/state_notes 全部达标(逐文件 5~37 个 states,notes ≤10 条);**pass2 教训两轮**——第一轮 caps 误用 api 分支(SG 的 api 基本为空 → 空清单,19/20 判空),改用 state_notes 玩家语义(p6-stategraph)后 **14/20 有 aspects**(well 3 / partial 11:SGwerepig 5、SGBeefalo 4、SGanchor 3)。三类符号的 pass2 caps 差异已全部对齐为"玩家语义文本"模式。
- **pass2 策略对比(定案)**:behaviour 高价值(18/29)默认开;stategraph 修复 caps 后中等价值(14/20)默认开;brain 中等(部分判空系页面行为章缺口)默认开。`--pass2-names` 统一降级为省预算手段。
- **brains 第二批**:9 份(累计 12),零失败;第三批见 git log。

- **brains 第二批**(`--limit 10`,引用数排序):deerbrain / deergemmedbrain / rabbitkingbrain / beargerbrain / buzzardbrain / carratbrain / fruitflybrain / pollyrogerbrain / toadstoolbrain 共 9 份入库,零失败;现累计 12 份 brain 文档。fruitflybrain well_documented(4),carrat/deergemmed/pollyroger partial(1~2),其余判空。判空偏多的主因仍是页面行为章与 brain 语义清单的对齐缺口,改善方向同 §9.1(brain pass2 语义清单已改用 args_semantic,后续可再叠加 tunables 数值语义)。
- **SGhound 冒烟**(`--category stategraph --pick-names SGhound`):25 个 states 全部取自源码(含 timeline 生成态),state_notes 10 条精确到帧级语义(attack 第 16 帧伤害判定 / statue 无敌 / startle 0.8~1.1s),api 空数组 + api_note 正确;pass2 no_wiki_mention(8 页)符合"SG 弱事实"预判。**stategraph 类别管线验证通过,可按引用数排序分批推广**(建议排除玩家侧后从高页面价值 SG 开始,如 SGhound/SGspider/SGbeefalo 量级)。

**pass2 全量重估(2026-08-29)**:1a 抽样的正结果促使对 29 份 behaviour 全量补跑 pass2——**well_documented 10 / partial 8 / no_wiki_mention 11,18/29 有真实页面证据**,推翻"词典层无页面价值"的原判;movement 类(follow 6 / leash 6 / leashandavoid 7 条 aspects)最丰富。后续 behaviour 新增/重扫时 pass2 应默认开启,`--pass2-names` 仅作为省预算手段。

**stategraph 重开(2026-08-29)**:重开条件(brains pilot 完成)已满足,按 §7 决策落地代码:
- `--category stategraph`:pick_stategraphs 目录扫描,排除 SGwilson* / *_client / commonstates,要求源码含 `StateGraph(` 标记;反向边仅作排序信号;
- schema:`states`(全部状态名,严格取自源码)+ `state_notes`(≤10 个关键状态的玩家语义);validate:states 空 → 必须 api_note;prompt_rev p5-stategraph;
- **doc_id 命名定案(§10 开放问题 4)**:沿用文件词干(如 `stategraph__SGhound`),不做 SG 前缀剥离——path ↔ doc_id 保持 1:1,避免特例;
- 粒度定案:per-file + 截断声明(沿用管线 SOURCE_BYTES_CAP 头尾策略),不做 per-State 拆分;
- SGhound 冒烟结果见 §9.2。

**过程修复**(已入库)

- `CtorParam.name` 别名(param_name/param)、`default` 原生布尔/数字→文本、`ApiEntry` 裸字符串→{name, effect:""};prompt 逐字段写明 JSON 形状。
- `pick_brains` 从 top_symbols(反向边枚举)改为直接扫 `brains/` 目录:spider/beefalo 等动态 SetBrain 的 brain 在 atlas 中无 Brain 边,原逻辑选不到;反向边降级为排序信号。新增 `--pick-names`。

## 10. 开放问题

1. brain `bt_structure` 粒度(完整树 vs 意图摘要)——pilot 后按 LLM 输出质量定;
2. behaviour_invocations 的 args 解析分工:behaviour_calls 已能解析部分字面量/FnRef,Unknown 形态靠 LLM 读源码;两路交叉核对的不一致阈值待定;
3. brains/ 中 4 个 helper(braincommon 等)是被 brain require 的"子词典",性质接近 behaviour——是否单独建 doc,观察项;
4. SG doc_id 命名(SG 前缀去留)——SG 重开时定;
5. §1.3 疑点的核实方法(读历史快照脚本 / 实际游戏测试)——归入 UPDATE_IMPACT_PLAN 的 L1 人工比对类工作。
