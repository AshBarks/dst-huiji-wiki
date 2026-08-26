//! Impact attribution (M1b): map tree-diff hunks to prefab variants and
//! propagate associated-file changes to affected entities.
//!
//! Attribution uses NEW-side line numbers: the [`IndexArtifact`] fed here
//! must be built from the new snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::diffdata::{DiffStatus, TreeDiff};
use super::index::tuning::TuningVal;
use super::index::{role_for, IndexArtifact, Role, TuningTable};

/// When reverse propagation would enumerate more than this many entities,
/// collapse the list into a counted aggregate row (docs/UPDATE_IMPACT_PLAN.md §4.2.2).
const PROPAGATION_AGGREGATE_THRESHOLD: usize = 30;

/// Differences between two snapshots' tuning tables.
#[derive(Debug, Clone, Serialize, Default)]
pub struct TuningDiff {
    pub added: BTreeSet<String>,
    pub removed: BTreeSet<String>,
    /// Key -> (old value, new value).
    pub changed: BTreeMap<String, (TuningVal, TuningVal)>,
}

impl TuningDiff {
    pub fn diff(old: Option<&TuningTable>, new: Option<&TuningTable>) -> Self {
        let empty = TuningTable::default();
        let old = old.unwrap_or(&empty);
        let new = new.unwrap_or(&empty);
        let mut out = TuningDiff::default();
        for (k, v) in &new.values {
            match old.values.get(k) {
                None => {
                    out.added.insert(k.clone());
                }
                Some(pv) if pv != v => {
                    out.changed.insert(k.clone(), (pv.clone(), v.clone()));
                }
                Some(_) => {}
            }
        }
        for k in old.values.keys() {
            if !new.values.contains_key(k) {
                out.removed.insert(k.clone());
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// Per changed file: direct attribution + reverse propagation.
#[derive(Debug, Clone, Serialize)]
pub struct FileImpact {
    pub path: String,
    pub status: DiffStatus,
    /// Prefab variants whose builder fns overlap the hunks.
    pub attributed_variants: Vec<String>,
    /// Entities affected through the association reverse index
    /// (`prefabs/x#variant` entries).
    pub propagated_entities: Vec<String>,
    /// When `propagated_entities` is collapsed due to the aggregation
    /// threshold, the original count is stored here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub propagated_count: Option<usize>,
    pub hunk_count: usize,
}

/// Full impact report for one snapshot pair.
#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub old_id: String,
    pub new_id: String,
    pub files: Vec<FileImpact>,
    pub tuning: TuningDiff,
    /// Union of all affected entities, sorted and deduplicated.
    pub affected_entities: Vec<String>,
    /// Layer B grading summary — `Some` only when a corpus directory was
    /// supplied to update-scan (report-only; never mutates the wiki).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_b: Option<super::grade::LayerBSummary>,
}

fn hunk_overlaps_lines(hunks: &[super::diffdata::Hunk], start_line: u32, end_line: u32) -> bool {
    if start_line == 0 || end_line == 0 {
        return false;
    }
    hunks.iter().any(|h| {
        // Overlap on either side of the range pair; a pure deletion inside an
        // fn still attributes via its old-side range when new_lines == 0.
        let new_hit =
            h.new_lines > 0 && h.new_start <= end_line && h.new_start + h.new_lines > start_line;
        let old_hit =
            h.old_lines > 0 && h.old_start <= end_line && h.old_start + h.old_lines > start_line;
        new_hit || old_hit
    })
}

/// Attributes changed hunks to prefab variants:
/// - a hit inside a mapped fn range attributes to that fn's owners;
/// - a hit outside every mapped range (top-level registration code)
///   conservatively attributes to the union of all variants in the file.
fn variants_in_changed_fns(
    artifact: &IndexArtifact,
    path: &str,
    hunks: &[super::diffdata::Hunk],
) -> Vec<String> {
    let Some(ownership) = artifact.fn_owners.get(path) else {
        return Vec::new();
    };
    let ranges = artifact.fn_ranges.get(path);

    let mut hit: BTreeSet<String> = BTreeSet::new();
    let mut covered_lines = 0u64;
    for (fn_name, owners) in ownership {
        let Some(&(start, end)) = ranges.and_then(|r| r.get(fn_name)) else {
            continue;
        };
        if hunk_overlaps_lines(hunks, start, end) {
            covered_lines += (end - start + 1) as u64;
            hit.extend(owners.iter().cloned());
        }
    }

    if hit.is_empty() || covered_lines == 0 {
        // Unmapped region change: whole-file conservative attribution.
        for owners in ownership.values() {
            hit.extend(owners.iter().cloned());
        }
    }
    let mut out: Vec<String> = hit.into_iter().collect();
    out.sort();
    out
}

fn component_name_from_path(path: &str) -> Option<&str> {
    path.strip_prefix("components/")?.strip_suffix(".lua")
}

/// Prefab files that locally override any method of `component`.
/// These are the B-layer relevant users: their pages may carry hand-written
/// values that differ from component defaults.
fn component_override_files<'a>(artifact: &'a IndexArtifact, component: &str) -> BTreeSet<&'a str> {
    artifact
        .overrides
        .iter()
        .filter(|(_, marks)| marks.iter().any(|m| m.component == component))
        .map(|(file, _)| file.as_str())
        .collect()
}

/// Build the report for one snapshot pair.
pub fn build_report(
    diff: &TreeDiff,
    artifact: &IndexArtifact,
    old_tuning: Option<&TuningTable>,
    new_tuning: Option<&TuningTable>,
    old_id: &str,
    new_id: &str,
) -> ImpactReport {
    let mut files = Vec::new();
    let mut all_entities: BTreeSet<String> = BTreeSet::new();

    for fd in &diff.files {
        let role = role_for(&fd.path);
        let attributed_variants = match role {
            Role::Prefab if fd.status != DiffStatus::Removed => {
                variants_in_changed_fns(artifact, &fd.path, &fd.hunks)
            }
            _ => Vec::new(),
        };

        let mut propagated_entities = match role {
            Role::Prefab => Vec::new(),
            Role::Component => match component_name_from_path(&fd.path) {
                Some(component) => {
                    let override_files = component_override_files(artifact, component);
                    artifact
                        .reverse
                        .get(&fd.path)
                        .into_iter()
                        .flatten()
                        .filter(|entry| {
                            let file = entry.split('#').next().unwrap_or("");
                            override_files.contains(file)
                        })
                        .cloned()
                        .collect()
                }
                None => artifact.reverse.get(&fd.path).cloned().unwrap_or_default(),
            },
            _ => artifact.reverse.get(&fd.path).cloned().unwrap_or_default(),
        };
        let propagated_count = if propagated_entities.len() > PROPAGATION_AGGREGATE_THRESHOLD {
            let count = propagated_entities.len();
            propagated_entities.clear();
            all_entities.insert(format!("{}#aggregated({count})", fd.path));
            Some(count)
        } else {
            None
        };
        all_entities.extend(
            attributed_variants
                .iter()
                .map(|v| format!("{}#{v}", fd.path)),
        );
        all_entities.extend(propagated_entities.iter().cloned());

        files.push(FileImpact {
            path: fd.path.clone(),
            status: fd.status,
            attributed_variants,
            propagated_entities,
            propagated_count,
            hunk_count: fd.hunks.len(),
        });
    }

    ImpactReport {
        old_id: old_id.to_string(),
        new_id: new_id.to_string(),
        files,
        tuning: TuningDiff::diff(old_tuning, new_tuning),
        affected_entities: all_entities.into_iter().collect(),
        layer_b: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::index::build_from_sources;
    use crate::update::TreeDiff;
    use std::path::Path;

    const HOUND_OLD: &str = r#"
local function fncommon(inst)
    inst:AddComponent("combat")
    inst.components.combat:SetDefaultDamage(20)
    return inst
end

local function fndefault()
    return fncommon("hound")
end

return Prefab("hound", fndefault, {}, {})
"#;

    const HOUND_NEW: &str = r#"
local function fncommon(inst)
    inst:AddComponent("combat")
    inst.components.combat:SetDefaultDamage(30)
    return inst
end

local function fndefault()
    return fncommon("hound")
end

return Prefab("hound", fndefault, {}, {})
"#;

    const COMBAT_OLD: &str = "local Combat = Class(function(self, inst)\n    self.dmg = 10\nend)\n";
    const COMBAT_NEW: &str = "local Combat = Class(function(self, inst)\n    self.dmg = 99\nend)\n";

    fn write_tree(dir: &Path, files: &[(&str, &str)]) {
        for (path, content) in files {
            let abs = dir.join(path);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(abs, content).unwrap();
        }
    }

    #[test]
    fn prefab_hunk_attributes_to_variants_via_fn_ranges() {
        let tmp = std::env::temp_dir().join(format!("dst_impact_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let old = tmp.join("old");
        let new = tmp.join("new");
        write_tree(
            &old,
            &[
                ("prefabs/hound.lua", HOUND_OLD),
                ("components/combat.lua", COMBAT_OLD),
            ],
        );
        write_tree(
            &new,
            &[
                ("prefabs/hound.lua", HOUND_NEW),
                ("components/combat.lua", COMBAT_NEW),
            ],
        );

        // Atlas is built from the NEW tree (attribution uses new-side lines).
        let files: Vec<(String, String)> = vec![
            ("prefabs/hound.lua".into(), HOUND_NEW.into()),
            ("components/combat.lua".into(), COMBAT_NEW.into()),
        ];
        let artifact = build_from_sources(&files).unwrap();

        let diff = TreeDiff::diff_trees(&old, &new).unwrap();
        let report = build_report(&diff, &artifact, None, None, "old", "new");

        let hound = report
            .files
            .iter()
            .find(|f| f.path == "prefabs/hound.lua")
            .unwrap();
        assert_eq!(hound.attributed_variants, vec!["hound"]);
        assert_eq!(report.affected_entities, vec!["prefabs/hound.lua#hound"]);

        // combat.lua has no scan-derived ownership; propagation via reverse.
        let combat = report
            .files
            .iter()
            .find(|f| f.path == "components/combat.lua")
            .unwrap();
        assert!(combat
            .propagated_entities
            .contains(&"prefabs/hound.lua#hound".to_string()));
        assert!(combat.attributed_variants.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn component_change_only_propagates_to_local_overriders() {
        const PIG_SRC: &str = r#"
local function fnpig()
    local inst = CreateEntity()
    inst:AddComponent("combat")
    return inst
end
return Prefab("pig", fnpig, {}, {})
"#;
        let tmp = std::env::temp_dir().join(format!("dst_impact_narrow_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let old = tmp.join("old");
        let new = tmp.join("new");
        write_tree(
            &old,
            &[
                ("prefabs/hound.lua", HOUND_OLD),
                ("prefabs/pig.lua", PIG_SRC),
                ("components/combat.lua", COMBAT_OLD),
            ],
        );
        write_tree(
            &new,
            &[
                ("prefabs/hound.lua", HOUND_NEW),
                ("prefabs/pig.lua", PIG_SRC),
                ("components/combat.lua", COMBAT_NEW),
            ],
        );
        let files: Vec<(String, String)> = vec![
            ("prefabs/hound.lua".into(), HOUND_NEW.into()),
            ("prefabs/pig.lua".into(), PIG_SRC.into()),
            ("components/combat.lua".into(), COMBAT_NEW.into()),
        ];
        let artifact = build_from_sources(&files).unwrap();
        let diff = TreeDiff::diff_trees(&old, &new).unwrap();
        let report = build_report(&diff, &artifact, None, None, "old", "new");

        let combat = report
            .files
            .iter()
            .find(|f| f.path == "components/combat.lua")
            .unwrap();
        assert!(combat
            .propagated_entities
            .contains(&"prefabs/hound.lua#hound".to_string()));
        assert!(!combat
            .propagated_entities
            .contains(&"prefabs/pig.lua#pig".to_string()));
        assert_eq!(combat.propagated_entities.len(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn unmapped_region_change_attributes_to_whole_file() {
        const OLD: &str = concat!(
            "local function fncommon(inst)\n",
            "    inst:AddComponent(\"combat\")\n",
            "    return inst\n",
            "end\n",
            "\n",
            "local function fndefault()\n",
            "    return fncommon(\"hound\")\n",
            "end\n",
            "\n",
            "return Prefab(\"hound\", fndefault, {}, {})\n"
        );
        let new_src = OLD.replace(
            "return Prefab(\"hound\", fndefault, {}, {})",
            "return Prefab(\"hound\", fndefault, {}, {\"fire\"})",
        );
        let new_ref: &str = &new_src;
        let tmp = std::env::temp_dir().join(format!("dst_impact_top_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let old = tmp.join("old");
        let new = tmp.join("new");
        write_tree(&old, &[("prefabs/hound.lua", OLD)]);
        write_tree(&new, &[("prefabs/hound.lua", new_ref)]);

        let files: Vec<(String, String)> = vec![("prefabs/hound.lua".into(), new_ref.to_string())];
        let artifact = build_from_sources(&files).unwrap();
        let diff = TreeDiff::diff_trees(&old, &new).unwrap();
        let report = build_report(&diff, &artifact, None, None, "o", "n");
        let hound = report.files.first().unwrap();
        assert_eq!(hound.attributed_variants, vec!["hound"]);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn tuning_diff_reports_added_changed_removed() {
        let mk = |src: &str| crate::update::index::tuning::build_tuning(src).unwrap();
        let old = mk("TUNING={}\nfunction Tune()\nTUNING={A=1,B=2}\nend\nTune()\n");
        let new = mk("TUNING={}\nfunction Tune()\nTUNING={B=3,C=4}\nend\nTune()\n");
        let d = TuningDiff::diff(Some(&old), Some(&new));
        assert_eq!(d.added, BTreeSet::from(["C".to_string()]));
        assert_eq!(d.removed, BTreeSet::from(["A".to_string()]));
        assert_eq!(
            d.changed.get("B"),
            Some(&(TuningVal::Num(2.0), TuningVal::Num(3.0)))
        );
        assert!(!d.is_empty());
        assert!(TuningDiff::diff(Some(&old), Some(&old)).is_empty());
    }
}
