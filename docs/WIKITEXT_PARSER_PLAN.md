# Wikitext 解析器方案（定稿）

**状态**：需求定稿，M1 实施中
**日期**：2026-09-12
**讨论结论**：自研、读侧优先、表格按不透明块、节点持有字符串

## 1. 背景与定位

本项目各维护作业对 wikitext 的处理目前散落四处，全是 `find`/`contains`/regex 级别：

| 位置 | 做法 | 局限 |
|------|------|------|
| `src/corpus/segment.rs` | 手写花括号配平，提取信息框 span 和参数 | 只认识信息框一种模板 |
| `src/corpus/classify.rs` | `text.contains(marker)` 打信号 | 无法区分参数内外、注释内 |
| `src/corpus/facts.rs` | regex 抽取 | 同上 |
| `src/service/template_check.rs` | 按 `\|filter` 切分支、抠 `{{inv\|X}}` | 对模板结构调整脆弱 |

写侧现状：全部是"整页重写"（strings_wiki、skilltree_wiki）或"标记替换"（CopyClip，
且只针对 Lua 模块页）。**没有"只改页面里某个模板参数"的外科手术式编辑能力**，
这正是本模块要补的空白。

真实语料（`wikis/`，6909 页，其中 585 页含表格）确认了复杂度：

```
{{实体信息框/自动|dst
|图像=[[File:Shell Cluster Dropped.png|188px]]
|掉落={{Pic|32|低音贝壳钟}}×1，... <br>...
=={{标题|花絮}}==
[[Category:旧神归来]]
```

链接里嵌模板、标题里嵌模板、模板名/参数键全中文——正则方案在这三点上已不可行。

## 2. 设计原则

1. **无损往返是第一需求**：`parse(text).serialize() == text` 对任意输入成立，
   修改只体现在被编辑的节点上。工具底线是"只动想动的东西"，页面 diff 必须可审；
   任何"规范化重排"都是禁止项。
2. **解析永不失败**：`parse()` 无 Result，配不平的花括号、未闭合标签一律降级为
   文本节点。
3. **节点持有字符串**：节点保存原始文本，serialize 即拼接；不用 span-into-original
   模型（页面级文本规模下无性能收益，多次编辑合成复杂）。
4. **边界：模块页（Lua）不归它管**，继续走 full_moon + CopyClip 标记。

## 3. 需求分层

### 解析层（读）

- **必须有**：文本、模板（含解析器函数/魔字）、模板参数 `{{{x|默认}}}`、
  内部链接 `[[目标|标签]]`（含 `File:`/`Category:`）、外部链接、标题（1–6 级）、
  HTML 注释、`<nowiki>`/扩展标签、重定向。
- **应该有**：普通 HTML 标签（带属性）、表格 `{|...|}`。
- **明确不做**：粗斜体引号、列表/缩进、签名、`__NOTOC__` 等行为开关——一律留作
  文本。没有"改格式"的需求，结构化它们纯属增加出错面。

### 修改层（写，M3）

- 模板句柄：按归一化名字查找；参数读取/新增/更新/删除。
- **格式保持**：更新 `| 掉落 = xxx` 的值时保留 ` | 掉落 = ` 装饰原样；新增参数
  从兄弟参数推断插入位置与缩进风格。
- 章节操作：按标题取节/替换/追加；注意 `=={{标题|花絮}}==` 这类模板化标题，
  提供"解析 `{{标题|X}}` → X"的辅助。

## 4. 语法处理规则（已定）

1. **嵌套优先级**：注释 → nowiki/扩展标签 → `{{{` → `{{` → `[[`。
   `{{{` 与 `{{` 歧义按"最长匹配、失败回退"；配不平降级为文本。
2. **行首语义**：标题 `=`、表格 `{|` 只在行首生效。行首 run 须为 1–6 个 `=`，
   且行尾须有闭合 run（允许其后跟空白）；否则按文本。
3. **扩展标签名单**：内容不解析的 raw 标签（nowiki/pre/gallery/ref/poem/
   tabber/syntaxhighlight/source/math 等）用常量名单；名单外按普通 HTML 标签
   处理（内容继续解析）。未知/未闭合标签降级为文本。
4. **名字归一化**（M2 用）：模板/链接目标名 `_`≡空格、拉丁首字母大小写归一。
5. **表格按不透明块**：只识别边界（行首 `{|` 到配平的 `|}`，边界扫描须感知
   嵌套模板/链接/注释/标签/嵌套表格），内部整段原样保存。
   这是"推迟"不是"死路"：节点保存原始文本，将来可加 `parse_inner()` 下钻，
   M1 设计不变。

## 5. 已知偏差与限制（有意接受）

- 配不平的括号（如 `{{{a}}`）MediaWiki 渲染为字面文本，本解析器可能产出
  Template 节点——往返无损不受影响，消费者需容忍。
- 链接在第一个 `]]` 闭合（与 MediaWiki 后展开语义一致），嵌套 `[[` 不下钻。
- 表格/模板互相嵌套时（表格作为模板参数），非配对场景可能有边角偏差。
- 同名 HTML 标签嵌套（`<div><div></div></div>`）按深度计数配对；raw 标签
  第一个闭合即止。
- 扫描为线性+构造跳转，病态输入（大量未闭合括号）最坏 O(n²)；语料实测无此
  形态，递归深度上限兜底。

## 6. 测试策略

1. **语料全量往返测试**：`wikis/` 存在时启用——6909 页
   `parse().serialize() == 原文` + 二次解析树相等（幂等）。
2. **单元 golden 测试**：覆盖第 4 节每条规则的边界形态。
3. **迁移安全网（M2）**：`segment.rs` 信息框参数抽取新旧结果一致性对比，
   稳定后再删旧实现。

## 7. 分期

| 阶段 | 内容 |
|------|------|
| M1 | 核心：自研 tokenizer + 树构建 + serialize；上表"必须有"节点；表格不透明；语料往返测试 |
| M2 | 读侧：模板按归一化名查找、参数读取、章节读取；迁移 `segment.rs` 与 `template_check.rs`，带一致性对比 |
| M3 | 写侧：参数增删改 + 格式保持；第一个真实批量编辑用例（复用 `WriteMode`/`--dry-run`/`--report-json` 约定） |
| 可选 | `parse_inner()` 表格下钻；facts/classify 迁移 |

## 8. 选型结论

- **自研**（已定）：手写状态机而非 nom——wikitext 的歧义回退用组合子表达别扭
  且易指数回退。参考语义：MediaWiki preprocessor + mwparserfromhell。
- 现成 crate（[parse-wiki-text-2](https://crates.io/crates/parse-wiki-text-2)、
  [wikrs](https://crates.io/crates/wikrs)、
  [mediawiki_parser](https://docs.rs/mediawiki_parser/)、
  [wikitext-parser](https://crates.io/crates/wikitext-parser)）以抽取为主，
  无编辑 API 与格式保持，且 Huiji 是老版 MediaWiki 方言，兼容性未知——否决作
  基座，可作差分测试参照。
- Parsoid REST：Huiji 不提供，否决。

## 9. M1 落地记录（2026-09-12）

- 模块：`src/wikitext/mod.rs`（节点类型 + serialize + `templates()` 遍历），
  `src/wikitext/parse.rs`（手写扫描器：递归下降 + 构造跳转，字节级扫描、
  ASCII 边界切片）。入口 `Wikicode::parse` / `serialize` / `templates` /
  `plain`；`parse` 永不失败。
- 节点：Text / Comment / Template / TemplateParam / Link / ExtLink /
  Heading / Tag（Empty | Parsed | Raw）/ Table(不透明原文)。
- 花括号消歧最终规则：brace run 奇数（≥3）先试 `{{{` 参数、失败回退 `{{`
  模板；偶数（≥2）先试模板、失败（且 ≥3）回退参数；4 个花括号解析为
  嵌套两层模板（与 MediaWiki 栈式配对一致）。
- 注释范围以当前扫描区间的 `-->` 为界：区间内未闭合即视为吞掉余下区间
  （顶层=吞到文末），与"预处理器先剥注释"的语义一致。
- 扫描跳转与构造解析共用同一套判定（`construct` 与 `scan_inner` 同策略），
  保证边界一致；递归深度上限 500，超过后整段按文本返回。
- 验证：24 个单元测试 + 语料 6909 页全量往返/幂等测试
  （`wikitext::parse::tests::corpus_round_trip`，语料缺失时跳过），
  `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 全绿。
- 后续（M2 读侧）：模板名归一化（`_`≡空格、首字母大小写）查找、参数读取、
  章节读取与 `{{标题|X}}` 辅助；迁移 `segment.rs`/`template_check.rs` 并加
  新旧一致性对比。

## 10. M2/M3/M4 落地记录（2026-09-12）

### Span 与读侧 API（M2）

- 所有节点带字节 `start`/`end`（原文切片=节点原文）；**编辑后 span 作废**。
  语料往返测试扩为三重断言：往返一致 + 二次解析相等 + 每个节点 span 切片
  等于节点序列化结果。
- 读 API：`normalize_template_name`（trim、`_`≡空格、去 `模板:`/`Template:`
  前缀、首字母 ASCII 大写）、`templates_named` / `is_named`、`param` /
  `positional` / `TplArg::value_plain`、`sections()`（每个标题开新节）、
  `Heading::display_title()`（`=={{标题|X}}==` → `X`）、`switch_cases`
  （`#switch` 位置参数是后续命名参数的附加标签，`#default` 单列）。

### 编辑 API（M3）

- `Template::set_param`（保留原值首尾空白装饰，`| 图像 = x ` 风格不变；
  数字 key 改位置参数）、`add_param`（按既有参数推断换行/行内风格）、
  `remove_param`（删除后布局不变）。`Wikicode::set_template_param` /
  `remove_template_param` 按归一化名批量作用于全部匹配实例。
- 可变遍历用回调式 `for_each_template_mut` / `with_templates_named_mut`：
  嵌套模板的父子 `&mut` 借用重叠，`Vec<&mut Template>` 收集在安全 Rust
  里不可行。

### 迁移

- `template_check.rs::parse_crafting_branch` 改走解析器：外层 `#switch`
  的 `filter|dst` 分支 → 内层站筛 `#switch` 的 `switch_cases` 取 case 键 +
  分支值里 `{{inv}}` 首位置参数。既有 fixture 测试即一致性安全网，全过。
- `segment.rs` 信息框识别/参数锚点改走解析器（模板名归一化 + `dst` 首位置
  参数 + 变体名去注释截行；命名参数值 span = 值节点首尾，取值语义与旧
  实现一致）。旧实现保留作差分：语料 6909 页，旧 1469 vs 新 1524 个
  信息框，0 个未解释差异——全部差异均可归因于旧行扫描缺陷（单行多参数
  被吞、值尾吞 `}}`、行内参数漏识别、空/带注释/带 `=` 的垃圾变体），
  差分测试用字节覆盖判定固化（`differential_old_vs_new_infobox_extraction`）。

### 第一个写侧用例（M4）

- 新作业 `maintain-wikitext`（`JobKind::MaintainWikitext`，
  `src/service/wikitext_edit.rs`）：`--page`（可多次）+ `--template` +
  `--set key=value`（可多次）/ `--remove key`，逐页 fetch → parse → 编辑
  → serialize → diff → 按 `WriteMode` 决定写入；报告 JSON + 新页面文本
  落 `--output`。
- 验证（全部只读，零写入）：同值 set → `no_changes`；改值 + `--dry-run`
  → `dry_run` 状态，diff 仅目标参数一行，其余逐字节未动；interactive 模式
  下确认提示默认 N（EOF 安全默认）。实跑后复查线上页面未被修改。
