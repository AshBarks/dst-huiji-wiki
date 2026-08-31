# 剩余工作清单与执行指引

> 建立时间:2026-08-31。本文档盘点跨计划(知识线 M1~M3 / 语料线 / update 侧)的全部未完成项,每项给出**说明**(是什么、为什么)与**指导**(命令序列、判定标准、回填位置)。
> 约定:接手者按需认领单项执行,完成后在本文对应项打勾并回填结果;完成且不再有后续的项移入 §D 已闭案。
> 命令中的路径占位:`$BIN` = `target/release/dst-huiji-wiki`,`$SCRIPTS` = 游戏脚本根目录(默认 `"$HOME/.steam/debian-installation/steamapps/common/Don't Starve Together/data/databundles/scripts"`),语料 = `wikis/dontstarve.huijiwiki.com`。

## 0. 通用纪律(所有项适用)

- **写 wiki 永远人工执行**:管线只产出建议清单/应用清单;任何自动写 wiki 的想法都违反项目确认策略。
- **先 dry-run**:有 `--dry-run` 的命令先跑一遍看报告,再真跑。
- **raw 归档**:LLM 原始响应在 `output/knowledge/raw/`(不入库),排障时先看它。
- **提交纪律**:`cargo fmt --check && cargo clippy -- -D warnings && cargo test` 全绿再提交;`knowledge/` 入库,`output/` 不入库。
- **历史核实法**(涉及"页面 vs 代码"争议时):对全部快照(`$DST_ROOT/data/databundles/scripts_*`,40 个,2025-03-12 → 2026-05-29)逐版本比对常量与调用,数值零变化才能定案"页面错误"——参照 KNOWLEDGE_BEHAVIOUR_CHAIN.md §1.3 的做法,勿凭当前版本下结论。

---

## A. 可直接执行(长尾运营,无外部依赖)

### A1. ✅ 未审计 stub 池 436 → 227(2026-08-31 完成)

- **说明**:PageSymbolMap 的 stub 对(确定性方法未见证据)中曾余 436 对未跑 L2 审计。**已执行 4 轮**(exit 全 0):累计转换 42 对为提及,语义不一致 +35;有提及页面 430 → 446,直连提及对 942 → 999,**未审计 stub 436 → 227**。收益递减明显,余量留待下批自然消化。
- **指导**:
  ```bash
  # 循环执行直到"未审计 stub"不再明显下降(每轮 ~15 分钟到 ~4 小时不等)
  $BIN knowledge-scan-wiki "$SCRIPTS" --corpus wikis/dontstarve.huijiwiki.com --audit
  # 轮与轮之间或收尾时出报表核对数字(root 位置参数在 --report 模式下不使用,占位即可)
  $BIN knowledge-scan-wiki "$SCRIPTS" --corpus wikis/dontstarve.huijiwiki.com --report
  ```
- **验收**:未审计 stub 降至 227(<100 未达,但两轮转化已 <8 对,按收益递减止步;剩余随下次更新周期消化)。
- **回填**:✅ PAGE_MAP §7 注记已更新数字。

### A2. ✅ 语义不一致 C 类 7 对人工核(2026-08-31 完成)

- **说明**:`knowledge/inconsistent_classification.json` 中 `C_page_error`(疑似页面错误)的 7 对是"数值在任何可达代码中都找不到"的候选。人工核实后才能决定:改页面(B 类误分/动态值)或立页面纠错项(真错误)。
- **指导**:打开 `knowledge/inconsistent_classification.json`,逐对执行:①看 wording/note/reason;②到 `$SCRIPTS` 搜数值来源(注意 `aurafn` 类动态计算值、loot 表权重与百分比单位换算——这类**优先判 B**,回流文档侧);③按 §0 历史核实法定案;④直接编辑 json 的 `class` 字段与 `note`(加"人工核实: …"前缀),提交。
- **当前 7 对速览**(2026-08-31):坎普斯×burnable、蘑菇地精×sanityaura、厨师袋×inventoryitem(6 格)、恐怖利爪×transparentonsanity(-100)、噩梦燃料×useabletargeteditem(6)、完全正常的树×sanityaura(-40)、绝望投泥带×spellcaster。
- **结果**:4 对 → A_routing(坎普斯掉落属 lootdropper / 厨师袋容量属 container / 恐怖利爪理智光环属 sanityaura / 初始物品属角色 prefab 层);2 对 → B_variant_gap(sanityaura aurafn 动态值);1 对 → D_manual 保留(绝望投泥带×spellcaster,审计 verdict 疑似误判——页面描述的施法效果即组件语义,分类学缺"误报"类)。**C 类清零**。
- **机制沉淀**:新增 `knowledge/inconsistent_manual.json` 人工覆写层(重跑 --classify 不覆盖人工结论);PAGE_MAP §8 已回填。

### A3. ✅ sanityaura 变体缺口回流(2026-08-31 完成)

- **说明**:M2 审计与分类器均确认 sanityaura 的文档变体口径有缺口(完全正常的树 -40 / 蘑菇地精 aurafn 动态值的页面证据未归因)。这是"文档侧缺口"(B 类)而非页面问题。
- **指导**(二选一):
  1. **手工补(推荐,快且可审)**:编辑 `knowledge/symbols/component__sanityaura.json` 的 `wiki.aspects`,为两个页面各补一条带引文的 aspect(引文从 `wikis/.../pages/19785.wikitext` / `15677.wikitext` 原文摘录,逐字);**不得改动 `provenance.source_sha256`**;提交信息注明"手工修正变体口径"。
  2. ~~管线重跑~~(未采用:pass2 采样不保证命中这两页)
  - 补后 `--classify` 确认对应 C 类经 A2 覆写清零。
- **结果**:采用方案 1 手工补——文档新增 aspect「变体理智光环数值标注」(蘑菇地精 -25/战斗 -40 + 完全正常的树 -40,引文逐字摘自页面),provenance 未动。

### A4. ✅ pass2 二次确认(2026-08-31 完成)

- **说明**:对「检索命中 ≥3 页但 aspects=0」的组件,pass2 可能过严判空(空即结论的前提是采样充分)。二次确认 prompt 可再给一次机会。
- **指导**:实现点在 `src/knowledge/scan_symbols.rs` 的 enrich_with_wiki 后:条件(采样页数 ≥3 且 aspects 空)→ 追加一次"请再检查以下摘录是否有被忽略的能力表达"的调用;仍空则保持 no_wiki_mention。预算:触发面约 300+ 文档中的少数,建议加 `--confirm-empty` 开关控制,默认关闭。
- **结果**:落地为 `--confirm-empty` 开关(默认关)。对照 3 组件:amphibiouscreature **翻案 2 条 aspects**(离开/进入海洋行为),另 2 组件复查后保持判空(宁空勿造未被破坏)。355+18 测试全过,提交 `9f5ba93`。

### A5. ✅ brain pass2 判空改善(2026-08-31 完成,结论:无效)

- **说明**:brain 文档判空偏多(页面"行为"章与语义清单对齐缺口)。§9.1 已改用 args_semantic 文本;下一步可叠加 tunables 数值语义(常量名+值+使用语境)进 caps。
- **结果**:caps 提为 `brain_caps()` 并叠加 tunables 数值语义(NAME=值);5 脑对照(spider/beefalo/spiderqueen/bearger/bee)重跑后 **0/5 改善,仍全判空**。结论:判空主因是页面行为章确实不讨论 brain 内部调参(§9.1 判断成立),数值语义无济于事。**按既定标准不启动 187 全量重跑**;caps 改动保留(信息量无害),提交 `9f5ba93`。

### A6. ✅ 分类器 LLM 层方差治理(2026-08-31 完成)

- **结果**:实测三票多数仍不收敛(A 57↔45,B/C 亦摇摆,超 ≤2 验收线)→ 采纳方案②:**LLM 层降级为 `llm_suggest` 建议字段**(不改 class),class 只由确定性层+人工覆写(`inconsistent_manual.json`)决定。最终 2 连跑逐字一致(A9/B4/C2/D136),**零方差达成**,提交 `9f5ba93`+后续。
- **残余取舍**:D_manual 扩大到 136 对(失去 D→A/B 自动转化),人工按 llm_suggest 提示分诊;若嫌人工量大,后续可提高确定性层覆盖率(方案③)收敛 D 面。

---

## B. 外部依赖触发(等事件发生)

### B1. Tier2 真实版本端到端验证(等下次游戏更新)

- **说明**:M3 v1.2 + 复核闭环只在 SEE_DIST 30→33 模拟上验证过。真实版本变更(新快照落地)后应走一次全流程。
- **指导**(更新日执行):
  ```bash
  # 假设新快照为 <NEW>,上一版为 <OLD>
  $BIN knowledge-sync <OLD> <NEW> --corpus wikis/dontstarve.huijiwiki.com --draft --limit 20
  # → 检查 output/knowledge/sync_report.json 与 output/knowledge/draft_review.json
  # → 人工填写 decision(approve/reject)+ note(判定标准:数值语义确实来自变更常量)
  $BIN knowledge-sync --review output/knowledge/draft_review.json
  # → output/knowledge/sync_apply_list.md 人工执行 wiki 编辑(若需要)
  # 同时:重扫脏文档使 SymbolDoc 与新树对齐
  $BIN knowledge-sync <OLD> <NEW> --corpus ... --rescan --limit 20
  # 最后:刷新页面图并出分类
  $BIN knowledge-scan-wiki "$SCRIPTS" --corpus ... --audit
  $BIN knowledge-scan-wiki "$SCRIPTS" --corpus ... --report
  $BIN knowledge-scan-wiki "$SCRIPTS" --corpus ... --classify
  ```
- **验收**:KNOWLEDGE_SYNC.md §3.2 增补"真实版本实测"小节;复核驳回理由归类回填 §4.2.4 取舍配置。

### B2. RC 通道真实编辑捕获补验(WIKI_CORPUS_PLAN §12.7-1)

- **说明**:`corpus-fetch --rc` 的播种/增量/空窗幂等/dry-run 已实测;唯一未验的是"真实编辑后事件捕获"——等 wiki 自然编辑发生。
- **指导**:wiki 有编辑活动的当天跑 `$BIN corpus-fetch --rc`,检查:①`wikis/dontstarve.huijiwiki.com/events.jsonl` 新增行字段完整(rcid/ts/pageid/actor/actor_class/sha1/applied);②被编辑页 wikitext 与 meta.rev_sha1 已更新;③立即重跑一次,应零事件零重复行(幂等);④如出现 move/delete 事件,核对 logparams.target_title 字段名假设是否成立(不符则改 `rc.rs::move_target` 一处)。
- **验收**:结果回填 WIKI_CORPUS_PLAN.md §12 实施记录;§12.7 验收清单逐项打勾。

---

## C. 按需立项(功能扩展,先讨论再动手)

### C1. update 侧 region tier 四档落地(UPDATE_IMPACT_PLAN §6.3-2)

- **说明**:draft/flag/ignore/Tier0-only 的介入边界目前停留在设计表(§4.2.4 章节表),未写入 region/证据包并接入 grade。
- **指导**:入口 = `src/update/grade.rs` + CORPUS_CODE_ATLAS_CONTRACT.md §2 区域工件;把 §4.2.4 章节表编码为 per-region 的 tier 字段(语料侧 segmenter 产出或消费端打标),grade 时按 tier 调整 draft/flag 行为;用猎犬/蜘蛛页做对照样张。
- **前置**:无硬前置;建议在 B1 之后做(真实版本数据更有说服力)。

### C2. Code→Page 完整方向(UPDATE_IMPACT_PLAN §6.3-3)

- **说明**:M3 sync 只覆盖常量驱动的句子改写;完整方向包括"代码新增实体 → 建页建议(create-check 类)"与"关联链变更 → 跨页提示"。
- **指导**:复用 knowledge 线资产:新增符号(added_symbols)× prefab 路由 → 页面缺失检验(§4.2.4 制作章检验点同型);跨实体机制用链接不复制(§4.2.4 规律 4)。产出仍是建议清单。

### C3. M4:fn 级标注(UPDATE_IMPACT_PLAN v3.1 ①)

- **说明**:CodeTextAtlas 目前 file 级;fn 级说明文本作为 LLM 提示词资产排期 M4。
- **指导**:在 atlas 构建器上扩展 per-fn 提取(lua AST 已有 full_moon 基建);规模控制:先做 components/ 的 public fn,质量抽验后推广。

### C4. 系统机制专题页路由扩展(知识线 M2 遗留)

- **说明**:当前页面图只覆盖实体/prefab 页;蜘蛛/冬季等专题页是否纳入路由未评估。
- **指导**:先量化——统计专题页对 SymbolDoc search_terms 的命中率与预期归因价值;若 ≥20% 页面有 ≥1 强提及对,再扩 `build_variant_routes` 的路由面(需要专题页 → 符号的静态映射,语料侧可能要加分类索引)。
- **判定标准**:路由扩展的成本(索引改造)与增益(可归因页数)成比例才立项。

### C5. brains helper 子词典(braincommon 等 4 个)

- **说明**:被 brain require 的"子词典"(braincommon 等)性质接近 behaviour,目前无 doc,brain 引用其常量时语义靠 LLM 现猜。
- **指导**:判断标准——若 187 份 brain 文档中涉及 helper 常量的判空/误判集中出现(用 --classify 与审计 verdict 抽查),则建 doc(类别沿用 behaviour 模式,pick 扩展);否则保持观察。

### C6. Removed 符号处置自动化(低优先)

- **说明**:符号文件被删除时 sync 只在报告单列 removed_symbols,文档标记"待下线"靠人工。
- **指导**:在 `knowledge-sync` 中对 removed 符号生成"下线检查单"(提及页面列表 + aspects 清单),进报告的新字段;不自动删文档。

---

## D. 已闭案(勿重开)

| 事项 | 结论 | 依据 |
|---|---|---|
| "100 单位"疑点(§1.3) | 时间误读为距离;40 快照定案 | `56a8139` / `ad14c4a` / `eadb623` |
| 猎犬页 wiki 实际编辑 | **拍板不做**,建议稿存档备查 | `1a5b7bc` |
| UPDATE_IMPACT_PLAN §4.2.4 语境分句范例 | 已按定案修正 | `eadb623` |
| SG 覆盖范围 | 250/250;commonstates/SGwilson*/_client 拍板排除 | `d881186` |
| M1 p4 全量重扫 / search_terms 杠杆 | 已完成 | `1a5b7bc` 回填 |
| bt_structure 粒度 / SG doc_id 命名 | 实践定形(意图摘要 / 保留 SG 前缀) | 行为链 §10 |
| 语料线 touched 枚举对账 | 已实现,作为 RC 的兜底与校准 | WIKI_CORPUS_PLAN §12.1 |
| 开放问题 1/3/6(UPDATE_IMPACT_PLAN §5) | 已关闭/暂缓 | v3.1 结案标记 |
