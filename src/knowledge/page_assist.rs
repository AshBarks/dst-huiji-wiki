//! page-assist(M3 前身,目标 1/3):给定页面,从 PageSymbolMap 输出覆盖缺口
//! 建议清单。只读 `knowledge/pages/*.json`,零 LLM、零语料依赖。

use crate::error::{Error, Result};
use crate::knowledge::scan_wiki::PageSymbolMap;
use crate::service::Reporter;
use std::collections::BTreeMap;
use std::path::Path;

pub struct PageAssistParams {
    pub knowledge_dir: String,
    /// 页面 id(纯数字)或标题(精确匹配);--all 时忽略
    pub page: Option<String>,
    /// 全库缺口榜:按符号与页面聚合 stub/aspects_ignored,输出 Markdown
    pub all: bool,
    /// 输出 JSON 而非 Markdown
    pub json: bool,
}

pub async fn run_page_assist(
    params: &PageAssistParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let pages_dir = Path::new(&params.knowledge_dir).join("pages");
    let _ = reporter;

    if params.all {
        print!("{}", render_all_gaps(&pages_dir)?);
        return Ok(serde_json::json!({ "mode": "page_assist_all" }));
    }

    let Some(page) = &params.page else {
        return Err(Error::Config("需要 <page> 参数或 --all".to_string()));
    };
    let map = find_map(&pages_dir, page)?
        .ok_or_else(|| Error::Config(format!("未找到页面 {page} 的归因图")))?;

    if params.json {
        let body = serde_json::to_string_pretty(&map)?;
        println!("{body}");
        return Ok(serde_json::json!({ "mode": "page_assist", "pageid": map.pageid }));
    }

    print!("{}", render_markdown(&map));
    Ok(serde_json::json!({ "mode": "page_assist", "pageid": map.pageid }))
}

/// 全库缺口榜:按符号聚合 stub/未覆盖 aspects,按页面聚合缺口规模。
fn render_all_gaps(pages_dir: &Path) -> Result<String> {
    #[derive(Default)]
    struct SymAgg {
        routed: usize,
        mentioned: usize,
        stub: usize,
        ignored_aspects: usize,
    }
    let mut syms: BTreeMap<String, SymAgg> = BTreeMap::new();
    let mut page_rows: Vec<(i64, String, usize, usize)> = Vec::new();
    for entry in
        std::fs::read_dir(pages_dir)?.collect::<std::io::Result<Vec<std::fs::DirEntry>>>()?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(m) = serde_json::from_str::<PageSymbolMap>(
            &std::fs::read_to_string(&path).unwrap_or_default(),
        ) else {
            continue;
        };
        let mut page_gaps = 0usize;
        let mut page_stub = 0usize;
        for (sym, e) in &m.symbols {
            let agg = syms.entry(sym.clone()).or_default();
            agg.routed += 1;
            if e.mention_source.is_some() {
                agg.mentioned += 1;
            } else {
                if e.detail_level == "stub" {
                    page_stub += 1;
                }
                page_gaps += e.aspects_ignored.len();
                agg.stub += 1;
                agg.ignored_aspects += e.aspects_ignored.len();
            }
        }
        page_rows.push((m.pageid, m.title, page_stub, page_gaps));
    }

    let mut out = String::new();
    let total_stub: usize = syms.values().map(|a| a.stub).sum();
    let total_gaps: usize = syms.values().map(|a| a.ignored_aspects).sum();
    out.push_str("# 全库覆盖缺口榜\n\n");
    out.push_str(&format!(
        "页面 {} 个;归因对中 stub {} 个,未覆盖 aspects {} 条。\n\n",
        page_rows.len(),
        total_stub,
        total_gaps
    ));

    out.push_str("## 符号缺口榜(按未覆盖 aspects 降序,前 20)\n\n");
    out.push_str("| 符号 | routed | 提及 | stub | 未覆盖 aspects |\n|---|---:|---:|---:|---:|\n");
    let mut sym_rows: Vec<_> = syms.iter().collect();
    sym_rows.sort_by_key(|(_, a)| std::cmp::Reverse(a.ignored_aspects));
    for (sym, a) in sym_rows.iter().take(20) {
        out.push_str(&format!(
            "| `{sym}` | {} | {} | {} | {} |\n",
            a.routed, a.mentioned, a.stub, a.ignored_aspects
        ));
    }

    out.push_str("\n## 页面缺口榜(按未覆盖 aspects 降序,前 20)\n\n");
    out.push_str("| 页面 | pageid | stub 符号 | 未覆盖 aspects |\n|---|---:|---:|---:|\n");
    page_rows.sort_by_key(|r| std::cmp::Reverse(r.3));
    for (pid, title, stub, gaps) in page_rows.iter().take(20) {
        out.push_str(&format!("| {title} | {pid} | {stub} | {gaps} |\n"));
    }
    Ok(out)
}

fn find_map(pages_dir: &Path, page: &str) -> Result<Option<PageSymbolMap>> {
    if let Ok(pageid) = page.parse::<i64>() {
        let path = pages_dir.join(format!("{pageid}.json"));
        return match std::fs::read_to_string(&path) {
            Ok(raw) => Ok(Some(serde_json::from_str(&raw)?)),
            Err(_) => Ok(None),
        };
    }
    // 标题精确匹配(遍历图文件;756 份小 JSON,毫秒级)
    for entry in
        std::fs::read_dir(pages_dir)?.collect::<std::io::Result<Vec<std::fs::DirEntry>>>()?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<PageSymbolMap>(
            &std::fs::read_to_string(&path).unwrap_or_default(),
        ) {
            if m.title == page {
                return Ok(Some(m));
            }
        }
    }
    Ok(None)
}

fn render_markdown(map: &PageSymbolMap) -> String {
    let mut direct_mentioned: Vec<_> = map
        .symbols
        .iter()
        .filter(|(_, e)| e.route == "direct" && e.mention_source.is_some())
        .collect();
    direct_mentioned
        .sort_by_key(|(_, e)| std::cmp::Reverse(e.aspects_covered.len() + e.fact_matches.len()));
    let mut gaps: Vec<_> = map
        .symbols
        .iter()
        .filter(|(_, e)| e.route == "direct" && e.mention_source.is_none())
        .collect();
    gaps.sort_by_key(|(_, e)| std::cmp::Reverse(e.aspects_ignored.len()));
    let two_hop: Vec<_> = map
        .symbols
        .iter()
        .filter(|(_, e)| e.route == "two_hop")
        .collect();

    let mut out = String::new();
    out.push_str(&format!(
        "# 页面覆盖辅助:{}(pageid {})\n\n",
        map.title, map.pageid
    ));
    out.push_str(&format!(
        "页面深度 **{}**;直连符号 {}(提及 {})、二跳符号 {}(提及 {})。\n\n",
        map.page_detail_level.as_deref().unwrap_or("-"),
        direct_mentioned.len() + gaps.len(),
        direct_mentioned.len(),
        two_hop.len(),
        two_hop
            .iter()
            .filter(|(_, e)| e.mention_source.is_some())
            .count(),
    ));

    out.push_str(&format!(
        "## 已覆盖(直连 {} 个)\n\n",
        direct_mentioned.len()
    ));
    for (sym, e) in &direct_mentioned {
        let src = e.mention_source.as_deref().unwrap_or("?");
        out.push_str(&format!("- `{}` [{}] 来源 {src}", sym, e.detail_level));
        if !e.aspects_covered.is_empty() {
            let names: Vec<&str> = e
                .aspects_covered
                .iter()
                .map(|a| a.aspect.as_str())
                .collect();
            out.push_str(&format!(
                ";覆盖 aspects({}):{}",
                names.len(),
                names.join("、")
            ));
        }
        if !e.fact_matches.is_empty() {
            out.push_str(&format!(";数值配对 {} 条", e.fact_matches.len()));
        }
        if let Some(v) = &e.llm_verdict {
            if v.semantic_consistent == Some(false) {
                out.push_str(";⚠️ 审计标记语义不一致");
            }
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "\n## 覆盖缺口(直连 {} 个,按缺口大小排序)\n\n",
        gaps.len()
    ));
    for (sym, e) in &gaps {
        out.push_str(&format!(
            "- `{}` 未覆盖 aspects({}):",
            sym,
            e.aspects_ignored.len()
        ));
        if e.aspects_ignored.is_empty() {
            out.push_str("该符号文档无可归因 aspects(页面正文不描述属正常)");
        } else {
            out.push_str(&e.aspects_ignored.join("、"));
        }
        out.push('\n');
    }

    if !two_hop.is_empty() {
        out.push_str(&format!("\n## 二跳符号({} 个,供参考)\n\n", two_hop.len()));
        for (sym, e) in &two_hop {
            out.push_str(&format!(
                "- `{}` [{}]{}\n",
                sym,
                e.detail_level,
                if e.mention_source.is_some() {
                    format!(" 提及({})", e.mention_source.as_deref().unwrap_or("?"))
                } else {
                    String::new()
                }
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::scan_wiki::{AspectSnapshot, PageSymbolEntry};
    use std::collections::BTreeMap;

    fn sample_map() -> PageSymbolMap {
        let mut symbols = BTreeMap::new();
        symbols.insert(
            "components/health.lua".to_string(),
            PageSymbolEntry {
                mention_source: Some("fact_match".to_string()),
                aspects_covered: vec![AspectSnapshot {
                    aspect: "生命值数值标注".into(),
                    evidence: vec![],
                }],
                aspects_ignored: vec!["治疗相关描述".into()],
                fact_matches: vec![],
                route: "direct".to_string(),
                detail_level: "summary".to_string(),
                llm_verdict: None,
            },
        );
        symbols.insert(
            "components/combat.lua".to_string(),
            PageSymbolEntry {
                mention_source: None,
                aspects_covered: vec![],
                aspects_ignored: vec![
                    "仇恨距离描述".into(),
                    "攻击间隔标注".into(),
                    "攻击范围描述".into(),
                ],
                fact_matches: vec![],
                route: "direct".to_string(),
                detail_level: "stub".to_string(),
                llm_verdict: None,
            },
        );
        symbols.insert(
            "behaviours/wander.lua".to_string(),
            PageSymbolEntry {
                mention_source: None,
                aspects_covered: vec![],
                aspects_ignored: vec![],
                fact_matches: vec![],
                route: "two_hop".to_string(),
                detail_level: "stub".to_string(),
                llm_verdict: None,
            },
        );
        PageSymbolMap {
            schema_version: 1,
            pageid: 13857,
            title: "猎犬".to_string(),
            inputs: crate::knowledge::scan_wiki::MapInputs {
                wikitext_sha256: String::new(),
                symbol_doc_shas: BTreeMap::new(),
            },
            symbols,
            page_detail_level: Some("summary".to_string()),
        }
    }

    #[test]
    fn markdown_lists_gaps_sorted_by_size() {
        let md = render_markdown(&sample_map());
        assert!(md.contains("猎犬"));
        assert!(md.contains("已覆盖(直连 1 个)"));
        assert!(md.contains("覆盖缺口(直连 1 个"));
        // combat(3 条缺口)排在缺口节;health 的 ignored 1 条不在缺口节
        let gap_pos = md.find("覆盖缺口").unwrap();
        let combat_pos = md.find("components/combat.lua").unwrap();
        let health_in_cover = md.find("components/health.lua").unwrap();
        assert!(health_in_cover < gap_pos, "health 是提及对,应在已覆盖节");
        assert!(combat_pos > gap_pos);
        assert!(md.contains("仇恨距离描述、攻击间隔标注、攻击范围描述"));
    }

    #[test]
    fn find_map_by_title_and_id() {
        let dir = std::env::temp_dir().join(format!("page-assist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let map = sample_map();
        std::fs::write(
            dir.join("13857.json"),
            serde_json::to_string_pretty(&map).unwrap(),
        )
        .unwrap();
        assert_eq!(find_map(&dir, "13857").unwrap().unwrap().title, "猎犬");
        assert_eq!(find_map(&dir, "猎犬").unwrap().unwrap().pageid, 13857);
        assert!(find_map(&dir, "不存在").unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
