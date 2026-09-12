//! Local corpus layout under `wikis/<host>/` (gitignored).
//!
//! ```text
//! wikis/<host>/
//! ├── manifest.json            # per-round summary
//! ├── meta.jsonl               # authoritative PageMeta map (rewritten per round)
//! ├── pages/<pageid>.wikitext  # pageid-addressed (titles are not path-safe)
//! ├── index/redirects.json     # derived: redirect title -> target
//! ├── index/classes_summary.json # derived: class counts + unknown list
//! └── _removed/<round>/<pageid>.wikitext
//! ```
//!
//! Only raw facts live on disk; every derived view can be rebuilt from
//! `meta.jsonl` plus the page files.

use crate::corpus::model::{CorpusManifest, GameClass, PageMeta};
use crate::error::{Error, Result};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Consumes the leading `#重定向` / `#redirect` keyword (ASCII
/// case-insensitive) and returns the remainder of the line content.
fn strip_redirect_keyword(trimmed: &str) -> Option<&str> {
    if let Some(rest) = trimmed.strip_prefix("#重定向") {
        return Some(rest);
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= "#redirect".len()
        && bytes[.."#redirect".len()].eq_ignore_ascii_case(b"#redirect")
    {
        return Some(&trimmed["#redirect".len()..]);
    }
    None
}

pub struct CorpusStore {
    root: PathBuf,
}

impl CorpusStore {
    /// `base` is the corpus base directory (default `wikis/`), `host` the
    /// wiki host — one tree per wiki.
    pub fn new(base: &Path, host: &str) -> Self {
        Self {
            root: base.join(host),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn pages_dir(&self) -> PathBuf {
        self.root.join("pages")
    }

    pub fn page_path(&self, pageid: i64) -> PathBuf {
        self.pages_dir().join(format!("{}.wikitext", pageid))
    }

    /// Loads `meta.jsonl`; a missing file means an empty corpus.
    pub fn load_meta(&self) -> Result<BTreeMap<i64, PageMeta>> {
        let path = self.root.join("meta.jsonl");
        let mut map = BTreeMap::new();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(map),
            Err(e) => return Err(e.into()),
        };
        for (idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let meta: PageMeta = serde_json::from_str(line)
                .map_err(|e| Error::Config(format!("meta.jsonl 第 {} 行损坏: {}", idx + 1, e)))?;
            map.insert(meta.pageid, meta);
        }
        Ok(map)
    }

    /// Atomically-enough rewrite of `meta.jsonl` (temp file + rename).
    pub fn save_meta(&self, meta: &BTreeMap<i64, PageMeta>) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let target = self.root.join("meta.jsonl");
        let mut body = String::with_capacity(256 * meta.len());
        for m in meta.values() {
            body.push_str(&serde_json::to_string(m)?);
            body.push('\n');
        }
        crate::platform::fs::write_text_atomic(&target, &body)?;
        Ok(())
    }

    pub fn write_page(&self, pageid: i64, text: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(self.pages_dir())?;
        let path = self.page_path(pageid);
        crate::platform::fs::write_text_atomic(&path, text)?;
        Ok(path)
    }

    /// Local wikitext of a page, or `None` when the file is absent.
    pub fn read_page(&self, pageid: i64) -> Result<Option<String>> {
        match std::fs::read_to_string(self.page_path(pageid)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Moves a page file into `_removed/<tag>/`; returns the archive path or
    /// `None` when the page had no local file.
    pub fn archive_removed(&self, pageid: i64, tag: &str) -> Result<Option<PathBuf>> {
        let src = self.page_path(pageid);
        if !src.exists() {
            return Ok(None);
        }
        let dir = self.root.join("_removed").join(tag);
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(
            src.file_name()
                .ok_or_else(|| Error::Config(format!("pageid {} 产生非法文件名", pageid)))?,
        );
        std::fs::rename(&src, &dest)?;
        Ok(Some(dest))
    }

    pub fn save_manifest(&self, manifest: &CorpusManifest) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let path = self.root.join("manifest.json");
        crate::platform::fs::write_json_atomic(&path, manifest)?;
        Ok(())
    }

    fn read_page_text(&self, pageid: i64) -> Option<String> {
        std::fs::read_to_string(self.page_path(pageid)).ok()
    }

    /// Extracts the redirect target from redirect-page wikitext
    /// (`#重定向 [[目标]]` / `#REDIRECT [[Target]]`, optional leading `:`).
    pub fn redirect_target(wikitext: &str) -> Option<String> {
        let trimmed = wikitext.trim_start();
        let after_hash = strip_redirect_keyword(trimmed)?;
        let start = after_hash.find("[[")? + 2;
        let rest = &after_hash[start..];
        let end = rest.find(']')?;
        let target = &rest[..end];
        let target = target.split('|').next().unwrap_or(target).trim();
        if target.is_empty() {
            None
        } else {
            Some(target.trim_start_matches(':').to_string())
        }
    }

    /// Rebuilds the derived indexes from current metadata (+ redirect page
    /// files for targets): `index/redirects.json`, `index/classes_summary.json`.
    pub fn save_derived_indexes(&self, meta: &BTreeMap<i64, PageMeta>) -> Result<()> {
        let index_dir = self.root.join("index");
        std::fs::create_dir_all(&index_dir)?;

        let mut redirects = BTreeMap::new();
        for m in meta.values().filter(|m| m.redirect) {
            // The mapping is the point of keeping redirect pages; fall back to
            // an empty target when the file cannot be read rather than failing
            // the whole index build.
            let target = self
                .read_page_text(m.pageid)
                .as_deref()
                .and_then(Self::redirect_target)
                .unwrap_or_default();
            redirects.insert(m.title.clone(), target);
        }
        let redirects_path = index_dir.join("redirects.json");
        crate::platform::fs::write_json_atomic(&redirects_path, &redirects)?;

        let mut class_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut unknown: Vec<String> = Vec::new();
        for m in meta.values() {
            *class_counts
                .entry(m.game_class.name().to_string())
                .or_default() += 1;
            if m.game_class == GameClass::Unknown && !m.redirect {
                unknown.push(m.title.clone());
            }
        }
        let summary = json!({
            "counts": class_counts,
            "unknown_titles": unknown,
        });
        crate::platform::fs::write_json_atomic(index_dir.join("classes_summary.json"), &summary)?;
        Ok(())
    }
}

/// Unique temp dir without external dev-dependencies. Shared by sibling test
/// modules via `store::tests_temp_dir`.
#[cfg(test)]
pub(crate) fn tests_temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "corpus-test-{}-{}-{}",
        tag,
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp dir without external dev-dependencies.
    fn temp_root(tag: &str) -> PathBuf {
        tests_temp_dir(tag)
    }

    fn sample_meta(pageid: i64, title: &str) -> PageMeta {
        PageMeta {
            pageid,
            title: title.to_string(),
            touched: None,
            len: None,
            redirect: false,
            rev_sha1: None,
            categories: vec![],
            game_class: GameClass::Dst,
            class_signals: vec![],
            class_confidence: crate::corpus::model::ClassConfidence::High,
        }
    }

    #[test]
    fn meta_save_load_roundtrip_and_missing_file() {
        let root = temp_root("meta");
        let store = CorpusStore::new(&root, "example.huijiwiki.com");
        assert!(store.load_meta().unwrap().is_empty());

        let mut map = BTreeMap::new();
        map.insert(1, sample_meta(1, "猎犬"));
        map.insert(2, sample_meta(2, "虎鲨"));
        store.save_meta(&map).unwrap();

        let loaded = store.load_meta().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[&1].title, "猎犬");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn corrupted_meta_line_reports_config_error() {
        let root = temp_root("corrupt");
        let store = CorpusStore::new(&root, "example.huijiwiki.com");
        std::fs::create_dir_all(root.join("example.huijiwiki.com")).unwrap();
        std::fs::write(
            root.join("example.huijiwiki.com").join("meta.jsonl"),
            "{oops\n",
        )
        .unwrap();
        assert!(store.load_meta().is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn write_archive_and_derive_indexes() {
        let root = temp_root("derive");
        let store = CorpusStore::new(&root, "example.huijiwiki.com");

        store.write_page(7, "#重定向 [[猎犬]]\n").unwrap();
        assert!(store.page_path(7).exists());

        let mut meta = BTreeMap::new();
        let mut redir = sample_meta(7, "野狗");
        redir.redirect = true;
        redir.game_class = GameClass::Redirect;
        meta.insert(7, redir);
        meta.insert(8, sample_meta(8, "猎犬"));

        store.save_derived_indexes(&meta).unwrap();

        let redirects: BTreeMap<String, String> = serde_json::from_str(
            &std::fs::read_to_string(root.join("example.huijiwiki.com/index/redirects.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(redirects.get("野狗").map(String::as_str), Some("猎犬"));

        let summary: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("example.huijiwiki.com/index/classes_summary.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(summary["counts"]["redirect"], 1);
        assert_eq!(summary["counts"]["dst"], 1);

        // Archive removes the file from pages/.
        let dest = store.archive_removed(7, "r1").unwrap().unwrap();
        assert!(dest.exists());
        assert!(!store.page_path(7).exists());
        assert!(store.archive_removed(9999, "r1").unwrap().is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn redirect_target_parsing_variants() {
        assert_eq!(
            CorpusStore::redirect_target("#重定向 [[猎犬]]\n"),
            Some("猎犬".to_string())
        );
        assert_eq!(
            CorpusStore::redirect_target("#REDIRECT [[Hound]]"),
            Some("Hound".to_string())
        );
        assert_eq!(
            CorpusStore::redirect_target("#重定向:[[A|显示名]]"),
            Some("A".to_string())
        );
        assert_eq!(
            CorpusStore::redirect_target("#重定向 [[:命名空间:页]]"),
            Some("命名空间:页".to_string())
        );
        assert_eq!(CorpusStore::redirect_target("不是重定向页"), None);
        assert_eq!(CorpusStore::redirect_target("#重定向 [[]]"), None);
    }
}
