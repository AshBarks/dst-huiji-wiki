//! Function-level annotation skeletons (P4): deterministic markdown derived
//! strictly from the association index — no game facts, no LLM.
//! Output format validated on prefabs/hound.lua, then batch-applied.

use std::collections::{BTreeMap, BTreeSet};

use super::index::edges::{AssocEdge, Confidence, EdgeKind, IndexArtifact};
use crate::Result;

fn confidence_str(c: Confidence) -> &'static str {
    match c {
        Confidence::Direct => "Direct",
        Confidence::Folded => "Folded",
        Confidence::HelperExpanded => "HelperExpanded",
    }
}

fn render_edge_section<'a>(
    out: &mut String,
    title: &str,
    kind: EdgeKind,
    edges: impl Iterator<Item = &'a AssocEdge>,
) {
    let mut by_target: BTreeMap<&str, Vec<&'a AssocEdge>> = BTreeMap::new();
    for e in edges.filter(|e| e.kind == kind) {
        by_target.entry(e.target.as_str()).or_default().push(e);
    }
    if by_target.is_empty() {
        return;
    }

    out.push_str(&format!("### {title}\n"));
    for (target, list) in by_target {
        let variants: BTreeSet<&str> = list.iter().map(|e| e.prefab_variant.as_str()).collect();
        let confs: BTreeSet<&str> = list.iter().map(|e| confidence_str(e.confidence)).collect();
        let lines: BTreeSet<u32> = list.iter().map(|e| e.anchor_line).collect();
        let lines_str: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        out.push_str(&format!(
            "- `{target}`：{}（{}；line {}）\n",
            variants.into_iter().collect::<Vec<_>>().join(", "),
            confs.into_iter().collect::<Vec<_>>().join("/"),
            lines_str.join(","),
        ));
    }
    out.push('\n');
}

fn render_behaviour_section(out: &mut String, path: &str, artifact: &IndexArtifact) {
    let prefix = format!("{path}#");
    let mut by_brain: BTreeMap<&str, Vec<&_>> = BTreeMap::new();
    for b in artifact
        .behaviour_calls
        .iter()
        .filter(|b| b.prefab_variants.iter().any(|v| v.starts_with(&prefix)))
    {
        by_brain.entry(b.brain_file.as_str()).or_default().push(b);
    }
    if by_brain.is_empty() {
        return;
    }

    out.push_str("### 行为\n");
    for (brain, recs) in by_brain {
        let ctors: BTreeSet<&str> = recs.iter().map(|r| r.ctor.as_str()).collect();
        let variants: BTreeSet<&str> = recs
            .iter()
            .flat_map(|r| r.prefab_variants.iter().map(String::as_str))
            .filter(|v| v.starts_with(&prefix))
            .collect();
        out.push_str(&format!(
            "- `{brain}`：{}（{}）\n",
            ctors.into_iter().collect::<Vec<_>>().join(", "),
            variants.into_iter().collect::<Vec<_>>().join(", "),
        ));
    }
    out.push('\n');
}

fn render_associations(out: &mut String, path: &str, artifact: &IndexArtifact) {
    let relevant: Vec<&AssocEdge> = artifact
        .edges
        .iter()
        .filter(|e| e.prefab_file == path)
        .collect();
    let mut sections = String::new();

    render_edge_section(
        &mut sections,
        "组件",
        EdgeKind::Component,
        relevant.iter().copied(),
    );
    render_edge_section(
        &mut sections,
        "状态图",
        EdgeKind::StateGraph,
        relevant.iter().copied(),
    );
    render_edge_section(
        &mut sections,
        "大脑",
        EdgeKind::Brain,
        relevant.iter().copied(),
    );
    render_edge_section(
        &mut sections,
        "预制体依赖",
        EdgeKind::PrefabDep,
        relevant.iter().copied(),
    );
    render_edge_section(
        &mut sections,
        "生成引用",
        EdgeKind::SpawnPrefab,
        relevant.iter().copied(),
    );
    render_behaviour_section(&mut sections, path, artifact);

    if !sections.is_empty() {
        out.push_str("## 关联\n\n");
        out.push_str(&sections);
    }
}

/// Renders one file's fn-level skeleton to markdown.
pub fn annotate_file(path: &str, artifact: &IndexArtifact) -> String {
    let mut out = String::new();
    out.push_str(&format!("# `{path}`\n\n"));
    out.push_str(&format!("- 扫描角色：{path}\n"));
    if let Some(owners) = artifact.fn_owners.get(path) {
        let mut variants: Vec<&str> = owners.values().flatten().map(|s| s.as_str()).collect();
        variants.sort_unstable();
        variants.dedup();
        out.push_str(&format!(
            "- 归属变体（{} 个）：{}\n",
            variants.len(),
            variants.join(", ")
        ));
    }
    render_associations(&mut out, path, artifact);
    out.push_str("\n## 函数\n\n");
    if let Some(ranges) = artifact.fn_ranges.get(path) {
        for (name, (start, end)) in ranges {
            out.push_str(&format!("### {name}  [{start}–{end}]\n"));
            let owners = artifact
                .fn_owners
                .get(path)
                .and_then(|m| m.get(name))
                .map(|v| v.join(", "))
                .unwrap_or_else(|| "（未归属）".to_string());
            out.push_str(&format!("- 归属：{owners}\n"));
            // loot anchors inside this fn
            let loots: Vec<String> = artifact
                .loot
                .get(path)
                .map(|recs| {
                    recs.iter()
                        .filter(|r| r.line >= *start && r.line <= *end)
                        .map(|r| format!("{}:{}", r.line, r.items.join("+")))
                        .collect()
                })
                .unwrap_or_default();
            if !loots.is_empty() {
                out.push_str(&format!("- 掉落锚点：{}\n", loots.join("；")));
            }
            out.push('\n');
        }
    }
    out
}

/// Batch skeleton generation for a file list into `out_dir`.
pub fn batch_annotate(
    artifact: &IndexArtifact,
    out_dir: &std::path::Path,
    files: &[&str],
) -> Result<Vec<std::path::PathBuf>> {
    let mut written = Vec::new();
    for f in files {
        let target = out_dir.join(format!("{f}.md"));
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, annotate_file(f, artifact))?;
        written.push(target);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::index::loot::{LootKind, LootRecord};
    use std::collections::BTreeMap;

    #[test]
    fn skeleton_covers_fns_owners_and_loot_anchors() {
        let mut artifact = IndexArtifact::default();
        let mut owners = BTreeMap::new();
        let mut m = BTreeMap::new();
        m.insert(
            "fncommon".to_string(),
            vec!["hound".to_string(), "firehound".to_string()],
        );
        owners.insert("prefabs/hound.lua".to_string(), m);
        artifact.fn_owners = owners.clone();
        let mut ranges = BTreeMap::new();
        let mut rm = BTreeMap::new();
        rm.insert("fncommon".to_string(), (1u32, 90u32));
        ranges.insert("prefabs/hound.lua".to_string(), rm);
        artifact.fn_ranges = ranges;
        artifact.loot.insert(
            "prefabs/hound.lua".to_string(),
            vec![LootRecord {
                kind: LootKind::SetLoot,
                items: vec!["monstermeat".into()],
                line: 12,
                variants: vec!["hound".into()],
            }],
        );
        let md = annotate_file("prefabs/hound.lua", &artifact);
        assert!(md.contains("fncommon"));
        assert!(md.contains("hound, firehound"));
        assert!(md.contains("掉落锚点：12:monstermeat"));
    }

    #[test]
    fn header_dedups_variants_across_functions() {
        let mut artifact = IndexArtifact::default();
        let mut owners = BTreeMap::new();
        let mut m = BTreeMap::new();
        m.insert(
            "fncommon".to_string(),
            vec!["hound".to_string(), "firehound".to_string()],
        );
        m.insert(
            "fndefault".to_string(),
            vec!["hound".to_string(), "firehound".to_string()],
        );
        owners.insert("prefabs/hound.lua".to_string(), m);
        artifact.fn_owners = owners;
        let md = annotate_file("prefabs/hound.lua", &artifact);
        assert!(md.contains("归属变体（2 个）：firehound, hound"), "{md}");
        assert!(!md.contains("firehound, hound, firehound, hound"), "{md}");
    }

    #[test]
    fn associations_are_rendered_when_edges_exist() {
        use crate::update::index::edges::{AssocEdge, Confidence, EdgeKind};

        let mut artifact = IndexArtifact::default();
        artifact.edges.push(AssocEdge {
            kind: EdgeKind::Component,
            prefab_file: "prefabs/hound.lua".to_string(),
            prefab_variant: "hound".to_string(),
            target: "components/combat.lua".to_string(),
            anchor_line: 10,
            confidence: Confidence::Direct,
            via: None,
        });
        artifact.edges.push(AssocEdge {
            kind: EdgeKind::Brain,
            prefab_file: "prefabs/hound.lua".to_string(),
            prefab_variant: "moonhound".to_string(),
            target: "brains/moonbeastbrain.lua".to_string(),
            anchor_line: 20,
            confidence: Confidence::Folded,
            via: None,
        });
        let md = annotate_file("prefabs/hound.lua", &artifact);
        assert!(md.contains("## 关联"), "{md}");
        assert!(md.contains("### 组件"), "{md}");
        assert!(md.contains("### 大脑"), "{md}");
        assert!(md.contains("components/combat.lua"), "{md}");
        assert!(md.contains("brains/moonbeastbrain.lua"), "{md}");
    }
}
