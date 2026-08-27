//! knowledge/ 目录存储:文件命名、原子写入、sha256 增量判据。

use crate::error::{Error, Result};
use crate::knowledge::types::{Provenance, SymbolDoc, SymbolRefKey};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const SCHEMA_DIR: &str = "symbols";

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// `<knowledge_root>/symbols/<doc_id>.json`
pub fn doc_path(root: &Path, key: &SymbolRefKey) -> PathBuf {
    root.join(SCHEMA_DIR).join(format!("{}.json", key.doc_id()))
}

/// 原子写入(tmp + rename),父目录自动创建。
pub fn write_atomic(path: &Path, body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 读取已有文档;不存在 → Ok(None);存在但解析失败 → 报错(人工修复)。
pub fn load_doc(path: &Path) -> Result<Option<SymbolDoc>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| Error::Config(format!("已有文档损坏 {}: {e}", path.display())))
}

/// 增量判据:文档存在 且 sha256 一致 且 prompt_rev 一致 → 跳过。
pub fn is_fresh(doc: &SymbolDoc, source_sha: &str, prompt_rev: &str) -> bool {
    doc.provenance.source_sha256 == source_sha && doc.prompt_rev == prompt_rev
}

pub fn provenance(
    source_sha: &str,
    source_bytes: usize,
    build_id: Option<String>,
    model: &str,
) -> Provenance {
    Provenance {
        source_sha256: source_sha.to_string(),
        source_bytes,
        build_id,
        model: model.to_string(),
        // 与仓库既有产物一致的 epoch 毫秒约定(report_smoke.json 等)。
        generated_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_path_layout() {
        let key = SymbolRefKey {
            kind: "component".into(),
            path: "components/health.lua".into(),
        };
        let p = doc_path(Path::new("knowledge"), &key);
        assert_eq!(p, Path::new("knowledge/symbols/component__health.json"));
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn atomic_write_and_freshness() {
        let dir = std::env::temp_dir().join(format!("kn-test-{}", std::process::id()));
        let p = dir.join("symbols/x.json");
        write_atomic(&p, "{\"a\":1}").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{\"a\":1}");
        assert!(load_doc(&p).is_err()); // 不是合法 SymbolDoc → 报错而非 None
        assert!(matches!(load_doc(&dir.join("symbols/none.json")), Ok(None)));
        let _ = std::fs::remove_dir_all(dir);
    }
}
