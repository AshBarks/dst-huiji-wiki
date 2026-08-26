//! Code association index (infrastructure A).
//!
//! See `docs/CODE_ASSOCIATION_INFRA.md`. Pass 1 ([`scan`]) collects per-file
//! AST facts; Pass 2 ([`resolve`]) turns them into edges, override marks,
//! behaviour-call records and the unresolved ledger; [`genericity`] tiers
//! components by usage.

pub mod edges;
pub mod genericity;
pub mod resolve;
pub mod scan;
pub mod symbols;
pub mod tuning;

use std::collections::BTreeMap;
use std::path::Path;

use crate::Result;
pub use edges::IndexArtifact;
use edges::{AssocEdge, EdgeKind, UnresolvedNote, INDEX_SCHEMA_VERSION};
use symbols::{FileKey, FileScan, Role};
pub use tuning::TuningTable;

/// Directories scanned relative to the scripts root.
const SCAN_DIRS: &[&str] = &[
    "prefabs",
    "components",
    "stategraphs",
    "brains",
    "behaviours",
];
/// Root-level helper files profiled for `MakeXxx` expansion (E2).
const UTIL_FILES: &[&str] = &["standardcomponents.lua", "prefabutil.lua"];

fn role_for(path: &str) -> Role {
    if path.starts_with("prefabs/") {
        Role::Prefab
    } else if path.starts_with("components/") {
        Role::Component
    } else if path.starts_with("stategraphs/") {
        Role::StateGraph
    } else if path.starts_with("brains/") {
        Role::Brain
    } else if path.starts_with("behaviours/") {
        Role::Behaviour
    } else {
        Role::Other
    }
}

/// Build the index from an in-memory file set: `(relative path, content)`.
/// Pass 1 only: scan every source, recording parse failures.
fn scan_files(files: &[(String, String)]) -> (BTreeMap<FileKey, FileScan>, Vec<UnresolvedNote>) {
    let mut scans: BTreeMap<FileKey, FileScan> = BTreeMap::new();
    let mut parse_failures = Vec::new();
    for (path, content) in files {
        let role = role_for(path);
        match scan::Scanner::scan(path.clone(), role, content) {
            Ok(file_scan) => {
                scans.insert(path.clone(), file_scan);
            }
            Err(errors) => parse_failures.push(UnresolvedNote {
                file: path.clone(),
                line: 0,
                kind: "parse_failure",
                detail: format!("{errors:?}"),
            }),
        }
    }
    (scans, parse_failures)
}

pub fn build_from_sources<S: AsRef<str>>(files: &[(S, S)]) -> Result<IndexArtifact> {
    let mut sorted: Vec<(String, String)> = files
        .iter()
        .map(|(p, c)| (normalize(p.as_ref()), c.as_ref().to_string()))
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let (scans, parse_failures) = scan_files(&sorted);
    Ok(assemble(scans, parse_failures, None))
}

/// Collect all association-relevant script sources from a scripts root.
fn gather_script_files(root: &Path) -> Result<Vec<(String, String)>> {
    let mut files: Vec<(String, String)> = Vec::new();
    for dir in SCAN_DIRS {
        let dir_abs = root.join(dir);
        let entries = std::fs::read_dir(&dir_abs)
            .map_err(crate::Error::Io)?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(crate::Error::Io)?;
        for entry in entries {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "lua") {
                let rel = format!("{dir}/{}", entry.file_name().to_string_lossy());
                let content = std::fs::read_to_string(&p)?;
                files.push((rel, content));
            }
        }
    }
    for util in UTIL_FILES {
        let abs = root.join(util);
        if abs.is_file() {
            let content = std::fs::read_to_string(&abs)?;
            files.push(((*util).to_string(), content));
        }
    }
    Ok(files)
}

/// Build the index from a game scripts directory (current tree or snapshot).
pub fn build_from_dir(root: &Path) -> Result<IndexArtifact> {
    let files = gather_script_files(root)?;
    build_from_sources(&files)
}

/// Combined output of one full atlas build (association index + tuning).
#[derive(Debug, Clone, serde::Serialize)]
pub struct AtlasBuild {
    pub schema_version: u32,
    /// `version.txt` build identifier when available.
    pub build_id: Option<String>,
    pub index: IndexArtifact,
    pub tuning: TuningTable,
}

/// Build the atlas (index + tuning) from a scripts root directory.
///
/// `tuning.lua` and `version.txt` are read from the root when present.
pub fn build_atlas_from_dir(root: &Path) -> Result<AtlasBuild> {
    let files = gather_script_files(root)?;
    let index = build_from_sources(&files)?;

    let tuning_src = std::fs::read_to_string(root.join("tuning.lua")).ok();
    let tuning = match tuning_src {
        Some(src) => Some(tuning::build_tuning(&src)?),
        None => None,
    }
    .unwrap_or_default();

    let build_id = std::fs::read_to_string(root.join("version.txt"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    Ok(AtlasBuild {
        schema_version: ATLAS_SCHEMA_VERSION,
        build_id,
        index,
        tuning,
    })
}

/// Serialization contract version of the combined [`AtlasBuild`] output.
pub const ATLAS_SCHEMA_VERSION: u32 = 1;

fn assemble(
    scans: BTreeMap<FileKey, FileScan>,
    parse_failures: Vec<UnresolvedNote>,
    tuning: Option<&TuningTable>,
) -> IndexArtifact {
    let resolution = resolve::Resolver::run(&scans, tuning);

    // Reverse index over file-like targets; raw prefab names in PrefabDep
    // edges are promoted to their conventional path when the target exists.
    let known_prefab_files: std::collections::HashSet<&str> = scans
        .keys()
        .filter(|k| k.starts_with("prefabs/"))
        .map(String::as_str)
        .collect();

    let mut reverse: BTreeMap<FileKey, Vec<String>> = BTreeMap::new();
    let mut edges_out: Vec<AssocEdge> = Vec::with_capacity(resolution.edges.len());
    for edge in resolution.edges {
        let target = match edge.kind {
            EdgeKind::PrefabDep => {
                let candidate = format!("prefabs/{}.lua", edge.target);
                if known_prefab_files.contains(candidate.as_str()) {
                    candidate
                } else {
                    edge.target.clone()
                }
            }
            _ => edge.target.clone(),
        };
        reverse
            .entry(target.clone())
            .or_default()
            .push(format!("{}#{}", edge.prefab_file, edge.prefab_variant));
        edges_out.push(AssocEdge { target, ..edge });
    }
    for variants in reverse.values_mut() {
        variants.sort();
        variants.dedup();
    }

    let genericity = genericity::compute(&edges_out);

    IndexArtifact {
        schema_version: INDEX_SCHEMA_VERSION,
        scanned_files: scans.len(),
        parse_failures,
        edges: edges_out,
        reverse,
        overrides: resolution.overrides,
        behaviour_calls: resolution.behaviour_calls,
        unresolved: resolution.unresolved,
        genericity,
    }
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use edges::{ArgValue, Confidence, EdgeKind};
    use tuning::TuningVal;

    const STD_COMPONENTS: &str = r#"
function MakeHauntableChangePrefab(inst, data)
    inst:AddComponent("hauntable")
    return inst
end

function MakeInventoryPhysics(inst)
    inst:AddComponent("physics")
    return inst
end
"#;

    const HOUND: &str = r#"
local brain = require("brains/houndbrain")
local moonbrain = require("brains/moonbeastbrain")
local assets = {}
local prefabs = { "houndfire", "eyeflame" }

local SHARE_TARGET_DIST = 30

local function fncommon(bank, build, morphlist, custombrain, tag, data)
    local inst = CreateEntity()
    inst:AddComponent("spawnfader")
    inst:AddComponent("combat")
    inst.components.combat:SetDefaultDamage(TUNING.HOUND_DAMAGE)
    inst.components.combat:SetRange(TUNING.HOUND_ATTACK_RANGE)
    inst:SetStateGraph("SGhound")
    inst:SetBrain(custombrain or brain)
    MakeHauntableChangePrefab(inst)
    return inst
end

local function fnmoon()
    return fncommon("hound", "hound_ocean", nil, moonbrain, "moonbeast", false)
end

local function fndefault()
    return fncommon("hound", "hound", nil, nil, nil, nil)
end

return Prefab("hound", fndefault, assets, prefabs),
       Prefab("moonhound", fnmoon, assets, prefabs)
"#;

    const WARGLET: &str = r#"
local assets = {}
local function fnwarglet()
    local inst = CreateEntity()
    inst:SetStateGraph("SGhound")
    inst:AddComponent("combat")
    return inst
end
return Prefab("warglet", fnwarglet, assets)
"#;

    const SG_HOUND: &str = r#"
require("stategraphs/commonstates")
local states = {
    State{ name = "idle" },
    EventHandler("attacked", function(inst, data)
        inst.components.combat:SetTarget(data.attacker)
    end),
}
return states
"#;

    const WANDER_BEHAVIOUR: &str = r#"
Wander = Class(BehaviourNode, function(self, inst, homelocation, max_dist, times, getdirectionFn)
    BehaviourNode._ctor(self, inst)
end)
"#;

    const CHASE_BEHAVIOUR: &str = r#"
ChaseAndAttack = Class(BehaviourNode, function(self, inst, chase_time)
    BehaviourNode._ctor(self, inst)
end)
"#;

    const HOUND_BRAIN: &str = r#"
local Wander = require("behaviours/wander")
local BrainCommon = require("brains/braincommon")

local SEE_DIST = 30

local function GetHomePos(inst)
    return inst.components.knownlocations:GetLocation("home")
end

local root = PriorityNode({
    ChaseAndAttack(self.inst, 10),
    Wander(self.inst, GetHomePos, 8),
    Wander(inst, GetWanderPoint, 20),
}, 1)

return root
"#;

    const DYNAMIC_SG: &str = r#"
local function fndynamic(data)
    local inst = CreateEntity()
    inst:SetStateGraph(data.sg)
    inst:SetBrain(nil)
    MakeUnknownHelper(inst)
    return inst
end
return Prefab("dynamicsg", fndynamic, {})
"#;

    const ALIAS_TEST: &str = r#"
local function fntest(inst)
    local me = inst
    me.components.health:SetMaxHealth(150)
    target.components.combat:SetDefaultDamage(5)
    inst.components.locomotor:SetDist(4)
    return inst
end
return Prefab("aliastest", fntest, {})
"#;

    fn fixture_files() -> Vec<(String, String)> {
        vec![
            ("standardcomponents.lua".into(), STD_COMPONENTS.into()),
            ("prefabs/hound.lua".into(), HOUND.into()),
            ("prefabs/warglet.lua".into(), WARGLET.into()),
            ("stategraphs/SGhound.lua".into(), SG_HOUND.into()),
            ("behaviours/wander.lua".into(), WANDER_BEHAVIOUR.into()),
            (
                "behaviours/chaseandattack.lua".into(),
                CHASE_BEHAVIOUR.into(),
            ),
            ("brains/houndbrain.lua".into(), HOUND_BRAIN.into()),
            ("prefabs/dynamic_sg.lua".into(), DYNAMIC_SG.into()),
            ("prefabs/alias_test.lua".into(), ALIAS_TEST.into()),
        ]
    }

    fn find<'a>(artifact: &'a IndexArtifact, variant: &str, target: &str) -> Vec<&'a AssocEdge> {
        artifact
            .edges
            .iter()
            .filter(|e| e.prefab_variant == variant && e.target == target)
            .collect()
    }

    #[test]
    fn hound_association_chain_resolves() {
        let artifact = build_from_sources(&fixture_files()).unwrap();

        // E1: literal component edges owned by BOTH variants (shared fncommon).
        assert_eq!(find(&artifact, "hound", "components/combat.lua").len(), 1);
        assert_eq!(
            find(&artifact, "moonhound", "components/combat.lua").len(),
            1
        );
        assert_eq!(
            find(&artifact, "hound", "components/spawnfader.lua").len(),
            1
        );

        // E2: helper expansion through MakeHauntableChangePrefab.
        let haunt = find(&artifact, "hound", "components/hauntable.lua");
        assert_eq!(haunt.len(), 1);
        assert_eq!(haunt[0].confidence, Confidence::HelperExpanded);
        assert_eq!(haunt[0].via.as_deref(), Some("MakeHauntableChangePrefab"));

        // E3: stategraph edge.
        assert_eq!(find(&artifact, "hound", "stategraphs/SGhound.lua").len(), 1);

        // E4: parameter flow — default hound gets only houndbrain, while the
        // moon variant folds to BOTH brains via `custombrain or brain`.
        let hb_hound = find(&artifact, "hound", "brains/houndbrain.lua");
        assert_eq!(hb_hound.len(), 1);
        assert_eq!(hb_hound[0].confidence, Confidence::Direct);
        assert!(find(&artifact, "hound", "brains/moonbeastbrain.lua").is_empty());
        let mb_moon = find(&artifact, "moonhound", "brains/moonbeastbrain.lua");
        assert_eq!(mb_moon.len(), 1);
        assert_eq!(mb_moon[0].confidence, Confidence::Folded);
        assert_eq!(
            find(&artifact, "moonhound", "brains/houndbrain.lua").len(),
            1
        );

        // E5: declared deps kept as raw prefab names (target file not scanned).
        assert_eq!(find(&artifact, "hound", "houndfire").len(), 1);

        // Reverse index: SGhound shared by hound variants AND warglet.
        let sg_reverse = &artifact.reverse["stategraphs/SGhound.lua"];
        assert!(sg_reverse.contains(&"prefabs/hound.lua#hound".to_string()));
        assert!(sg_reverse.contains(&"prefabs/warglet.lua#warglet".to_string()));

        // Overrides recorded only for the constructed entity's receivers.
        let hound_overrides = &artifact.overrides["prefabs/hound.lua"];
        assert!(hound_overrides
            .iter()
            .any(|o| o.component == "combat" && o.method == "SetDefaultDamage"));
        let alias_overrides = &artifact.overrides["prefabs/alias_test.lua"];
        assert!(alias_overrides
            .iter()
            .any(|o| o.component == "health" && o.method == "SetMaxHealth"));
        assert!(alias_overrides.iter().any(|o| o.component == "locomotor"));
        // `target.components.combat:...` must NOT be attributed.
        assert!(!alias_overrides.iter().any(|o| o.component == "combat"));

        // Unresolved ledger: dynamic stategraph + unknown helper; SetBrain(nil) silent.
        let dynamic_notes: Vec<_> = artifact
            .unresolved
            .iter()
            .filter(|u| u.file == "prefabs/dynamic_sg.lua")
            .collect();
        assert!(dynamic_notes.iter().any(|u| u.kind == "sg_unresolved"));
        assert!(dynamic_notes.iter().any(|u| u.kind == "helper_unknown"));
        assert!(!dynamic_notes.iter().any(|u| u.kind == "brain_unresolved"));

        // Genericity L0 blacklist.
        assert!(artifact
            .genericity
            .blacklisted
            .contains(&"spawnfader".to_string()));
    }

    #[test]
    fn tuning_args_resolve_in_behaviour_calls() {
        const TT_PREFAB: &str = r#"
local brain = require("brains/ttbrain")

return CreatePrefab("tt", function(inst)
    inst:SetBrain(brain)
end)
"#;
        const TT_BRAIN: &str = r#"
require "behaviours/wander"

local function ttbrain(inst)
    local root = PriorityNode(
    {
        Wander(inst, GetHomePos, TUNING.HOUND_TARGET_DIST, 5),
    }, 1)
    return Brain(inst, root)
end
"#;
        let files: Vec<(String, String)> = vec![
            ("prefabs/tt.lua".into(), TT_PREFAB.into()),
            ("brains/ttbrain.lua".into(), TT_BRAIN.into()),
            ("behaviours/wander.lua".into(), WANDER_BEHAVIOUR.into()),
        ];
        let mut sorted = files.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let (scans, failures) = scan_files(&sorted);
        assert!(failures.is_empty());

        // Without the tuning table the arg stays Unknown.
        let plain = assemble(scans.clone(), Vec::new(), None);
        let call = plain
            .behaviour_calls
            .iter()
            .find(|c| c.ctor == "Wander")
            .unwrap();
        assert_eq!(
            call.args.iter().find(|(p, _)| p == "max_dist"),
            Some(&("max_dist".to_string(), ArgValue::Unknown))
        );

        // With it, `TUNING.HOUND_TARGET_DIST` resolves to its scalar value.
        let tuning = super::super::index::tuning::build_tuning(
            "TUNING = {}\nfunction Tune()\nTUNING = { HOUND_TARGET_DIST = 20 }\nend\nTune()\n",
        )
        .unwrap();
        let with_tt = assemble(scans, Vec::new(), Some(&tuning));
        let call = with_tt
            .behaviour_calls
            .iter()
            .find(|c| c.ctor == "Wander")
            .unwrap();
        assert_eq!(
            call.args.iter().find(|(p, _)| p == "max_dist"),
            Some(&("max_dist".to_string(), ArgValue::Num("20".to_string())))
        );
    }

    #[test]
    fn behaviour_calls_capture_arguments() {
        let artifact = build_from_sources(&fixture_files()).unwrap();
        let wanders: Vec<_> = artifact
            .behaviour_calls
            .iter()
            .filter(|c| c.ctor == "Wander")
            .collect();
        assert_eq!(wanders.len(), 2);
        let first = wanders[0];
        assert_eq!(first.brain_file, "brains/houndbrain.lua");
        assert_eq!(
            first.args.iter().find(|(p, _)| p == "max_dist"),
            Some(&("max_dist".to_string(), ArgValue::Num("8".to_string())))
        );
        assert_eq!(
            first.args.iter().find(|(p, _)| p == "homelocation"),
            Some(&("homelocation".to_string(), ArgValue::FnRef))
        );
        let second = wanders[1];
        assert_eq!(
            second.args.iter().find(|(p, _)| p == "max_dist"),
            Some(&("max_dist".to_string(), ArgValue::Num("20".to_string())))
        );
        // ChaseAndAttack was NOT required in this brain -> not indexed.
        assert!(!artifact
            .behaviour_calls
            .iter()
            .any(|c| c.ctor == "ChaseAndAttack"));
        // Prefab attribution flows back through resolved Brain edges.
        assert!(wanders[0]
            .prefab_variants
            .contains(&"prefabs/hound.lua#hound".to_string()));
    }

    #[test]
    fn parse_failure_is_recorded_not_fatal() {
        let files = vec![
            ("prefabs/broken.lua".to_string(), "local x = ".to_string()),
            (
                "prefabs/tiny.lua".to_string(),
                "local deps = { \"spark\" }\nreturn Prefab(\"tiny\", fntiny, {}, deps)".to_string(),
            ),
        ];
        let artifact = build_from_sources(&files).unwrap();
        assert_eq!(artifact.parse_failures.len(), 1);
        assert_eq!(artifact.parse_failures[0].file, "prefabs/broken.lua");
        assert!(artifact
            .edges
            .iter()
            .any(|e| e.prefab_variant == "tiny" && e.kind == EdgeKind::PrefabDep));
    }

    /// Real-tree smoke validation; skips gracefully without DST__ROOT.
    #[test]
    fn real_tree_smoke_if_available() {
        let root = std::env::var("DST__ROOT")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(load_dst_root_from_dotenv);
        let Some(root) = root else {
            eprintln!("skipping real-tree smoke: DST__ROOT not set");
            return;
        };
        let base = std::path::Path::new(&root);
        let scripts = base.join("data/databundles/scripts");
        let scripts = if scripts.join("prefabs").is_dir() {
            scripts
        } else {
            base.to_path_buf()
        };
        let atlas =
            build_atlas_from_dir(&scripts).unwrap_or_else(|e| panic!("build failed: {e:?}"));
        let artifact = &atlas.index;
        // TuningTable: scalar leaves resolve on the real tuning.lua.
        assert!(
            atlas.tuning.values.len() > 3000,
            "tuning values {}",
            atlas.tuning.values.len()
        );
        assert_eq!(
            atlas.tuning.resolve("TUNING.HOUND_DAMAGE"),
            Some(&TuningVal::Num(20.0))
        );
        assert_eq!(
            atlas.tuning.resolve("TUNING.HOUND_TARGET_DIST"),
            Some(&TuningVal::Num(20.0))
        );
        assert!(
            artifact.scanned_files > 2500,
            "scanned {}",
            artifact.scanned_files
        );
        assert!(artifact.parse_failures.is_empty());
        let combat = &artifact.reverse["components/combat.lua"];
        assert!(combat.len() > 200, "combat users {}", combat.len());
        // hound multi-brain resolution holds on the real tree.
        assert!(artifact
            .edges
            .iter()
            .any(|e| e.prefab_variant == "moonhound"
                && e.target == "brains/moonbeastbrain.lua"
                && e.confidence == Confidence::Folded));
        if std::env::var("IDX_DEBUG").is_ok() {
            let mut per_ukind: BTreeMap<String, usize> = BTreeMap::new();
            for u in &artifact.unresolved {
                *per_ukind.entry(u.kind.to_string()).or_default() += 1;
            }
            let mut unk: BTreeMap<String, usize> = BTreeMap::new();
            for u in &artifact.unresolved {
                if u.kind == "helper_unknown" {
                    *unk.entry(u.detail.clone()).or_default() += 1;
                }
            }
            let mut top: Vec<_> = unk.into_iter().collect();
            top.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            eprintln!("top unknown helpers: {:?}", &top[..top.len().min(12)]);
            eprintln!(
                "smoke detail: edges={} bcalls={} unresolved={:?} genericity_top={:?}",
                artifact.edges.len(),
                artifact.behaviour_calls.len(),
                per_ukind,
                artifact
                    .genericity
                    .components
                    .iter()
                    .take(5)
                    .map(|c| (c.component.clone(), c.prefab_count))
                    .collect::<Vec<_>>()
            );
        }
        assert!(!artifact.behaviour_calls.is_empty());
        assert!(artifact.behaviour_calls.iter().any(|c| c.ctor == "Wander"
            && c.args
                .iter()
                .any(|(p, v)| p == "max_dist" && matches!(v, ArgValue::Num(_)))));
        eprintln!(
            "smoke ok: {} files, {} edges, {} behaviour calls, {} unresolved",
            artifact.scanned_files,
            artifact.edges.len(),
            artifact.behaviour_calls.len(),
            artifact.unresolved.len()
        );
    }

    fn load_dst_root_from_dotenv() -> Option<String> {
        let content = std::fs::read_to_string(".env").ok()?;
        content.lines().find_map(|line| {
            line.strip_prefix("DST__ROOT=")
                .map(|v| v.trim().trim_matches('"').to_string())
                .filter(|v| !v.is_empty())
        })
    }
}
