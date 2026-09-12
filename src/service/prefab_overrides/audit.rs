//! 预制体重定向只读审计：线上 `模块:ItemTable/PrefabOverrides` × 本地推导，
//! 对每条映射做 key / target 双重存在性验证。
//!
//! 输出三张清单：
//! - `derivable`：本地已能推导且值一致；
//! - `needs_patch`：key/target 存在性可证，但本地尚未推导（含值不一致与
//!   待补的生成族）；
//! - `noop_suspicious`：identity 空操作，或 key/target 找不到存在性证据。
//!
//! 纯只读：离线 `--wiki-file` 时不产生任何网络访问。

use crate::error::{Error, Result};
use crate::platform::progress::Reporter;
use crate::scripts_sync::anim::index::collect_lua_files;
use crate::scripts_sync::anim::skin_index::default_scripts_root;
use crate::wiki::WikiClient;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::derive_from_scripts_root;

/// 线上页面标题。
pub const PAGE_TITLE: &str = "模块:ItemTable/PrefabOverrides";
const AUDIT_SCHEMA_VERSION: u32 = 1;
/// 数据驱动族识别的香料后缀。
const SPICE_SUFFIXES: [&str; 4] = ["spice_chili", "spice_garlic", "spice_salt", "spice_sugar"];

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub prefab: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    pub reasons: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditStats {
    pub online: usize,
    pub derived: usize,
    pub derivable: usize,
    pub needs_patch: usize,
    pub noop_suspicious: usize,
    pub derived_only: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub schema_version: u32,
    pub scripts_root: String,
    pub page: String,
    pub stats: AuditStats,
    pub derivable: Vec<AuditEntry>,
    pub needs_patch: Vec<AuditEntry>,
    pub noop_suspicious: Vec<AuditEntry>,
    pub derived_only: Vec<AuditEntry>,
}

/// 从预加载源码提取的存在性证据（字面量 / 裸表键 / prefablist）。
#[derive(Debug, Default)]
pub(crate) struct EvidenceIndex {
    pub literals: BTreeSet<String>,
    pub bare_keys: BTreeSet<String>,
    pub prefablist: BTreeSet<String>,
}

impl EvidenceIndex {
    pub(crate) fn from_sources(sources: &[(String, String)]) -> Self {
        let literal_re = Regex::new(r#"["']([A-Za-z0-9_][A-Za-z0-9_\.\-]*)["']"#)
            .expect("literal regex is valid");
        let bare_key_re = Regex::new(r"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=[^=]")
            .expect("bare key regex is valid");
        let mut index = Self::default();
        for (path, text) in sources {
            for capture in literal_re.captures_iter(text) {
                index.literals.insert(capture[1].to_string());
            }
            for capture in bare_key_re.captures_iter(text) {
                index.bare_keys.insert(capture[1].to_string());
            }
            if path.ends_with("prefablist.lua") {
                for capture in literal_re.captures_iter(text) {
                    index.prefablist.insert(capture[1].to_string());
                }
            }
        }
        index
    }

    pub(crate) fn build(root: &Path) -> Result<Self> {
        let mut files = Vec::new();
        collect_lua_files(root, &mut files)?;
        files.sort();
        let mut sources = Vec::with_capacity(files.len());
        for path in files {
            if let Ok(text) = std::fs::read_to_string(&path) {
                sources.push((path.display().to_string(), text));
            }
        }
        Ok(Self::from_sources(&sources))
    }

    fn has(&self, name: &str) -> bool {
        self.literals.contains(name)
            || self.bare_keys.contains(name)
            || self.prefablist.contains(name)
    }
}

/// 解析线上页面原文中的 JSON 数据块（`[[ ... ]]` 包裹）。
pub(crate) fn parse_wiki_overrides(raw: &str) -> Result<BTreeMap<String, String>> {
    let start = raw
        .find("[[")
        .ok_or_else(|| Error::Config("页面缺少 [[ 数据块".to_string()))?;
    let end = raw
        .rfind("]]")
        .ok_or_else(|| Error::Config("页面缺少 ]] 数据块".to_string()))?;
    if end <= start + 2 {
        return Err(Error::Config("页面数据块为空".to_string()));
    }
    let map: BTreeMap<String, String> = serde_json::from_str(&raw[start + 2..end])?;
    Ok(map)
}

fn derived_pairs(derived: &BTreeMap<String, serde_json::Value>) -> BTreeMap<String, String> {
    derived
        .iter()
        .filter_map(|(prefab, value)| {
            value
                .get("override_name")
                .and_then(|v| v.as_str())
                .map(|target| (prefab.clone(), target.to_string()))
        })
        .collect()
}

fn family_hint(prefab: &str, evidence: &EvidenceIndex) -> Option<&'static str> {
    for suffix in SPICE_SUFFIXES {
        if let Some(base) = prefab.strip_suffix(&format!("_{suffix}")) {
            if evidence.has(base) {
                return Some("spice");
            }
        }
    }
    if prefab.starts_with("winter_ornament_") && evidence.has("winter_ornament_") {
        return Some("ornament");
    }
    None
}

fn entry(
    prefab: &str,
    target: &str,
    expected: Option<String>,
    reasons: Vec<String>,
    evidence: Vec<String>,
) -> AuditEntry {
    AuditEntry {
        prefab: prefab.to_string(),
        target: target.to_string(),
        expected,
        reasons,
        evidence,
    }
}

fn evidence_tags(name: &str, evidence: &EvidenceIndex) -> Vec<String> {
    let mut tags = Vec::new();
    if evidence.literals.contains(name) {
        tags.push("literal".to_string());
    }
    if evidence.bare_keys.contains(name) {
        tags.push("bare_key".to_string());
    }
    if evidence.prefablist.contains(name) {
        tags.push("prefablist".to_string());
    }
    tags
}

/// 线上条目 × 本地推导的三分类（纯函数）。
pub(crate) fn classify(
    online: &BTreeMap<String, String>,
    derived: &BTreeMap<String, serde_json::Value>,
    evidence: &EvidenceIndex,
    scripts_root: &Path,
) -> AuditReport {
    let pairs = derived_pairs(derived);
    let derived_keys: BTreeSet<&String> = pairs.keys().collect();
    let derived_values: BTreeSet<&String> = pairs.values().collect();
    let online_keys: BTreeSet<&String> = online.keys().collect();

    let mut derivable = Vec::new();
    let mut needs_patch = Vec::new();
    let mut noop_suspicious = Vec::new();

    for (prefab, target) in online {
        let mut tags = evidence_tags(prefab, evidence);
        tags.extend(
            evidence_tags(target, evidence)
                .into_iter()
                .map(|t| format!("target_{t}")),
        );
        if derived_keys.contains(prefab) {
            tags.push("derived_key".to_string());
        }
        if derived_values.contains(target) {
            tags.push("target_derived".to_string());
        }

        if prefab == target {
            noop_suspicious.push(entry(
                prefab,
                target,
                None,
                vec!["identity".to_string()],
                tags,
            ));
            continue;
        }

        if let Some(expected) = pairs.get(prefab) {
            if expected.eq_ignore_ascii_case(target) {
                derivable.push(entry(prefab, target, None, Vec::new(), tags));
            } else {
                needs_patch.push(entry(
                    prefab,
                    target,
                    Some(expected.clone()),
                    vec!["value_mismatch".to_string()],
                    tags,
                ));
            }
            continue;
        }

        let family = family_hint(prefab, evidence);
        if let Some(family) = family {
            tags.push(format!("family:{family}"));
        }
        let key_ok = evidence.has(prefab) || family.is_some() || derived_keys.contains(prefab);
        let target_ok = evidence.has(target)
            || derived_values.contains(target)
            || derived_keys.contains(target)
            || online_keys.contains(target);

        if key_ok && target_ok {
            let mut reasons = vec!["not_derived".to_string()];
            if let Some(family) = family {
                reasons.push(format!("family_pending:{family}"));
            }
            needs_patch.push(entry(prefab, target, None, reasons, tags));
        } else {
            let mut reasons = Vec::new();
            if !key_ok {
                reasons.push("key_not_found".to_string());
            }
            if !target_ok {
                reasons.push("target_not_found".to_string());
            }
            noop_suspicious.push(entry(prefab, target, None, reasons, tags));
        }
    }

    let derived_only: Vec<AuditEntry> = pairs
        .iter()
        .filter(|(prefab, _)| !online.contains_key(*prefab))
        .map(|(prefab, target)| {
            entry(
                prefab,
                target,
                None,
                vec!["derived_only".to_string()],
                evidence_tags(prefab, evidence),
            )
        })
        .collect();

    AuditReport {
        schema_version: AUDIT_SCHEMA_VERSION,
        scripts_root: scripts_root.display().to_string(),
        page: PAGE_TITLE.to_string(),
        stats: AuditStats {
            online: online.len(),
            derived: pairs.len(),
            derivable: derivable.len(),
            needs_patch: needs_patch.len(),
            noop_suspicious: noop_suspicious.len(),
            derived_only: derived_only.len(),
        },
        derivable,
        needs_patch,
        noop_suspicious,
        derived_only,
    }
}

/// `prefab-overrides-audit` 入口：只读，`wiki_file` 缺省时在线拉取。
pub async fn run_prefab_overrides_audit(
    scripts: Option<&str>,
    wiki_file: Option<&str>,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("审计预制体重定向");
    let root = resolve_scripts_root(scripts)?;
    if !root.is_dir() {
        return Err(Error::Config(format!(
            "脚本根目录不存在：{}",
            root.display()
        )));
    }
    reporter.log(format!("脚本根：{}", root.display()));

    reporter.stage("本地推导");
    let derived = derive_from_scripts_root(&root, reporter)?;

    reporter.stage("收集存在性证据");
    let evidence = EvidenceIndex::build(&root)?;
    reporter.log(format!(
        "字面量 {} / 裸键 {} / prefablist {}",
        evidence.literals.len(),
        evidence.bare_keys.len(),
        evidence.prefablist.len()
    ));

    reporter.stage("获取线上页面");
    let raw = read_page_raw(wiki_file).await?;
    let online = parse_wiki_overrides(&raw)?;
    reporter.log(format!("线上条目 {}", online.len()));

    let report = classify(&online, &derived.mapping, &evidence, &root);
    reporter.log(format!(
        "可推导 {} / 需人工补丁 {} / no-op+可疑 {} / 本地独有 {}",
        report.stats.derivable,
        report.stats.needs_patch,
        report.stats.noop_suspicious,
        report.stats.derived_only
    ));

    let text = serde_json::to_string_pretty(&report)?;
    match output {
        Some(path) => {
            crate::platform::fs::write_text_atomic(&path, &text)?;
            reporter.log(format!("审计报告已写入 {}", path.display()));
        }
        None => reporter.log(text),
    }

    Ok(serde_json::json!({
        "schema_version": AUDIT_SCHEMA_VERSION,
        "stats": report.stats,
    }))
}

/// 读取线上页面原文：显式文件优先，否则登录后在线拉取（只读）。
pub(crate) async fn read_page_raw(wiki_file: Option<&str>) -> Result<String> {
    match wiki_file {
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None => {
            let client = WikiClient::from_env()?;
            client.login().await?;
            client
                .get_page(PAGE_TITLE)
                .await?
                .content
                .ok_or_else(|| Error::Config("线上页面无内容".to_string()))
        }
    }
}

pub(crate) fn resolve_scripts_root(scripts: Option<&str>) -> Result<PathBuf> {
    match scripts {
        Some(path) => Ok(PathBuf::from(path)),
        None => default_scripts_root().ok_or_else(|| {
            Error::Config(
                "未找到游戏脚本目录：请设置 DST__ROOT（data/databundles/scripts）\
                 或显式指定 --scripts"
                    .to_string(),
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derived(mapping: &[(&str, &str)]) -> BTreeMap<String, serde_json::Value> {
        mapping
            .iter()
            .map(|(prefab, target)| {
                (
                    prefab.to_string(),
                    serde_json::json!({ "override_name": target, "type": "static" }),
                )
            })
            .collect()
    }

    #[test]
    fn test_parse_wiki_overrides() {
        let raw = r#"local overrides = [[
{
    "a": "b",
    "c": "d"
}
]]
return mw.text.jsonDecode(overrides)"#;
        let map = parse_wiki_overrides(raw).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["a"], "b");
        assert!(parse_wiki_overrides("no data here").is_err());
    }

    #[test]
    fn test_evidence_index_extracts_tokens() {
        let sources = vec![
            (
                "prefabs/a.lua".to_string(),
                "local x = \"literal_name\"\nbare_key = 1\n".to_string(),
            ),
            (
                "prefablist.lua".to_string(),
                "PREFABFILES = {\n  \"listed_name\",\n}\n".to_string(),
            ),
        ];
        let index = EvidenceIndex::from_sources(&sources);
        assert!(index.literals.contains("literal_name"));
        assert!(index.bare_keys.contains("bare_key"));
        assert!(index.prefablist.contains("listed_name"));
        assert!(index.has("bare_key"));
        assert!(!index.has("nope"));
    }

    #[test]
    fn test_classify_buckets() {
        let online: BTreeMap<String, String> = [
            ("a", "b"),
            ("c", "c"),
            ("d", "missing"),
            ("e", "g"),
            ("f", "h"),
            ("asparagussoup_spice_chili", "asparagussoup"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let derived = derived(&[("a", "b"), ("e", "x")]);
        let sources = vec![
            (
                "prefabs/a.lua".to_string(),
                "\"b\" \"g\" asparagussoup = 1 d = 2".to_string(),
            ),
            (
                "preparedfoods.lua".to_string(),
                "asparagussoup = {}".to_string(),
            ),
        ];
        let evidence = EvidenceIndex::from_sources(&sources);
        let report = classify(&online, &derived, &evidence, Path::new("/scripts"));

        assert_eq!(report.stats.online, 6);
        assert_eq!(report.stats.derivable, 1);
        assert_eq!(report.derivable[0].prefab, "a");

        let mismatch = report.needs_patch.iter().find(|e| e.prefab == "e").unwrap();
        assert_eq!(mismatch.expected.as_deref(), Some("x"));

        let spice = report
            .needs_patch
            .iter()
            .find(|e| e.prefab == "asparagussoup_spice_chili")
            .unwrap();
        assert!(spice.reasons.iter().any(|r| r == "family_pending:spice"));

        assert!(report
            .noop_suspicious
            .iter()
            .any(|e| e.prefab == "c" && e.reasons.iter().any(|r| r == "identity")));
        assert!(report.noop_suspicious.iter().any(|e| e.prefab == "d"));
        assert!(report.noop_suspicious.iter().any(|e| e.prefab == "f"));
    }

    #[test]
    fn test_collect_lua_files_is_reused() {
        let sources = vec![(
            "prefabs/x.lua".to_string(),
            "return Prefab(\"x\", function() end)".to_string(),
        )];
        let index = EvidenceIndex::from_sources(&sources);
        assert!(index.has("x"));
        assert!(collect_lua_files(Path::new("/definitely/missing"), &mut Vec::new()).is_err());
    }
}
