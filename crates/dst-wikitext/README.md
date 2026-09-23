# dst-wikitext

MediaWiki wikitext 的**无损（round-trip）**解析与编辑库：`parse → 节点树 → serialize`
逐字节还原原文，只让被修改的节点改变输出。

从 [dst-huiji-wiki](https://github.com/AshBarks/dst-huiji-wiki) 抽取的独立 crate，
**零第三方依赖**（仅 std）。

## 特性

- **解析永不失败**：配不平的花括号、未闭合标签一律降级为文本节点，不抛错。
- **逐字节还原**：节点持有原始文本，`serialize()` 即拼接；未修改的内容与输入完全一致。
- **格式保持的编辑**：`set_param` / `add_param` / `remove_param` 只改目标参数，不重排其余内容。
- **原文 span**：节点带字节 `start`/`end`（对原文切片即节点原文）；编辑后 span 作废，仅供只读场景（语料锚点、差分）。
- 结构化节点：模板、模板参数、标题、内/外部链接、HTML 注释、扩展标签、switch 分支；
  表格按不透明块处理（只识别边界，内部整段原样保存）。

## 快速开始

```rust
use dst_wikitext::Wikicode;

let mut code = Wikicode::parse("{{Infobox|name=Wilson}}\n== 生平 ==\n内容");

// 未修改时逐字节还原
assert_eq!(code.serialize(), "{{Infobox|name=Wilson}}\n== 生平 ==\n内容");

// 只改目标参数，其余原文保持
code.set_template_param("Infobox", "name", "Willow");
assert!(code.serialize().contains("name=Willow"));
assert!(code.serialize().ends_with("== 生平 ==\n内容"));

// 只读查询
let tpls = code.templates_named("Infobox");
assert_eq!(tpls.len(), 1);
assert_eq!(tpls[0].param("name").as_deref(), Some("Willow"));
```

## API 概览

| 入口 | 说明 |
|---|---|
| `Wikicode::parse(&str)` | 解析（永不失败） |
| `Wikicode::serialize()` | 序列化回 wikitext |
| `templates()` / `templates_named(name)` / `templates_in(nodes)` | 模板查询 |
| `with_templates_named_mut(name, f)` | 可变遍历（回调式，避免借用冲突） |
| `set_template_param` / `remove_template_param` | 按模板名批量改/删参数 |
| `Template::param` / `positional` / `set_param` / `add_param` / `remove_param` | 单模板参数读写 |
| `sections()` | 按标题切分顶层节点 |
| `switch_cases(t)` | 展开 `#switch` 分支 |
| `plain(nodes)` / `nodes_span(nodes)` | 纯文本 / 节点原文区间 |

## 设计文档

语法规则与取舍见主仓库 `docs/WIKITEXT_PARSER_PLAN.md`。

## License

MIT
