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

/// End offset of the balanced `{{…}}` template starting at `open` (pointing
/// at the first `{`). Falls back to the zone end when braces never balance
/// (malformed corpus).
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

/// One entity-infobox instance inside the lead zone.
struct InfoboxSpan {
    start: usize,
    end: usize,
    variant: String,
}

/// Named-parameter value spans inside one infobox template body.
///
/// Line-oriented: a parameter starts at a line beginning `\|name =`; its
/// value runs to the next parameter line, the closing `}}` line, or the
/// span end (so wrapped multi-line values stay inside one anchor). The
/// positional head (`\|dst\|<prefab>`) is not a named parameter and yields
/// nothing.
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

    let params_at: Vec<Option<(String, usize)>> =
        line_starts.iter().map(|&ls| param_line(text, ls, end)).collect();

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
            spans.push(InfoboxSpan {
                start: open,
                end: template_end(text, open, zone_end),
                variant: arg.trim().to_string(),
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
    let spans = infobox_spans(wikitext, zone_end);
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
        let params = infobox_params(wikitext, span.start, stop);
        push(&mut out, kind, title, span.start, stop, params);
        cursor = stop;
    }
    push(&mut out, "intro", None, cursor, zone_end, Vec::new());

    // h2 sections; h3 folded into the parent. Contiguous through EOF.
    for (idx, &(start, ref title)) in headings.iter().enumerate() {
        let end = headings
            .get(idx + 1)
            .map(|&(s, _)| s)
            .unwrap_or(wikitext.len());
        push(&mut out, "section", Some(title.clone()), start, end, Vec::new());
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
        assert!(value.contains("Monster_Meat") && value.ends_with("（50%）"), "{value:?}");
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
}
