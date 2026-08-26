//! Numeric-fact candidate extraction over segmented regions
//! (docs/CORPUS_CODE_ATLAS_CONTRACT.md §3).
//!
//! These are *candidates*, not assertions: they give update-scan's
//! old-literal lookup a normalized surface to match against and give the
//! atlas join stable region anchors. Semantic field naming happens later.
//!
//! Priority and span-claiming: a family claims the byte spans it matches so
//! lower-priority families skip overlapping shapes (the "12 秒" inside
//! "6 ~ 12 秒" must not double-fire). Order: interval > loot > quantity >
//! percent.

use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize)]
pub struct FactCandidate {
    pub pageid: i64,
    pub region_id: String,
    /// `interval` | `quantity` | `loot` | `percent`.
    pub fact_kind: &'static str,
    /// The exact matched text — the old_literal matching key.
    pub raw: String,
    /// Normalized values, meaning depends on `fact_kind`:
    /// interval=[lo, hi]; quantity=[n]; loot=[count, percent?]; percent=[p].
    pub values: Vec<f64>,
    pub unit: Option<String>,
    /// Match surroundings (±~24 chars, newlines flattened) for review UIs.
    pub snippet: String,
}

fn num(text: &str) -> Option<f64> {
    text.parse().ok()
}

/// Unit equivalence for old-literal matching (§4.2.6 step 3: 距离单位写法
/// 容差). The wiki writes combat distances both as `N 单位` and
/// `N 距离单位`; every other unit compares verbatim, `None` only matches
/// `None`.
pub fn units_compatible(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y || (is_distance_unit(x) && is_distance_unit(y)),
        _ => false,
    }
}

fn is_distance_unit(u: &str) -> bool {
    u == "单位" || u == "距离单位"
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9_f64.max(a.abs() * 1e-9)
}

/// Numeric tolerance per §4.2.6: `0.125 ↔ 12.5%` — one side scaled by 100
/// still matches.
pub fn numbers_compatible(a: f64, b: f64) -> bool {
    approx(a, b) || approx(a * 100.0, b) || approx(b * 100.0, a)
}

/// Whether a code-side old literal (`values`, `unit`) can refer to the same
/// page fact as a candidate extracted by this module. Requires the same
/// value count plus pairwise numeric and unit compatibility.
pub fn literal_matches(values: &[f64], unit: Option<&str>, fact: &FactCandidate) -> bool {
    values.len() == fact.values.len()
        && values
            .iter()
            .zip(&fact.values)
            .all(|(a, b)| numbers_compatible(*a, *b))
        && units_compatible(unit, fact.unit.as_deref())
}

static INTERVAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?<lo>\d+(?:\.\d+)?)\s*~\s*(?<hi>\d+(?:\.\d+)?)\s*(?<unit>距离单位|单位|秒|分钟|天)?",
    )
    .unwrap()
});

/// Loot chains: File links or Pic templates followed by `×N（P%）`.
static LOOT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:\[\[[Ff]ile:[^\]\n]+\]\]|\{\{Pic\|\d*\|[^}\n]+\}\})×(?<count>\d+)(?:（(?<pct>\d+(?:\.\d+)?)%）)?",
    )
    .unwrap()
});

static QUANTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<n>\d+(?:\.\d+)?)\s*(?<unit>距离单位|单位|秒|分钟|天|个|次|点|层)").unwrap()
});

static PERCENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"（(?<p>\d{1,3}(?:\.\d+)?)%）").unwrap());

struct Claimed {
    start: usize,
    end: usize,
}

impl Claimed {
    fn hits(&self, start: usize, end: usize) -> bool {
        self.start < end && start < self.end
    }
}

/// Largest byte offset ≤ `i` that is a char boundary (CJK-safe context
/// windows).
fn floor_boundary(text: &str, mut i: usize) -> usize {
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Smallest byte offset ≥ `i` that is a char boundary.
fn ceil_boundary(text: &str, mut i: usize) -> usize {
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[allow(clippy::too_many_arguments)]
fn push(
    out: &mut Vec<FactCandidate>,
    claimed: &mut Vec<Claimed>,
    m: &regex::Captures<'_>,
    kind: &'static str,
    values: Vec<f64>,
    unit: Option<String>,
    text: &str,
    pageid: i64,
    region_id: &str,
) {
    let whole = m.get(0).unwrap();
    let (s, e) = (whole.start(), whole.end());
    let rs = floor_boundary(text, s.saturating_sub(24));
    let re = ceil_boundary(text, (e + 24).min(text.len()));
    let mut snippet: String = text[rs..re]
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    let trimmed = snippet.trim_end().len();
    snippet.truncate(trimmed);
    out.push(FactCandidate {
        pageid,
        region_id: region_id.to_string(),
        fact_kind: kind,
        raw: text[s..e].to_string(),
        values,
        unit,
        snippet,
    });
    claimed.push(Claimed { start: s, end: e });
}

/// Extracts fact candidates from one region's text, in document order.
pub fn extract(pageid: i64, region_id: &str, text: &str) -> Vec<FactCandidate> {
    let mut out = Vec::new();
    let mut claimed: Vec<Claimed> = Vec::new();

    for c in INTERVAL.captures_iter(text) {
        let (Some(lo), Some(hi)) = (
            c.name("lo").and_then(|g| num(g.as_str())),
            c.name("hi").and_then(|g| num(g.as_str())),
        ) else {
            continue;
        };
        let unit = c.name("unit").map(|g| g.as_str().to_string());
        push(
            &mut out,
            &mut claimed,
            &c,
            "interval",
            vec![lo, hi],
            unit,
            text,
            pageid,
            region_id,
        );
    }

    for c in LOOT.captures_iter(text) {
        let mut values = Vec::with_capacity(2);
        if let Some(v) = c.name("count").and_then(|g| num(g.as_str())) {
            values.push(v);
        }
        if let Some(v) = c.name("pct").and_then(|g| num(g.as_str())) {
            values.push(v);
        }
        push(
            &mut out,
            &mut claimed,
            &c,
            "loot",
            values,
            None,
            text,
            pageid,
            region_id,
        );
    }

    for c in QUANTITY.captures_iter(text) {
        let whole = c.get(0).unwrap();
        if claimed.iter().any(|k| k.hits(whole.start(), whole.end())) {
            continue;
        }
        let Some(n) = c.name("n").and_then(|g| num(g.as_str())) else {
            continue;
        };
        let unit = c.name("unit").map(|g| g.as_str().to_string());
        push(
            &mut out,
            &mut claimed,
            &c,
            "quantity",
            vec![n],
            unit,
            text,
            pageid,
            region_id,
        );
    }

    for c in PERCENT.captures_iter(text) {
        let whole = c.get(0).unwrap();
        if claimed.iter().any(|k| k.hits(whole.start(), whole.end())) {
            continue;
        }
        let Some(p) = c.name("p").and_then(|g| num(g.as_str())) else {
            continue;
        };
        push(
            &mut out,
            &mut claimed,
            &c,
            "percent",
            vec![p],
            None,
            text,
            pageid,
            region_id,
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RID: &str = "13857:section:1";

    #[test]
    fn interval_claims_span_so_quantity_does_not_double_fire() {
        let facts = extract(1, RID, "点燃时燃烧 6 ~ 12 秒，之后熄灭。");
        assert_eq!(facts.len(), 1, "{facts:?}");
        assert_eq!(facts[0].fact_kind, "interval");
        assert_eq!(facts[0].values, vec![6.0, 12.0]);
        assert_eq!(facts[0].unit.as_deref(), Some("秒"));
        assert!(facts[0].raw.contains('~'));
    }

    #[test]
    fn file_loot_chain_with_optional_percent() {
        let text = "掉落 [[File:Monster_Meat.png|28px|link=怪物肉]]×2<br>\
                    [[File:Tentacle_Spike.png|28px|link=触手尖刺]]×1（50%）";
        let facts = extract(2, RID, text);
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].fact_kind, "loot");
        assert_eq!(facts[0].values, vec![2.0]);
        assert_eq!(facts[1].values, vec![1.0, 50.0]);
    }

    #[test]
    fn pic_loot_form() {
        let facts = extract(3, RID, "{{Pic|32|CutGrass}}×1（12.5%）");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].fact_kind, "loot");
        assert_eq!(facts[0].values, vec![1.0, 12.5]);
        assert!(!facts[0].snippet.contains('\n'));
    }

    #[test]
    fn quantities_and_distance_units() {
        let facts = extract(4, RID, "仇恨范围为 100 距离单位，攻击间隔 3 秒。");
        let kinds: Vec<&str> = facts.iter().map(|f| f.fact_kind).collect();
        assert_eq!(kinds, vec!["quantity", "quantity"]);
        assert_eq!(facts[0].unit.as_deref(), Some("距离单位"));
        assert_eq!(facts[1].values, vec![3.0]);
    }

    #[test]
    fn bare_numbers_are_ignored() {
        assert!(extract(5, RID, "版本号 16000 出现在历史记录里。").is_empty());
    }

    #[test]
    fn percent_fraction_tolerance_both_directions() {
        assert!(numbers_compatible(0.125, 12.5));
        assert!(numbers_compatible(12.5, 0.125));
        assert!(numbers_compatible(30.0, 30.0));
        assert!(!numbers_compatible(0.5, 51.0));
    }

    #[test]
    fn distance_unit_phrasing_is_equivalent() {
        assert!(units_compatible(Some("单位"), Some("距离单位")));
        assert!(units_compatible(None, None));
        assert!(!units_compatible(Some("秒"), Some("距离单位")));
        assert!(!units_compatible(Some("秒"), None));
    }

    #[test]
    fn literal_match_end_to_end() {
        // Code-side old loot chance 0.125 (fraction) vs page candidate 12.5%.
        let facts = extract(6, RID, "{{Pic|32|CutGrass}}×1（12.5%）");
        assert!(literal_matches(&[1.0, 0.125], None, &facts[0]));
        // Distance phrasing: code TUNING 100 vs page "100 距离单位".
        let d = extract(7, RID, "仇恨范围为 100 距离单位");
        assert!(literal_matches(&[100.0], Some("单位"), &d[0]));
        // Wrong unit family must not match even with equal numbers.
        let s = extract(8, RID, "燃烧 12 秒");
        assert!(!literal_matches(&[12.0], Some("天"), &s[0]));
    }
}
