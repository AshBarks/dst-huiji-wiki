//! Function-level annotation skeletons (P4): deterministic markdown derived
//! strictly from the association index — no game facts, no LLM.
//! Output format validated on prefabs/hound.lua, then batch-applied.

use super::index::edges::IndexArtifact;
use crate::Result;
use std::collections::BTreeMap;

/// Renders one file's fn-level skeleton to markdown.
pub fn annotate_file(path: &str, artifact: &IndexArtifact) -> String {
    let mut out = String::new();
    out.push_str(&format!("# `{path}`\n\n"));
    out.push_str(&format!("- 扫描角色：{path}\n"));
    if let Some(owners) = artifact.fn_owners.get(path) {
        let variants: Vec<&str> = owners.values().flatten().map(|s| s.as_str()).collect();
        out.push_str(&format!(
            "- 归属变体（{} 个）：{}\n",
            variants.len(),
            variants.join(", ")
        ));
    }
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
            out.push_str("\n");
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
}
