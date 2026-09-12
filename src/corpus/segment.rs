//! PageSegmenter: cuts a page into stable, non-overlapping byte regions
//! (docs/CORPUS_CODE_ATLAS_CONTRACT.md §2).
//!
//! Model (docs/CORPUS_PAGE_PATTERNS.md §7): the untitled lead zone (template
//! stack + identity formula + mechanic summary) before the first h2, one
//! region per h2 section (h3 folded into its parent), plus one region per
//! entity-infobox instance inside the lead zone (`tab` when the page carries
//! several variants, `infobox` for a single one).
//!
//! Stability rules: an id binds to structural position only — editing text
//! updates the FNV-1a hash, not the id; inserting/removing earlier structure
//! shifts subsequent ordinals, so consumers match across runs via
//! `(pageid, kind, title)` instead.

use serde::Serialize;

/// One non-overlapping slice of a page.
#[derive(Debug, Clone, Serialize)]
pub struct Region {
    /// `{pageid}:{kind}:{ordinal}` — ordinal counts within the same kind.
    pub id: String,
    /// `intro` | `infobox` | `tab` | `section`.
    pub kind: &'static str,
    /// Section heading (unwrapped from `{{标题|…}}`) or tab variant name.
    pub title: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    /// FNV-1a 64 of the slice content — deterministic across runs/releases.
    pub hash: u64,
    /// Named-parameter value anchors inside `infobox`/`tab` regions
    /// (absolute byte spans, same coordinate system as the region). Empty
    /// for other kinds. Lets F1-class extractors anchor old-value lookups
    /// to the exact `\|掉落 = …` bytes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamSpan>,
}

/// Byte span of one named parameter's *value* inside an infobox template.
#[derive(Debug, Clone, Serialize)]
pub struct ParamSpan {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub hash: u64,
}

/// FNV-1a 64-bit. Chosen over `DefaultHasher` because the std one makes no
/// stability guarantees across releases.
pub fn fnv1a64(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Head bytes of entity-infobox templates carrying a prefab argument.
const INFOBOX_HEADS: &[&str] = &["{{实体信息框/自动|dst|", "{{实体信息框|dst|"];

/// Unwraps `{{标题|X}}` → `X`; returns other titles verbatim.
fn unwrap_title(raw: &str) -> String {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("{{标题|") {
        rest.strip_suffix("}}").unwrap_or(rest).trim().to_string()
    } else {
        t.to_string()
    }
}

/// All h2 headings (`==…==`, exactly two `=`) from `from` on, as
/// `(line_start_offset, unwrapped_title)`.
fn h2_lines(text: &str, from: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut scan = |line_start: usize| {
        let rest = &text[line_start..];
        if !rest.starts_with("==") || rest.starts_with("===") {
            return;
        }
        let Some(nl) = rest.find('\n') else {
            return;
        };
        // Line body between the opening `==` and the closing `\n==`.
        let inner = rest[2..nl].trim_end();
        let Some(title_raw) = inner.strip_suffix("==") else {
            return;
        };
        out.push((line_start, unwrap_title(title_raw)));
    };
    if text.len() > from {
        scan(from);
    }
    for (i, _) in text[from..].match_indices('\n') {
        scan(from + i + 1);
    }
    out
}

/// One entity-infobox instance inside the lead zone.
struct InfoboxSpan {
    start: usize,
    end: usize,
    variant: String,
    params: Vec<ParamSpan>,
}

/// 命名参数值 span（解析器版）：与旧实现相同的取值语义——跳过 `=` 后的
/// 空格/制表符、值尾 trim；空值不产出锚点。
fn parser_infobox_params(text: &str, t: &crate::wikitext::Template) -> Vec<ParamSpan> {
    let mut out = Vec::new();
    for arg in &t.args {
        if !arg.named {
            continue;
        }
        let name = crate::wikitext::plain(&arg.name).trim().to_string();
        if name.is_empty() {
            continue;
        }
        let Some((vs, ve)) = crate::wikitext::nodes_span(&arg.value) else {
            continue;
        };
        let raw = &text[vs..ve];
        let lead = raw.len() - raw.trim_start_matches([' ', '\t']).len();
        let trimmed = raw[lead..].trim_end();
        if trimmed.is_empty() {
            continue;
        }
        out.push(ParamSpan {
            name,
            start_byte: vs + lead,
            end_byte: vs + lead + trimmed.len(),
            hash: fnv1a64(trimmed),
        });
    }
    out
}

/// 变体名提取：第 2 个位置参数的值，去注释、截断到行首、trim。
/// （旧行扫描对 variant 的取法等价于此；空变体返回 `None`。）
fn infobox_variant(t: &crate::wikitext::Template) -> Option<String> {
    let arg = t.args.iter().filter(|a| !a.named).nth(1)?;
    let mut s = crate::wikitext::plain(&arg.value);
    while let Some(a) = s.find("<!--") {
        match s[a..].find("-->") {
            Some(rel) => s.replace_range(a..a + rel + 3, ""),
            None => break,
        }
    }
    let s = s.split('\n').next().unwrap_or("").trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// wikitext 解析器驱动的信息框识别（取代旧的字面头匹配 + 行式参数扫描）。
///
/// 识别条件与旧 `INFOBOX_HEADS` 等价：模板名归一化匹配
/// `实体信息框/自动` / `实体信息框`，首个位置参数为 `dst`，且带第 2 个
/// 位置参数（变体名，非空）。只取引言区（首个 h2 之前）的实例。
fn parser_infoboxes(text: &str, zone_end: usize) -> Vec<InfoboxSpan> {
    let code = crate::wikitext::Wikicode::parse(text);
    let mut out: Vec<InfoboxSpan> = Vec::new();
    for t in code.templates() {
        if t.start >= zone_end {
            continue;
        }
        if !(t.is_named("实体信息框/自动") || t.is_named("实体信息框")) {
            continue;
        }
        if t.positional(1).as_deref() != Some("dst") {
            continue;
        }
        let Some(variant) = infobox_variant(t) else {
            continue;
        };
        let params = parser_infobox_params(text, t);
        out.push(InfoboxSpan {
            start: t.start,
            end: t.end,
            variant,
            params,
        });
    }
    out.sort_by_key(|s| s.start);
    out.dedup_by(|a, b| a.start == b.start);
    out
}

// ---------------------------------------------------------------------------
// 旧实现（字面头匹配 + 行式参数扫描）。迁移期保留供语料差分对比
// （`differential_old_vs_new_infobox_extraction`），稳定后删除。
// ---------------------------------------------------------------------------

/// End offset of the balanced `{{…}}` template starting at `open` (pointing
/// at the first `{`). Falls back to the zone end when braces never balance
/// (malformed corpus).
#[cfg_attr(not(test), allow(dead_code))]
fn template_end(text: &str, open: usize, zone_end: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = open;
    while i + 1 < text.len() && i < zone_end {
        match &bytes[i..i + 2] {
            b"{{" => {
                depth += 1;
                i += 2;
            }
            b"}}" => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    zone_end
}

/// Named-parameter value spans inside one infobox template body.
///
/// Line-oriented: a parameter starts at a line beginning `\|name =`; its
/// value runs to the next parameter line, the closing `}}` line, or the
/// span end (so wrapped multi-line values stay inside one anchor). The
/// positional head (`\|dst\|<prefab>`) is not a named parameter and yields
/// nothing.
#[cfg_attr(not(test), allow(dead_code))]
fn infobox_params(text: &str, start: usize, end: usize) -> Vec<ParamSpan> {
    let mut line_starts = vec![start];
    for (i, _) in text[start..end].match_indices('\n') {
        line_starts.push(start + i + 1);
    }

    /// Classifies one line: `Some((name, value_start))` for `\|name = …`,
    /// `None` otherwise.
    fn param_line(text: &str, ls: usize, end: usize) -> Option<(String, usize)> {
        let le = text[ls..end].find('\n').map(|r| ls + r).unwrap_or(end);
        let line = text.get(ls..le)?;
        let rest = line.strip_prefix('|')?;
        if rest.starts_with('}') {
            return None;
        }
        let eq = rest.find('=')?;
        let name = rest[..eq].trim();
        if name.is_empty() || name.contains('|') || name.contains('}') {
            return None;
        }
        // Value starts right after '=', skipping blanks (char-boundary safe).
        let mut vs = ls + 1 + eq + 1;
        while vs < le {
            let c = text[vs..].chars().next()?;
            if c == ' ' || c == '\t' {
                vs += c.len_utf8();
            } else {
                break;
            }
        }
        Some((name.to_string(), vs))
    }

    let params_at: Vec<Option<(String, usize)>> = line_starts
        .iter()
        .map(|&ls| param_line(text, ls, end))
        .collect();

    let mut out = Vec::new();
    for (idx, entry) in params_at.iter().enumerate() {
        let Some((name, value_start)) = entry else {
            continue;
        };
        // Value ends at the next param/closing line, or the span end.
        let value_end = params_at[idx + 1..]
            .iter()
            .zip(line_starts[idx + 1..].iter().copied())
            .find(|(p, ls)| p.is_some() || text[*ls..].starts_with("}}"))
            .map(|(_, ls)| ls)
            .unwrap_or(end)
            .min(end);
        let trimmed = text[*value_start..value_end].trim_end();
        let value_end = value_start + trimmed.len();
        if value_end <= *value_start {
            continue;
        }
        out.push(ParamSpan {
            name: name.clone(),
            start_byte: *value_start,
            end_byte: value_end,
            hash: fnv1a64(trimmed),
        });
    }
    out
}

#[cfg_attr(not(test), allow(dead_code))]
fn infobox_spans(text: &str, zone_end: usize) -> Vec<InfoboxSpan> {
    let mut spans: Vec<InfoboxSpan> = Vec::new();
    for head in INFOBOX_HEADS {
        let mut from = 0usize;
        while let Some(rel) = text[from..zone_end].find(head) {
            let open = from + rel;
            let arg_start = open + head.len();
            let arg: String = text[arg_start..]
                .chars()
                .take_while(|c| !matches!(c, '|' | '}' | '\n' | '['))
                .collect();
            let end = template_end(text, open, zone_end);
            spans.push(InfoboxSpan {
                start: open,
                end,
                variant: arg.trim().to_string(),
                params: infobox_params(text, open, end),
            });
            from = arg_start;
        }
    }
    spans.sort_by_key(|s| s.start);
    spans.dedup_by(|a, b| a.start == b.start);
    spans
}

/// Segments one page into ordered, non-overlapping regions covering the
/// whole document.
pub fn segment(pageid: i64, wikitext: &str) -> Vec<Region> {
    let headings = h2_lines(wikitext, 0);
    let zone_end = headings.first().map(|&(s, _)| s).unwrap_or(wikitext.len());
    let spans = parser_infoboxes(wikitext, zone_end);
    let multi_tab = spans.len() > 1;

    let mut out: Vec<Region> = Vec::new();
    let mut counters: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    let mut push = |out: &mut Vec<Region>,
                    kind: &'static str,
                    title: Option<String>,
                    start: usize,
                    end: usize,
                    params: Vec<ParamSpan>| {
        if end <= start || wikitext[start..end].trim().is_empty() {
            return;
        }
        let n = counters.entry(kind).or_insert(0);
        *n += 1;
        out.push(Region {
            id: format!("{pageid}:{kind}:{}", *n),
            kind,
            title,
            start_byte: start,
            end_byte: end,
            hash: fnv1a64(&wikitext[start..end]),
            params,
        });
    };

    // Lead zone: alternate prose fragments and infobox instances.
    let mut cursor = 0usize;
    for span in &spans {
        push(&mut out, "intro", None, cursor, span.start, Vec::new());
        let kind = if multi_tab { "tab" } else { "infobox" };
        let title = if multi_tab {
            Some(span.variant.clone())
        } else {
            None
        };
        let stop = span.end.max(span.start);
        push(&mut out, kind, title, span.start, stop, span.params.clone());
        cursor = stop;
    }
    push(&mut out, "intro", None, cursor, zone_end, Vec::new());

    // h2 sections; h3 folded into the parent. Contiguous through EOF.
    for (idx, &(start, ref title)) in headings.iter().enumerate() {
        let end = headings
            .get(idx + 1)
            .map(|&(s, _)| s)
            .unwrap_or(wikitext.len());
        push(
            &mut out,
            "section",
            Some(title.clone()),
            start,
            end,
            Vec::new(),
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_known_vectors() {
        assert_eq!(fnv1a64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn rich_tab_page_splits_tabs_intro_and_sections() {
        let page = "{{置顶导航}}\n\
                    {{RichTab/信息框\n|普通猎犬|\n{{实体信息框/自动|dst|hound\n|掉落 = x}}\n\
                    |火焰猎犬|\n{{实体信息框/自动|dst|firehound}}\n}}\n\
                    '''猎犬'''是联机版中的一种生物。\n\
                    =={{标题|行为}}==\n成群袭击玩家。\n===战斗===\n硬直。\n\
                    =={{标题|花絮}}==\n改名史。";
        let regions = segment(13857, page);
        let kinds: Vec<&str> = regions.iter().map(|r| r.kind).collect();
        // The `|火焰猎犬|` RichTab label line between the two infoboxes
        // surfaces as a tiny intro fragment — honest byte-partition.
        assert_eq!(
            kinds,
            vec!["intro", "tab", "intro", "tab", "intro", "section", "section"]
        );
        assert_eq!(regions[1].title.as_deref(), Some("hound"));
        assert_eq!(regions[3].title.as_deref(), Some("firehound"));
        assert_eq!(regions[5].title.as_deref(), Some("行为"));
        // Contiguity: regions tile the whole document without gaps.
        let mut cursor = 0;
        for r in &regions {
            assert_eq!(r.start_byte, cursor, "gap before {}", r.id);
            assert!(r.end_byte > r.start_byte);
            cursor = r.end_byte;
        }
        assert_eq!(cursor, page.len());
        // h3 folded into 行为: its body lives inside the section region.
        let behavior = &regions[5];
        assert!(page[behavior.start_byte..behavior.end_byte].contains("硬直。"));
        // Ids follow {pageid}:{kind}:{ordinal}.
        assert_eq!(regions[1].id, "13857:tab:1");
        assert_eq!(regions[3].id, "13857:tab:2");
        assert_eq!(regions[6].id, "13857:section:2");
        // Hashes are content-bound.
        assert_ne!(regions[1].hash, regions[3].hash);
    }

    #[test]
    fn single_infobox_page_and_no_heading_page() {
        let page = "{{实体信息框/自动|dst|tentacle\n|生成自 = y}}\n正文一段。\n另一段。";
        let regions = segment(13379, page);
        let kinds: Vec<&str> = regions.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec!["infobox", "intro"]);
        assert_eq!(regions[0].id, "13379:infobox:1");

        let plain = "没有任何章节的机制页。";
        let regions = segment(7, plain);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].kind, "intro");
        assert_eq!(regions[0].start_byte, 0);
        assert_eq!(regions[0].end_byte, plain.len());
    }

    #[test]
    fn page_starting_with_heading_is_handled() {
        let regions = segment(5, "==获取==\n只能由烹饪产生。");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].kind, "section");
        assert_eq!(regions[0].title.as_deref(), Some("获取"));
        assert_eq!(regions[0].start_byte, 0);
    }

    #[test]
    fn bare_h2_titles_are_kept_verbatim() {
        let page = "前言。\n==获取==\n只能由烹饪产生。";
        let regions = segment(9, page);
        assert_eq!(regions[0].kind, "intro");
        assert_eq!(regions[1].kind, "section");
        assert_eq!(regions[1].title.as_deref(), Some("获取"));
    }

    #[test]
    fn infobox_params_anchor_named_values_with_hashes() {
        let page = "{{实体信息框/自动|dst|tentacle\n\
                    |掉落 = [[File:Monster_Meat.png|28px|link=怪物肉]]×2<br>×1（50%）\n\
                    |生成自 = [[File:On_Tentacles.png|28px|link=触手的召唤]]\n\
                    }}\n正文。";
        let regions = segment(13379, page);
        let tab = regions
            .iter()
            .find(|r| r.kind == "infobox")
            .expect("infobox region");
        let names: Vec<&str> = tab.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["掉落", "生成自"]);
        let loot = &tab.params[0];
        let value = &page[loot.start_byte..loot.end_byte];
        assert!(
            value.contains("Monster_Meat") && value.ends_with("（50%）"),
            "{value:?}"
        );
        assert_eq!(loot.hash, fnv1a64(value));
        // Positional head (dst|tentacle) is not a named param.
        assert!(!names.contains(&"dst"));
    }

    #[test]
    fn wrapped_multiline_values_stay_in_one_anchor() {
        let page = "{{实体信息框/自动|dst|x\n\
                    |掉落 = 第一行<br>\n第二行继续\n\
                    |生成自 = y\n}}";
        let regions = segment(1, page);
        let tab = regions.iter().find(|r| r.kind == "infobox").unwrap();
        assert_eq!(tab.params.len(), 2);
        let v = &page[tab.params[0].start_byte..tab.params[0].end_byte];
        assert!(v.contains("第一行") && v.contains("第二行继续"), "{v:?}");
    }

    /// 语料差分：旧（字面头匹配 + 行式扫描）与新（wikitext 解析器）的
    /// 信息框提取对比。旧实现的行式启发在单行多参数、值含嵌套模板换行、
    /// 行内参数等形态上会把相邻参数/闭合括号吞进值里；断言允许且仅允许
    /// 这类"旧值 = 新值 ± 纯语法片段"的偏差，其余一律失败。
    /// 语料缺失时跳过。
    #[test]
    fn differential_old_vs_new_infobox_extraction() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("wikis/dontstarve.huijiwiki.com/pages");
        if !dir.is_dir() {
            eprintln!("语料不存在，跳过差分测试");
            return;
        }
        let mut pages = 0u32;
        let mut old_total = 0usize;
        let mut new_total = 0usize;
        let mut issues: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("读取语料目录") {
            let path = entry.expect("读取目录项").path();
            if path.extension().and_then(|e| e.to_str()) != Some("wikitext") {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("读取 {}: {e}", path.display()));
            pages += 1;
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            let zone_end = h2_lines(&text, 0)
                .first()
                .map(|&(s, _)| s)
                .unwrap_or(text.len());
            let old_spans = infobox_spans(&text, zone_end);
            let new_spans = parser_infoboxes(&text, zone_end);
            old_total += old_spans.len();
            new_total += new_spans.len();
            let new_code = crate::wikitext::Wikicode::parse(&text);
            let new_by_start: std::collections::HashMap<usize, &InfoboxSpan> =
                new_spans.iter().map(|s| (s.start, s)).collect();
            for o in &old_spans {
                // 旧实现的垃圾条目：空变体（`|dst|` 后直接换行/闭括号）、
                // 命名参数被当作变体（`|dst|分类=矿石`）、变体带 HTML 注释
                // （旧字面扫描不认识注释）——新提取器有意跳过或剥注释
                if o.variant.is_empty() || o.variant.contains('=') || o.variant.contains("<!--") {
                    continue;
                }
                let Some(n) = new_by_start.get(&o.start) else {
                    issues.push(format!(
                        "{file}: 旧有新无 @{} variant={:?}",
                        o.start, o.variant
                    ));
                    continue;
                };
                if o.variant != n.variant {
                    issues.push(format!(
                        "{file}: variant 不同 @{}: {:?} vs {:?}",
                        o.start, o.variant, n.variant
                    ));
                }
                if o.end != n.end {
                    issues.push(format!(
                        "{file}: span end 不同 @{}: {} vs {}",
                        o.start, o.end, n.end
                    ));
                }
                // 覆盖性判定：旧参数的字节范围必须被新模板各参数的
                // 名字/值 span 完全覆盖（旧行扫描会把同行兄弟参数或
                // 闭合括号吞进值里）；未覆盖处只允许分隔语法。
                let new_tpl = new_code
                    .templates()
                    .into_iter()
                    .find(|t| t.start == o.start);
                let arg_spans: Vec<(usize, usize)> = new_tpl
                    .map(|t| {
                        t.args
                            .iter()
                            .flat_map(|a| {
                                [
                                    crate::wikitext::nodes_span(&a.name),
                                    crate::wikitext::nodes_span(&a.value),
                                ]
                            })
                            .flatten()
                            .collect()
                    })
                    .unwrap_or_default();
                for p in &o.params {
                    let mut uncovered: Vec<(usize, char)> = Vec::new();
                    for (i, c) in text[p.start_byte..p.end_byte].char_indices() {
                        let pos = p.start_byte + i;
                        if arg_spans.iter().any(|(a, z)| pos >= *a && pos < *z) {
                            continue;
                        }
                        if matches!(c, '|' | '=' | '{' | '}') || c.is_whitespace() {
                            continue;
                        }
                        uncovered.push((pos, c));
                    }
                    if !uncovered.is_empty() {
                        let oval = &text[p.start_byte..p.end_byte];
                        issues.push(format!(
                            "{file}: 旧参数 {:?} 值含无法覆盖的内容 @{}: {:?}（首处 {:?} @{}）",
                            p.name, p.start_byte, oval, uncovered[0].1, uncovered[0].0
                        ));
                    }
                }
            }
        }
        eprintln!(
            "差分：{pages} 页，旧 {old_total} 个信息框，新 {new_total} 个，问题 {} 个",
            issues.len()
        );
        assert!(pages > 100, "语料页数异常: {pages}");
        assert!(
            new_total >= old_total,
            "新提取器信息框数不应少于旧实现: {new_total} < {old_total}"
        );
        assert!(
            issues.is_empty(),
            "差分问题 {} 个:\n{}",
            issues.len(),
            issues
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
