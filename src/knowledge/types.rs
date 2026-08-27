//! SymbolDoc schema (v1, prompt_rev p1) — 见 docs/KNOWLEDGE_PIPELINE.md §3。
//!
//! LLM 只产出 [`SymbolDocLlm`](inner payload);`reference`/`schema_version`/
//! `prompt_rev`/`provenance` 由管线回填后落盘为 [`SymbolDoc`]。

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;
pub const PROMPT_REV: &str = "p3";

/// 一个符号的稳定标识(kind + path/name),决定文档文件名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRefKey {
    pub kind: String,
    pub path: String,
}

impl SymbolRefKey {
    /// `components/health.lua` + `component` → `component__health`
    pub fn doc_id(&self) -> String {
        let name = self
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&self.path)
            .trim_end_matches(".lua");
        format!("{}__{}", self.kind, name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub effect: String,
}

/// pass2(link-wiki)产物:一条被语料证实的「页面写作方面」及其引文。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WikiAspect {
    pub aspect: String,
    /// 证据页 → 玩家语言引文(短)。
    pub evidence: Vec<WikiEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WikiEvidence {
    pub pageid: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

/// pass2 的 LLM 原始载荷(aspects 证据尚未过滤)。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WikiLinkLlm {
    #[serde(default)]
    pub aspects: Vec<WikiAspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_evidence_reason: Option<String>,
}

/// pass2 结果:空 aspects + no_evidence_reason 是合法且常见的负结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WikiSection {
    pub scanned_pageids: Vec<i64>,
    #[serde(default)]
    pub aspects: Vec<WikiAspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_evidence_reason: Option<String>,
    /// no_wiki_mention(0 条)| partial(1~2 条)| well_documented(≥3 条)
    pub coverage_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedRef {
    pub kind: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_sha256: String,
    pub source_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_id: Option<String>,
    pub model: String,
    pub generated_at_ms: u64,
}

/// 落盘的完整文档(管线回填元数据后)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolDoc {
    pub schema_version: u32,
    pub prompt_rev: String,
    pub reference: SymbolRefKey,
    pub category: String,
    pub display_name: String,
    pub summary: String,
    pub api: Vec<ApiEntry>,
    #[serde(default)]
    pub events_published: Vec<String>,
    #[serde(default)]
    pub events_listened: Vec<String>,
    #[serde(default)]
    pub netvars: Vec<String>,
    #[serde(default)]
    pub tunables: Vec<String>,
    /// pass2 语料归因结果;pass1 生成时为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiki: Option<WikiSection>,
    #[serde(default)]
    pub search_terms: Vec<String>,
    #[serde(default)]
    pub gameplay_tags: Vec<String>,
    #[serde(default)]
    pub related_symbols: Vec<RelatedRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_note: Option<String>,
    pub provenance: Provenance,
}

/// LLM 应返回的 inner payload(不含管线回填字段)。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SymbolDocLlm {
    /// 可由管线从 SymbolRefKey 推导;模型省略时回填。
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub display_name: String,
    pub summary: String,
    #[serde(default)]
    pub api: Vec<ApiEntry>,
    #[serde(default)]
    pub events_published: Vec<String>,
    #[serde(default)]
    pub events_listened: Vec<String>,
    #[serde(default)]
    pub netvars: Vec<String>,
    #[serde(default)]
    pub tunables: Vec<String>,
    #[serde(default)]
    pub gameplay_tags: Vec<String>,
    /// 面向维基全文检索的玩家语言词汇(中文为主,可混英文);pass2 采样依据。
    #[serde(default)]
    pub search_terms: Vec<String>,
    #[serde(default)]
    pub related_symbols: Vec<RelatedRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_note: Option<String>,
}

impl SymbolDocLlm {
    /// 硬校验:component 必有 summary 与 api。
    pub fn validate(&self) -> Result<(), String> {
        if self.summary.trim().is_empty() {
            return Err("summary 为空".to_string());
        }
        if self.api.is_empty() {
            return Err("api 为空(component 必有方法)".to_string());
        }
        Ok(())
    }
}

/// 由 LLM payload + 管线元数据合成落盘文档。
pub fn assemble(
    llm: &SymbolDocLlm,
    key: &SymbolRefKey,
    source_sha: &str,
    source_bytes: usize,
    build_id: Option<String>,
    model: &str,
) -> SymbolDoc {
    SymbolDoc {
        schema_version: SCHEMA_VERSION,
        prompt_rev: PROMPT_REV.to_string(),
        reference: key.clone(),
        // 模型未回填时用管线推导值兜底(kind / 文件名词干)。
        category: if llm.category.is_empty() {
            key.kind.clone()
        } else {
            llm.category.clone()
        },
        display_name: if llm.display_name.is_empty() {
            key.path
                .rsplit('/')
                .next()
                .unwrap_or(&key.path)
                .trim_end_matches(".lua")
                .to_string()
        } else {
            llm.display_name.clone()
        },
        summary: llm.summary.clone(),
        api: llm.api.clone(),
        events_published: llm.events_published.clone(),
        events_listened: llm.events_listened.clone(),
        netvars: llm.netvars.clone(),
        tunables: llm.tunables.clone(),
        wiki: None,
        search_terms: llm.search_terms.clone(),
        gameplay_tags: llm.gameplay_tags.clone(),
        related_symbols: llm.related_symbols.clone(),
        truncation_note: llm.truncation_note.clone(),
        provenance: crate::knowledge::store::provenance(source_sha, source_bytes, build_id, model),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SymbolDocLlm {
        serde_json::from_str(
            r#"{
              "category": "component",
              "display_name": "health",
              "summary": "生命值组件。",
              "api": [{"name": "DoDelta", "effect": "扣血回血"}],
              "events_published": ["healthdelta"]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn llm_payload_minimal_parses_with_defaults() {
        let d = sample();
        assert_eq!(d.display_name, "health");
        assert_eq!(d.api.len(), 1);
        d.validate().unwrap();
    }

    #[test]
    fn missing_category_display_name_defaults_from_key() {
        // 模型省略 category/display_name 时仍可解析,assemble 用 key 兜底。
        let payload = r#"{"summary":"s","api":[{"name":"a","effect":"e"}]}"#;
        let llm: SymbolDocLlm = serde_json::from_str(payload).unwrap();
        assert!(llm.category.is_empty());
        let key = SymbolRefKey {
            kind: "component".into(),
            path: "components/health.lua".into(),
        };
        let doc = crate::knowledge::types::assemble(&llm, &key, "sha", 1, None, "m");
        assert_eq!(doc.category, "component");
        assert_eq!(doc.display_name, "health");
    }

    #[test]
    fn validate_rejects_empty_summary_or_api() {
        let mut d = sample();
        d.summary = "  ".to_string();
        assert!(d.validate().is_err());
        let mut d = sample();
        d.api.clear();
        assert!(d.validate().is_err());
    }

    #[test]
    fn doc_id_naming() {
        assert_eq!(
            SymbolRefKey {
                kind: "component".into(),
                path: "components/health.lua".into()
            }
            .doc_id(),
            "component__health"
        );
    }

    #[test]
    fn full_doc_roundtrip() {
        let llm = sample();
        let doc = crate::knowledge::types::assemble(
            &llm,
            &SymbolRefKey {
                kind: "component".into(),
                path: "components/health.lua".into(),
            },
            "abc123",
            22115,
            Some("62704002".to_string()),
            "hy3",
        );
        assert_eq!(doc.schema_version, SCHEMA_VERSION);
        let txt = serde_json::to_string(&doc).unwrap();
        let back: SymbolDoc = serde_json::from_str(&txt).unwrap();
        assert_eq!(back, doc);
    }
}
