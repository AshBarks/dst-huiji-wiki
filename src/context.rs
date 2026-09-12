use crate::error::Result;
use crate::models::PoFile;
use crate::parser::PoParser;
use crate::platform::game_source::GameSource;
use crate::wiki::WikiClient;
use std::path::{Path, PathBuf};

/// A historical scripts snapshot discovered under `data/databundles/`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotInfo {
    /// Directory name, e.g. `scripts_202605291134`.
    pub name: String,
}

/// 游戏数据上下文：`GameSource`（snapshot→live→zip 唯一读取语义）+ wiki 客户端。
///
/// 读取一律委托 [`GameSource`]，不再自行缓存 `ZipArchive`——每作业读取次数
/// 很少（≤5），重开 zip 的代价可忽略，换来的是全项目只有一处读取顺序定义。
pub struct DstContext {
    pub version: String,
    pub dst_root: String,
    /// Optional extracted snapshot directory name (e.g. `scripts_202605291134`).
    /// When set, script files are read from that directory instead of `scripts.zip`.
    pub snapshot: Option<String>,
    source: GameSource,
    pub(crate) client: WikiClient,
}

impl DstContext {
    pub fn from_env() -> Result<Self> {
        let dst_root = crate::platform::config::dst_root_str()?;
        Self::new(dst_root, None)
    }

    /// Creates a context with an optional snapshot selection.
    pub fn new(dst_root: String, snapshot: Option<String>) -> Result<Self> {
        // GameSource::new 校验 dst_root 存在、snapshot 目录存在。
        let source = GameSource::new(PathBuf::from(&dst_root), snapshot.clone())?;

        let version_file = Path::new(&dst_root).join("version.txt");
        let version = match std::fs::read_to_string(&version_file) {
            Ok(v) => v.trim().to_string(),
            Err(e) => {
                tracing::warn!("Failed to read version.txt: {}, defaulting to 'unknown'", e);
                "unknown".to_string()
            }
        };

        let client = WikiClient::from_env().map_err(|e| {
            crate::error::Error::Config(format!("Failed to create wiki client: {}", e))
        })?;

        Ok(Self {
            version,
            dst_root,
            snapshot,
            source,
            client,
        })
    }

    fn databundles_dir(dst_root: &Path) -> PathBuf {
        dst_root.join("data/databundles")
    }

    /// Lists all extracted scripts snapshots (`scripts_<timestamp>` directories),
    /// sorted from newest to oldest.
    pub fn list_snapshots() -> Vec<SnapshotInfo> {
        let mut result = Vec::new();
        let Some(dst_root) = crate::platform::config::dst_root_opt() else {
            return result;
        };
        let dst_root = dst_root.to_string_lossy().into_owned();
        let bundles = Self::databundles_dir(Path::new(&dst_root));
        let Ok(entries) = std::fs::read_dir(&bundles) else {
            return result;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("scripts_") && entry.path().is_dir() {
                result.push(SnapshotInfo { name });
            }
        }
        // Newest first: lexicographic descending works for the timestamp suffix.
        result.sort_by(|a, b| b.name.cmp(&a.name));
        result
    }

    /// 底层游戏数据源（snapshot 严格 → live 解压树 → scripts.zip）。
    pub fn game_source(&self) -> &GameSource {
        &self.source
    }

    /// Reads a game script file, honouring the selected snapshot.
    ///
    /// `rel_path` is relative to the `scripts/` root (a leading `scripts/` is
    /// tolerated). Resolution order is defined by [`GameSource`]:
    /// 1. selected snapshot directory (strict, if any)
    /// 2. live extracted `data/databundles/scripts/` directory (if present)
    /// 3. `scripts.zip` archive
    pub fn read_script_file(&self, rel_path: &str) -> Result<String> {
        self.source.read(rel_path)
    }

    pub fn parse_po_file(&self, path: &str) -> Result<PoFile> {
        let content = self.read_script_file(path)?;
        PoParser::parse(&content)
    }

    pub fn sources(&self) -> String {
        match &self.snapshot {
            Some(s) => format!("Extract data from snapshot {}", s),
            None => format!("Extract data from patch {}", self.version),
        }
    }

    pub fn wiki(&self) -> &WikiClient {
        &self.client
    }

    pub fn wiki_mut(&mut self) -> &mut WikiClient {
        &mut self.client
    }
}
