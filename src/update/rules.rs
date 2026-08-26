//! Tier0 impact rules (M1c): map changed-path prefixes to existing
//! maintain-* job names for scheduling and reconciliation registration.
//!
//! The report is read-only: it lists which static pipeline jobs WOULD run;
//! actual execution stays a human decision (per plan v2 §4.2.0).

use serde::Serialize;

/// One rule: changed paths matching `prefix` schedule the listed jobs.
#[derive(Debug, Clone, Serialize)]
pub struct Tier0Rule {
    pub prefix: String,
    pub label: String,
    pub jobs: Vec<String>,
}

impl Tier0Rule {
    pub fn matches(&self, path: &str) -> bool {
        path.starts_with(&self.prefix)
    }
}

/// Default registry mirroring the shipped maintenance pipelines.
pub fn default_rules() -> Vec<Tier0Rule> {
    vec![
        Tier0Rule {
            prefix: "languages/".to_string(),
            label: "PO 翻译".to_string(),
            jobs: vec!["parse-po".to_string()],
        },
        Tier0Rule {
            prefix: "recipes.lua".to_string(),
            label: "配方".to_string(),
            jobs: vec![
                "map-recipes".to_string(),
                "maintain-dst-recipes".to_string(),
            ],
        },
        Tier0Rule {
            prefix: "recipe_filter".to_string(),
            label: "配方过滤".to_string(),
            jobs: vec!["maintain-dst-recipes".to_string()],
        },
        Tier0Rule {
            prefix: "tuning.lua".to_string(),
            label: "TUNING 数值（进入 F2 数值提取）".to_string(),
            jobs: vec![],
        },
        Tier0Rule {
            prefix: "strings".to_string(),
            label: "游戏字符串（核对 DST JSON bot 是否同步）".to_string(),
            jobs: vec!["maintain-item-table".to_string()],
        },
    ]
}

/// Evaluate all rules against the changed paths.
pub fn evaluate(rules: &[Tier0Rule], changed_paths: &[String]) -> Vec<RuleHit> {
    let mut hits = Vec::new();
    for rule in rules {
        let matched: Vec<&String> = changed_paths.iter().filter(|p| rule.matches(p)).collect();
        if matched.is_empty() {
            continue;
        }
        hits.push(RuleHit {
            label: rule.label.clone(),
            jobs: rule.jobs.clone(),
            files: matched.into_iter().cloned().collect(),
        });
    }
    hits
}

/// A triggered (or informational) rule with its matched files.
#[derive(Debug, Clone, Serialize)]
pub struct RuleHit {
    pub label: String,
    pub jobs: Vec<String>,
    pub files: Vec<String>,
}
