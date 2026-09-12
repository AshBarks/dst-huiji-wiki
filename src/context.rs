use crate::error::{Error, Result};
use crate::models::PoFile;
use crate::parser::PoParser;
use crate::wiki::WikiClient;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// A historical scripts snapshot discovered under `data/databundles/`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotInfo {
    /// Directory name, e.g. `scripts_202605291134`.
    pub name: String,
}

pub struct DstContext {
    pub version: String,
    pub dst_root: String,
    /// Optional extracted snapshot directory name (e.g. `scripts_202605291134`).
    /// When set, script files are read from that directory instead of `scripts.zip`.
    pub snapshot: Option<String>,
    archive: Option<ZipArchive<BufReader<std::fs::File>>>,
    pub(crate) client: WikiClient,
}

impl DstContext {
    pub fn from_env() -> Result<Self> {
        let dst_root = crate::platform::config::dst_root_str()?;
        Self::new(dst_root, None)
    }

    /// Creates a context with an optional snapshot selection.
    pub fn new(dst_root: String, snapshot: Option<String>) -> Result<Self> {
        let dst_path = Path::new(&dst_root);
        if !dst_path.exists() {
            return Err(Error::DstDirNotFound(dst_root.clone()));
        }

        if let Some(name) = &snapshot {
            let dir = Self::snapshot_dir(dst_path, name);
            if !dir.exists() {
                return Err(Error::Config(format!(
                    "Snapshot directory does not exist: {}",
                    dir.to_string_lossy()
                )));
            }
        }

        let version_file = dst_path.join("version.txt");
        let version = match std::fs::read_to_string(&version_file) {
            Ok(v) => v.trim().to_string(),
            Err(e) => {
                tracing::warn!("Failed to read version.txt: {}, defaulting to 'unknown'", e);
                "unknown".to_string()
            }
        };

        let client = WikiClient::from_env()
            .map_err(|e| Error::Config(format!("Failed to create wiki client: {}", e)))?;

        Ok(Self {
            version,
            dst_root,
            snapshot,
            archive: None,
            client,
        })
    }

    fn databundles_dir(dst_root: &Path) -> PathBuf {
        dst_root.join("data/databundles")
    }

    fn snapshot_dir(dst_root: &Path, name: &str) -> PathBuf {
        Self::databundles_dir(dst_root).join(name)
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

    pub(crate) fn open_scripts_zip(&mut self) -> Result<&mut ZipArchive<BufReader<std::fs::File>>> {
        if self.archive.is_none() {
            let scripts_zip = Path::new(&self.dst_root).join("data/databundles/scripts.zip");
            if !scripts_zip.exists() {
                return Err(Error::DstDirNotFound(
                    scripts_zip.to_string_lossy().to_string(),
                ));
            }

            let file = std::fs::File::open(&scripts_zip)?;
            let reader = BufReader::new(file);
            let archive = ZipArchive::new(reader)?;

            self.archive = Some(archive);
        }

        Ok(self
            .archive
            .as_mut()
            .expect("archive was verified/initialized as Some above"))
    }

    pub fn read_zip_file(&mut self, path: &str) -> Result<String> {
        let archive = self.open_scripts_zip()?;

        let mut file = archive
            .by_name(path)
            .map_err(|e| Error::ArchiveFileNotFound(format!("{}: {}", path, e)))?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }

    /// Reads a game script file, honouring the selected snapshot.
    ///
    /// `rel_path` is relative to the `scripts/` root (a leading `scripts/` is
    /// tolerated). Resolution order:
    /// 1. selected snapshot directory (if any)
    /// 2. live extracted `data/databundles/scripts/` directory (if present)
    /// 3. `scripts.zip` archive
    pub fn read_script_file(&mut self, rel_path: &str) -> Result<String> {
        let rel = rel_path.strip_prefix("scripts/").unwrap_or(rel_path);

        if let Some(snapshot) = &self.snapshot {
            let path = Self::snapshot_dir(Path::new(&self.dst_root), snapshot).join(rel);
            return std::fs::read_to_string(&path).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "read {}: {}",
                    path.to_string_lossy(),
                    e
                )))
            });
        }

        let live = Self::databundles_dir(Path::new(&self.dst_root))
            .join("scripts")
            .join(rel);
        if live.exists() {
            return std::fs::read_to_string(&live).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "read {}: {}",
                    live.to_string_lossy(),
                    e
                )))
            });
        }

        self.read_zip_file(&format!("scripts/{}", rel))
    }

    pub fn parse_po_file(&mut self, path: &str) -> Result<PoFile> {
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
