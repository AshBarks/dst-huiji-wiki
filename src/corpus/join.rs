//! Corpus ↔ code-side index join validation (docs/CORPUS_CODE_ATLAS_CONTRACT.md §5–§6).
//!
//! Consumes the code association `index.json` artifact by path (no module
//! dependency on `crate::update`) and grades the prefab-variant join key:
//! exact matches, case-normalized extras, naming drift, and both dangling
//! sides. The report is the repeatable calibration input for wiki infobox
//! parameter fixes and code-side coverage follow-ups.

use crate::error::{Error, Result};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use super::prefab_index::PrefabRegistry;

pub const JOIN_SCHEMA_VERSION: u32 = 1;

/// Why a wiki-only variant failed to join — drives calibration work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DanglingBucket {
    /// Case difference only (`ARMOR_YOTH_KNIGHT` vs `armor_yoth_knight`);
    /// joins after lowercase normalization.
    CaseOnly,
    /// Underscore/spacing drift with exactly one squash-equal counterpart
    /// (`abyss_pillar` vs `abysspillar`) — wiki infobox fix candidate.
    NamingDrift,
    /// No counterpart found by either rule; genuine review item.
    Unresolved,
}

#[derive(Debug, Clone, Serialize)]
pub struct DanglingVariant {
    pub variant: String,
    pub pageids: Vec<i64>,
    pub bucket: DanglingBucket,
    /// Code-side counterpart for `case_only` / `naming_drift`.
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JoinReport {
    pub schema_version: u32,
    pub generated_at_ms: u64,
    pub code_index_path: String,
    pub code_variants: usize,
    pub wiki_variants: usize,
    /// Raw string equality against code-side variants.
    pub exact_matches: usize,
    /// Extra joins gained from lowercase normalization (= `CaseOnly` count).
    pub normalized_extra: usize,
    /// `(exact + normalized) / wiki_variants`.
    pub match_rate_wiki: f64,
    pub dangling_wiki: Vec<DanglingVariant>,
    /// Code-side variants no wiki page references (FX/internal prefabs are
    /// expected here).
    pub code_only_count: usize,
    pub code_only_sample: Vec<String>,
}

/// Minimal view of the code-side artifact: only edge variant strings are
/// read; unknown fields are ignored.
#[derive(serde::Deserialize)]
struct CodeIndexView {
    #[serde(default)]
    edges: Vec<EdgeView>,
}

#[derive(serde::Deserialize)]
struct EdgeView {
    #[serde(rename = "prefab_variant", default)]
    variant: String,
}

fn squash(v: &str) -> String {
    v.to_lowercase().replace('_', "")
}

/// Joins the registry against the code-side variant set and buckets every
/// dangling wiki variant into exactly one bucket.
pub fn build_join_report(
    registry: &PrefabRegistry,
    code_index_path: &Path,
    now_ms: u64,
) -> Result<JoinReport> {
    let raw = std::fs::read_to_string(code_index_path)?;
    let view: CodeIndexView = serde_json::from_str(&raw)
        .map_err(|e| Error::Config(format!("解析代码侧索引失败: {e}")))?;
    let code: BTreeSet<String> = view
        .edges
        .into_iter()
        .map(|e| e.variant)
        .filter(|v| !v.is_empty())
        .collect();

    // Bucket lookup tables over the code side.
    let code_lower: HashMap<String, Vec<&String>> =
        HashMap::from_iter(code.iter().map(|c| (c.to_lowercase(), vec![c])));
    let mut code_squash: HashMap<String, Vec<&String>> = HashMap::new();
    for c in &code {
        code_squash.entry(squash(c)).or_default().push(c);
    }

    let mut exact_matches = 0usize;
    let mut normalized_extra = 0usize;
    let mut dangling: Vec<DanglingVariant> = Vec::new();
    let mut unresolved: BTreeSet<String> = BTreeSet::new();

    for (variant, pageids) in &registry.prefabs {
        if code.contains(variant) {
            exact_matches += 1;
            continue;
        }
        // case-only: unique lowercase counterpart
        if let Some([c]) = code_lower.get(&variant.to_lowercase()).map(Vec::as_slice) {
            normalized_extra += 1;
            dangling.push(DanglingVariant {
                variant: variant.clone(),
                pageids: pageids.clone(),
                bucket: DanglingBucket::CaseOnly,
                suggestion: Some((*c).clone()),
            });
            continue;
        }
        // naming drift: exactly one squash-equal counterpart
        let sq = squash(variant);
        let hits = code_squash.get(&sq);
        if !sq.is_empty() && matches!(hits, Some(v) if v.len() == 1) {
            let suggestion = hits.unwrap()[0].clone();
            dangling.push(DanglingVariant {
                variant: variant.clone(),
                pageids: pageids.clone(),
                bucket: DanglingBucket::NamingDrift,
                suggestion: Some(suggestion),
            });
            continue;
        }
        unresolved.insert(variant.clone());
    }
    for variant in unresolved {
        dangling.push(DanglingVariant {
            pageids: registry.prefabs[&variant].clone(),
            bucket: DanglingBucket::Unresolved,
            suggestion: None,
            variant,
        });
    }
    dangling.sort_by(|a, b| a.variant.cmp(&b.variant));

    let wiki_variants = registry.prefabs.len();
    let joined = exact_matches + normalized_extra;
    // Code-only excludes variants a dangling entry already points at via
    // case/naming-drift suggestions — those prefabs DO have pages, just
    // spelled differently on the wiki.
    let suggested: BTreeSet<String> = dangling
        .iter()
        .filter_map(|d| d.suggestion.clone())
        .collect();
    let wiki_names: BTreeSet<String> = registry.prefabs.keys().cloned().chain(suggested).collect();
    let code_only: Vec<String> = code.difference(&wiki_names).cloned().collect();

    Ok(JoinReport {
        schema_version: JOIN_SCHEMA_VERSION,
        generated_at_ms: now_ms,
        code_index_path: code_index_path.display().to_string(),
        code_variants: code.len(),
        wiki_variants,
        exact_matches,
        normalized_extra,
        match_rate_wiki: if wiki_variants == 0 {
            0.0
        } else {
            joined as f64 / wiki_variants as f64
        },
        dangling_wiki: dangling,
        code_only_count: code_only.len(),
        code_only_sample: code_only.into_iter().take(24).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::model::{ClassConfidence, GameClass, PageMeta};
    use crate::corpus::prefab_index;
    use crate::corpus::store::{self, CorpusStore};
    use std::collections::BTreeMap;

    fn meta(pageid: i64, title: &str) -> PageMeta {
        PageMeta {
            pageid,
            title: title.to_string(),
            touched: None,
            len: None,
            redirect: false,
            rev_sha1: None,
            categories: Vec::new(),
            game_class: GameClass::Dst,
            class_signals: Vec::new(),
            class_confidence: ClassConfidence::High,
        }
    }

    #[test]
    fn join_buckets_case_drift_and_exact() -> Result<()> {
        let base = store::tests_temp_dir("join");
        let st = CorpusStore::new(&base, "t.example.com");
        let mut metas = BTreeMap::new();
        for (id, t) in [(1i64, "猎犬"), (2, "暗影甲"), (3, "触手"), (4, "谜页")] {
            metas.insert(id, meta(id, t));
            st.write_page(
                id,
                match id {
                    1 => "{{实体信息框/自动|dst|hound}}",
                    2 => "{{实体信息框/自动|dst|ARMOR_YOTH_KNIGHT}}",
                    3 => "{{实体信息框/自动|dst|abyss_pillar}}",
                    _ => "纯正文",
                },
            )?;
        }
        let reg = prefab_index::build_registry(&st, &metas, 7)?;

        // Code side: exact hit, lowercase counterpart, squash drift, plus an FX-only entry.
        let code_json = serde_json::json!({
            "edges": [
                {"prefab_variant": "hound"},
                {"prefab_variant": "armor_yoth_knight"},
                {"prefab_variant": "abysspillar"},
                {"prefab_variant": "abigail_flame_fx"}
            ]
        });
        let path = base.join("code_index.json");
        std::fs::write(&path, code_json.to_string())?;

        let report = build_join_report(&reg, &path, 9)?;
        assert_eq!(report.exact_matches, 1); // hound
        assert_eq!(report.normalized_extra, 1); // ARMOR_YOTH_KNIGHT
        let by_bucket = |b: DanglingBucket| -> Vec<&DanglingVariant> {
            report
                .dangling_wiki
                .iter()
                .filter(|d| d.bucket == b)
                .collect()
        };
        assert_eq!(by_bucket(DanglingBucket::CaseOnly).len(), 1);
        let drifts = by_bucket(DanglingBucket::NamingDrift);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].variant, "abyss_pillar");
        assert_eq!(drifts[0].suggestion.as_deref(), Some("abysspillar"));
        assert_eq!(report.code_only_count, 1); // abigail_flame_fx
        assert!((report.match_rate_wiki - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(report.schema_version, JOIN_SCHEMA_VERSION);
        std::fs::remove_dir_all(&base).ok();
        Ok(())
    }
}
