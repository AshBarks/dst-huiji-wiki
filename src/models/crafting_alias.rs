//! 制作站别名派生：`模块:Constants/CraftingNames.crafting_stations` 需要
//! 补哪些键。
//!
//! wiki 端 `模块:DSTRecipe.get_recipe_filter_cn` 用 `TECH` 常量的首个
//! 科技树键（如 `CARNIVAL_GOLFPROPS`）去查 `crafting_stations`；当树键与
//! PO 站筛键（`STRINGS.UI.CRAFTING_STATION_FILTERS.<X>`）不同名时必须在
//! 表中补一个别名条目，否则页面会 `assert('制作站缺少翻译：X')`。
//!
//! 派生链：`TECH` 常量 → 科技树键 →（tuning.lua `PROTOTYPER_TREES` 反查）
//! → 条目键（恰为 PO 站筛键，如 `CARNIVALGAME_GOLFGAME`）→
//! `crafting_stations` 里的中文名。

use std::collections::{BTreeMap, BTreeSet};

/// 一条可自动生成的制作站别名。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StationAlias {
    /// TECH 常量首个科技树键（wiki 查询用），如 `CARNIVAL_GOLFPROPS`。
    pub tree_key: String,
    /// PO `STRINGS.UI.CRAFTING_STATION_FILTERS.<X>` 的 X，如
    /// `CARNIVALGAME_GOLFGAME`。
    pub filter_key: String,
    /// 触发该别名的配方样例（升序去重）。
    pub sample_recipes: Vec<String>,
}

/// 无法派生的站科技树键（只报告，不写数据）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UnresolvedTreeKey {
    pub tree_key: String,
    pub reason: UnresolvedReason,
    pub sample_recipes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    /// TECH 常量映射到多个科技树键（如 `TECH.LOST`），wiki 端首键不可确定。
    MultiKeyTech,
    /// tuning.lua 反查得到多个不同站筛键。
    AmbiguousStation,
    /// tuning.lua 与人工兜底都派生不出站筛键。
    NoStation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct StationAliasReport {
    pub aliases: Vec<StationAlias>,
    pub unresolved: Vec<UnresolvedTreeKey>,
}

/// [`derive_station_aliases`] 的输入。
pub struct StationAliasInputs<'a> {
    /// `constants.lua` TECH：常量名 → 科技树键。
    pub tech_constants: &'a BTreeMap<String, Vec<String>>,
    /// `tuning.lua` `PROTOTYPER_TREES`：原型/制作站名 → 科技树键。
    /// 条目名与 PO `CRAFTING_STATION_FILTERS` 键同名（如
    /// `CARNIVALGAME_GOLFGAME`）。
    pub prototyper_trees: &'a BTreeMap<String, Vec<String>>,
    /// 已在 CraftingNames.crafting_stations 里的站筛键。
    pub known_station_keys: &'a BTreeSet<String>,
    /// `CRAFTING_FILTERS.CRAFTING_STATION.recipes`。
    pub station_recipes: &'a [String],
    /// 配方名 → `TECH.X`（仅解析到的 `Recipe2`）。
    pub recipe_techs: &'a BTreeMap<String, String>,
    /// 人工兜底：科技树键 → 站筛键。
    pub fallback: &'a BTreeMap<String, String>,
}

/// 派生站别名与未决项。纯函数，无 I/O。
pub fn derive_station_aliases(inputs: &StationAliasInputs<'_>) -> StationAliasReport {
    // 科技树键 → 携带它的 PROTOTYPER_TREES 条目名（即 PO 站筛键）。
    let mut entries_of_tree: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (entry, tree_keys) in inputs.prototyper_trees {
        for tree_key in tree_keys {
            entries_of_tree
                .entry(tree_key.clone())
                .or_default()
                .insert(entry.clone());
        }
    }

    let mut aliases: BTreeMap<String, StationAlias> = BTreeMap::new();
    let mut unresolved: BTreeMap<String, UnresolvedTreeKey> = BTreeMap::new();

    for recipe in inputs.station_recipes {
        let Some(tech) = inputs.recipe_techs.get(recipe) else {
            continue;
        };
        let Some(tech_key) = tech.strip_prefix("TECH.") else {
            continue;
        };
        let Some(tree_keys) = inputs.tech_constants.get(tech_key) else {
            continue;
        };
        // NONE = TechTree.Create()：无制作站。
        if tree_keys.is_empty() {
            continue;
        }
        if tree_keys.len() > 1 {
            push_unresolved(
                &mut unresolved,
                tree_keys.join("|"),
                UnresolvedReason::MultiKeyTech,
                recipe,
            );
            continue;
        }
        let tree_key = tree_keys[0].as_str();
        // wiki 端对 `*OFFERING` 特判为「供奉」。
        if tree_key.ends_with("OFFERING") {
            continue;
        }
        if inputs.known_station_keys.contains(tree_key) {
            continue;
        }

        // 候选 = 携带该树键、且已是 CraftingNames 条目的 PROTOTYPER_TREES 名。
        let candidates: Vec<&String> = entries_of_tree
            .get(tree_key)
            .map(|set| {
                set.iter()
                    .filter(|entry| inputs.known_station_keys.contains(entry.as_str()))
                    .collect()
            })
            .unwrap_or_default();

        let resolved = inputs
            .fallback
            .get(tree_key)
            .filter(|key| inputs.known_station_keys.contains(key.as_str()))
            .cloned()
            .or_else(|| match candidates.as_slice() {
                [only] => Some((*only).clone()),
                _ => None,
            });

        match resolved {
            Some(filter_key) => {
                aliases
                    .entry(tree_key.to_string())
                    .or_insert_with(|| StationAlias {
                        tree_key: tree_key.to_string(),
                        filter_key,
                        sample_recipes: Vec::new(),
                    })
                    .sample_recipes
                    .push(recipe.clone());
            }
            None => {
                let reason = if candidates.len() > 1 {
                    UnresolvedReason::AmbiguousStation
                } else {
                    UnresolvedReason::NoStation
                };
                push_unresolved(&mut unresolved, tree_key.to_string(), reason, recipe);
            }
        }
    }

    let finish = |mut samples: Vec<String>| {
        samples.sort();
        samples.dedup();
        samples
    };
    StationAliasReport {
        aliases: aliases
            .into_values()
            .map(|mut a| {
                a.sample_recipes = finish(a.sample_recipes);
                a
            })
            .collect(),
        unresolved: unresolved
            .into_values()
            .map(|mut u| {
                u.sample_recipes = finish(u.sample_recipes);
                u
            })
            .collect(),
    }
}

fn push_unresolved(
    out: &mut BTreeMap<String, UnresolvedTreeKey>,
    tree_key: String,
    reason: UnresolvedReason,
    recipe: &str,
) {
    out.entry(tree_key.clone())
        .or_insert_with(|| UnresolvedTreeKey {
            tree_key,
            reason,
            sample_recipes: Vec::new(),
        })
        .sample_recipes
        .push(recipe.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(
        tech: &'a BTreeMap<String, Vec<String>>,
        trees: &'a BTreeMap<String, Vec<String>>,
        known: &'a BTreeSet<String>,
        recipes: &'a [String],
        techs: &'a BTreeMap<String, String>,
        fallback: &'a BTreeMap<String, String>,
    ) -> StationAliasInputs<'a> {
        StationAliasInputs {
            tech_constants: tech,
            prototyper_trees: trees,
            known_station_keys: known,
            station_recipes: recipes,
            recipe_techs: techs,
            fallback,
        }
    }

    #[test]
    fn test_derives_carnival_and_vault_aliases() {
        let mut tech = BTreeMap::new();
        tech.insert(
            "CARNIVAL_GOLFPROPS_ONE".to_string(),
            vec!["CARNIVAL_GOLFPROPS".to_string()],
        );
        tech.insert(
            "VAULT_REFINE_ONE".to_string(),
            vec!["VAULT_REFINE".to_string()],
        );
        let mut trees = BTreeMap::new();
        trees.insert(
            "CARNIVALGAME_GOLFGAME".to_string(),
            vec!["CARNIVAL_GOLFPROPS".to_string()],
        );
        trees.insert(
            "VAULT_REFINER_PEDESTAL".to_string(),
            vec!["VAULT_REFINE".to_string()],
        );
        let mut known = BTreeSet::new();
        known.insert("CARNIVALGAME_GOLFGAME".to_string());
        known.insert("VAULT_REFINER_PEDESTAL".to_string());
        let recipes = vec!["golf_kit".to_string(), "vault_orb".to_string()];
        let mut techs = BTreeMap::new();
        techs.insert(
            "golf_kit".to_string(),
            "TECH.CARNIVAL_GOLFPROPS_ONE".to_string(),
        );
        techs.insert("vault_orb".to_string(), "TECH.VAULT_REFINE_ONE".to_string());
        let fallback = BTreeMap::new();

        let report =
            derive_station_aliases(&inputs(&tech, &trees, &known, &recipes, &techs, &fallback));
        assert_eq!(report.aliases.len(), 2);
        assert_eq!(report.aliases[0].tree_key, "CARNIVAL_GOLFPROPS");
        assert_eq!(report.aliases[0].filter_key, "CARNIVALGAME_GOLFGAME");
        assert_eq!(report.aliases[0].sample_recipes, vec!["golf_kit"]);
        assert_eq!(report.aliases[1].tree_key, "VAULT_REFINE");
        assert!(report.unresolved.is_empty());
    }

    #[test]
    fn test_multi_key_tech_and_offering_and_known_are_skipped() {
        let mut tech = BTreeMap::new();
        tech.insert(
            "LOST".to_string(),
            vec!["MAGIC".to_string(), "SCIENCE".to_string()],
        );
        tech.insert(
            "PERDOFFERING_THREE".to_string(),
            vec!["PERDOFFERING".to_string()],
        );
        tech.insert("SCIENCE_ONE".to_string(), vec!["SCIENCE".to_string()]);
        let trees = BTreeMap::new();
        let mut known = BTreeSet::new();
        known.insert("SCIENCE".to_string());
        let recipes = vec![
            "lost_thing".to_string(),
            "shrine".to_string(),
            "bench".to_string(),
        ];
        let mut techs = BTreeMap::new();
        techs.insert("lost_thing".to_string(), "TECH.LOST".to_string());
        techs.insert("shrine".to_string(), "TECH.PERDOFFERING_THREE".to_string());
        techs.insert("bench".to_string(), "TECH.SCIENCE_ONE".to_string());
        let fallback = BTreeMap::new();

        let report =
            derive_station_aliases(&inputs(&tech, &trees, &known, &recipes, &techs, &fallback));
        assert!(report.aliases.is_empty());
        assert_eq!(report.unresolved.len(), 1);
        assert_eq!(report.unresolved[0].tree_key, "MAGIC|SCIENCE");
        assert_eq!(report.unresolved[0].reason, UnresolvedReason::MultiKeyTech);
    }

    #[test]
    fn test_fallback_overrides_and_ambiguous_station() {
        let mut tech = BTreeMap::new();
        tech.insert("A_ONE".to_string(), vec!["TREE_A".to_string()]);
        tech.insert("B_ONE".to_string(), vec!["TREE_B".to_string()]);
        let mut trees = BTreeMap::new();
        // TREE_A 有两个不同站（均为已知站筛键）→ 歧义
        trees.insert("STATION_X".to_string(), vec!["TREE_A".to_string()]);
        trees.insert("STATION_Y".to_string(), vec!["TREE_A".to_string()]);
        trees.insert("STATION_Z".to_string(), vec!["TREE_B".to_string()]);
        let mut known = BTreeSet::new();
        known.insert("STATION_X".to_string());
        known.insert("STATION_Y".to_string());
        known.insert("STATION_Z".to_string());
        let recipes = vec!["a_item".to_string(), "b_item".to_string()];
        let mut techs = BTreeMap::new();
        techs.insert("a_item".to_string(), "TECH.A_ONE".to_string());
        techs.insert("b_item".to_string(), "TECH.B_ONE".to_string());
        let mut fallback = BTreeMap::new();
        fallback.insert("TREE_B".to_string(), "STATION_Z".to_string());

        let report =
            derive_station_aliases(&inputs(&tech, &trees, &known, &recipes, &techs, &fallback));
        // TREE_A 歧义 → unresolved；TREE_B 由 fallback 解决
        assert_eq!(report.unresolved.len(), 1);
        assert_eq!(report.unresolved[0].tree_key, "TREE_A");
        assert_eq!(
            report.unresolved[0].reason,
            UnresolvedReason::AmbiguousStation
        );
        assert_eq!(report.aliases.len(), 1);
        assert_eq!(report.aliases[0].tree_key, "TREE_B");
        assert_eq!(report.aliases[0].filter_key, "STATION_Z");
    }
}
