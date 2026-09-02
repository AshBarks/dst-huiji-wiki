//! 两源扫描与合并视图。
//!
//! 来源：
//! - **zip 源**：`<dst>/data/databundles/images.zip`（条目形如 `images/<base>.tex/.xml`）；
//! - **loose 源**：`<dst>/data/images/`（Steam 就地更新的散装目录，其中混有
//!   早期人工处理遗留的 png —— 只计数，绝不读取、绝不写入）。
//!
//! 实测两源同名 xml 零重叠、互补构成完整 atlas 集合；同名 tex 可能同时存在
//! （内容一致 → 去重，不一致 → 冲突记录并以 loose 为准）。合并视图按
//! base 名联接 xml↔tex，供 ktech / split 阶段消费。

use crate::error::Result;
use crate::scripts_sync::images::history::hash_bytes;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// zip 源解压后的落盘根（`current/unzipped/images/...`）。
pub const UNZIP_DIR_NAME: &str = "unzipped";

/// 一个 tex 文件（合并后，每个 base 至多一条）。
#[derive(Debug, Clone, Serialize)]
pub struct TexEntry {
    /// 文件名去扩展名（atlas 联接键）。
    pub base: String,
    /// 实际读取路径：loose 原位，或 zip 解压后的 `current/unzipped/...`。
    pub path: PathBuf,
    /// 内容 sha256。
    pub hash: String,
    /// 来源："zip" 或 "loose"；同名冲突时 loose 胜出。
    pub source: &'static str,
}

/// 一个 atlas（xml 及其同名 tex，tex 可能缺失）。
#[derive(Debug, Clone, Serialize)]
pub struct AtlasEntry {
    pub base: String,
    /// xml 实际读取路径（split 阶段解析布局用）。
    pub xml_path: PathBuf,
    pub xml_hash: String,
    pub xml_source: &'static str,
    /// 同名 tex；缺失时该 atlas 无法切割（计入 missing_tex）。
    pub tex: Option<TexEntry>,
}

/// 扫描结果（合并视图）。
#[derive(Debug, Default, Serialize)]
pub struct ScanResult {
    /// 有 xml 的 atlas（split 阶段输入）。
    pub atlases: Vec<AtlasEntry>,
    /// 无同名 xml 的 tex（其 kteched png 即最终产物）。
    pub lone_tex: Vec<TexEntry>,
    /// 全部 tex（ktech 阶段输入，= atlas.tex + lone_tex）。
    pub all_tex: Vec<TexEntry>,
    /// 两源同名 tex 但内容不一致（以 loose 为准处理）。
    pub conflicts: Vec<String>,
    /// 两源同名 xml 但内容不一致（以 loose 为准处理）。
    pub xml_conflicts: Vec<String>,
    /// loose 目录中的遗留 png 计数（仅统计，不读取）。
    pub stale_png: u64,
    /// zip 中有同名 tex 的 xml 数（诊断信息）。
    pub zip_tex_hits: u64,
}

impl ScanResult {
    /// 增量判定的输入 hash 表：`tex/<base>`、`xml/<base>` → sha256。
    pub fn input_hashes(&self) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        for tex in &self.all_tex {
            map.insert(format!("tex/{}", tex.base), tex.hash.clone());
        }
        for atlas in &self.atlases {
            map.insert(format!("xml/{}", atlas.base), atlas.xml_hash.clone());
        }
        map
    }
}

/// 扫描两源并构建合并视图。
///
/// `zip_out_dir` 是 zip 源 tex/xml 解压后的目录（`current/unzipped`），
/// 仅用于拼装 zip 条目的磁盘路径，本函数不执行解压。
pub fn scan(zip_path: &Path, loose_dir: &Path, zip_out_dir: &Path) -> Result<ScanResult> {
    let mut result = ScanResult::default();

    // -- zip 源：直接从 archive 读条目并计 hash（不解压） --------------------
    let mut zip_tex: BTreeMap<String, (String, String)> = BTreeMap::new(); // base → (zip 内路径, hash)
    let mut zip_xml: BTreeMap<String, (String, String)> = BTreeMap::new();
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().replace('\\', "/");
        let base = match entry_base(&name) {
            Some(b) => b,
            None => continue,
        };
        let ext = entry_ext(&name);
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        let hash = hash_bytes(&bytes);
        match ext.as_str() {
            "tex" => {
                zip_tex.insert(base, (name, hash));
            }
            "xml" => {
                zip_xml.insert(base, (name, hash));
            }
            _ => {}
        }
    }

    // -- loose 源：递归遍历目录（png 仅计数） --------------------------------
    let mut loose_tex: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();
    let mut loose_xml: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();
    let mut loose_png = 0u64;
    if loose_dir.exists() {
        collect_dir(loose_dir, &mut |path| -> Result<()> {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            match ext.as_str() {
                "png" => loose_png += 1,
                "tex" | "xml" => {
                    let base = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .expect("UTF-8 file stem")
                        .to_string();
                    let hash = crate::scripts_sync::images::history::hash_file(path)?;
                    if ext == "tex" {
                        loose_tex.insert(base, (path.to_path_buf(), hash));
                    } else {
                        loose_xml.insert(base, (path.to_path_buf(), hash));
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
    }
    result.stale_png = loose_png;

    // -- tex 合并：loose 优先，同名不一致记冲突 ------------------------------
    let mut tex_entries: BTreeMap<String, TexEntry> = BTreeMap::new();
    for (base, (zip_name, hash)) in &zip_tex {
        // 解压保留 zip 内相对路径：images/foo.tex → <out>/images/foo.tex。
        tex_entries.insert(
            base.clone(),
            TexEntry {
                base: base.clone(),
                path: zip_out_dir.join(zip_name),
                hash: hash.clone(),
                source: "zip",
            },
        );
    }
    for (base, (path, hash)) in &loose_tex {
        let winner = TexEntry {
            base: base.clone(),
            path: path.clone(),
            hash: hash.clone(),
            source: "loose",
        };
        match tex_entries.get(base) {
            Some(existing) if existing.hash != *hash => result.conflicts.push(base.clone()),
            _ => {}
        }
        // 内容一致或冲突均以 loose 原位路径为准（免去解压依赖 / 运行时形态）。
        tex_entries.insert(base.clone(), winner);
    }

    // -- xml 合并：同样 loose 优先 -------------------------------------------
    let mut xml_entries: BTreeMap<String, (PathBuf, String, &'static str)> = BTreeMap::new();
    for (base, (zip_name, hash)) in &zip_xml {
        xml_entries.insert(
            base.clone(),
            (zip_out_dir.join(zip_name), hash.clone(), "zip"),
        );
    }
    for (base, (path, hash)) in &loose_xml {
        match xml_entries.get(base) {
            Some((_, existing_hash, _)) if existing_hash == hash => {
                xml_entries.insert(base.clone(), (path.clone(), hash.clone(), "loose"));
            }
            Some(_) => {
                result.xml_conflicts.push(base.clone());
                xml_entries.insert(base.clone(), (path.clone(), hash.clone(), "loose"));
            }
            None => {
                xml_entries.insert(base.clone(), (path.clone(), hash.clone(), "loose"));
            }
        }
    }
    result.zip_tex_hits = zip_xml.keys().filter(|b| zip_tex.contains_key(*b)).count() as u64;

    // -- 联接：xml ↔ tex by base；无 xml 的 tex 进入 lone_tex ----------------
    let mut consumed_tex = std::collections::BTreeSet::new();
    for (base, (xml_path, xml_hash, xml_source)) in &xml_entries {
        let tex = tex_entries.get(base).cloned();
        if tex.is_some() {
            consumed_tex.insert(base.clone());
        }
        result.atlases.push(AtlasEntry {
            base: base.clone(),
            xml_path: xml_path.clone(),
            xml_hash: xml_hash.clone(),
            xml_source,
            tex,
        });
    }
    for (base, entry) in &tex_entries {
        if !consumed_tex.contains(base) {
            result.lone_tex.push(entry.clone());
        }
    }
    result.all_tex = tex_entries.values().cloned().collect();
    Ok(result)
}

/// zip 条目名 → base 名（去目录与扩展名）；忽略目录条目与其它扩展名。
fn entry_base(name: &str) -> Option<String> {
    let file = name.rsplit('/').next()?;
    if file.is_empty() || name.ends_with('/') {
        return None;
    }
    let ext = file.rsplit_once('.')?;
    match ext.1.to_ascii_lowercase().as_str() {
        "tex" | "xml" => Some(ext.0.to_string()),
        _ => None,
    }
}

fn entry_ext(name: &str) -> String {
    name.rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// 递归遍历目录，对每个文件调用 `f`。
fn collect_dir(dir: &Path, f: &mut impl FnMut(&Path) -> Result<()>) -> Result<()> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                f(&path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_file(path: &Path, content: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// 构造最小 images.zip：条目名自动加 `images/` 前缀。
    fn make_zip(zip_path: &Path, files: &[(&str, &[u8])]) {
        std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        for (name, content) in files {
            zw.start_file(
                format!("images/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(content).unwrap();
        }
        zw.finish().unwrap();
    }

    #[test]
    fn test_entry_base() {
        assert_eq!(entry_base("images/foo.tex"), Some("foo".into()));
        assert_eq!(entry_base("images/foo.XML"), Some("foo".into()));
        assert_eq!(entry_base("images/foo.png"), None);
        assert_eq!(entry_base("images/"), None);
    }

    #[test]
    fn test_scan_merge_and_missing_tex() {
        let ws = std::env::temp_dir().join(format!("ktool_scan_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dst = ws.join("dst");
        let loose = dst.join("data/images");
        let zip = dst.join("data/databundles/images.zip");
        let out = ws.join("out");

        // zip 源：foo 有 xml+tex；baz 只有 xml（tex 在 loose）
        make_zip(
            &zip,
            &[
                ("foo.tex", b"foo-tex-zip"),
                ("foo.xml", b"<a/>"),
                ("baz.xml", b"<b/>"),
            ],
        );
        // loose 源：baz.tex（baz 的配对）、bar.tex（无 xml）、遗留 png
        write_file(&loose.join("baz.tex"), b"baz-tex-loose");
        write_file(&loose.join("bar.tex"), b"bar-tex");
        write_file(&loose.join("stale.png"), b"stale png");

        let scan = scan(&zip, &loose, &out).unwrap();
        assert_eq!(scan.stale_png, 1);
        assert!(scan.conflicts.is_empty());

        let bases: Vec<&str> = scan.atlases.iter().map(|a| a.base.as_str()).collect();
        assert_eq!(bases, vec!["baz", "foo"]);
        let foo = scan.atlases.iter().find(|a| a.base == "foo").unwrap();
        assert_eq!(foo.tex.as_ref().unwrap().source, "zip");
        let baz = scan.atlases.iter().find(|a| a.base == "baz").unwrap();
        assert_eq!(baz.tex.as_ref().unwrap().source, "loose");
        assert!(baz.tex.as_ref().unwrap().path.is_absolute());

        let lone: Vec<&str> = scan.lone_tex.iter().map(|t| t.base.as_str()).collect();
        assert_eq!(lone, vec!["bar"]);
        assert_eq!(scan.all_tex.len(), 3);

        // zip tex 的磁盘路径指向解压位置
        let foo_tex = foo.tex.as_ref().unwrap();
        assert_eq!(
            foo_tex.path,
            out.join("images").join("foo.tex"),
            "{}",
            foo_tex.path.display()
        );

        let inputs = scan.input_hashes();
        assert!(inputs.contains_key("tex/foo") && inputs.contains_key("xml/foo"));
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_scan_conflict_prefers_loose() {
        let ws = std::env::temp_dir().join(format!("ktool_scan2_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dst = ws.join("dst");
        let loose = dst.join("data/images");
        let zip = dst.join("data/databundles/images.zip");

        make_zip(&zip, &[("foo.tex", b"zip-version")]);
        write_file(&loose.join("foo.tex"), b"loose-version");

        let scan = scan(&zip, &loose, &ws.join("out")).unwrap();
        assert_eq!(scan.conflicts, vec!["foo"]);
        let tex = scan.all_tex.iter().find(|t| t.base == "foo").unwrap();
        assert_eq!(tex.source, "loose");
        assert_eq!(tex.hash, hash_bytes(b"loose-version"));
        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_scan_same_content_dedup() {
        let ws = std::env::temp_dir().join(format!("ktool_scan3_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dst = ws.join("dst");
        let loose = dst.join("data/images");
        let zip = dst.join("data/databundles/images.zip");

        make_zip(&zip, &[("foo.tex", b"identical")]);
        write_file(&loose.join("foo.tex"), b"identical");

        let scan = scan(&zip, &loose, &ws.join("out")).unwrap();
        assert!(scan.conflicts.is_empty());
        assert_eq!(scan.all_tex.len(), 1);
        assert_eq!(scan.all_tex[0].source, "loose");
        std::fs::remove_dir_all(&ws).ok();
    }
}
