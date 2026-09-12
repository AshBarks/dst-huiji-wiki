use crate::error::Result;
use full_moon::ast;
use std::collections::{BTreeMap, HashMap, HashSet};

mod analyze;
mod cond;
mod eval;
mod scan;

use analyze::*;
use cond::*;
use eval::*;
use scan::*;

/// A single skill node extracted from a `skilltree_<character>.lua` file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillNode {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub group: Option<String>,
    pub root: bool,
    pub connects: Vec<String>,
    pub icon: Option<String>,
    pub lock: bool,
    pub locks: Vec<String>,
    pub lock_open: Option<serde_json::Value>,
    pub tags: Vec<String>,
    /// Presence markers for fields the wiki renderer only needs as flags.
    pub onactivate: bool,
    pub ondeactivate: bool,
    pub defaultfocus: bool,
    pub infographic: bool,
    pub forced_focus: Option<serde_json::Value>,
    pub button_decorations: bool,
    /// 静态装饰图（`button_decorations` 的 `CreateShelfDecor` 形式，如
    /// 薇诺娜的货架）；动态装饰（沃拓克斯天秤）仍只给标记。
    pub decorations: Vec<SkillDecoration>,
}

/// 一张静态装饰图，位置为游戏部件根坐标（已应用 `CreateShelfDecor` 里的
/// `y - 50` 偏移），渲染时按图片中心对齐。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillDecoration {
    pub img: String,
    pub pos: [f64; 2],
    /// `ScaleToSize(width, height)`；与 `scale` 互斥。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 2]>,
    /// `SetScale(scale)`（按原图尺寸缩放）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

/// A full character skill tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillTree {
    pub character: String,
    pub nodes: Vec<SkillNode>,
    /// `BACKGROUND_SETTINGS`（目前只有薇诺娜显式设置）。
    pub background: Option<BackgroundSettings>,
}

/// 角色背景的展示设置（`skilltree_<char>.lua` 的 `BACKGROUND_SETTINGS`）。
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BackgroundSettings {
    /// 前端是否给背景叠金色 tint；`Some(false)` 表示显式关闭。
    pub tint_bright: Option<bool>,
}

impl SkillTree {
    /// Returns the distinct group names in first-seen order.
    pub fn groups(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for node in &self.nodes {
            if let Some(g) = &node.group {
                if !seen.contains(g) {
                    seen.push(g.clone());
                }
            }
        }
        seen
    }
}

#[derive(Debug, Default, Clone)]
struct RawSkillDef {
    pos: Option<(f64, f64)>,
    group: Option<String>,
    root: bool,
    connects: Vec<String>,
    icon: Option<String>,
    lock: bool,
    locks: Vec<String>,
    lock_open: Option<serde_json::Value>,
    tags: Vec<String>,
    onactivate: bool,
    ondeactivate: bool,
    defaultfocus: bool,
    infographic: bool,
    forced_focus: Option<serde_json::Value>,
    button_decorations: bool,
    decorations: Vec<RawDecoration>,
}

/// 解析阶段的静态装饰图定义。
#[derive(Debug, Clone)]
struct RawDecoration {
    img: String,
    pos: (f64, f64),
    size: Option<(f64, f64)>,
    scale: Option<f64>,
}

impl RawSkillDef {
    fn is_lock(&self) -> bool {
        self.lock
    }
}

/// A skill-like local table captured during the scan (e.g. `skills`,
/// `sisturn_skills`, `potion_skills`), kept in declaration order.
struct SkillTable {
    defs: Vec<(String, RawSkillDef)>,
}

/// Shared resolution context for one parsed file.
struct ScanCtx<'a> {
    constants: &'a BTreeMap<String, f64>,
    local_fns: &'a BTreeMap<String, &'a ast::Block>,
    /// `Table.field` -> function body, e.g.
    /// `CUSTOM_FUNCTIONS.CalculateInclination`. Used to resolve character
    /// custom lock helpers without hard-coding their bodies.
    table_fns: &'a BTreeMap<String, &'a ast::Block>,
    /// 局部变量名 -> 静态装饰列表（`CreateShelfDecor({...})` 赋值）。
    decorations: &'a BTreeMap<String, Vec<RawDecoration>>,
}

/// Parses a `skilltree_<character>.lua` source into a structured tree.
///
/// The game files define skills as keyed table entries (usually a local
/// `skills` table, sometimes split into per-theme subsets merged through a
/// `finalize_skill_group`-style helper, as in wendy). Coordinates may live in
/// a separate `POSITIONS` table or inline in each def, and may reference
/// numeric locals or simple arithmetic (`math.floor` included), so a small
/// constant folder is included.
///
/// `lock_open` closures are translated into the declarative JSON condition
/// language used by the wiki (`CountTags` / `CountSkills` / comparisons /
/// `And`/`Or`/`Not`, plus `Inclination` for Wortox's nice/naughty balance).
/// Conditions that cannot be resolved statically (external achievements,
/// custom functions) fall back to "open", mirroring the wiki's own handling;
/// the condition text is documented in the skill description.
pub fn parse_skill_tree(source: &str, character: &str) -> Result<SkillTree> {
    parse_skill_tree_with_tuning(source, character, &BTreeMap::new())
}

/// Same as [`parse_skill_tree`], but resolves `TUNING.*` references found in
/// the skill source (e.g. Wortox's inclination threshold). `tuning` maps
/// dotted keys **without** the `TUNING.` prefix to numeric values, as
/// produced by `update::index::tuning::build_tuning_leaves`.
pub fn parse_skill_tree_with_tuning(
    source: &str,
    character: &str,
    tuning: &BTreeMap<String, f64>,
) -> Result<SkillTree> {
    let ast = full_moon::parse(source).map_err(crate::Error::LuaParse)?;

    let mut constants = BTreeMap::new();
    collect_numeric_locals(ast.nodes().stmts(), &mut constants);
    // TUNING.* lookups share the constants map; keys keep the full
    // expression text (`TUNING.SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD`).
    for (key, value) in tuning {
        constants.insert(format!("TUNING.{}", key), *value);
    }

    let mut local_fns: BTreeMap<String, &ast::Block> = BTreeMap::new();
    collect_local_functions(ast.nodes().stmts(), &mut local_fns);

    let mut table_fns: BTreeMap<String, &ast::Block> = BTreeMap::new();
    collect_table_functions(ast.nodes().stmts(), &mut table_fns);

    let mut decorations: BTreeMap<String, Vec<RawDecoration>> = BTreeMap::new();
    collect_decorations(ast.nodes().stmts(), &constants, &mut decorations);

    let ctx = ScanCtx {
        constants: &constants,
        local_fns: &local_fns,
        table_fns: &table_fns,
        decorations: &decorations,
    };

    let mut scan = ScanState::default();
    let mut positions: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    scan_block(ast.nodes(), &ctx, &mut scan, &mut positions);

    let defs = scan.finalize();

    let mut nodes = Vec::new();
    for (name, def) in defs {
        let position = def.pos.or_else(|| positions.get(&name).copied());
        let Some((x, y)) = position else {
            continue;
        };
        let lock = def.is_lock();
        // Mirror the per-file post-processing found at the bottom of every
        // game skilltree file: locks carry a "lock" tag, regular skills carry
        // their group as a tag and default their icon to the skill name.
        let mut tags = def.tags;
        if lock {
            if !tags.iter().any(|t| t == "lock") {
                tags.push("lock".to_string());
            }
        } else if let Some(group) = &def.group {
            if !tags.iter().any(|t| t == group) {
                tags.push(group.clone());
            }
        }
        // 显式声明的 icon 一律保留：信息板锁（如沃拓克斯的天秤好/坏倾向）
        // 既有 lock_open 又有 icon，游戏里图标同样显示；只有没有显式 icon
        // 的锁才保持无图标，普通技能则回退为同名图标。
        let icon = def
            .icon
            .or_else(|| if lock { None } else { Some(name.clone()) });
        let decorations = def
            .decorations
            .into_iter()
            .map(|d| SkillDecoration {
                img: d.img,
                pos: [d.pos.0, d.pos.1],
                size: d.size.map(|(w, h)| [w, h]),
                scale: d.scale,
            })
            .collect();
        nodes.push(SkillNode {
            name,
            x,
            y,
            group: def.group,
            root: def.root,
            connects: def.connects,
            icon,
            lock,
            locks: def.locks,
            lock_open: def.lock_open,
            tags,
            onactivate: def.onactivate,
            ondeactivate: def.ondeactivate,
            defaultfocus: def.defaultfocus,
            infographic: def.infographic,
            forced_focus: def.forced_focus,
            button_decorations: def.button_decorations,
            decorations,
        });
    }

    Ok(SkillTree {
        character: character.to_string(),
        nodes,
        background: parse_background_settings(ast.nodes().stmts()),
    })
}

/// Reads the top-level `BACKGROUND_SETTINGS = { tint_bright = ... }` table.
/// Non-table (or table) tint values both mean "tint enabled"; only an
/// explicit `false` disables it. `None` means the file has no settings.
fn parse_background_settings<'a>(
    stmts: impl Iterator<Item = &'a ast::Stmt>,
) -> Option<BackgroundSettings> {
    for stmt in stmts {
        let ast::Stmt::LocalAssignment(assignment) = stmt else {
            continue;
        };
        for (name, expr) in assignment
            .names()
            .iter()
            .zip(assignment.expressions().iter())
        {
            if name.token().to_string() != "BACKGROUND_SETTINGS" {
                continue;
            }
            let ast::Expression::TableConstructor(t) = expr else {
                continue;
            };
            let mut settings = BackgroundSettings::default();
            for field in t.fields() {
                if field_name(field).as_deref() != Some("tint_bright") {
                    continue;
                }
                settings.tint_bright = match field_value(field) {
                    Some(ast::Expression::Symbol(s)) if s.token().to_string() == "false" => {
                        Some(false)
                    }
                    Some(_) => Some(true),
                    None => None,
                };
            }
            return Some(settings);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
local TORCH_X = -190
local ALCHEMY_X = -58

local ORDERS =
{
    {"torch",           { TORCH_X   , 176 + 30 }},
    {"alchemy",         { ALCHEMY_X , 176 + 30 }},
}

local function BuildSkillsData(SkillTreeFns)
    local skills =
    {
        wilson_alchemy_1 = {
            title = STRINGS.SKILLTREE.WILSON.WILSON_ALCHEMY_1_TITLE,
            desc = STRINGS.SKILLTREE.WILSON.WILSON_ALCHEMY_1_DESC,
            icon = "wilson_alchemy_1",
            pos = {ALCHEMY_X, 176},
            group = "alchemy",
            tags = {"alchemy"},
            root = true,
            connects = {
                "wilson_alchemy_2",
                "wilson_alchemy_3",
            },
        },
        wilson_alchemy_2 = {
            icon = "wilson_alchemy_gem_1",
            pos = {ALCHEMY_X, 176-54},
            group = "alchemy",
            connects = {
                "wilson_alchemy_5",
            },
        },
        wilson_torch_1 = {
            icon = "wilson_torch",
            pos = {TORCH_X, 100},
            group = "torch",
            tags = {"torch"},
            root = true,
        },
        wilson_torch_lock = {
            group = "torch",
            root = true,
            lock_open = function(prefabname, activatedskills, readonly)
                return SkillTreeFns.CountTags(prefabname, "torch1", activatedskills) > 2
            end,
            connects = {
                "wilson_torch_7",
            },
        },
        wilson_torch_7 = {
            icon = "wilson_torch_throw",
            pos = {TORCH_X,58-38},
            group = "torch",
            tags = {"torch"},
            locks = {"wilson_torch_lock"},
        },
    }

    for name, data in pairs(skills) do
        local uppercase_name = string.upper(name)
        data.pos = data.pos
        data.desc = data.desc or STRINGS.SKILLTREE.WILSON[uppercase_name.."_DESC"]
        if not data.lock_open then
            data.title = data.title or STRINGS.SKILLTREE.WILSON[uppercase_name.."_TITLE"]
            data.icon = data.icon or name
        end
    end

    return {
        SKILLS = skills,
        ORDERS = ORDERS,
    }
end

return BuildSkillsData
"#;

    #[test]
    fn test_parse_sample_tree() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        assert!(tree.nodes.iter().any(|n| n.name == "wilson_alchemy_1"));
        let alchemy1 = tree
            .nodes
            .iter()
            .find(|n| n.name == "wilson_alchemy_1")
            .unwrap();
        assert_eq!(alchemy1.x, -58.0);
        assert_eq!(alchemy1.y, 176.0);
        assert_eq!(alchemy1.icon.as_deref(), Some("wilson_alchemy_1"));
        assert!(alchemy1.root);
        assert_eq!(
            alchemy1.connects,
            vec!["wilson_alchemy_2", "wilson_alchemy_3"]
        );

        let alchemy2 = tree
            .nodes
            .iter()
            .find(|n| n.name == "wilson_alchemy_2")
            .unwrap();
        assert_eq!(alchemy2.y, 122.0); // 176 - 54
        assert_eq!(alchemy2.icon.as_deref(), Some("wilson_alchemy_gem_1"));

        let torch = tree
            .nodes
            .iter()
            .find(|n| n.name == "wilson_torch_1")
            .unwrap();
        assert_eq!(torch.x, -190.0);
        assert_eq!(torch.y, 100.0);
    }

    #[test]
    fn test_parse_sample_groups() {
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        let groups = tree.groups();
        assert!(groups.contains(&"alchemy".to_string()));
        assert!(groups.contains(&"torch".to_string()));
        let tree = parse_skill_tree(SAMPLE, "wilson").unwrap();
        assert!(tree.nodes.iter().all(|n| n.name.starts_with("wilson_")));
    }

    #[test]
    fn test_parse_positions_table_and_lock_helpers() {
        let source = r#"
local POSITIONS = {
    skill_one = { 10, 20 },
    lock_one = { x = 30, y = 40 },
}

local function BuildSkillsData(SkillTreeFns)
    local function MakeLock()
        return { lock_open = function() return true end }
    end

    local skills = {
        skill_one = { group = "g" },
        lock_one = MakeLock(),
    }

    return SkillTreeFns.CreateSkillTree(skills)
end
"#;
        let tree = parse_skill_tree(source, "test").unwrap();
        assert_eq!(tree.nodes.len(), 2);

        let skill = tree.nodes.iter().find(|n| n.name == "skill_one").unwrap();
        assert_eq!((skill.x, skill.y), (10.0, 20.0));
        assert!(!skill.lock);
        assert_eq!(skill.icon.as_deref(), Some("skill_one"));

        let lock = tree.nodes.iter().find(|n| n.name == "lock_one").unwrap();
        assert_eq!((lock.x, lock.y), (30.0, 40.0));
    }

    #[test]
    fn test_group_tag_and_lock_tag_post_processing() {
        let source = r#"
local skills = {
    a_one = { pos = {0, 0}, group = "g", tags = {"custom"} },
    b_lock = { pos = {1, 1}, group = "g", lock_open = function() return true end },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let a = tree.nodes.iter().find(|n| n.name == "a_one").unwrap();
        assert_eq!(a.tags, vec!["custom", "g"]);
        let b = tree.nodes.iter().find(|n| n.name == "b_lock").unwrap();
        assert_eq!(b.tags, vec!["lock"]);
        assert!(b.icon.is_none());
    }

    #[test]
    fn test_inline_count_tags_lock_translation() {
        let source = r#"
local skills = {
    torch_lock = {
        pos = {0, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountTags(prefabname, "torch1", activatedskills) > 2
        end,
    },
    bernie_lock = {
        pos = {1, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            local bernie_skills = SkillTreeFns.CountTags(prefabname, "bernie4", activatedskills)
            return bernie_skills >= 4
        end,
    },
    skills_lock = {
        pos = {2, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountSkills(prefabname, activatedskills) >= 12
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let torch = tree.nodes.iter().find(|n| n.name == "torch_lock").unwrap();
        assert_eq!(
            torch.lock_open,
            Some(serde_json::json!({
                "GreaterThan": { "left": { "CountTags": "torch1" }, "right": 2 }
            }))
        );
        let bernie = tree.nodes.iter().find(|n| n.name == "bernie_lock").unwrap();
        assert_eq!(
            bernie.lock_open,
            Some(serde_json::json!({
                "GreaterOrEqThan": { "left": { "CountTags": "bernie4" }, "right": 4 }
            }))
        );
        let skills_lock = tree.nodes.iter().find(|n| n.name == "skills_lock").unwrap();
        assert_eq!(
            skills_lock.lock_open,
            Some(serde_json::json!({
                "GreaterOrEqThan": { "left": { "CountSkills": "CountSkills" }, "right": 12 }
            }))
        );
    }

    #[test]
    fn test_external_lock_defaults_open() {
        let source = r#"
local function CreateAccomplishmentLockFn(key)
    return
        function(prefabname, activatedskills, readonly)
            return readonly and "question" or TheGenericKV:GetKV(key) == "1"
        end
end

local skills = {
    song_lock = {
        pos = {0, 0},
        lock_open = CreateAccomplishmentLockFn("wathgrithr_horn_played"),
    },
    affinity_lock = {
        pos = {1, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            if readonly then
                return "question"
            end
            return TheGenericKV:GetKV("fuelweaver_killed") == "1"
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        for name in ["song_lock", "affinity_lock"] {
            let node = tree.nodes.iter().find(|n| n.name == name).unwrap();
            assert_eq!(node.lock_open, Some(serde_json::json!(true)), "{}", name);
        }
    }

    #[test]
    fn test_compound_lock_translation() {
        let source = r#"
local skills = {
    beaver_lock = {
        pos = {0, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return
                SkillTreeFns.CountTags(prefabname, "beaver", activatedskills) >= 3 and
                SkillTreeFns.CountTags(prefabname, "moose_epic", activatedskills) == 0 and
                SkillTreeFns.CountTags(prefabname, "goose_epic", activatedskills) == 0
        end,
    },
    portable_lock = {
        pos = {1, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return activatedskills and activatedskills["winona_portable_structures"] and SkillTreeFns.CountTags(prefabname, "lowshelf", activatedskills) > 2
        end,
    },
    shelf_lock = {
        pos = {2, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountTags(prefabname, "lowshelf", activatedskills) + SkillTreeFns.CountTags(prefabname, "midshelf", activatedskills) > 5
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let beaver = tree.nodes.iter().find(|n| n.name == "beaver_lock").unwrap();
        assert_eq!(
            beaver.lock_open,
            Some(serde_json::json!({
                "And": {
                    "left": { "GreaterOrEqThan": { "left": { "CountTags": "beaver" }, "right": 3 } },
                    "right": {
                        "And": {
                            "left": { "Eq": { "left": { "CountTags": "moose_epic" }, "right": 0 } },
                            "right": { "Eq": { "left": { "CountTags": "goose_epic" }, "right": 0 } }
                        }
                    }
                }
            }))
        );
        let portable = tree
            .nodes
            .iter()
            .find(|n| n.name == "portable_lock")
            .unwrap();
        assert_eq!(
            portable.lock_open,
            Some(serde_json::json!({
                "And": {
                    "left": { "ActivatedSkill": "winona_portable_structures" },
                    "right": { "GreaterThan": { "left": { "CountTags": "lowshelf" }, "right": 2 } }
                }
            }))
        );
        let shelf = tree.nodes.iter().find(|n| n.name == "shelf_lock").unwrap();
        assert_eq!(
            shelf.lock_open,
            Some(serde_json::json!({
                "GreaterThan": {
                    "left": {
                        "Add": {
                            "left": { "CountTags": "lowshelf" },
                            "right": { "CountTags": "midshelf" }
                        }
                    },
                    "right": 5
                }
            }))
        );
    }

    #[test]
    fn test_local_fn_resolution_and_guards() {
        let source = r#"
local function BasicShadowAllegianceLockFn(prefabname, activatedskills, readonly)
    if SkillTreeFns.CountTags(prefabname, "lunar_favor", activatedskills) > 0 then
        return false
    end
    if readonly then
        return "question"
    end
    return TheGenericKV:GetKV("fuelweaver_killed") == "1"
end

local skills = {
    woby_shadow_lock = {
        pos = {0, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            if not SkillTreeFns.HasTag(prefabname, "woby_dash", activatedskills) then
                return false
            end
            return BasicShadowAllegianceLockFn(prefabname, activatedskills, readonly)
        end,
    },
    direct_ref_lock = {
        pos = {1, 0},
        lock_open = BasicShadowAllegianceLockFn,
    },
    guarded_count_lock = {
        pos = {2, 0},
        lock_open = function(prefabname, activatedskills, readonly)
            local maxbodies = SkillTreeFns.CountTags(prefabname, "wx78_maxbody", activatedskills)
            if maxbodies == 0 then
                return false
            end
            local shadow_skills = SkillTreeFns.CountTags(prefabname, "shadow_favor", activatedskills)
            if shadow_skills > 0 then
                return false
            end
            if readonly then
                return "question"
            end
            return TheGenericKV:GetKV("celestialchampion_killed") == "1"
        end,
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        // woby_shadow_lock：HasTag(woby_dash) 前提 + BasicShadow… 的
        // CountTags(lunar_favor)==0，两者 AND。
        let woby_expected = serde_json::json!({
            "And": {
                "left": { "GreaterOrEqThan": { "left": { "CountTags": "woby_dash" }, "right": 1 } },
                "right": { "Eq": { "left": { "CountTags": "lunar_favor" }, "right": 0 } }
            }
        });
        let node = tree
            .nodes
            .iter()
            .find(|n| n.name == "woby_shadow_lock")
            .unwrap();
        assert_eq!(node.lock_open, Some(woby_expected));
        // direct_ref_lock 没有 HasTag 守卫，只剩 allegiance 条件。
        let expected = serde_json::json!({
            "Eq": { "left": { "CountTags": "lunar_favor" }, "right": 0 }
        });
        let node = tree
            .nodes
            .iter()
            .find(|n| n.name == "direct_ref_lock")
            .unwrap();
        assert_eq!(node.lock_open, Some(expected));
        let guarded = tree
            .nodes
            .iter()
            .find(|n| n.name == "guarded_count_lock")
            .unwrap();
        assert_eq!(
            guarded.lock_open,
            Some(serde_json::json!({
                "And": {
                    "left": { "GreaterOrEqThan": { "left": { "CountTags": "wx78_maxbody" }, "right": 1 } },
                    "right": { "Eq": { "left": { "CountTags": "shadow_favor" }, "right": 0 } }
                }
            }))
        );
    }

    #[test]
    fn test_wendy_style_subset_tables() {
        let source = r#"
local function BuildSkillsData(SkillTreeFns)
    local skills = {}
    local function finalize_skill_group(skill_subset, group_name)
        for skill_name, skill_data in pairs(skill_subset) do
            skills[skill_name] = skill_data
        end
    end

    local sisturn_skills =
    {
        wendy_sisturn_1 = {
            pos = {103,173},
            tags = {"sisturn"},
            root = true,
            connects = { "wendy_sisturn_2" },
            defaultfocus = true,
        },
        wendy_sisturn_2 = {
            pos = {144,156},
            tags = {"sisturn"},
            onactivate = function(inst) end,
        },
    }

    finalize_skill_group(sisturn_skills, "sisturn_upgrades")

    local potion_skills =
    {
        wendy_potion_1 = { pos = {10, 10}, tags = {"potion"} },
    }
    finalize_skill_group(potion_skills, "potion_upgrades")

    return { SKILLS = skills }
end
"#;
        let tree = parse_skill_tree(source, "wendy").unwrap();
        assert_eq!(tree.nodes.len(), 3);
        let sisturn = tree
            .nodes
            .iter()
            .find(|n| n.name == "wendy_sisturn_1")
            .unwrap();
        assert_eq!(sisturn.group.as_deref(), Some("sisturn_upgrades"));
        assert!(sisturn.tags.contains(&"sisturn_upgrades".to_string()));
        assert!(sisturn.defaultfocus);
        let sisturn2 = tree
            .nodes
            .iter()
            .find(|n| n.name == "wendy_sisturn_2")
            .unwrap();
        assert!(sisturn2.onactivate);
        let potion = tree
            .nodes
            .iter()
            .find(|n| n.name == "wendy_potion_1")
            .unwrap();
        assert_eq!(potion.group.as_deref(), Some("potion_upgrades"));
    }

    #[test]
    fn test_presence_fields() {
        let source = r#"
local skills = {
    inf = {
        pos = {0, 0},
        infographic = true,
        forced_focus = { left = "a", right = "b" },
        button_decorations = { init = function() end },
    },
}
"#;
        let tree = parse_skill_tree(source, "t").unwrap();
        let inf = tree.nodes.iter().find(|n| n.name == "inf").unwrap();
        assert!(inf.infographic);
        assert!(inf.button_decorations);
        assert_eq!(
            inf.forced_focus,
            Some(serde_json::json!({ "left": "a", "right": "b" }))
        );
    }

    const WORTOX_INCLINATION: &str = r#"
local CUSTOM_FUNCTIONS;CUSTOM_FUNCTIONS = {
    CalculateInclination = function(nice, naughty, affinitytype)
        local diff = nice - naughty
        if affinitytype then
            if diff < 0 then
                diff = diff - 1
            elseif diff > 0 then
                diff = diff + 1
            end
        end
        if math.abs(diff) >= TUNING.SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD then
            if nice > naughty then
                return "nice"
            else
                return "naughty"
            end
        end
        return nil
    end,
}

local skills = {
    wortox_inclination_meter = {
        pos = {4, 202},
        group = "neutral",
        root = true,
        infographic = true,
        icon = "wortox_scales",
        button_decorations = { init = function() end },
    },
    wortox_inclination_nice = {
        pos = {-166, 230},
        group = "neutral",
        root = true,
        infographic = true,
        icon = "wortox_inclination_nice",
        lock_open = function(prefabname, activatedskills, readonly)
            local nice = SkillTreeFns.CountTags(prefabname, "nice", activatedskills)
            local naughty = SkillTreeFns.CountTags(prefabname, "naughty", activatedskills)
            local affinitytype = activatedskills and (activatedskills["wortox_allegiance_lunar"] and "lunar" or activatedskills["wortox_allegiance_shadow"] and "shadow") or nil
            return CUSTOM_FUNCTIONS.CalculateInclination(nice, naughty, affinitytype) == "nice"
        end,
    },
}
"#;

    #[test]
    fn test_inclination_condition_uses_tuning_threshold() {
        let tuning = BTreeMap::from([("SKILLS.WORTOX.TIPPED_BALANCE_THRESHOLD".to_string(), 3.0)]);
        let tree = parse_skill_tree_with_tuning(WORTOX_INCLINATION, "wortox", &tuning).unwrap();
        let node = tree
            .nodes
            .iter()
            .find(|n| n.name == "wortox_inclination_nice")
            .unwrap();
        // 信息板锁保留显式 icon（游戏里图标照常显示）。
        assert!(node.lock);
        assert!(node.infographic);
        assert_eq!(node.icon.as_deref(), Some("wortox_inclination_nice"));

        let lock_open = node.lock_open.as_ref().unwrap();
        assert_eq!(lock_open["Eq"]["right"], serde_json::json!("nice"));
        let inclination = &lock_open["Eq"]["left"]["Inclination"];
        assert_eq!(
            inclination["nice"],
            serde_json::json!({ "CountTags": "nice" })
        );
        assert_eq!(
            inclination["naughty"],
            serde_json::json!({ "CountTags": "naughty" })
        );
        assert_eq!(inclination["threshold"], serde_json::json!(3));
        let affinity = inclination["affinity"].to_string();
        assert!(affinity.contains("wortox_allegiance_lunar"));
        assert!(affinity.contains("lunar"));
        assert!(affinity.contains("wortox_allegiance_shadow"));
    }

    #[test]
    fn test_inclination_without_tuning_stays_open() {
        let tree = parse_skill_tree(WORTOX_INCLINATION, "wortox").unwrap();
        let node = tree
            .nodes
            .iter()
            .find(|n| n.name == "wortox_inclination_nice")
            .unwrap();
        // 阈值无法解析时退回“视为解锁”，不产生半截条件。
        assert_eq!(node.lock_open, Some(serde_json::json!(true)));
    }

    const WINONA_DECOR: &str = r#"
local SHELF_WIDTH = 520

local function CreateShelfDecor(shelfdata)
    return {
        init = function(button, root, fromfrontend) end,
        onlocked = function(button) end,
        onunlocked = function(button) end,
    }
end

local WINONA_SHELF_LOCK_DECOR_LOW = CreateShelfDecor({{
    imagename = "winona_background1.tex",
    width = SHELF_WIDTH,
    height = 90,
    x = -3,
    y = -18,
}})

local WINONA_DECOR_WAGSTAFF = CreateShelfDecor({{
    imagename = "winona_background4.tex",
    scale = 0.65,
    x = 3,
    y = 219,
}})

local BACKGROUND_SETTINGS = {
    tint_bright = false,
    tint_dim = false,
}

local skills = {
    winona_lowshelf_lock = {
        pos = {-220, 20},
        root = true,
        tags = {"lock"},
        lock_open = function(prefabname, activatedskills, readonly)
            return SkillTreeFns.CountTags(prefabname, "lowshelf", activatedskills) > 2
        end,
        button_decorations = WINONA_SHELF_LOCK_DECOR_LOW,
    },
    winona_wagstaff_2 = {
        icon = "winona_wagstaff_2",
        pos = {110, 60},
        button_decorations = WINONA_DECOR_WAGSTAFF,
    },
}
"#;

    #[test]
    fn test_winona_shelf_decorations_and_background_settings() {
        let tree = parse_skill_tree(WINONA_DECOR, "winona").unwrap();
        let low = tree
            .nodes
            .iter()
            .find(|n| n.name == "winona_lowshelf_lock")
            .unwrap();
        assert!(low.button_decorations);
        assert_eq!(low.decorations.len(), 1);
        let dec = &low.decorations[0];
        assert_eq!(dec.img, "winona_background1");
        // CreateShelfDecor 里的 SetPosition(x, y-50)。
        assert_eq!(dec.pos, [-3.0, -68.0]);
        assert_eq!(dec.size, Some([520.0, 90.0]));
        assert_eq!(dec.scale, None);

        let wagstaff = tree
            .nodes
            .iter()
            .find(|n| n.name == "winona_wagstaff_2")
            .unwrap();
        let dec = &wagstaff.decorations[0];
        assert_eq!(dec.img, "winona_background4");
        assert_eq!(dec.pos, [3.0, 169.0]);
        assert_eq!(dec.size, None);
        assert_eq!(dec.scale, Some(0.65));

        // 背景设置：tint_bright = false。
        let background = tree.background.as_ref().unwrap();
        assert_eq!(background.tint_bright, Some(false));
    }
}
