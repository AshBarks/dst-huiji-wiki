//! Component genericity tiering and L0 noise blacklist.

use std::collections::{BTreeMap, HashSet};

use super::edges::{AssocEdge, ComponentTier, EdgeKind, GenericityReport, Tier};

/// L0 heuristic blacklist: name fragments that mark purely technical
/// components (net-var sync / render plumbing) with no page-facing facts.
const BLACKLIST_FRAGMENTS: &[&str] = &["fader", "updater", "looper", "netvar"];
/// Seed list confirmed by manual review (2026-08-26, Universal tier 24
/// components): `placer` 纯放置预览 UI、`knownlocations` 引擎位置缓存、
/// `timer` 通用计时管道——三者均无页面事实。
const BLACKLIST_SEEDS: &[&str] = &[
    "spawnfader",
    "updatelooper",
    "placer",
    "knownlocations",
    "timer",
];

pub fn compute(edges: &[AssocEdge]) -> GenericityReport {
    // Distinct prefab variants per component target.
    let mut usage: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for edge in edges {
        if edge.kind == EdgeKind::Component {
            usage
                .entry(component_name(&edge.target))
                .or_default()
                .insert(format!("{}#{}", edge.prefab_file, edge.prefab_variant));
        }
    }

    let mut components: Vec<ComponentTier> = usage
        .into_iter()
        .map(|(component, variants)| {
            let prefab_count = variants.len();
            let tier = if prefab_count >= 100 {
                Tier::Universal
            } else if prefab_count >= 10 {
                Tier::Common
            } else if prefab_count >= 2 {
                Tier::Niche
            } else {
                Tier::Singleton
            };
            ComponentTier {
                component,
                prefab_count,
                tier,
            }
        })
        .collect();

    let blacklisted: Vec<String> = components
        .iter()
        .map(|c| c.component.clone())
        .filter(|name| {
            BLACKLIST_SEEDS.contains(&name.as_str())
                || BLACKLIST_FRAGMENTS.iter().any(|f| name.contains(f))
        })
        .collect();

    components.sort_by(|a, b| {
        b.prefab_count
            .cmp(&a.prefab_count)
            .then(a.component.cmp(&b.component))
    });

    GenericityReport {
        components,
        blacklisted,
    }
}

fn component_name(target: &str) -> String {
    target
        .strip_prefix("components/")
        .and_then(|s| s.strip_suffix(".lua"))
        .unwrap_or(target)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::index::edges::Confidence;

    fn component_edge(variant: &str, component: &str) -> AssocEdge {
        AssocEdge {
            kind: EdgeKind::Component,
            prefab_file: format!("prefabs/{variant}.lua"),
            prefab_variant: variant.to_string(),
            target: format!("components/{component}.lua"),
            anchor_line: 1,
            confidence: Confidence::Direct,
            via: None,
        }
    }

    #[test]
    fn tiers_follow_usage_count() {
        let mut edges = Vec::new();
        for i in 0..100 {
            edges.push(component_edge(&format!("p{i}"), "combat"));
        }
        for i in 0..10 {
            edges.push(component_edge(&format!("q{i}"), "sleeper"));
        }
        edges.push(component_edge("rare", "moonstormstatic"));
        let report = compute(&edges);
        let combat = report
            .components
            .iter()
            .find(|c| c.component == "combat")
            .unwrap();
        assert_eq!(combat.prefab_count, 100);
        assert_eq!(combat.tier, Tier::Universal);
        let sleeper = report
            .components
            .iter()
            .find(|c| c.component == "sleeper")
            .unwrap();
        assert_eq!(sleeper.tier, Tier::Common);
        let rare = report
            .components
            .iter()
            .find(|c| c.component == "moonstormstatic")
            .unwrap();
        assert_eq!(rare.tier, Tier::Singleton);
    }

    #[test]
    fn technical_components_are_blacklisted() {
        let edges = vec![
            component_edge("a", "spawnfader"),
            component_edge("b", "updatelooper"),
            component_edge("c", "combat"),
        ];
        let report = compute(&edges);
        assert!(report.blacklisted.contains(&"spawnfader".to_string()));
        assert!(report.blacklisted.contains(&"updatelooper".to_string()));
        assert!(!report.blacklisted.contains(&"combat".to_string()));
    }
}
