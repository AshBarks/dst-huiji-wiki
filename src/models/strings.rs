//! `模块:Strings` 数据模型。
//!
//! 维基端 [`模块:Strings`] 以 `Data:<V>_Strings_Index.json` 的边界数组二分
//! 定位桶，再 `mw.loadData("模块:<V> Strings <LANG> <NN>")` 取数据。桶页
//! 形如：
//!
//! ```lua
//! return {
//!   ["ACTIONS.ABANDON"] = "遗弃",
//!   ["CHARACTERS.DESCRIBE.BERRYBUSH.GENERIC"] = {
//!     ["wilson"] = "我觉得这些应该能吃。",
//!   },
//! }
//! ```
//!
//! 本模块负责：PO `msgctxt` → 站内 key 的纯变换、按桶拆分、索引构造、
//! Lua 渲染/转义与新旧数据对比。不涉及 I/O 与维基访问。

use super::po::PoEntry;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 桶页里一个顶层 key 的值：普通字符串，或 `CHARACTERS.*` 的角色表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringValue {
    Text(String),
    Characters(BTreeMap<String, String>),
}

impl StringValue {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            StringValue::Text(text) => Some(text),
            StringValue::Characters(_) => None,
        }
    }

    pub fn as_characters(&self) -> Option<&BTreeMap<String, String>> {
        match self {
            StringValue::Text(_) => None,
            StringValue::Characters(table) => Some(table),
        }
    }
}

/// `msgctxt` 变换结果：站内顶层 key + 可选的说话者代码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiStringKey {
    pub key: String,
    /// `CHARACTERS.*` 条目：小写角色代码（`GENERIC` → `wilson`）。
    pub character: Option<String>,
}

/// `STRINGS.CHARACTERS.<SPEAKER>.<REST>` 的说话者代码。
///
/// 游戏 PO 用 `GENERIC` 表示默认台词，站内 `模块:Strings` 的
/// `resolve_character` 把它映射成 `wilson`，数据表键必须一致。
pub fn speaker_code(speaker: &str) -> String {
    if speaker.eq_ignore_ascii_case("GENERIC") {
        "wilson".to_string()
    } else {
        speaker.to_ascii_lowercase()
    }
}

/// 把 PO `msgctxt` 变换为站内 key；非 `STRINGS.` 前缀返回 `None`。
///
/// 站内数据把整条路径规范化为 ASCII 大写（`模块:Strings` 的调用方也按
/// 大写传参），例如 `STRINGS.LUCY.beaver_down_early.1` →
/// `LUCY.BEAVER_DOWN_EARLY.1`；`CHARACTERS.*` 的说话者段移入小写角色表键。
pub fn wiki_string_key(msgctxt: &str) -> Option<WikiStringKey> {
    let rest = msgctxt.strip_prefix("STRINGS.")?;
    if let Some(chars) = rest.strip_prefix("CHARACTERS.") {
        let (speaker, path) = chars.split_once('.')?;
        if path.is_empty() {
            return None;
        }
        return Some(WikiStringKey {
            key: format!("CHARACTERS.{}", path.to_ascii_uppercase()),
            character: Some(speaker_code(speaker)),
        });
    }
    Some(WikiStringKey {
        key: rest.to_ascii_uppercase(),
        character: None,
    })
}

/// `build_language_map` 的统计信息（用于报告）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LanguageStats {
    /// PO 条目总数。
    pub entries: usize,
    /// 成功写入（顶层 key）。
    pub values: usize,
    /// 缺少 msgctxt 或非 `STRINGS.` 前缀。
    pub skipped_not_strings: usize,
    /// 文本为空（未翻译）。
    pub skipped_empty: usize,
    /// 同一 key/角色被重复写入（后者覆盖）。
    pub duplicates: usize,
}

/// 把 PO 条目构建成「顶层 key → 值」映射。
///
/// `use_msgstr = true` 取译文（chinese_s.po），`false` 取原文（strings.pot
/// 的 msgid）。`CHARACTERS.*` 条目合并进同一顶层表；空文本跳过。
pub fn build_language_map(
    entries: &[PoEntry],
    use_msgstr: bool,
) -> (BTreeMap<String, StringValue>, LanguageStats) {
    let mut map: BTreeMap<String, StringValue> = BTreeMap::new();
    let mut stats = LanguageStats {
        entries: entries.len(),
        ..LanguageStats::default()
    };

    for entry in entries {
        let Some(msgctxt) = entry.msgctxt.as_deref() else {
            stats.skipped_not_strings += 1;
            continue;
        };
        let Some(wiki_key) = wiki_string_key(msgctxt) else {
            stats.skipped_not_strings += 1;
            continue;
        };
        let text = if use_msgstr {
            entry.msgstr.trim_end_matches(['\r', '\n'])
        } else {
            entry.msgid.trim_end_matches(['\r', '\n'])
        };
        if text.trim().is_empty() {
            stats.skipped_empty += 1;
            continue;
        }
        let text = text.to_string();

        match wiki_key.character {
            Some(character) => match map.get_mut(&wiki_key.key) {
                Some(StringValue::Characters(table)) => {
                    if table.insert(character, text).is_some() {
                        stats.duplicates += 1;
                    }
                }
                Some(_) => {
                    let mut table = BTreeMap::new();
                    table.insert(character, text);
                    map.insert(wiki_key.key, StringValue::Characters(table));
                    stats.duplicates += 1;
                }
                None => {
                    let mut table = BTreeMap::new();
                    table.insert(character, text);
                    map.insert(wiki_key.key, StringValue::Characters(table));
                }
            },
            None => {
                if map.insert(wiki_key.key, StringValue::Text(text)).is_some() {
                    stats.duplicates += 1;
                }
            }
        }
    }

    stats.values = map.len();
    (map, stats)
}

/// `Data:<V>_Strings_Index.json` 的一项：桶内首个 key + 桶号字符串。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub key: String,
    pub bucket: String,
}

/// `Data:<V>_Strings_Index.json` 的页面结构（`mw.huiji.loadJson` 读 `.data`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringsIndexFile {
    pub data: Vec<IndexEntry>,
}

/// 一个桶：桶号 + 有序 key 列表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bucket {
    pub id: String,
    pub keys: Vec<String>,
}

/// 分桶方案：桶列表与配套索引（自洽：`index[i].key == buckets[i].keys[0]`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BucketPlan {
    pub buckets: Vec<Bucket>,
    pub index: Vec<IndexEntry>,
}

/// 按数量等分（重新分桶时使用）。`keys` 必须已排序。
pub fn plan_equal_count(keys: &[String], count: usize) -> BucketPlan {
    if keys.is_empty() || count == 0 {
        return BucketPlan {
            buckets: Vec::new(),
            index: Vec::new(),
        };
    }
    let chunk = keys.len().div_ceil(count).max(1);
    let mut buckets = Vec::new();
    for (i, chunk_keys) in keys.chunks(chunk).enumerate() {
        buckets.push(Bucket {
            id: format!("{i:02}"),
            keys: chunk_keys.to_vec(),
        });
    }
    let index = buckets
        .iter()
        .map(|bucket| IndexEntry {
            key: bucket.keys[0].clone(),
            bucket: bucket.id.clone(),
        })
        .collect();
    BucketPlan { buckets, index }
}

/// 沿用现有索引边界分桶：每个 key 归入「最后一个边界 ≤ key」的桶。
///
/// 边界 key 在游戏更新中消失时，该桶的新边界顺延到桶内最小现存 key；
/// 空桶从输出中省略（对应旧页面会变为 `return {}` 或保留待 M2 清理）。
/// `old` 为空时回退到 100 桶等分。
pub fn plan_with_boundaries(keys: &[String], old: &[IndexEntry]) -> BucketPlan {
    let mut bounds = old.to_vec();
    bounds.sort_by(|a, b| a.key.cmp(&b.key));
    bounds.dedup_by(|a, b| a.key == b.key);
    if bounds.is_empty() {
        return plan_equal_count(keys, 100);
    }

    let mut buckets: Vec<Bucket> = Vec::new();
    for key in keys {
        let idx = bounds.partition_point(|entry| entry.key.as_str() <= key.as_str());
        let id = if idx == 0 {
            bounds[0].bucket.clone()
        } else {
            bounds[idx - 1].bucket.clone()
        };
        match buckets.last_mut() {
            Some(last) if last.id == id => last.keys.push(key.clone()),
            _ => buckets.push(Bucket {
                id,
                keys: vec![key.clone()],
            }),
        }
    }

    let index = buckets
        .iter()
        .map(|bucket| IndexEntry {
            key: bucket.keys[0].clone(),
            bucket: bucket.id.clone(),
        })
        .collect();
    BucketPlan { buckets, index }
}

/// 校验分桶方案与索引自洽（索引必须能二分：key 与桶首一致且严格升序）。
pub fn validate_plan(plan: &BucketPlan) -> std::result::Result<(), String> {
    if plan.index.len() != plan.buckets.len() {
        return Err(format!(
            "index entries {} != buckets {}",
            plan.index.len(),
            plan.buckets.len()
        ));
    }
    let mut previous: Option<&str> = None;
    for (entry, bucket) in plan.index.iter().zip(&plan.buckets) {
        if entry.bucket != bucket.id {
            return Err(format!(
                "index bucket {} != bucket id {}",
                entry.bucket, bucket.id
            ));
        }
        let first = bucket
            .keys
            .first()
            .ok_or_else(|| format!("bucket {} is empty", bucket.id))?;
        if &entry.key != first {
            return Err(format!(
                "bucket {} index key {} != first key {}",
                bucket.id, entry.key, first
            ));
        }
        if let Some(prev) = previous {
            if prev >= entry.key.as_str() {
                return Err(format!(
                    "index keys not strictly ascending: {} >= {}",
                    prev, entry.key
                ));
            }
        }
        previous = Some(&entry.key);
    }
    if plan.buckets.iter().map(|b| b.keys.len()).sum::<usize>() == 0 && !plan.buckets.is_empty() {
        return Err("plan has buckets but no keys".to_string());
    }
    Ok(())
}

/// 转义为 Lua 双引号字符串字面量内容（不含引号）。
pub fn escape_lua_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0007}' => out.push_str("\\a"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000B}' => out.push_str("\\v"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                out.push_str(&format!("\\{:03}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn lua_quote(text: &str) -> String {
    format!("\"{}\"", escape_lua_string(text))
}

/// 渲染一个桶页（key 升序、2 空格缩进，与站内现有页面一致）。
pub fn render_module(values: &BTreeMap<String, StringValue>) -> String {
    let mut out = String::from("return {\n");
    for (key, value) in values {
        match value {
            StringValue::Text(text) => {
                out.push_str(&format!("  [{}] = {},\n", lua_quote(key), lua_quote(text)));
            }
            StringValue::Characters(chars) => {
                out.push_str(&format!("  [{}] = {{\n", lua_quote(key)));
                for (name, text) in chars {
                    out.push_str(&format!(
                        "    [{}] = {},\n",
                        lua_quote(name),
                        lua_quote(text)
                    ));
                }
                out.push_str("  },\n");
            }
        }
    }
    out.push_str("}\n");
    out
}

/// 新数据相对旧数据的差异统计。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub chars_added: usize,
    pub chars_removed: usize,
    pub chars_changed: usize,
}

impl MapDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

/// 逐 key（含角色表逐角色）对比新旧映射。
pub fn diff_values(
    new: &BTreeMap<String, StringValue>,
    old: &BTreeMap<String, StringValue>,
) -> MapDiff {
    let mut diff = MapDiff::default();

    for (key, new_value) in new {
        let Some(old_value) = old.get(key) else {
            diff.added.push(key.to_string());
            continue;
        };
        match (new_value, old_value) {
            (StringValue::Text(new_text), StringValue::Text(old_text)) => {
                if new_text != old_text {
                    diff.changed.push(key.to_string());
                }
            }
            (StringValue::Characters(new_chars), StringValue::Characters(old_chars)) => {
                let mut changed = false;
                for (name, new_text) in new_chars {
                    match old_chars.get(name) {
                        Some(old_text) if old_text != new_text => {
                            diff.chars_changed += 1;
                            changed = true;
                        }
                        None => {
                            diff.chars_added += 1;
                            changed = true;
                        }
                        _ => {}
                    }
                }
                for name in old_chars.keys() {
                    if !new_chars.contains_key(name) {
                        diff.chars_removed += 1;
                        changed = true;
                    }
                }
                if changed {
                    diff.changed.push(key.to_string());
                }
            }
            _ => diff.changed.push((*key).clone()),
        }
    }

    for key in old.keys() {
        if !new.contains_key(key) {
            diff.removed.push(key.to_string());
        }
    }

    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(msgctxt: Option<&str>, msgid: &str, msgstr: &str) -> PoEntry {
        PoEntry {
            msgctxt: msgctxt.map(str::to_string),
            msgid: msgid.to_string(),
            msgstr: msgstr.to_string(),
            comment: None,
        }
    }

    #[test]
    fn test_wiki_key_flat() {
        let key = wiki_string_key("STRINGS.ACTIONS.ABANDON").unwrap();
        assert_eq!(key.key, "ACTIONS.ABANDON");
        assert!(key.character.is_none());
        // 游戏 PO 里的段并非总是大写，站内统一为 ASCII 大写。
        let key = wiki_string_key("STRINGS.UI.COLOUR.Black").unwrap();
        assert_eq!(key.key, "UI.COLOUR.BLACK");
        let key = wiki_string_key("STRINGS.CHARACTER_BIOS.walter.1.desc").unwrap();
        assert_eq!(key.key, "CHARACTER_BIOS.WALTER.1.DESC");
    }

    #[test]
    fn test_wiki_key_characters_moves_speaker_to_table() {
        let key = wiki_string_key("STRINGS.CHARACTERS.GENERIC.DESCRIBE.BERRYBUSH.GENERIC").unwrap();
        assert_eq!(key.key, "CHARACTERS.DESCRIBE.BERRYBUSH.GENERIC");
        assert_eq!(key.character.as_deref(), Some("wilson"));
        let key = wiki_string_key("STRINGS.CHARACTERS.WX78.ANNOUNCE_CHARGE").unwrap();
        assert_eq!(key.key, "CHARACTERS.ANNOUNCE_CHARGE");
        assert_eq!(key.character.as_deref(), Some("wx78"));
        // 路径段大小写归一，但角色表键保持小写。
        let key = wiki_string_key("STRINGS.CHARACTERS.WX78.announce_charge").unwrap();
        assert_eq!(key.key, "CHARACTERS.ANNOUNCE_CHARGE");
        assert_eq!(key.character.as_deref(), Some("wx78"));
    }

    #[test]
    fn test_wiki_key_rejects_non_strings() {
        assert!(wiki_string_key("SOMETHING.ELSE").is_none());
    }

    #[test]
    fn test_build_language_map_flat_and_characters() {
        let entries = vec![
            entry(Some("STRINGS.NAMES.AXE"), "Axe", "斧头"),
            entry(
                Some("STRINGS.CHARACTERS.GENERIC.DESCRIBE.AXE"),
                "An axe",
                "一把斧头",
            ),
            entry(
                Some("STRINGS.CHARACTERS.WX78.DESCRIBE.AXE"),
                "An axe",
                "斧头。高效。",
            ),
            entry(Some("STRINGS.NAMES.EMPTY"), "Empty", "  "),
            entry(None, "no context", "x"),
            entry(Some("OTHER.KEY"), "Other", "y"),
        ];
        let (map, stats) = build_language_map(&entries, true);
        assert_eq!(stats.entries, 6);
        assert_eq!(stats.skipped_empty, 1);
        assert_eq!(stats.skipped_not_strings, 2);
        assert_eq!(map.len(), 2);
        assert_eq!(map["NAMES.AXE"], StringValue::Text("斧头".into()));
        let chars = map["CHARACTERS.DESCRIBE.AXE"].as_characters().unwrap();
        assert_eq!(chars["wilson"], "一把斧头");
        assert_eq!(chars["wx78"], "斧头。高效。");
    }

    #[test]
    fn test_build_language_map_uses_msgid_for_english() {
        let entries = vec![entry(Some("STRINGS.NAMES.AXE"), "Axe", "")];
        let (map, stats) = build_language_map(&entries, false);
        assert_eq!(stats.skipped_empty, 0);
        assert_eq!(map["NAMES.AXE"], StringValue::Text("Axe".into()));
    }

    #[test]
    fn test_plan_equal_count() {
        let keys: Vec<String> = (0..10).map(|i| format!("K{i:02}")).collect();
        let plan = plan_equal_count(&keys, 3);
        assert_eq!(plan.buckets.len(), 3);
        assert_eq!(plan.buckets[0].keys, vec!["K00", "K01", "K02", "K03"]);
        assert_eq!(plan.index[1].key, "K04");
        assert_eq!(plan.index[2].key, "K08");
        assert_eq!(plan.index[2].bucket, "02");
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn test_plan_equal_count_more_buckets_than_keys() {
        let keys: Vec<String> = vec!["A".into(), "B".into()];
        let plan = plan_equal_count(&keys, 100);
        assert_eq!(plan.buckets.len(), 2);
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn test_plan_with_boundaries_keeps_bucket_ids() {
        let keys: Vec<String> = (0..10).map(|i| format!("K{i:02}")).collect();
        let old = vec![
            IndexEntry {
                key: "K00".into(),
                bucket: "05".into(),
            },
            IndexEntry {
                key: "K04".into(),
                bucket: "09".into(),
            },
        ];
        let plan = plan_with_boundaries(&keys, &old);
        assert_eq!(plan.buckets.len(), 2);
        assert_eq!(plan.buckets[0].id, "05");
        assert_eq!(plan.buckets[0].keys, vec!["K00", "K01", "K02", "K03"]);
        assert_eq!(plan.buckets[1].id, "09");
        assert_eq!(plan.index[1].key, "K04");
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn test_plan_with_boundaries_missing_boundary_key_shifts() {
        let keys: Vec<String> = vec!["B".into(), "D".into(), "E".into()];
        let old = vec![
            IndexEntry {
                key: "A".into(),
                bucket: "00".into(),
            },
            IndexEntry {
                key: "C".into(),
                bucket: "01".into(),
            },
        ];
        let plan = plan_with_boundaries(&keys, &old);
        assert_eq!(plan.buckets.len(), 2);
        // C 被删除后，桶 01 的新边界顺延到 D。
        assert_eq!(plan.index[1].key, "D");
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn test_plan_with_boundaries_empty_old_falls_back() {
        let keys: Vec<String> = vec!["A".into()];
        let plan = plan_with_boundaries(&keys, &[]);
        assert_eq!(plan.buckets.len(), 1);
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn test_validate_plan_rejects_out_of_order() {
        let plan = BucketPlan {
            buckets: vec![
                Bucket {
                    id: "00".into(),
                    keys: vec!["B".into()],
                },
                Bucket {
                    id: "01".into(),
                    keys: vec!["A".into()],
                },
            ],
            index: vec![
                IndexEntry {
                    key: "B".into(),
                    bucket: "00".into(),
                },
                IndexEntry {
                    key: "A".into(),
                    bucket: "01".into(),
                },
            ],
        };
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn test_escape_lua_string() {
        let escaped = escape_lua_string("a\"b\\c\nd\te\u{0007}");
        assert_eq!(escaped, "a\\\"b\\\\c\\nd\\te\\a");
    }

    #[test]
    fn test_render_module() {
        let mut values = BTreeMap::new();
        values.insert(
            "ACTIONS.ABANDON".to_string(),
            StringValue::Text("遗弃".into()),
        );
        let mut chars = BTreeMap::new();
        chars.insert("wilson".to_string(), "他说\"你好\"".to_string());
        values.insert(
            "CHARACTERS.DESCRIBE.AXE".to_string(),
            StringValue::Characters(chars),
        );
        let text = render_module(&values);
        assert_eq!(
            text,
            "return {\n  [\"ACTIONS.ABANDON\"] = \"遗弃\",\n  [\"CHARACTERS.DESCRIBE.AXE\"] = {\n    [\"wilson\"] = \"他说\\\"你好\\\"\",\n  },\n}\n"
        );
    }

    #[test]
    fn test_diff_values() {
        let mut old = BTreeMap::new();
        old.insert("A".to_string(), StringValue::Text("old".into()));
        old.insert("B".to_string(), StringValue::Text("gone".into()));
        let mut old_chars = BTreeMap::new();
        old_chars.insert("wilson".to_string(), "hi".to_string());
        old.insert("C".to_string(), StringValue::Characters(old_chars.clone()));

        let mut new = BTreeMap::new();
        new.insert("A".to_string(), StringValue::Text("new".into()));
        new.insert("D".to_string(), StringValue::Text("added".into()));
        let mut new_chars = BTreeMap::new();
        new_chars.insert("wilson".to_string(), "hello".to_string());
        new_chars.insert("wx78".to_string(), "BEEP".to_string());
        new.insert("C".to_string(), StringValue::Characters(new_chars));

        let diff = diff_values(&new, &old);
        assert_eq!(diff.added, vec!["D"]);
        assert_eq!(diff.removed, vec!["B"]);
        assert_eq!(diff.changed, vec!["A", "C"]);
        assert_eq!(diff.chars_added, 1);
        assert_eq!(diff.chars_changed, 1);
        assert_eq!(diff.chars_removed, 0);
        assert!(!diff.is_empty());
    }
}
