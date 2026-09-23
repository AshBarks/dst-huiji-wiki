//! 手写扫描器：递归下降 + 构造跳转（见 `docs/WIKITEXT_PARSER_PLAN.md` 第 4 节）。
//!
//! 所有语法分隔符均为 ASCII：按字节扫描、在 ASCII 边界切片，多字节字符
//! 自然落入文本节点。嵌套优先级：注释 → nowiki/扩展标签 → `{{{` → `{{`
//! → `[[`；花括号 run 按"奇数先试参数、偶数先试模板、失败回退"消歧。

use super::{
    ExternalLink, Heading, InternalLink, Node, Tag, TagContent, Template, TemplateParam, TplArg,
    RAW_CONTENT_TAGS, VOID_TAGS,
};
use std::cell::Cell;

/// 递归深度上限：超过后不再识别构造（整段按文本返回），防病态输入爆栈。
const MAX_DEPTH: u32 = 500;

/// 扫描 `[start, end)` 内的节点序列；遇到 `stops` 中的字节时停在该字节处。
pub fn parse_nodes(
    text: &str,
    start: usize,
    end: usize,
    block: bool,
    stops: &[u8],
) -> (Vec<Node>, Option<(usize, u8)>) {
    let scanner = Scanner {
        text,
        depth: Cell::new(0),
    };
    scanner.nodes(start, end, block, stops)
}

/// 扫描时寻找的闭合标记。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Closer {
    /// `}}`
    Braces2,
    /// `}}}`
    Braces3,
    /// `]]`
    LinkClose,
    /// `]`
    Bracket,
    /// `|}`（表格闭合，不要求行首）
    TableClose,
}

#[derive(Debug)]
struct TagName {
    lower: String,
    end: usize,
}

struct Scanner<'a> {
    text: &'a str,
    depth: Cell<u32>,
}

impl<'a> Scanner<'a> {
    // ------------------------------------------------------------------
    // 基础
    // ------------------------------------------------------------------

    fn b(&self, i: usize) -> u8 {
        self.text.as_bytes().get(i).copied().unwrap_or(0)
    }

    fn line_start(&self, i: usize) -> bool {
        i == 0 || self.text.as_bytes()[i - 1] == b'\n'
    }

    fn brace_run(&self, i: usize, end: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut n = 0;
        while i + n < end && bytes[i + n] == b'{' {
            n += 1;
        }
        n
    }

    fn eq_run(&self, i: usize, end: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut n = 0;
        while i + n < end && bytes[i + n] == b'=' {
            n += 1;
        }
        n
    }

    fn find_sub(&self, from: usize, end: usize, pat: &[u8]) -> Option<usize> {
        let bytes = self.text.as_bytes();
        if pat.is_empty() || end < pat.len() {
            return None;
        }
        (from..=end - pat.len()).find(|&j| &bytes[j..j + pat.len()] == pat)
    }

    fn enter(&self) -> bool {
        let d = self.depth.get();
        if d >= MAX_DEPTH {
            return false;
        }
        self.depth.set(d + 1);
        true
    }

    fn leave(&self) {
        self.depth.set(self.depth.get().saturating_sub(1));
    }

    // ------------------------------------------------------------------
    // 节点解析主循环
    // ------------------------------------------------------------------

    fn nodes(
        &self,
        start: usize,
        end: usize,
        block: bool,
        stops: &[u8],
    ) -> (Vec<Node>, Option<(usize, u8)>) {
        if start >= end {
            return (Vec::new(), None);
        }
        if !self.enter() {
            return (
                vec![Node::Text {
                    text: self.text[start..end].to_string(),
                    start,
                    end,
                }],
                None,
            );
        }
        let mut out: Vec<Node> = Vec::new();
        let mut text_start = start;
        let mut i = start;
        while i < end {
            let c = self.b(i);
            if stops.contains(&c) {
                if text_start < i {
                    out.push(self.text_node(text_start, i));
                }
                self.leave();
                return (out, Some((i, c)));
            }
            if let Some((node, next)) = self.construct(i, end, block) {
                if text_start < i {
                    out.push(self.text_node(text_start, i));
                }
                out.push(node);
                i = next;
                text_start = next;
            } else {
                i += 1;
            }
        }
        if text_start < end {
            out.push(self.text_node(text_start, end));
        }
        self.leave();
        (out, None)
    }

    fn text_node(&self, start: usize, end: usize) -> Node {
        Node::Text {
            text: self.text[start..end].to_string(),
            start,
            end,
        }
    }

    /// `i` 处是否是一个可识别构造；是则返回（节点，构造结束位置）。
    fn construct(&self, i: usize, end: usize, block: bool) -> Option<(Node, usize)> {
        match self.b(i) {
            b'{' if block && self.line_start(i) && self.b(i + 1) == b'|' => self.try_table(i, end),
            b'{' => {
                let run = self.brace_run(i, end);
                if run < 2 {
                    None
                } else if run % 2 == 1 {
                    self.try_param(i, end).or_else(|| self.try_template(i, end))
                } else {
                    let tpl = self.try_template(i, end);
                    if tpl.is_some() {
                        tpl
                    } else if run >= 3 {
                        self.try_param(i, end)
                    } else {
                        None
                    }
                }
            }
            b'[' if self.b(i + 1) == b'[' => self.try_link(i, end),
            b'[' => self.try_extlink(i, end),
            b'<' => self.try_comment(i, end).or_else(|| self.try_tag(i, end)),
            b'=' if block && self.line_start(i) => self.try_heading(i, end),
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // 各构造
    // ------------------------------------------------------------------

    fn try_comment(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        if self.b(i + 1) != b'!' || self.b(i + 2) != b'-' || self.b(i + 3) != b'-' {
            return None;
        }
        match self.find_sub(i + 4, end, b"-->") {
            Some(p) => Some((
                Node::Comment {
                    raw: self.text[i..p + 3].to_string(),
                    start: i,
                    end: p + 3,
                },
                p + 3,
            )),
            None => Some((
                Node::Comment {
                    raw: self.text[i..end].to_string(),
                    start: i,
                    end,
                },
                end,
            )),
        }
    }

    fn try_table(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        if self.b(i + 1) != b'|' {
            return None;
        }
        let (_, past) = self.scan_inline(i + 2, end, Closer::TableClose)?;
        Some((
            Node::Table {
                raw: self.text[i..past].to_string(),
                start: i,
                end: past,
            },
            past,
        ))
    }

    fn try_param(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        let (cs, past) = self.scan_inline(i + 3, end, Closer::Braces3)?;
        let (name, stop) = self.nodes(i + 3, cs, false, b"|");
        let default = stop.map(|(p, _)| self.nodes(p + 1, cs, false, &[]).0);
        Some((
            Node::TemplateParam(TemplateParam {
                name,
                default,
                start: i,
                end: past,
            }),
            past,
        ))
    }

    fn try_template(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        if self.brace_run(i, end) < 2 {
            return None;
        }
        let (cs, past) = self.scan_inline(i + 2, end, Closer::Braces2)?;
        let (name, stop) = self.nodes(i + 2, cs, false, b"|");
        let mut args = Vec::new();
        let mut cursor = stop.map(|(p, _)| p + 1);
        while let Some(p) = cursor {
            let (seg, seg_stop) = self.nodes(p, cs, false, b"|=");
            match seg_stop {
                Some((q, b'=')) => {
                    let (value, vstop) = self.nodes(q + 1, cs, false, b"|");
                    args.push(TplArg {
                        name: seg,
                        value,
                        named: true,
                    });
                    cursor = vstop.map(|(r, _)| r + 1);
                }
                _ => {
                    args.push(TplArg {
                        name: Vec::new(),
                        value: seg,
                        named: false,
                    });
                    cursor = seg_stop.map(|(r, _)| r + 1);
                }
            }
        }
        Some((
            Node::Template(Template {
                name,
                args,
                start: i,
                end: past,
            }),
            past,
        ))
    }

    fn try_link(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        if self.b(i + 1) != b'[' {
            return None;
        }
        let (cs, past) = self.scan_inline(i + 2, end, Closer::LinkClose)?;
        let mut segments = Vec::new();
        let mut cursor = Some(i + 2);
        while let Some(p) = cursor {
            let (seg, stop) = self.nodes(p, cs, false, b"|");
            segments.push(seg);
            cursor = stop.map(|(q, _)| q + 1);
        }
        Some((
            Node::Link(InternalLink {
                segments,
                start: i,
                end: past,
            }),
            past,
        ))
    }

    fn try_extlink(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        let j = i + 1;
        if self.b(j) == b'[' || !self.is_url_start(j) {
            return None;
        }
        let (cs, past) = self.scan_inline(j, end, Closer::Bracket)?;
        let bytes = self.text.as_bytes();
        let url_end = bytes[j..cs]
            .iter()
            .position(|c| c.is_ascii_whitespace())
            .map_or(cs, |k| j + k);
        let mut sep_end = url_end;
        while sep_end < cs && bytes[sep_end].is_ascii_whitespace() {
            sep_end += 1;
        }
        let url = self.text[j..url_end].to_string();
        let sep = self.text[url_end..sep_end].to_string();
        let (label, _) = self.nodes(sep_end, cs, false, &[]);
        Some((
            Node::ExtLink(ExternalLink {
                url,
                sep,
                label,
                start: i,
                end: past,
            }),
            past,
        ))
    }

    /// `[` 之后是否是一个 URL 开头（`scheme://`、白名单 scheme: 或 `//host`）。
    fn is_url_start(&self, j: usize) -> bool {
        let bytes = self.text.as_bytes();
        if bytes.get(j) == Some(&b'/') && bytes.get(j + 1) == Some(&b'/') {
            return true;
        }
        if !bytes.get(j).is_some_and(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        let mut k = j + 1;
        while bytes
            .get(k)
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
        {
            k += 1;
        }
        if bytes.get(k) != Some(&b':') {
            return false;
        }
        if bytes.get(k + 1) == Some(&b'/') && bytes.get(k + 2) == Some(&b'/') {
            return true;
        }
        let scheme = bytes[j..k].to_ascii_lowercase();
        matches!(
            &scheme[..],
            b"mailto"
                | b"irc"
                | b"ircs"
                | b"tel"
                | b"gopher"
                | b"sms"
                | b"news"
                | b"xmpp"
                | b"magnet"
        )
    }

    fn try_heading(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        let level = self.eq_run(i, end);
        if level == 0 || level > 6 {
            return None;
        }
        let bytes = self.text.as_bytes();
        let mut le = i;
        while le < end && bytes[le] != b'\n' {
            le += 1;
        }
        // 闭合 run：从行尾回溯（允许 = 后跟空白/CR）
        let mut ws_end = le;
        while ws_end > i + level && matches!(bytes[ws_end - 1], b' ' | b'\t' | b'\r') {
            ws_end -= 1;
        }
        let mut close = 0usize;
        while ws_end > i + level + close && bytes[ws_end - 1 - close] == b'=' {
            close += 1;
        }
        if close == 0 || close > 6 {
            return None;
        }
        let (title, _) = self.nodes(i + level, ws_end - close, false, &[]);
        Some((
            Node::Heading(Heading {
                level,
                close_level: close,
                trailing_ws: self.text[ws_end..le].to_string(),
                title,
                start: i,
                end: le,
            }),
            le,
        ))
    }

    fn try_tag(&self, i: usize, end: usize) -> Option<(Node, usize)> {
        if self.b(i + 1) == b'/' {
            // 孤立闭合标签
            let name = self.tag_name(i + 2)?;
            let gt = self.find_attr_end(name.end, end)?;
            return Some((
                Node::Tag(Tag {
                    name: name.lower,
                    raw_open: self.text[i..gt + 1].to_string(),
                    content: TagContent::Empty,
                    raw_close: String::new(),
                    start: i,
                    end: gt + 1,
                }),
                gt + 1,
            ));
        }
        let name = self.tag_name(i + 1)?;
        let gt = self.find_attr_end(name.end, end)?;
        let open_end = gt + 1;
        let raw_open = self.text[i..open_end].to_string();
        let self_closing = self.b(gt - 1) == b'/';
        if self_closing || VOID_TAGS.contains(&name.lower.as_str()) {
            return Some((
                Node::Tag(Tag {
                    name: name.lower,
                    raw_open,
                    content: TagContent::Empty,
                    raw_close: String::new(),
                    start: i,
                    end: open_end,
                }),
                open_end,
            ));
        }
        if RAW_CONTENT_TAGS.contains(&name.lower.as_str()) {
            return match self.find_close_tag(open_end, end, &name.lower, false) {
                Some((cs, ce)) => Some((
                    Node::Tag(Tag {
                        name: name.lower,
                        raw_open,
                        content: TagContent::Raw(self.text[open_end..cs].to_string()),
                        raw_close: self.text[cs..ce].to_string(),
                        start: i,
                        end: ce,
                    }),
                    ce,
                )),
                // 未闭合：开标签降级为文本
                None => Some((self.text_node(i, open_end), open_end)),
            };
        }
        match self.find_close_tag(open_end, end, &name.lower, true) {
            Some((cs, ce)) => {
                let (children, _) = self.nodes(open_end, cs, true, &[]);
                Some((
                    Node::Tag(Tag {
                        name: name.lower,
                        raw_open,
                        content: TagContent::Parsed(children),
                        raw_close: self.text[cs..ce].to_string(),
                        start: i,
                        end: ce,
                    }),
                    ce,
                ))
            }
            // 未闭合：开标签降级为文本，内部继续正常解析
            None => Some((self.text_node(i, open_end), open_end)),
        }
    }

    // ------------------------------------------------------------------
    // 标签辅助
    // ------------------------------------------------------------------

    fn tag_name(&self, pos: usize) -> Option<TagName> {
        let bytes = self.text.as_bytes();
        if !bytes.get(pos).is_some_and(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let mut k = pos + 1;
        while bytes.get(k).is_some_and(|c| c.is_ascii_alphanumeric()) {
            k += 1;
        }
        Some(TagName {
            lower: self.text[pos..k].to_ascii_lowercase(),
            end: k,
        })
    }

    /// 从 `pos` 扫到标签的 `>`（引号内的 `>` 不算），返回其下标。
    fn find_attr_end(&self, pos: usize, end: usize) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let mut quote = 0u8;
        let mut k = pos;
        while k < end {
            let c = bytes[k];
            if quote != 0 {
                if c == quote {
                    quote = 0;
                }
            } else if c == b'"' || c == b'\'' {
                quote = c;
            } else if c == b'>' {
                return Some(k);
            }
            k += 1;
        }
        None
    }

    /// 找 `</name>`；`depth_count` 时按同名开/闭标签深度配对。
    /// 返回（闭标签起点，闭标签结束后的下标）。
    fn find_close_tag(
        &self,
        from: usize,
        end: usize,
        name: &str,
        depth_count: bool,
    ) -> Option<(usize, usize)> {
        let bytes = self.text.as_bytes();
        let nb = name.as_bytes();
        let mut depth = 1u32;
        let mut j = from;
        while j < end {
            if bytes[j] == b'<' {
                if bytes.get(j + 1) == Some(&b'/') && self.name_matches(j + 2, nb) {
                    let gt = self.find_attr_end(j + 2 + nb.len(), end)?;
                    if !depth_count || depth == 1 {
                        return Some((j, gt + 1));
                    }
                    depth -= 1;
                    j = gt + 1;
                    continue;
                }
                if depth_count && self.name_matches(j + 1, nb) {
                    depth += 1;
                    j += 1 + nb.len();
                    continue;
                }
            }
            j += 1;
        }
        None
    }

    /// `j` 处开始是否是标签名 `name`（后跟边界字符）。
    fn name_matches(&self, j: usize, name: &[u8]) -> bool {
        let bytes = self.text.as_bytes();
        if j + name.len() > bytes.len() || !bytes[j..j + name.len()].eq_ignore_ascii_case(name) {
            return false;
        }
        matches!(
            bytes.get(j + name.len()),
            None | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
        )
    }

    // ------------------------------------------------------------------
    // 扫描（寻找闭合，跳过嵌套构造；不建节点）
    // ------------------------------------------------------------------

    fn scan_inline(&self, start: usize, end: usize, closer: Closer) -> Option<(usize, usize)> {
        if !self.enter() {
            return None;
        }
        let result = self.scan_inner(start, end, closer);
        self.leave();
        result
    }

    fn scan_inner(&self, start: usize, end: usize, closer: Closer) -> Option<(usize, usize)> {
        let bytes = self.text.as_bytes();
        let mut j = start;
        while j < end {
            if let Some(past) = self.closer_at(closer, j) {
                return Some((j, past));
            }
            match bytes[j] {
                b'{' => {
                    let run = self.brace_run(j, end);
                    let mut past = None;
                    if run >= 3 && run % 2 == 1 {
                        past = self
                            .scan_inline(j + 3, end, Closer::Braces3)
                            .map(|p| p.1)
                            .or_else(|| self.scan_inline(j + 2, end, Closer::Braces2).map(|p| p.1));
                    } else if run >= 2 {
                        past = self
                            .scan_inline(j + 2, end, Closer::Braces2)
                            .map(|p| p.1)
                            .or_else(|| {
                                if run >= 3 {
                                    self.scan_inline(j + 3, end, Closer::Braces3).map(|p| p.1)
                                } else {
                                    None
                                }
                            });
                    }
                    if let Some(p) = past {
                        j = p;
                        continue;
                    }
                    // 嵌套表格：只在外层寻找表格闭合时跳转
                    if closer == Closer::TableClose
                        && self.line_start(j)
                        && bytes.get(j + 1) == Some(&b'|')
                    {
                        if let Some(p) = self
                            .scan_inline(j + 2, end, Closer::TableClose)
                            .map(|p| p.1)
                        {
                            j = p;
                            continue;
                        }
                    }
                    j += 1;
                }
                b'[' => {
                    // 链接在第一个 ]] 闭合：扫链接内容时不下钻嵌套 [[
                    if closer != Closer::LinkClose && bytes.get(j + 1) == Some(&b'[') {
                        if let Some(p) =
                            self.scan_inline(j + 2, end, Closer::LinkClose).map(|p| p.1)
                        {
                            j = p;
                            continue;
                        }
                    }
                    j += 1;
                }
                b'<' => {
                    if self.is_comment_start(j) {
                        // 未闭合注释吞掉余下范围，闭合标记不可能再出现
                        let p = self.find_sub(j + 4, end, b"-->")?;
                        j = p + 3;
                        continue;
                    }
                    if let Some(p) = self.tag_end(j, end) {
                        j = p;
                        continue;
                    }
                    j += 1;
                }
                _ => j += 1,
            }
        }
        None
    }

    fn closer_at(&self, closer: Closer, j: usize) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let at = |k: usize, c: u8| bytes.get(k) == Some(&c);
        match closer {
            Closer::Braces2 if at(j, b'}') && at(j + 1, b'}') => Some(j + 2),
            Closer::Braces3 if at(j, b'}') && at(j + 1, b'}') && at(j + 2, b'}') => Some(j + 3),
            Closer::LinkClose if at(j, b']') && at(j + 1, b']') => Some(j + 2),
            Closer::Bracket if at(j, b']') => Some(j + 1),
            Closer::TableClose if at(j, b'|') && at(j + 1, b'}') => Some(j + 2),
            _ => None,
        }
    }

    fn is_comment_start(&self, i: usize) -> bool {
        self.b(i + 1) == b'!' && self.b(i + 2) == b'-' && self.b(i + 3) == b'-'
    }

    /// 供扫描跳转：`i` 处是完整标签（`<`），返回其结束后的下标。
    /// 未闭合的配对标签返回 `None`（调用方退为单字符推进）。
    fn tag_end(&self, i: usize, end: usize) -> Option<usize> {
        if self.b(i + 1) == b'/' {
            let name = self.tag_name(i + 2)?;
            let gt = self.find_attr_end(name.end, end)?;
            return Some(gt + 1);
        }
        let name = self.tag_name(i + 1)?;
        let gt = self.find_attr_end(name.end, end)?;
        if self.b(gt - 1) == b'/' || VOID_TAGS.contains(&name.lower.as_str()) {
            return Some(gt + 1);
        }
        let deep = !RAW_CONTENT_TAGS.contains(&name.lower.as_str());
        self.find_close_tag(gt + 1, end, &name.lower, deep)
            .map(|(_, ce)| ce)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Wikicode;

    /// 语料全量往返测试：语料目录存在时，每页必须满足
    /// `parse().serialize() == 原文` 且二次解析树相等（幂等）。
    /// 语料（`wikis/`）被 gitignore，缺失时跳过。
    #[test]
    fn corpus_round_trip() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("wikis/dontstarve.huijiwiki.com/pages");
        if !dir.is_dir() {
            eprintln!("语料不存在，跳过 corpus_round_trip");
            return;
        }
        let mut pages = 0u32;
        let mut failed = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("读取语料目录") {
            let path = entry.expect("读取目录项").path();
            if path.extension().and_then(|e| e.to_str()) != Some("wikitext") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("读取 {}: {e}", path.display());
            });
            let code = Wikicode::parse(&text);
            let out = code.serialize();
            if out != text {
                if let Some(pos) = text.bytes().zip(out.bytes()).position(|(a, b)| a != b) {
                    failed.push(format!(
                        "{} 首个差异 @{pos}: 原文 {:?} vs 输出 {:?}",
                        path.file_name().unwrap().to_string_lossy(),
                        &text[pos.saturating_sub(20)..(pos + 20).min(text.len())],
                        &out[pos.saturating_sub(20)..(pos + 20).min(out.len())],
                    ));
                } else {
                    failed.push(format!(
                        "{} 长度不同: {} vs {}",
                        path.display(),
                        text.len(),
                        out.len()
                    ));
                }
                continue;
            }
            let reparsed = Wikicode::parse(&out);
            if reparsed != code {
                failed.push(format!("{} 二次解析不相等", path.display()));
                continue;
            }
            pages += 1;
        }
        assert!(
            failed.is_empty(),
            "往返失败 {} 页:\n{}",
            failed.len(),
            failed.join("\n")
        );
        assert!(pages > 100, "语料页数异常: {pages}");
        eprintln!("corpus_round_trip: {pages} 页全部通过");
    }
}
