//! 无损 wikitext 解析：`parse → 节点树 → serialize` 逐字节还原原文。
//!
//! 设计与语法规则见 `docs/WIKITEXT_PARSER_PLAN.md`。要点：
//! - 解析永不失败：配不平的花括号、未闭合标签一律降级为文本节点；
//! - 节点持有原始文本，serialize 即拼接，只有被修改的节点会改变输出；
//! - 表格按不透明块处理（只识别边界，内部整段原样保存）；
//! - 每个节点带字节 `start`/`end`（对原文切片即节点原文）；**编辑后 span
//!   作废**（不随修改更新），只供只读场景（语料锚点、差分）使用；
//! - 模块页（Lua）不归本模块管，继续走 `full_moon` + CopyClip 标记。

mod parse;

/// 内容不解析、整段原样保存的扩展标签（Huiji/Fandom 常用集）。
pub const RAW_CONTENT_TAGS: &[&str] = &[
    "nowiki",
    "pre",
    "gallery",
    "ref",
    "references",
    "poem",
    "tabber",
    "syntaxhighlight",
    "source",
    "math",
    "charinsert",
    "imagemap",
    "choose",
];

/// HTML 空元素（无内容、无闭合标签）。
const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// 本维基章节标题约定使用的模板名：`=={{标题|X}}==`。
const TITLE_TEMPLATE_NAME: &str = "标题";

/// wikitext 文档：顶层节点列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikicode {
    pub nodes: Vec<Node>,
}

/// wikitext 节点。变体覆盖 `docs/WIKITEXT_PARSER_PLAN.md` 第 3 节"必须有"清单。
///
/// `start`/`end` 是节点在**原文**中的字节区间（解析后有效；编辑树之后不作数）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// 普通文本（含引号、列表、`__魔字__` 等不结构化的内容）。
    Text {
        text: String,
        start: usize,
        end: usize,
    },
    /// HTML 注释，含 `<!--`/`-->`；未闭合时吞到当前范围末尾。
    Comment {
        raw: String,
        start: usize,
        end: usize,
    },
    /// `{{模板|参数}}`，含解析器函数与魔字。
    Template(Template),
    /// `{{{参数|默认}}}`（模板页里的参数引用）。
    TemplateParam(TemplateParam),
    /// `[[目标|标签]]`，`File:`/`Category:` 同此。
    Link(InternalLink),
    /// `[url 标签]` 外部链接；无协议前缀的 `[...]` 不是外部链接。
    ExtLink(ExternalLink),
    /// 行首 `== 标题 ==`（1–6 级）。
    Heading(Heading),
    /// HTML/扩展标签。
    Tag(Tag),
    /// 表格，不透明块：从行首 `{|` 到配平 `|}` 的原始文本。
    Table {
        raw: String,
        start: usize,
        end: usize,
    },
}

/// `{{名字|参数...}}`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// 模板名（原始文本，含空白；归一化读取见 [`Template::name_normalized`]）。
    pub name: Vec<Node>,
    /// 参数表；位置参数 `named == false` 且 `name` 为空。
    pub args: Vec<TplArg>,
    pub start: usize,
    pub end: usize,
}

/// 模板的一个参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TplArg {
    /// 命名参数名；位置参数为空。
    pub name: Vec<Node>,
    /// 参数值。
    pub value: Vec<Node>,
    /// 是否命名参数（存在顶层 `=`）。
    pub named: bool,
}

/// `{{{参数名|默认值}}}`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParam {
    pub name: Vec<Node>,
    /// 第一个顶层 `|` 之后的全部内容；无默认值为 `None`。
    pub default: Option<Vec<Node>>,
    pub start: usize,
    pub end: usize,
}

/// `[[...]]`，按顶层 `|` 分段；`segments[0]` 是目标，其余为标签/选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalLink {
    pub segments: Vec<Vec<Node>>,
    pub start: usize,
    pub end: usize,
}

/// `[url 标签]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLink {
    /// URL 原文（到首个空白为止，内部不解析）。
    pub url: String,
    /// URL 与标签之间的空白原文；无标签时为空。
    pub sep: String,
    /// 标签节点；无标签时为空。
    pub label: Vec<Node>,
    pub start: usize,
    pub end: usize,
}

/// 标题。level 与 close_level 可以不同（`==a=`），均为 1–6。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: usize,
    pub close_level: usize,
    /// 闭合 `=` 之后、行尾之前的空白（往返需要）。
    pub trailing_ws: String,
    pub title: Vec<Node>,
    pub start: usize,
    pub end: usize,
}

/// HTML/扩展标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// 小写标签名。
    pub name: String,
    /// 开标签原文（含属性）。
    pub raw_open: String,
    pub content: TagContent,
    /// 闭标签原文（`</x>`）；无闭合为空串。
    pub raw_close: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagContent {
    /// 空元素、自闭合或孤立闭合标签。
    Empty,
    /// 内容继续解析成节点（普通 HTML 标签）。
    Parsed(Vec<Node>),
    /// 内容原样保存（[`RAW_CONTENT_TAGS`] 名单内）。
    Raw(String),
}

impl Wikicode {
    /// 解析 wikitext。永不失败：无法识别的构造一律降级为文本。
    pub fn parse(text: &str) -> Wikicode {
        let (nodes, stop) = parse::parse_nodes(text, 0, text.len(), true, &[]);
        debug_assert!(stop.is_none());
        Wikicode { nodes }
    }

    /// 序列化回 wikitext。未修改的树逐字节还原原文。
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        serialize_nodes(&self.nodes, &mut out);
        out
    }

    /// 深度优先收集全部模板（含链接标签、模板参数、标题、标签内容里的；
    /// 不含表格内部与 raw 标签内容——它们是不透明块）。
    pub fn templates(&self) -> Vec<&Template> {
        let mut out = Vec::new();
        collect_templates(&self.nodes, &mut out);
        out
    }

    /// [`templates`](Self::templates) 的可变访问：对树中每个模板调用 `f`
    /// （父先于子）。采用回调而非 `Vec<&mut Template>`——嵌套模板的父子
    /// 可变借用会重叠，安全 Rust 收集不了。
    pub fn for_each_template_mut(&mut self, mut f: impl FnMut(&mut Template)) {
        for node in &mut self.nodes {
            node.visit_templates_mut(&mut f);
        }
    }

    /// 按归一化名查找模板（`_`≡空格、忽略 `模板:`/`Template:` 前缀、
    /// 首字母 ASCII 大小写不敏感）。
    pub fn templates_named(&self, name: &str) -> Vec<&Template> {
        let want = normalize_template_name(name);
        self.templates()
            .into_iter()
            .filter(|t| t.name_normalized() == want)
            .collect()
    }

    /// [`templates_named`](Self::templates_named) 的可变版本（回调式）。
    pub fn with_templates_named_mut(&mut self, name: &str, mut f: impl FnMut(&mut Template)) {
        let want = normalize_template_name(name);
        self.for_each_template_mut(|t| {
            if t.name_normalized() == want {
                f(t);
            }
        });
    }

    /// 在所有匹配模板上设置命名/位置参数；返回发生变更的模板数。
    pub fn set_template_param(&mut self, template: &str, key: &str, value: &str) -> bool {
        let mut changed = false;
        self.with_templates_named_mut(template, |t| {
            changed |= t.set_param(key, value);
        });
        changed
    }

    /// 在所有匹配模板上删除参数；返回发生变更的模板数。
    pub fn remove_template_param(&mut self, template: &str, key: &str) -> bool {
        let mut changed = false;
        self.with_templates_named_mut(template, |t| {
            changed |= t.remove_param(key);
        });
        changed
    }

    /// 顶层节点按标题切分：第一个标题之前是 `heading == None` 的引言段，
    /// 之后每个标题开一个新节（无论级别）。
    pub fn sections(&self) -> Vec<Section<'_>> {
        let mut out = Vec::new();
        let mut current = Section {
            heading: None,
            nodes: Vec::new(),
        };
        for node in &self.nodes {
            if let Node::Heading(h) = node {
                if !(current.nodes.is_empty() && current.heading.is_none()) {
                    out.push(std::mem::replace(
                        &mut current,
                        Section {
                            heading: None,
                            nodes: Vec::new(),
                        },
                    ));
                }
                current.heading = Some(h);
            } else {
                current.nodes.push(node);
            }
        }
        if !current.nodes.is_empty() || current.heading.is_some() {
            out.push(current);
        }
        out
    }
}

/// [`Wikicode::sections`] 的一个分节。
#[derive(Debug)]
pub struct Section<'a> {
    pub heading: Option<&'a Heading>,
    /// 节内的节点（不含标题节点本身）。
    pub nodes: Vec<&'a Node>,
}

impl Section<'_> {
    /// 节内容文本。
    pub fn text(&self) -> String {
        let mut out = String::new();
        for n in &self.nodes {
            n.serialize_into(&mut out);
        }
        out
    }
}

/// `{{#switch:…}}` 的一个分支（位置参数是后续命名参数的附加标签）。
#[derive(Debug)]
pub struct SwitchCase<'a> {
    /// 全部标签（trim 后）；`#default` 分支只有一个 `#default`。
    pub labels: Vec<String>,
    /// 分支值节点。
    pub value: &'a [Node],
    pub is_default: bool,
}

/// 把 `#switch` 模板的参数按 MediaWiki 语义分组为分支。
/// 末尾没有 `=` 的孤儿位置参数（默认值用法）不产出分支。
pub fn switch_cases(t: &Template) -> Vec<SwitchCase<'_>> {
    let mut pending: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for arg in &t.args {
        if arg.named {
            let label = plain(&arg.name).trim().to_string();
            let mut labels = std::mem::take(&mut pending);
            labels.push(label.clone());
            out.push(SwitchCase {
                labels,
                value: &arg.value,
                is_default: label == "#default",
            });
        } else {
            pending.push(plain(&arg.value).trim().to_string());
        }
    }
    out
}

/// MediaWiki 标题归一化（模板/链接目标名查找用）：trim、`_`≡空格、
/// 去 `模板:`/`Template:` 前缀、首字母 ASCII 大写。
pub fn normalize_template_name(name: &str) -> String {
    let s = name.trim().replace('_', " ");
    let s = s.strip_prefix("模板:").unwrap_or(&s);
    // 常见大小写枚举去 Template: 前缀（避免 ASCII 比较切进多字节字符）
    let s = s
        .strip_prefix("template:")
        .or_else(|| s.strip_prefix("Template:"))
        .or_else(|| s.strip_prefix("TEMPLATE:"))
        .unwrap_or(s);
    // 首字母 ASCII 大写（对中文等非 ASCII 首字符是恒等变换）
    match s.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => {
            c.to_ascii_uppercase().to_string() + &s[c.len_utf8()..]
        }
        _ => s.to_string(),
    }
}

/// 节点序列的文本拼接（等价 serialize，用于读取名字/参数值）。
pub fn plain(nodes: &[Node]) -> String {
    let mut out = String::new();
    serialize_nodes(nodes, &mut out);
    out
}

/// 收集节点序列里的全部模板（递归；用途同 [`Wikicode::templates`]）。
pub fn templates_in(nodes: &[Node]) -> Vec<&Template> {
    let mut out = Vec::new();
    collect_templates(nodes, &mut out);
    out
}

pub fn serialize_nodes(nodes: &[Node], out: &mut String) {
    for node in nodes {
        node.serialize_into(out);
    }
}

/// 节点序列的字节区间（首尾节点 span 的并集）；空序列为 `None`。
pub fn nodes_span(nodes: &[Node]) -> Option<(usize, usize)> {
    let first = nodes.first()?.span();
    let last = nodes.last()?.span();
    Some((first.0, last.1))
}

impl Node {
    pub fn serialize_into(&self, out: &mut String) {
        match self {
            Node::Text { text, .. }
            | Node::Comment { raw: text, .. }
            | Node::Table { raw: text, .. } => {
                out.push_str(text);
            }
            Node::Template(t) => {
                out.push_str("{{");
                serialize_nodes(&t.name, out);
                for arg in &t.args {
                    out.push('|');
                    if arg.named {
                        serialize_nodes(&arg.name, out);
                        out.push('=');
                    }
                    serialize_nodes(&arg.value, out);
                }
                out.push_str("}}");
            }
            Node::TemplateParam(p) => {
                out.push_str("{{{");
                serialize_nodes(&p.name, out);
                if let Some(default) = &p.default {
                    out.push('|');
                    serialize_nodes(default, out);
                }
                out.push_str("}}}");
            }
            Node::Link(l) => {
                out.push_str("[[");
                for (i, seg) in l.segments.iter().enumerate() {
                    if i > 0 {
                        out.push('|');
                    }
                    serialize_nodes(seg, out);
                }
                out.push_str("]]");
            }
            Node::ExtLink(e) => {
                out.push('[');
                out.push_str(&e.url);
                out.push_str(&e.sep);
                serialize_nodes(&e.label, out);
                out.push(']');
            }
            Node::Heading(h) => {
                out.push_str(&"=".repeat(h.level));
                serialize_nodes(&h.title, out);
                out.push_str(&"=".repeat(h.close_level));
                out.push_str(&h.trailing_ws);
            }
            Node::Tag(t) => {
                out.push_str(&t.raw_open);
                match &t.content {
                    TagContent::Empty => {}
                    TagContent::Raw(s) => out.push_str(s),
                    TagContent::Parsed(ns) => serialize_nodes(ns, out),
                }
                out.push_str(&t.raw_close);
            }
        }
    }

    /// 节点在原文中的字节区间。
    pub fn span(&self) -> (usize, usize) {
        match self {
            Node::Text { start, end, .. }
            | Node::Comment { start, end, .. }
            | Node::Table { start, end, .. } => (*start, *end),
            Node::Template(t) => (t.start, t.end),
            Node::TemplateParam(p) => (p.start, p.end),
            Node::Link(l) => (l.start, l.end),
            Node::ExtLink(e) => (e.start, e.end),
            Node::Heading(h) => (h.start, h.end),
            Node::Tag(t) => (t.start, t.end),
        }
    }

    /// 深度优先遍历本节点及全部后代。
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Node>) {
        out.push(self);
        match self {
            Node::Template(t) => {
                for n in &t.name {
                    n.walk(out);
                }
                for arg in &t.args {
                    for n in arg.name.iter().chain(arg.value.iter()) {
                        n.walk(out);
                    }
                }
            }
            Node::TemplateParam(p) => {
                for n in p.name.iter().chain(p.default.iter().flatten()) {
                    n.walk(out);
                }
            }
            Node::Link(l) => {
                for seg in &l.segments {
                    for n in seg {
                        n.walk(out);
                    }
                }
            }
            Node::ExtLink(e) => {
                for n in &e.label {
                    n.walk(out);
                }
            }
            Node::Heading(h) => {
                for n in &h.title {
                    n.walk(out);
                }
            }
            Node::Tag(t) => {
                if let TagContent::Parsed(ns) = &t.content {
                    for n in ns {
                        n.walk(out);
                    }
                }
            }
            Node::Text { .. } | Node::Comment { .. } | Node::Table { .. } => {}
        }
    }

    /// 可变 visitor：对以本节点为根的子树中的每个模板调用 `f`。
    fn visit_templates_mut(&mut self, f: &mut impl FnMut(&mut Template)) {
        match self {
            Node::Template(t) => {
                f(t);
                for n in &mut t.name {
                    n.visit_templates_mut(f);
                }
                for arg in &mut t.args {
                    for n in arg.name.iter_mut().chain(arg.value.iter_mut()) {
                        n.visit_templates_mut(f);
                    }
                }
            }
            Node::TemplateParam(p) => {
                for n in p.name.iter_mut().chain(p.default.iter_mut().flatten()) {
                    n.visit_templates_mut(f);
                }
            }
            Node::Link(l) => {
                for seg in &mut l.segments {
                    for n in seg {
                        n.visit_templates_mut(f);
                    }
                }
            }
            Node::ExtLink(e) => {
                for n in &mut e.label {
                    n.visit_templates_mut(f);
                }
            }
            Node::Heading(h) => {
                for n in &mut h.title {
                    n.visit_templates_mut(f);
                }
            }
            Node::Tag(t) => {
                if let TagContent::Parsed(ns) = &mut t.content {
                    for n in ns {
                        n.visit_templates_mut(f);
                    }
                }
            }
            Node::Text { .. } | Node::Comment { .. } | Node::Table { .. } => {}
        }
    }
}

impl Template {
    /// 模板名的文本形式（未做空白/大小写归一化）。
    pub fn name_plain(&self) -> String {
        plain(&self.name)
    }

    /// 归一化模板名（[`normalize_template_name`]）。
    pub fn name_normalized(&self) -> String {
        normalize_template_name(&self.name_plain())
    }

    /// 模板名按 MediaWiki 规则匹配（两侧都归一化）。
    pub fn is_named(&self, name: &str) -> bool {
        self.name_normalized() == normalize_template_name(name)
    }

    /// 位置参数（1 起）的值文本（trim 后）。
    pub fn positional(&self, index: usize) -> Option<String> {
        self.args
            .iter()
            .filter(|a| !a.named)
            .nth(index.saturating_sub(1))
            .map(|a| plain(&a.value).trim().to_string())
    }

    /// 读取参数：`key` 为数字时按位置（1 起），否则按命名参数
    /// （名字 trim、`_`≡空格 后比较）。返回值的文本形式（trim 后）。
    pub fn param(&self, key: &str) -> Option<String> {
        self.arg_node(key)
            .map(|a| plain(&a.value).trim().to_string())
    }

    fn arg_node(&self, key: &str) -> Option<&TplArg> {
        if let Ok(index) = key.parse::<usize>() {
            return self
                .args
                .iter()
                .filter(|a| !a.named)
                .nth(index.saturating_sub(1));
        }
        let want = key.trim().replace('_', " ");
        self.args
            .iter()
            .filter(|a| a.named)
            .find(|a| plain(&a.name).trim().replace('_', " ") == want)
    }

    /// 设置参数值，保留原值的首尾空白装饰（`| 图像 = x ` 风格不变）。
    /// 数字 `key` 修改对应位置参数。返回是否发生变更。
    ///
    /// 不存在的命名参数不会被隐式创建（用 [`Template::add_param`]）。
    pub fn set_param(&mut self, key: &str, value: &str) -> bool {
        let arg = if let Ok(index) = key.parse::<usize>() {
            self.args
                .iter_mut()
                .filter(|a| !a.named)
                .nth(index.saturating_sub(1))
        } else {
            let want = key.trim().replace('_', " ");
            self.args
                .iter_mut()
                .find(|a| a.named && plain(&a.name).trim().replace('_', " ") == want)
        };
        let Some(arg) = arg else {
            return false;
        };
        // 保留原值首尾的空白装饰
        let old = plain(&arg.value);
        let lead: String = old.chars().take_while(|c| c.is_whitespace()).collect();
        let trail_start = old
            .char_indices()
            .rev()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let trail = old[trail_start..].to_string();
        let candidate = format!("{lead}{value}{trail}");
        if old == candidate {
            return false;
        }
        arg.value = vec![Node::Text {
            text: candidate,
            start: 0,
            end: 0,
        }];
        true
    }

    /// 追加参数（命名 `key` 传 `Some`，位置传 `None`）。
    ///
    /// 风格推断：若模板以换行分隔参数（最后一个值以 `\n` 结尾或无参数而
    /// 名字以 `\n` 结尾），新参数值补一个尾部 `\n`，使 `}}` 独占一行；
    /// 否则按行内风格追加。
    pub fn add_param(&mut self, key: Option<&str>, value: &str) {
        let line_style = self
            .args
            .last()
            .map(|a| plain(&a.value).ends_with('\n'))
            .unwrap_or_else(|| self.name_plain().ends_with('\n'));
        let value_text = if line_style {
            format!("{value}\n")
        } else {
            value.to_string()
        };
        self.args.push(match key {
            Some(key) => TplArg {
                name: vec![Node::Text {
                    text: key.trim().to_string(),
                    start: 0,
                    end: 0,
                }],
                value: vec![Node::Text {
                    text: value_text,
                    start: 0,
                    end: 0,
                }],
                named: true,
            },
            None => TplArg {
                name: Vec::new(),
                value: vec![Node::Text {
                    text: value_text,
                    start: 0,
                    end: 0,
                }],
                named: false,
            },
        });
    }

    /// 删除参数（数字 `key` 按位置，否则按命名）。返回是否删除。
    pub fn remove_param(&mut self, key: &str) -> bool {
        let pos = if let Ok(index) = key.parse::<usize>() {
            self.args
                .iter()
                .enumerate()
                .filter(|(_, a)| !a.named)
                .nth(index.saturating_sub(1))
                .map(|(i, _)| i)
        } else {
            let want = key.trim().replace('_', " ");
            self.args
                .iter()
                .position(|a| a.named && plain(&a.name).trim().replace('_', " ") == want)
        };
        match pos {
            Some(i) => {
                self.args.remove(i);
                true
            }
            None => false,
        }
    }
}

impl TplArg {
    /// 参数值文本（trim 后）。
    pub fn value_plain(&self) -> String {
        plain(&self.value).trim().to_string()
    }

    /// 命名参数名文本（trim 后）。
    pub fn name_plain(&self) -> String {
        plain(&self.name).trim().to_string()
    }
}

impl Heading {
    /// 标题显示文本：`{{标题|X}}` 解析为 `X`（本维基章节标题约定），
    /// 其余标题 trim 后原样返回。
    pub fn display_title(&self) -> String {
        // `== {{标题|X}} ==` 的 title 节点是 [空白, 模板, 空白]
        let meaningful: Vec<&Node> = self
            .title
            .iter()
            .filter(|n| !matches!(n, Node::Text { text, .. } if text.trim().is_empty()))
            .collect();
        if let [Node::Template(t)] = meaningful[..] {
            if t.name_normalized() == TITLE_TEMPLATE_NAME {
                if let Some(first) = t.positional(1) {
                    return first;
                }
            }
        }
        plain(&self.title).trim().to_string()
    }
}

fn collect_templates<'a>(nodes: &'a [Node], out: &mut Vec<&'a Template>) {
    for node in nodes {
        match node {
            Node::Template(t) => {
                out.push(t);
                collect_templates(&t.name, out);
                for arg in &t.args {
                    collect_templates(&arg.name, out);
                    collect_templates(&arg.value, out);
                }
            }
            Node::TemplateParam(p) => {
                collect_templates(&p.name, out);
                if let Some(d) = &p.default {
                    collect_templates(d, out);
                }
            }
            Node::Link(l) => {
                for seg in &l.segments {
                    collect_templates(seg, out);
                }
            }
            Node::ExtLink(e) => collect_templates(&e.label, out),
            Node::Heading(h) => collect_templates(&h.title, out),
            Node::Tag(t) => {
                if let TagContent::Parsed(ns) = &t.content {
                    collect_templates(ns, out);
                }
            }
            Node::Text { .. } | Node::Comment { .. } | Node::Table { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Wikicode {
        Wikicode::parse(s)
    }

    fn assert_rt(s: &str) -> Wikicode {
        let code = parse(s);
        assert_eq!(code.serialize(), s, "往返不一致");
        let reparsed = parse(&code.serialize());
        assert_eq!(reparsed, code, "二次解析不相等");
        code
    }

    /// 每个节点的 span 切片必须等于节点自身序列化结果。
    fn assert_spans(text: &str, code: &Wikicode) {
        let mut all = Vec::new();
        for n in &code.nodes {
            n.walk(&mut all);
        }
        for n in &all {
            let (s, e) = n.span();
            let mut rendered = String::new();
            n.serialize_into(&mut rendered);
            assert_eq!(&text[s..e], rendered, "span 与内容不符: {rendered:?}");
        }
    }

    fn assert_rt_spans(s: &str) -> Wikicode {
        let code = assert_rt(s);
        assert_spans(s, &code);
        code
    }

    #[test]
    fn text_and_empty() {
        assert_rt("");
        assert_rt("纯文本，没有构造。'单引号' __NOTOC__ ~~~~ ----");
        let code = parse("abc");
        assert_eq!(
            code.nodes,
            vec![Node::Text {
                text: "abc".to_string(),
                start: 0,
                end: 3
            }]
        );
    }

    #[test]
    fn spans_match_content() {
        assert_rt_spans("{{a|b=[[c|d]]}}前\n=={{标题|x}}==\n<!-- 注 -->\n{| t |}\n[https://e x]");
    }

    #[test]
    fn simple_template() {
        let code = assert_rt("{{a}}");
        let t = &code.templates()[0];
        assert_eq!(t.name_plain(), "a");
        assert!(t.args.is_empty());
    }

    #[test]
    fn template_with_named_args() {
        let code =
            assert_rt("{{实体信息框/自动|dst|图像=[[File:X.png|188px]]|生成自=[[隐士小岛]]}}");
        let t = &code.templates()[0];
        assert_eq!(t.name_plain(), "实体信息框/自动");
        assert_eq!(t.args.len(), 3);
        assert!(!t.args[0].named);
        assert_eq!(plain(&t.args[0].value), "dst");
        assert!(t.args[1].named);
        assert_eq!(plain(&t.args[1].name), "图像");
        // 链接里的 | 不是参数分隔符
        assert_eq!(plain(&t.args[1].value), "[[File:X.png|188px]]");
    }

    #[test]
    fn template_value_keeps_extra_equals() {
        let code = assert_rt("{{a|b=c=d}}");
        let t = &code.templates()[0];
        assert!(t.args[0].named);
        assert_eq!(plain(&t.args[0].name), "b");
        assert_eq!(plain(&t.args[0].value), "c=d");
    }

    #[test]
    fn template_empty_and_edge_args() {
        assert_rt("{{}}");
        assert_rt("{{|}}");
        assert_rt("{{a|}}");
        assert_rt("{{a|=v}}");
        let code = assert_rt("{{a||b}}");
        let t = &code.templates()[0];
        assert_eq!(t.args.len(), 2);
        assert_eq!(plain(&t.args[0].value), "");
        assert_eq!(plain(&t.args[1].value), "b");
    }

    #[test]
    fn nested_template_and_walker() {
        let code = assert_rt("前{{a|x={{b|y}}}}后");
        assert_eq!(code.templates().len(), 2);
    }

    #[test]
    fn template_inside_link_caption() {
        let code = assert_rt("[[a|{{b}}]]");
        assert_eq!(code.templates().len(), 1);
        let Node::Link(l) = &code.nodes[0] else {
            panic!("应为链接");
        };
        assert_eq!(l.segments.len(), 2);
    }

    #[test]
    fn template_params() {
        let code = assert_rt("{{{1|默认}}}");
        let Node::TemplateParam(p) = &code.nodes[0] else {
            panic!("应为参数");
        };
        assert_eq!(plain(&p.name), "1");
        assert_eq!(plain(p.default.as_ref().unwrap()), "默认");

        let code = assert_rt("{{{a}}}");
        let Node::TemplateParam(p) = &code.nodes[0] else {
            panic!("应为参数");
        };
        assert!(p.default.is_none());

        // 默认值含模板与后续 |
        assert_rt("{{{a|{{b}}|c}}}");
    }

    #[test]
    fn brace_ambiguity() {
        // 4 个花括号 → 嵌套两层模板
        let code = assert_rt("{{{{x}}}}");
        assert_eq!(code.templates().len(), 2);

        // `{{a}}}` → 模板 + 文本 }
        let code = assert_rt("{{a}}}");
        assert_eq!(code.templates().len(), 1);
        assert!(code
            .nodes
            .last()
            .is_some_and(|n| matches!(n, Node::Text { text, .. } if text == "}")));

        // `{{{a}}`：参数配不平 → 回退为模板（已知偏差，见方案文档第 5 节）
        let code = assert_rt("{{{a}}");
        assert_eq!(code.templates().len(), 1);
        assert_eq!(code.templates()[0].name_plain(), "{a");

        // 5 个花括号 → 参数（名字为内层模板）+ 尾文本
        assert_rt("{{{{{x}}}}}");
    }

    #[test]
    fn unclosed_braces_degrade_to_text() {
        for s in ["{{a", "[[a", "{{{a", "{{{{", "{"] {
            let code = parse(s);
            assert_eq!(code.serialize(), s);
        }
    }

    #[test]
    fn comment_masks_braces() {
        let code = assert_rt("a<!-- {{b}} [[c]] -->d");
        assert!(code.templates().is_empty());
        assert!(matches!(&code.nodes[1], Node::Comment { raw, .. } if raw.contains("{{b}}")));
    }

    #[test]
    fn unclosed_comment_runs_to_end() {
        let code = parse("x<!-- y {{z}}");
        assert_eq!(code.serialize(), "x<!-- y {{z}}");
        assert!(code.templates().is_empty());
    }

    #[test]
    fn nowiki_is_raw() {
        let code = assert_rt("<nowiki>{{a}}[[b]]</nowiki>");
        assert!(code.templates().is_empty());
        let Node::Tag(t) = &code.nodes[0] else {
            panic!("应为标签");
        };
        assert_eq!(t.name, "nowiki");
        assert!(matches!(&t.content, TagContent::Raw(s) if s == "{{a}}[[b]]"));
    }

    #[test]
    fn headings() {
        let code = assert_rt("== {{标题|花絮}} ==\n正文");
        let Node::Heading(h) = &code.nodes[0] else {
            panic!("应为标题");
        };
        assert_eq!(h.level, 2);
        assert_eq!(h.close_level, 2);
        // 闭合 = 之前的空间属于标题原文，= 之后无空白
        assert_eq!(h.trailing_ws, "");
        assert_eq!(code.templates().len(), 1);
        assert_eq!(h.display_title(), "花絮");

        // 闭合级数与开头不同
        let code = assert_rt("==a=\n");
        let Node::Heading(h) = &code.nodes[0] else {
            panic!("应为标题");
        };
        assert_eq!(h.level, 2);
        assert_eq!(h.close_level, 1);

        assert_rt("=x=\n");
        // 7 个 = 不是标题
        let code = parse("======= x =======\n");
        assert!(matches!(&code.nodes[0], Node::Text { text, .. } if text.starts_with("=======")));
        // 未闭合不是标题
        let code = parse("==abc\n");
        assert!(!matches!(&code.nodes[0], Node::Heading(_)));
        // 行中 = 不是标题
        let code = parse("a == b\n");
        assert!(!matches!(&code.nodes[0], Node::Heading(_)));
    }

    #[test]
    fn table_is_opaque() {
        let text = "{| class=\"wikitable\"\n!增加电荷\n|\n* [[闪电]]回复 100 {{生命值}}。\n|-\n!损失电荷\n| {{理智值}}\n|}";
        let code = assert_rt(text);
        // 表格内部对全页模板遍历不可见（不透明块）
        assert!(code.templates().is_empty());
        assert!(
            matches!(&code.nodes[0], Node::Table { raw, .. } if raw.starts_with("{|") && raw.ends_with("|}"))
        );
    }

    #[test]
    fn table_with_nested_table() {
        let text = "{| a\n{| b |}\n|}";
        let code = assert_rt(text);
        let Node::Table { raw, .. } = &code.nodes[0] else {
            panic!("应为表格");
        };
        assert!(raw.contains("{| b |}"));
    }

    #[test]
    fn table_inside_template_arg_is_text() {
        // 表格作为模板参数：模板正常闭合，表格文本留在参数值里
        let text = "{{a|\n{|x|}\n}}";
        let code = assert_rt(text);
        assert_eq!(code.templates().len(), 1);
    }

    #[test]
    fn external_links() {
        let code = assert_rt("[https://a.b/c 标签 {{x}}]");
        let Node::ExtLink(e) = &code.nodes[0] else {
            panic!("应为外部链接");
        };
        assert_eq!(e.url, "https://a.b/c");
        assert_eq!(e.sep, " ");
        assert_eq!(code.templates().len(), 1);

        assert!(matches!(parse("[https://a.b]").nodes[0], Node::ExtLink(_)));
        assert!(matches!(parse("[//a.b x]").nodes[0], Node::ExtLink(_)));
        assert!(matches!(parse("[mailto:a@b x]").nodes[0], Node::ExtLink(_)));

        // 无协议不是外部链接
        for s in ["[注1]", "[foo:bar]", "[ foo]", "[]"] {
            assert!(!matches!(parse(s).nodes[0], Node::ExtLink(_)), "{s}");
        }
    }

    #[test]
    fn redirect_page() {
        let code = assert_rt("#REDIRECT[[舞台剧/黑罩]]");
        assert!(matches!(&code.nodes[1], Node::Link(_)));
    }

    #[test]
    fn html_tags() {
        // 属性与子内容
        let code = assert_rt("<span class=\"a\">x{{y}}z</span>");
        assert_eq!(code.templates().len(), 1);
        // 引号里的 > 不结束标签
        assert_rt("<span title=\"a>b\">x</span>");
        // 空元素
        assert_rt("a<br>b");
        assert_rt("a<br/>b");
        assert_rt("a<BR/>b");
        // 孤立闭合标签
        assert_rt("a</div>b");
        // 未闭合降级为文本，但内部继续解析
        let code = parse("<div>{{a}}");
        assert_eq!(code.serialize(), "<div>{{a}}");
        assert_eq!(code.templates().len(), 1);
        // 同名嵌套按深度配对
        let code = assert_rt("<div><div>x</div>{{y}}</div>");
        assert_eq!(code.templates().len(), 1);
        let Node::Tag(t) = &code.nodes[0] else {
            panic!("应为标签");
        };
        let TagContent::Parsed(children) = &t.content else {
            panic!("应为解析内容");
        };
        assert!(children.iter().any(|n| matches!(n, Node::Tag(_))));
    }

    #[test]
    fn ref_content_is_raw() {
        let code = assert_rt("<ref name=\"a\">见{{x}}</ref>");
        assert!(code.templates().is_empty());
        // 自闭合 ref
        assert_rt("<ref name=\"b\"/>");
    }

    // ------------------------------------------------------------------
    // M2 读侧 API
    // ------------------------------------------------------------------

    #[test]
    fn name_normalization() {
        assert_eq!(
            normalize_template_name(" 实体信息框/自动 "),
            "实体信息框/自动"
        );
        assert_eq!(
            normalize_template_name("实体_信息框/自动"),
            "实体 信息框/自动"
        );
        assert_eq!(normalize_template_name("infobox"), "Infobox");
        assert_eq!(normalize_template_name("template:Infobox"), "Infobox");
        assert_eq!(normalize_template_name("模板:Infobox"), "Infobox");
        assert_eq!(normalize_template_name("{{标题}}"), "{{标题}}");
    }

    #[test]
    fn templates_named_lookup() {
        let code = assert_rt("{{实体信息框/自动|dst}} {{Infobox}} {{infobox|x}}");
        assert_eq!(code.templates_named("infobox").len(), 2);
        assert_eq!(code.templates_named("模板:实体信息框/自动").len(), 1);
        assert_eq!(code.templates_named("不存在").len(), 0);
    }

    #[test]
    fn param_read_and_positional() {
        let code = assert_rt("{{t|dst|shell| 图像 = x |生成_自=y}}");
        let t = &code.templates()[0];
        assert_eq!(t.positional(1).as_deref(), Some("dst"));
        assert_eq!(t.positional(2).as_deref(), Some("shell"));
        assert_eq!(t.positional(3), None);
        assert_eq!(t.param("图像").as_deref(), Some("x"));
        // 参数名 `_`≡空格
        assert_eq!(t.param("生成 自").as_deref(), Some("y"));
        assert_eq!(t.param("missing"), None);
    }

    #[test]
    fn sections_split() {
        let code = assert_rt("引言\n==A==\na1\n===B===\nb\n==C==\nc");
        let sections = code.sections();
        // 无论级别，每个标题都开新节
        assert_eq!(sections.len(), 4);
        assert!(sections[0].heading.is_none());
        assert_eq!(sections[0].text(), "引言\n");
        assert_eq!(
            sections[1].heading.map(|h| h.display_title()),
            Some("A".to_string())
        );
        assert_eq!(
            sections[2].heading.map(|h| h.display_title()),
            Some("B".to_string())
        );
        // 标题节点不含行尾换行，换行落在节内容开头
        assert_eq!(sections[2].text(), "\nb\n");
        assert_eq!(
            sections[3].heading.map(|h| h.display_title()),
            Some("C".to_string())
        );
    }

    #[test]
    fn switch_cases_grouping() {
        let code =
            assert_rt("{{#switch: {{{1|ds}}}\n|filter\n|dst =A\n|光源|照明=B\n|#default =Z}}");
        let t = &code.templates()[0];
        assert!(t.name_plain().starts_with("#switch:"));
        let cases = switch_cases(t);
        assert_eq!(cases.len(), 3);
        assert!(cases[0].labels == vec!["filter", "dst"]);
        assert!(cases[1].labels == vec!["光源", "照明"]);
        assert_eq!(plain(cases[1].value).trim(), "B");
        assert!(cases[2].is_default);
        assert_eq!(plain(cases[2].value).trim(), "Z");
    }

    // ------------------------------------------------------------------
    // M3 编辑 API
    // ------------------------------------------------------------------

    #[test]
    fn set_param_preserves_decorations() {
        let mut code =
            assert_rt("{{实体信息框/自动|dst\n| 图像 = [[File:old.png]]\n|生成自=x\n|shell}}");
        assert!(code.set_template_param("实体信息框/自动", "图像", "[[File:new.png]]"));
        assert_eq!(
            code.serialize(),
            "{{实体信息框/自动|dst\n| 图像 = [[File:new.png]]\n|生成自=x\n|shell}}"
        );
        // 不存在则不变
        assert!(!code.set_template_param("实体信息框/自动", "missing", "v"));
        assert_eq!(
            code.serialize(),
            "{{实体信息框/自动|dst\n| 图像 = [[File:new.png]]\n|生成自=x\n|shell}}"
        );
    }

    #[test]
    fn set_param_positional_and_value_equal_noop() {
        let mut code = assert_rt("{{t|dst|图像=x}}");
        assert!(!code.set_template_param("t", "1", "dst"));
        assert!(code.set_template_param("t", "1", "ds"));
        assert_eq!(code.serialize(), "{{t|ds|图像=x}}");
    }

    #[test]
    fn add_param_line_and_inline_style() {
        let mut code = assert_rt("{{t\n| a = 1\n| b = 2\n}}");
        code.with_templates_named_mut("t", |t| t.add_param(Some("c"), "3"));
        assert_eq!(code.serialize(), "{{t\n| a = 1\n| b = 2\n|c=3\n}}");

        let mut code = assert_rt("{{t|x|y}}");
        code.with_templates_named_mut("t", |t| t.add_param(Some("z"), "3"));
        assert_eq!(code.serialize(), "{{t|x|y|z=3}}");

        let mut code = assert_rt("{{t}}");
        code.with_templates_named_mut("t", |t| t.add_param(None, "pos"));
        assert_eq!(code.serialize(), "{{t|pos}}");
    }

    #[test]
    fn remove_param_keeps_layout() {
        let mut code = assert_rt("{{t\n| a = 1\n| b = 2\n| c = 3\n}}");
        assert!(code.remove_template_param("t", "b"));
        assert_eq!(code.serialize(), "{{t\n| a = 1\n| c = 3\n}}");
        assert!(!code.remove_template_param("t", "b"));
    }

    #[test]
    fn edited_tree_round_trips() {
        let mut code = assert_rt("{{实体信息框/自动|dst\n|图像=[[File:a.png]]\n|shell}}");
        assert!(code.set_template_param("实体信息框/自动", "图像", "[[File:b.png|188px]]"));
        code.with_templates_named_mut("实体信息框/自动", |t| {
            t.add_param(Some("生成自"), "[[隐士小岛]]")
        });
        let out = code.serialize();
        assert_eq!(
            out,
            "{{实体信息框/自动|dst\n|图像=[[File:b.png|188px]]\n|shell|生成自=[[隐士小岛]]}}"
        );
        // 编辑后的输出重新解析仍往返一致
        assert_rt(&out);
    }

    #[test]
    fn real_infobox_page() {
        let text = "\
{{实体信息框/自动|dst
|图像=[[File:Shell Cluster Dropped.png|188px]]
|生成自=[[隐士小岛]]
|掉落={{Pic|32|低音贝壳钟}}×1，{{Pic|32|中音贝壳钟}}×1，{{Pic|32|高音贝壳钟}}×1，<br>{{Pic|32|低音贝壳钟}}×1（50%）<br>{{Pic|32|中音贝壳钟}}×1（50%）<br>{{Pic|32|高音贝壳钟}}×1（50%）
|shell_cluster}}
{{全角色台词|dst|shell_cluster}}
'''贝壳堆（{{En|Shell Cluster}}）'''是[[饥荒：联机版]]{{DST}}中的一种实体，生成在[[隐士小岛]]周围的海底。

贝壳堆属于[[重物]]。用[[鹤嘴锄]]开采贝壳堆会获得三种[[贝壳钟]]各1~2个。

[[寄居蟹隐士]]的一项任务要求用[[夹夹绞盘]]打捞出[[隐士小岛]]周围所有的贝壳堆。

=={{标题|花絮}}==
{{加入|shell_cluster}}
[[Category:旧神归来]]";
        let code = assert_rt(text);
        assert_spans(text, &code);
        let tpls = code.templates();
        // 信息框 + 6 Pic + 全角色台词 + En + DST + 加入 + 标题
        assert_eq!(tpls.len(), 12, "模板数不符: {}", tpls.len());
        assert_eq!(tpls[0].name_plain(), "实体信息框/自动");
        assert_eq!(tpls[0].args.len(), 5);
        assert_eq!(plain(&tpls[0].args[0].value), "dst\n");
        assert_eq!(plain(&tpls[0].args[3].name), "掉落");
        assert!(plain(&tpls[0].args[3].value).contains("{{Pic|32|低音贝壳钟}}"));
        assert_eq!(plain(&tpls[0].args[4].value), "shell_cluster");
        // 章节切分：引言 + 花絮
        let sections = code.sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(
            sections[1].heading.map(|h| h.display_title()),
            Some("花絮".to_string())
        );
    }
}
