//! M2 审计遗留的「语义不一致」对的三分类(确定性,零 LLM)。
//!
//! 按 [KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md](../../docs/KNOWLEDGE_PAGE_MAP_AUDIT_REPORT.md)
//! §3.4/§5 的教训,`semantic_consistent=false` 不能直接当页面纠错清单——
//! M3 消费前必须分类:
//!
//! - **A 路由过近似**:句子事实属于同页兄弟符号(数值命中兄弟符号常量,
//!   或与兄弟 aspects 引文 bigram 高度重叠)→ 页面没问题,从 M3 候选中剔除;
//! - **B 变体/跨文件缺口**:数值命中页面 prefab 文件常量(变体级覆盖)→
//!   页面没问题,回流为 SymbolDoc 文档侧修正项;
//! - **C 疑似页面错误**:数值在任何可达代码里都找不到(如猎犬"100 单位"
//!   时间误读案)→ Tier2/人工纠错候选;
//! - **D 人工**:无数值句、数值仅命中本文件(语义分歧)等无法确定性归类的。
//!
//! 产物 `knowledge/inconsistent_classification.json`;命令入口
//! `knowledge-scan-wiki --classify`(与 `--report` 同级的聚合模式,不重建地图)。

use crate::error::Result;
use crate::knowledge::scan_wiki::{extract_named_constants, trim_num, PageSymbolMap};
use crate::platform::progress::Reporter;
use crate::update::build_atlas_from_dir;
use crate::update::grade::CorpusPageView;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub const CLASS_ROUTING: &str = "A_routing";
pub const CLASS_VARIANT: &str = "B_variant_gap";
pub const CLASS_PAGE_ERROR: &str = "C_page_error";
pub const CLASS_MANUAL: &str = "D_manual";

/// 与 scan_wiki D1 相同的阈值:|v| < 3 的数值碰撞面过大,不参与匹配。
fn matchable(v: f64) -> bool {
    v.abs() >= 3.0
}

/// 从句子中提取数值(支持小数),trim_num 归一后返回。
fn extract_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            if let Ok(v) = cur.parse::<f64>() {
                if matchable(v) {
                    let t = trim_num(v);
                    if !out.contains(&t) {
                        out.push(t);
                    }
                }
            }
            cur.clear();
        }
    };
    for ch in text.chars() {
        let takes = ch.is_ascii_digit()
            || (ch == '.' && !cur.is_empty() && cur.chars().all(|c| c.is_ascii_digit()));
        if takes {
            cur.push(ch);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// 归一化:仅保留字母数字与 CJK,ascii 转小写。
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || (*c as u32) >= 0x4E_00)
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 字符 bigram Dice 系数(0~1):两串归一后重叠度。
pub(crate) fn bigram_dice(a: &str, b: &str) -> f64 {
    let norm_a = normalize(a);
    let norm_b = normalize(b);
    if norm_a.chars().count() < 2 || norm_b.chars().count() < 2 {
        return 0.0;
    }
    let grams = |s: &str| -> BTreeMap<(char, char), usize> {
        let cs: Vec<char> = s.chars().collect();
        let mut m = BTreeMap::new();
        for w in cs.windows(2) {
            *m.entry((w[0], w[1])).or_default() += 1;
        }
        m
    };
    let (ga, gb) = (grams(&norm_a), grams(&norm_b));
    let inter: usize = ga
        .iter()
        .map(|(k, n)| (*n).min(*gb.get(k).unwrap_or(&0)))
        .sum();
    let total = ga.values().sum::<usize>() + gb.values().sum::<usize>();
    if total == 0 {
        0.0
    } else {
        2.0 * inter as f64 / total as f64
    }
}

/// 一条不一致对的判定信号(纯数据,便于测试)。
#[derive(Debug, Default)]
pub struct ClassSignals {
    /// wording 中可参与匹配的数值(trim_num 归一)
    pub numbers: Vec<String>,
    /// 兄弟符号文件常量命中:`path: NAME=v`
    pub sibling_match: Option<String>,
    /// 页面 prefab 文件常量命中
    pub prefab_match: Option<String>,
    /// 页面 prefab 文件含同值字面量(属性赋值如 aura=-40,非命名常量)
    pub prefab_literal: bool,
    /// 本符号文件常量命中
    pub own_match: Option<String>,
    /// 兄弟 aspects 引文与 wording 的最大 bigram 重叠
    pub sibling_overlap: f64,
}

/// 兄弟引文重叠阈值:≥0.5 视为句子归属于兄弟符号。
const SIBLING_OVERLAP_THRESHOLD: f64 = 0.5;

pub fn classify(s: &ClassSignals) -> &'static str {
    if s.sibling_match.is_some() || s.sibling_overlap >= SIBLING_OVERLAP_THRESHOLD {
        CLASS_ROUTING
    } else if s.prefab_match.is_some() || s.prefab_literal {
        CLASS_VARIANT
    } else if !s.numbers.is_empty() {
        if s.own_match.is_some() {
            CLASS_MANUAL
        } else {
            CLASS_PAGE_ERROR
        }
    } else {
        CLASS_MANUAL
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassifiedPair {
    pub pageid: i64,
    pub title: String,
    pub path: String,
    pub class: &'static str,
    /// 审计 verdict 的页面原句(fact_match 对回退其 raw)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wording: Option<String>,
    /// 审计 note(人工复核用)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// LLM 辅助分类理由(仅 D 类 residual 二层分类产出)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// LLM 建议类别(仅建议;class 由确定性层+人工覆写决定,零方差)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_suggest: Option<String>,
    /// 命中证据:"path: NAME=v"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// wording 中参与匹配的数值
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub numbers: Vec<String>,
}

/// 常量缓存:符号文件与 prefab 文件共用。
fn constants_of<'a>(
    cache: &'a mut HashMap<String, Vec<(String, f64)>>,
    scripts_root: &Path,
    path: &str,
) -> &'a Vec<(String, f64)> {
    cache.entry(path.to_string()).or_insert_with(|| {
        std::fs::read_to_string(scripts_root.join(path))
            .map(|src| extract_named_constants(&src))
            .unwrap_or_default()
    })
}

/// prefab 源码缓存(字面量交叉检查用)。
fn prefab_source<'a>(
    cache: &'a mut HashMap<String, String>,
    scripts_root: &Path,
    path: &str,
) -> &'a str {
    cache
        .entry(path.to_string())
        .or_insert_with(|| std::fs::read_to_string(scripts_root.join(path)).unwrap_or_default())
}

/// 在常量表里找与 numbers 相等的第一个命中。
fn first_match(constants: &[(String, f64)], numbers: &[String]) -> Option<(String, f64)> {
    constants
        .iter()
        .filter(|(_, v)| matchable(*v) && numbers.contains(&trim_num(*v)))
        .map(|(n, v)| (n.clone(), *v))
        .next()
}

pub async fn run_classify(
    scripts_root: &Path,
    knowledge_root: &Path,
    corpus_root: &Path,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("加载页面图与审计 verdict");
    let dir = knowledge_root.join("pages");
    let mut maps = Vec::new();
    for entry in std::fs::read_dir(&dir)?.collect::<std::io::Result<Vec<std::fs::DirEntry>>>()? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<PageSymbolMap>(&std::fs::read_to_string(&path)?) {
            maps.push(m);
        }
    }
    let inconsistent: Vec<(
        &PageSymbolMap,
        String,
        &crate::knowledge::scan_wiki::LlmVerdict,
    )> = maps
        .iter()
        .flat_map(|m| {
            m.symbols.iter().filter_map(move |(path, e)| {
                let v = e.llm_verdict.as_ref()?;
                (v.semantic_consistent == Some(false)).then(|| (m, path.clone(), v))
            })
        })
        .collect();
    if inconsistent.is_empty() {
        reporter.log("无不一致对,无需分类".to_string());
        return Ok(serde_json::json!({"pairs": 0}));
    }

    reporter.stage("构建匹配信号(常量索引 / 语料路由)");
    let atlas = build_atlas_from_dir(scripts_root)?;
    // pageid → 关联 prefab 文件集(variant 反查)
    let mut variant_prefabs: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for e in &atlas.index.edges {
        variant_prefabs
            .entry(e.prefab_variant.as_str())
            .or_default()
            .insert(e.prefab_file.as_str());
    }
    let view = CorpusPageView::load(corpus_root)?;
    let mut page_prefabs: HashMap<i64, BTreeSet<&str>> = HashMap::new();
    for (variant, ids) in &view.pages {
        let Some(files) = variant_prefabs.get(variant.as_str()) else {
            continue;
        };
        for id in ids {
            page_prefabs
                .entry(*id)
                .or_default()
                .extend(files.iter().copied());
        }
    }

    let mut cache: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    let mut sources: HashMap<String, String> = HashMap::new();
    let mut pairs: Vec<ClassifiedPair> = Vec::new();
    for (map, path, verdict) in &inconsistent {
        let wording = verdict.wording.clone().or_else(|| {
            map.symbols
                .get(path)
                .and_then(|e| e.fact_matches.first().map(|f| f.raw.clone()))
        });
        let numbers = wording.as_deref().map(extract_numbers).unwrap_or_default();
        // 兄弟符号:同页其余符号(常量命中 + 引文重叠)
        let mut sibling_match = None;
        let mut sibling_overlap = 0.0_f64;
        for (other, entry) in &map.symbols {
            if other == path {
                continue;
            }
            if sibling_match.is_none() && !numbers.is_empty() {
                let cs = constants_of(&mut cache, scripts_root, other);
                if let Some((n, v)) = first_match(cs, &numbers) {
                    sibling_match = Some(format!("{other}: {n}={}", trim_num(v)));
                }
            }
            for q in entry
                .aspects_covered
                .iter()
                .flat_map(|a| a.evidence.iter())
                .filter_map(|e| e.quote.as_deref())
                .chain(entry.fact_matches.iter().map(|f| f.raw.as_str()))
            {
                if let Some(w) = wording.as_deref() {
                    sibling_overlap = sibling_overlap.max(bigram_dice(w, q));
                }
            }
        }
        let own_match = if numbers.is_empty() {
            None
        } else {
            first_match(constants_of(&mut cache, scripts_root, path), &numbers)
                .map(|(n, v)| format!("{path}: {n}={}", trim_num(v)))
        };
        let mut prefab_match = None;
        let mut prefab_literal = false;
        if !numbers.is_empty() {
            if let Some(files) = page_prefabs.get(&map.pageid) {
                for f in files {
                    let cs = constants_of(&mut cache, scripts_root, f);
                    if let Some((n, v)) = first_match(cs, &numbers) {
                        prefab_match = Some(format!("{f}: {n}={}", trim_num(v)));
                        break;
                    }
                }
                if prefab_match.is_none() {
                    // 属性赋值(如 inst.components.sanityaura.aura = -40)不是命名
                    // 常量,但数值真实存在于 prefab 源码 → 仍属文档侧缺口而非页面错误
                    prefab_literal = files.iter().any(|f| {
                        let src = prefab_source(&mut sources, scripts_root, f);
                        extract_numbers(src).iter().any(|n| numbers.contains(n))
                    });
                }
            }
        }
        let signals = ClassSignals {
            numbers: numbers.clone(),
            sibling_match,
            prefab_match,
            prefab_literal,
            own_match,
            sibling_overlap,
        };
        let class = classify(&signals);
        pairs.push(ClassifiedPair {
            pageid: map.pageid,
            title: map.title.clone(),
            path: path.clone(),
            class,
            wording: wording.clone(),
            note: verdict.note.clone(),
            reason: None,
            llm_suggest: None,
            evidence: signals
                .sibling_match
                .or(signals.prefab_match)
                .or(signals.own_match),
            numbers,
        });
    }

    // 二层:D 类 residual 批量 LLM 辅助分类(审计 note 已含归因语义)
    llm_classify_manual(&mut pairs, reporter).await;
    // 三层:人工复核覆写(最后应用,自动层不覆盖人工结论)
    apply_manual_overrides(knowledge_root, &mut pairs, reporter);

    pairs.sort_by(|a, b| {
        (a.class, a.pageid, a.path.as_str()).cmp(&(b.class, b.pageid, b.path.as_str()))
    });
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &pairs {
        *counts.entry(p.class).or_default() += 1;
    }
    for (class, n) in &counts {
        reporter.log(format!("{}: {} 对", class, n));
    }
    let out = serde_json::json!({
        "schema_version": 1,
        "counts": counts,
        "pairs": pairs,
    });
    let out_path = knowledge_root.join("inconsistent_classification.json");
    std::fs::write(&out_path, serde_json::to_string_pretty(&out)?)?;
    reporter.log(format!(
        "分类完成:共 {} 对,报告 {}",
        pairs.len(),
        out_path.display()
    ));
    Ok(out)
}

/// 人工复核覆写文件(`knowledge/inconsistent_manual.json`):自动层重跑
/// 不会覆盖人工结论。条目:{pageid, path, class, note}。
fn apply_manual_overrides(
    knowledge_root: &Path,
    pairs: &mut [ClassifiedPair],
    reporter: &dyn Reporter,
) {
    #[derive(serde::Deserialize)]
    struct Override {
        pageid: i64,
        path: String,
        class: String,
        #[serde(default)]
        note: String,
    }
    let path = knowledge_root.join("inconsistent_manual.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(entries) = serde_json::from_str::<Vec<Override>>(&raw) else {
        reporter.log("人工覆写文件解析失败,忽略".to_string());
        return;
    };
    let mut applied = 0usize;
    for o in &entries {
        let class = match o.class.as_str() {
            "A_routing" => CLASS_ROUTING,
            "B_variant_gap" => CLASS_VARIANT,
            "C_page_error" => CLASS_PAGE_ERROR,
            "D_manual" => CLASS_MANUAL,
            _ => {
                reporter.log(format!("覆写 {} 非法 class `{}`,忽略", o.pageid, o.class));
                continue;
            }
        };
        if let Some(p) = pairs
            .iter_mut()
            .find(|p| p.pageid == o.pageid && p.path == o.path)
        {
            p.class = class;
            if !o.note.is_empty() {
                p.note = Some(o.note.clone());
            }
            p.reason = Some("人工核实覆写".to_string());
            applied += 1;
        }
    }
    reporter.log(format!("人工覆写应用 {} 条({})", applied, path.display()));
}

/// 二层分类:对确定性层判为 D_manual 的对,批量 LLM 复核(25 条/批)。
/// 无 LLM 配置或调用失败时保持 D_manual(宁人工勿错分)。
async fn llm_classify_manual(pairs: &mut [ClassifiedPair], reporter: &dyn Reporter) {
    let residual: Vec<usize> = pairs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.class == CLASS_MANUAL)
        .map(|(i, _)| i)
        .collect();
    if residual.is_empty() {
        return;
    }
    let Some(config) = crate::llm::LlmConfig::from_env() else {
        reporter.log(format!(
            "D_manual {} 对,无 LLM 配置,保持人工分类",
            residual.len()
        ));
        return;
    };
    const SYSTEM: &str = "你是饥荒维基页面审计分类员。必须只输出一个合法 JSON 对象。";
    const CLASS_DOC: &str = "- A_routing 路由过近似:句子事实实际属于同页其它符号(页面没错,归因错了)\n- B_variant_gap 变体/跨文件缺口:数值或事实来自 prefab 层/变体覆盖(页面没错,文档侧缺口)\n- C_page_error 疑似页面错误:数值或表述与代码矛盾且无代码出处(纠错候选)\n- D_manual 无法判定";
    let mut done = 0usize;
    for batch in residual.chunks(25) {
        let mut listing = String::new();
        for (offset, idx) in batch.iter().enumerate() {
            let p = &pairs[*idx];
            listing.push_str(&format!(
                "{}. 页面《{}》× {}\n   原句:{}\n   审计理由:{}\n   数值:{}\n",
                offset + 1,
                p.title,
                p.path,
                p.wording.as_deref().unwrap_or("(无)"),
                p.note.as_deref().unwrap_or("(无)"),
                if p.numbers.is_empty() {
                    "(无)".to_string()
                } else {
                    p.numbers.join("/")
                },
            ));
        }
        let prompt = format!(
            "页面图审计判定的「语义不一致」对需要分类:\n{CLASS_DOC}\n\n待分类(按 index):\n{listing}\n请对每条给出 class 与简短中文理由。只输出一个 JSON 对象,形如:{{\"items\":[{{\"index\":1,\"class\":\"A_routing\",\"reason\":\"句子描述掉落,属 lootdropper\"}}]}}"
        );
        reporter.log(format!("LLM 辅助分类 {} 对(三票多数)", batch.len()));
        // 三票多数:同批独立调用 3 次,≥2 票一致才改判,否则保持人工
        #[derive(serde::Deserialize)]
        struct Item {
            index: usize,
            class: String,
            #[serde(default)]
            reason: String,
        }
        #[derive(serde::Deserialize)]
        struct Payload {
            #[serde(default)]
            items: Vec<Item>,
        }
        let parse = |raw: &str| -> Option<Vec<(usize, String, String)>> {
            let trimmed = raw.trim();
            let (a, b) = (trimmed.find('{')?, trimmed.rfind('}')?);
            if a >= b {
                return None;
            }
            let p: Payload = serde_json::from_str(&trimmed[a..=b]).ok()?;
            Some(
                p.items
                    .into_iter()
                    .filter(|i| i.index >= 1 && i.index <= batch.len())
                    .map(|i| (i.index, i.class, i.reason))
                    .collect(),
            )
        };
        let mut votes: BTreeMap<usize, Vec<(String, String)>> = BTreeMap::new();
        let mut ok_runs = 0usize;
        for _ in 0..3 {
            match config.complete_streaming(SYSTEM, &prompt, |_| {}).await {
                Ok(raw) => {
                    if let Some(items) = parse(&raw) {
                        ok_runs += 1;
                        for (index, class, reason) in items {
                            votes.entry(index).or_default().push((class, reason));
                        }
                    }
                }
                Err(e) => {
                    reporter.log(format!("LLM 辅助分类单次失败(计入缺票):{e}"));
                }
            }
        }
        if ok_runs == 0 {
            reporter.log("LLM 辅助分类三次均失败,本批保持人工".to_string());
            continue;
        }
        for (index, vs) in votes {
            let mut tally: BTreeMap<String, (usize, String)> = BTreeMap::new();
            for (class, reason) in vs {
                let e = tally.entry(class).or_insert((0, String::new()));
                e.0 += 1;
                if e.1.is_empty() {
                    e.1 = reason;
                }
            }
            let Some((class, (n, reason))) = tally.into_iter().max_by_key(|(_, (n, _))| *n) else {
                continue;
            };
            if n < 2 {
                continue; // 无多数 → 保持人工
            }
            let class = match class.as_str() {
                "A_routing" => CLASS_ROUTING,
                "B_variant_gap" => CLASS_VARIANT,
                "C_page_error" => CLASS_PAGE_ERROR,
                "D_manual" => CLASS_MANUAL,
                _ => continue,
            };
            let idx = batch[index - 1];
            let pair = &mut pairs[idx];
            // 只落 llm_suggest 建议字段:class 由确定性层+人工覆写决定,
            // 保证报告零方差(实测三票也无法稳定 A/B 边界,见 REMAINING_WORK A6)
            pair.llm_suggest = Some(class.to_string());
            pair.reason = (!reason.is_empty()).then_some(reason);
            done += 1;
        }
    }
    reporter.log(format!(
        "LLM 建议产出 {done} 对(不改 class,人工按 suggest 分诊)"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_extraction_filters_small_and_dedups() {
        let nums = extract_numbers("仇恨范围 100 单位,持续 2 秒,共 100.0 只,0.25 概率");
        assert_eq!(nums, vec!["100"], "|v|<3 剔除,100/100.0 归一去重后仅 1 个");
    }

    #[test]
    fn classify_prefers_sibling_then_prefab() {
        let base = |sibling: Option<&str>, prefab: Option<&str>, own: Option<&str>| ClassSignals {
            numbers: vec!["30".into()],
            sibling_match: sibling.map(str::to_string),
            prefab_match: prefab.map(str::to_string),
            prefab_literal: false,
            own_match: own.map(str::to_string),
            sibling_overlap: 0.0,
        };
        assert_eq!(
            classify(&base(
                Some("components/lootdropper.lua: X=30"),
                None,
                Some("own")
            )),
            CLASS_ROUTING
        );
        assert_eq!(
            classify(&base(
                None,
                Some("prefabs/hound.lua: SHARE_TARGET_DIST=30"),
                None
            )),
            CLASS_VARIANT
        );
        assert_eq!(
            classify(&base(None, None, None)),
            CLASS_PAGE_ERROR,
            "数值无出处 → 疑似页面错误"
        );
        assert_eq!(
            classify(&base(None, None, Some("own"))),
            CLASS_MANUAL,
            "数值仅命中本文件 → 语义分歧人工"
        );
        let no_num = ClassSignals {
            numbers: vec![],
            ..Default::default()
        };
        assert_eq!(classify(&no_num), CLASS_MANUAL, "无数值句 → 人工");
    }

    #[test]
    fn overlap_detects_routing_by_quote() {
        // 兄弟引文与 wording 高重叠(非数值的语义归属信号)
        let d = bigram_dice("死亡时有 1/3 概率掉落战利品", "死亡时三选一掉落战利品");
        assert!(d >= 0.5, "重叠度 {d} 应达到路由判定阈值");
        assert!(bigram_dice(" completely unrelated sentence ", "生命值归零") < 0.5);
    }
}
