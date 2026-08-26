//! F3 行为常量子集（plan §4.2.3）：brains 文件级数值常量提取与配对。
use super::fact::{EvidenceRef, FactChange, FactKind, Literal};
use crate::Result;
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct ConstFact {
    pub name: String,
    pub value: f64,
    pub line: u32,
    pub file: String,
}

/// Collects `local NAME = <number>` file-level constants from `brains/*.lua`.
pub fn collect_brain_consts(root: &Path) -> Result<Vec<ConstFact>> {
    let dir = root.join("brains");
    let mut out = Vec::new();
    let re = match Regex::new(r"^\s*local\s+([A-Za-z_]\w*)\s*=\s*(\d+(?:\.\d+)?)\s*$") {
        Ok(r) => r,
        Err(e) => {
            return crate::Result::Err(crate::error::Error::Config(format!("consts regex: {e}")))
        }
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "lua").unwrap_or(true) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let file = format!("brains/{}", p.file_name().unwrap().to_string_lossy());
        for (i, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("--") {
                continue;
            }
            if let Some(c) = re.captures(line) {
                if let Ok(v) = c[2].parse::<f64>() {
                    out.push(ConstFact {
                        name: c[1].to_string(),
                        value: v,
                        line: (i + 1) as u32,
                        file: file.clone(),
                    });
                }
            }
        }
    }
    Ok(out)
}

/// Pairs two snapshots' constants; (file,name) key, value change only.
pub fn pair_const_changes(old: &[ConstFact], new: &[ConstFact]) -> Vec<FactChange> {
    #[derive(Default)]
    struct Pair {
        o: Option<f64>,
        n: Option<f64>,
        file: String,
        line: u32,
    }
    let mut m: BTreeMap<(String, String), Pair> = BTreeMap::new();
    for f in old {
        let e = m
            .entry((f.file.clone(), f.name.clone()))
            .or_insert_with(|| Pair {
                file: f.file.clone(),
                line: f.line,
                ..Default::default()
            });
        e.o = Some(f.value);
    }
    for f in new {
        let e = m
            .entry((f.file.clone(), f.name.clone()))
            .or_insert_with(|| Pair {
                file: f.file.clone(),
                line: f.line,
                ..Default::default()
            });
        e.n = Some(f.value);
    }
    let mut out = Vec::new();
    for ((file, name), e) in m {
        let Pair {
            o,
            n,
            file: f,
            line: l,
        } = e;
        if o == n {
            continue;
        }
        let prefab = file
            .trim_end_matches(".lua")
            .rsplit('/')
            .next()
            .unwrap_or("?")
            .to_string();
        out.push(FactChange {
            prefab,
            kind: FactKind::Behavior,
            field: format!("const[{file}]:{name}"),
            context: None,
            source_file: f,
            old: o.map(Literal::Num),
            new: n.map(Literal::Num),
            derivation_depth: 2,
            evidence: vec![EvidenceRef { file, line: l }],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_brain(src: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "f3-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("brains")).unwrap();
        let f = dir.join("brains").join("houndbrain.lua");
        let mut w = std::fs::File::create(&f).unwrap();
        w.write_all(src.as_bytes()).unwrap();
        (dir, f)
    }

    #[test]
    fn extracts_consts_and_skips_comments_and_strings() {
        let (dir, _f) = temp_brain(
            "-- local SEED = 1\nlocal SEE_DIST = 30\nlocal NAME = \"x\"\nlocal RATE = 0.5\n",
        );
        let facts = collect_brain_consts(&dir).unwrap();
        let names: Vec<&str> = facts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["SEE_DIST", "RATE"]);
        assert_eq!(facts[0].value, 30.0);
        assert!(!facts.iter().any(|c| c.name == "NAME"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pairing_fires_only_on_value_change() {
        let (dir, _) = temp_brain("local SEE_DIST = 30\n");
        let old = collect_brain_consts(&dir).unwrap();
        let new = vec![ConstFact {
            name: "SEE_DIST".into(),
            value: 32.0,
            line: 1,
            file: "brains/houndbrain.lua".into(),
        }];
        let changes = pair_const_changes(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].derivation_depth, 2);
        assert!(pair_const_changes(&old, &old).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
