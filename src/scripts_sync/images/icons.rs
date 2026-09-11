//! 图标浏览索引：面向 WebUI「物品图标」板块，从 manifests 历史推导各
//! split 来源（物品栏图标 / 制作栏图标）下每个图标的「首次加入 build」与
//! 「历代版本」。
//!
//! 语义约定：
//! - 只统计 `complete == true` 的 manifest（与 diff 基线策略一致——partial
//!   的产物清单不完整，避免"因失败消失"被误读）；
//! - 「首次加入」= manifest 链（按 build 数值升序）中最先出现该文件的一份，
//!   最老一份 manifest 的存量视为首批；
//! - 「历代版本」= 沿链自旧向新，在内容 hash 变化处记一个版本，版本 build
//!   即该内容首次出现的 build（CAS 中可按 hash 取回对应图片）。

use crate::error::Result;
use crate::scripts_sync::images::history::{numeric_key, Manifest, ManifestStore};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

/// 一个图标来源：`current/split/<dir>/` 下的一类 atlas 切片。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconSource {
    /// 稳定标识（API 参数/元数据用），如 `inventory`。
    pub id: &'static str,
    /// manifest products 路径前缀，如 `split/inventoryimages/`。
    pub prefix: &'static str,
    /// split 目录名，如 `inventoryimages`。
    pub dir: &'static str,
    /// 展示名，如 `物品栏图标`。
    pub label: &'static str,
}

/// 受管理的图标来源（顺序即 WebUI 展示顺序）。
pub const ICON_SOURCES: &[IconSource] = &[
    IconSource {
        id: "inventory",
        prefix: "split/inventoryimages/",
        dir: "inventoryimages",
        label: "物品栏图标",
    },
    IconSource {
        id: "crafting",
        prefix: "split/crafting_menu_icons/",
        dir: "crafting_menu_icons",
        label: "制作栏图标",
    },
];

/// 按稳定标识查来源。
pub fn icon_source(id: &str) -> Option<&'static IconSource> {
    ICON_SOURCES.iter().find(|s| s.id == id)
}

/// 缺省来源（旧元数据没有 `source` 字段时）。
pub fn default_source() -> &'static IconSource {
    &ICON_SOURCES[0]
}

/// 一个当前存在的图标条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IconEntry {
    /// 文件名（如 `abigail_flower.png`）。
    pub file: String,
    /// 来源标识（[`IconSource::id`]），如 `inventory` / `crafting`。
    pub source: String,
    /// 当前内容 sha256（最新完整 manifest 的 products）。
    pub hash: String,
    /// 首次加入的 build 号。
    pub first_build: String,
    /// 首次加入那份 manifest 的保存时刻（unix 毫秒）；旧 manifest 无字段为 `None`。
    pub first_synced_at: Option<u64>,
}

/// 图标的一个内容版本（该内容首次出现的 build）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IconVersion {
    pub build: String,
    pub hash: String,
    pub synced_at: Option<u64>,
}

/// 排序方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSort {
    /// 按文件名升序。
    Name,
    /// 按首次加入 build 从新到旧，同 build 内文件名升序。
    History,
}

/// [`IconSort`] 对应的比较器。
pub fn compare_entries(a: &IconEntry, b: &IconEntry, sort: IconSort) -> Ordering {
    match sort {
        IconSort::Name => a.file.cmp(&b.file),
        IconSort::History => numeric_key(&b.first_build)
            .cmp(&numeric_key(&a.first_build))
            .then_with(|| a.file.cmp(&b.file)),
    }
}

/// 图标索引（[`build_icons_index`] 的产物）。
#[derive(Debug, Default)]
pub struct IconsIndex {
    /// 当前存在的图标，按文件名升序。
    pub entries: Vec<IconEntry>,
    /// file → 历代版本（自旧向新）。已移除文件的历史也保留。
    pub versions: HashMap<String, Vec<IconVersion>>,
    /// 最新完整 manifest 的 build。
    pub latest_build: Option<String>,
    /// 最新完整 manifest 的保存时刻（unix 毫秒）。
    pub latest_synced_at: Option<u64>,
}

impl IconsIndex {
    /// 指定文件是否在当前清单中（`entries` 按文件名有序，二分查找）。
    pub fn contains(&self, file: &str) -> bool {
        self.entries
            .binary_search_by(|e| e.file.as_str().cmp(file))
            .is_ok()
    }

    /// 指定文件的历代版本，自新向旧；无任何历史返回 `None`。
    pub fn version_history(&self, file: &str) -> Option<Vec<IconVersion>> {
        let mut chain = self.versions.get(file)?.clone();
        chain.reverse();
        Some(chain)
    }
}

/// 把 manifest product 路径（`split/<dir>/<file>`）匹配到图标来源。
fn source_of_product(path: &str) -> Option<(&'static IconSource, &str)> {
    ICON_SOURCES
        .iter()
        .find_map(|source| path.strip_prefix(source.prefix).map(|file| (source, file)))
}

/// 汇总 manifests 目录，产出图标索引。目录不存在时返回空索引。
pub fn build_icons_index(manifests_dir: &Path) -> Result<IconsIndex> {
    let store = ManifestStore::new(manifests_dir);
    let mut manifests: Vec<Manifest> = Vec::new();
    if manifests_dir.exists() {
        for entry in std::fs::read_dir(manifests_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let build_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            match store.load(build_id) {
                Ok(m) if m.as_ref().is_some_and(|m| m.complete) => {
                    manifests.push(m.expect("is_some 已判定"));
                }
                // partial 不参与历史推导；损坏的 manifest 跳过并告警。
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "跳过损坏的 manifest");
                }
            }
        }
    }
    manifests.sort_by_key(|a| numeric_key(&a.build));

    let mut first_seen: HashMap<String, (String, Option<u64>)> = HashMap::new();
    let mut versions: HashMap<String, Vec<IconVersion>> = HashMap::new();
    for m in &manifests {
        for (path, hash) in &m.products {
            let Some((_, file)) = source_of_product(path) else {
                continue;
            };
            first_seen
                .entry(file.to_string())
                .or_insert_with(|| (m.build.clone(), m.synced_at));
            let chain = versions.entry(file.to_string()).or_default();
            if chain.last().map(|v| v.hash.as_str()) != Some(hash.as_str()) {
                chain.push(IconVersion {
                    build: m.build.clone(),
                    hash: hash.clone(),
                    synced_at: m.synced_at,
                });
            }
        }
    }

    let latest = manifests.last();
    let mut entries: Vec<IconEntry> = Vec::new();
    if let Some(latest) = latest {
        for (path, hash) in &latest.products {
            let Some((source, file)) = source_of_product(path) else {
                continue;
            };
            if let Some((build, at)) = first_seen.get(file) {
                entries.push(IconEntry {
                    file: file.to_string(),
                    source: source.id.to_string(),
                    hash: hash.clone(),
                    first_build: build.clone(),
                    first_synced_at: *at,
                });
            }
        }
    }
    entries.sort_by(|a, b| a.file.cmp(&b.file));

    Ok(IconsIndex {
        entries,
        versions,
        latest_build: latest.map(|m| m.build.clone()),
        latest_synced_at: latest.and_then(|m| m.synced_at),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts_sync::images::history::Diff;
    use std::collections::BTreeMap;

    fn manifest(build: &str, complete: bool, products: &[(&str, &str)]) -> Manifest {
        Manifest {
            build: build.into(),
            parent_build: None,
            complete,
            decoder: "ktex-rs/1".into(),
            split_version: "1".into(),
            products: products
                .iter()
                .map(|(p, h)| (p.to_string(), h.to_string()))
                .collect(),
            diff: Diff::default(),
            inputs: BTreeMap::new(),
            stats: BTreeMap::new(),
            synced_at: Some(build.parse::<u64>().unwrap() * 10),
        }
    }

    #[test]
    fn test_first_build_versions_and_partial_skipped() {
        let ws = std::env::temp_dir().join(format!("icons_idx_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dir = ws.join("manifests");
        let store = ManifestStore::new(&dir);
        store
            .save(&manifest(
                "100",
                true,
                &[
                    ("split/inventoryimages/a.png", "h1"),
                    ("split/inventoryimages/b.png", "h1"),
                    ("decoded/x.png", "hx"),
                ],
            ))
            .unwrap();
        // partial：仅在其中出现的文件不应存在，其中 b 的"变更"也不算数
        store
            .save(&manifest(
                "150",
                false,
                &[
                    ("split/inventoryimages/a.png", "h9"),
                    ("split/inventoryimages/ghost.png", "hg"),
                ],
            ))
            .unwrap();
        store
            .save(&manifest(
                "200",
                true,
                &[
                    ("split/inventoryimages/a.png", "h2"), // changed
                    ("split/inventoryimages/b.png", "h1"), // 不变
                    ("split/inventoryimages/c.png", "h3"), // 新增
                    ("split/inventoryimages/d.png", "h4"), // 下轮将被移除
                ],
            ))
            .unwrap();
        store
            .save(&manifest(
                "300",
                true,
                &[
                    ("split/inventoryimages/a.png", "h2"),
                    ("split/inventoryimages/b.png", "h1"),
                    ("split/inventoryimages/c.png", "h3"),
                ],
            ))
            .unwrap();

        let idx = build_icons_index(&dir).unwrap();
        assert_eq!(idx.latest_build.as_deref(), Some("300"));
        assert_eq!(idx.latest_synced_at, Some(3000));
        assert_eq!(idx.entries.len(), 3);
        assert!(idx.contains("a.png") && idx.contains("b.png") && idx.contains("c.png"));
        assert!(!idx.contains("d.png") && !idx.contains("ghost.png"));

        let a = idx.entries.iter().find(|e| e.file == "a.png").unwrap();
        assert_eq!(a.first_build, "100");
        assert_eq!(a.first_synced_at, Some(1000));
        assert_eq!(a.hash, "h2");
        assert_eq!(a.source, "inventory");
        let c = idx.entries.iter().find(|e| e.file == "c.png").unwrap();
        assert_eq!(c.first_build, "200");

        // 历代版本（自新向旧）：a 在 100 h1 → 200 h2；partial 的 h9 不算
        let va = idx.version_history("a.png").unwrap();
        assert_eq!(va.len(), 2);
        assert_eq!((va[0].build.as_str(), va[0].hash.as_str()), ("200", "h2"));
        assert_eq!((va[1].build.as_str(), va[1].hash.as_str()), ("100", "h1"));
        // 已移除的 d 历史仍可查
        let vd = idx.version_history("d.png").unwrap();
        assert_eq!(vd.len(), 1);
        assert_eq!((vd[0].build.as_str(), vd[0].hash.as_str()), ("200", "h4"));
        assert!(idx.version_history("missing.png").is_none());

        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_crafting_source_tracked() {
        let ws = std::env::temp_dir().join(format!("icons_src_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let dir = ws.join("manifests");
        let store = ManifestStore::new(&dir);
        store
            .save(&manifest(
                "100",
                true,
                &[
                    ("split/inventoryimages/axe.png", "h1"),
                    ("split/crafting_menu_icons/filter_tool.png", "h2"),
                    ("split/crafting_menu_icons/station_carpentry.png", "h3"),
                ],
            ))
            .unwrap();

        let idx = build_icons_index(&dir).unwrap();
        let axe = idx.entries.iter().find(|e| e.file == "axe.png").unwrap();
        assert_eq!(axe.source, "inventory");
        let filter = idx
            .entries
            .iter()
            .find(|e| e.file == "filter_tool.png")
            .unwrap();
        assert_eq!(filter.source, "crafting");
        let station = idx
            .entries
            .iter()
            .find(|e| e.file == "station_carpentry.png")
            .unwrap();
        assert_eq!(station.source, "crafting");

        std::fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn test_compare_entries_sorts() {
        let e = |file: &str, build: &str| IconEntry {
            file: file.into(),
            source: "inventory".into(),
            hash: "h".into(),
            first_build: build.into(),
            first_synced_at: None,
        };
        let mut v = [
            e("b.png", "100"),
            e("a.png", "200"),
            e("c.png", "200"),
            e("a.png", "90"),
        ];

        v.sort_by(|a, b| compare_entries(a, b, IconSort::Name));
        let names: Vec<&str> = v.iter().map(|x| x.file.as_str()).collect();
        assert_eq!(names, ["a.png", "a.png", "b.png", "c.png"]);

        // build 数值降序（200 > 100 > 90，非字典序），同 build 文件名升序
        v.sort_by(|a, b| compare_entries(a, b, IconSort::History));
        let keys: Vec<(u64, &str)> = v
            .iter()
            .map(|x| (x.first_build.parse().unwrap(), x.file.as_str()))
            .collect();
        assert_eq!(
            keys,
            [
                (200, "a.png"),
                (200, "c.png"),
                (100, "b.png"),
                (90, "a.png")
            ]
        );
    }

    #[test]
    fn test_missing_dir_yields_empty_index() {
        let ws = std::env::temp_dir().join(format!("icons_empty_{}", std::process::id()));
        std::fs::remove_dir_all(&ws).ok();
        let idx = build_icons_index(&ws.join("nonexistent")).unwrap();
        assert!(idx.entries.is_empty());
        assert_eq!(idx.latest_build, None);
        assert!(!idx.contains("a.png"));
        assert!(idx.version_history("a.png").is_none());
    }
}
