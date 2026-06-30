use crate::error::{Error, Result};
use crate::models::PoFile;
use crate::parser::PoParser;
use crate::wiki::WikiClient;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

pub struct DstContext {
    pub version: String,
    pub dst_root: String,
    archive: Option<ZipArchive<BufReader<std::fs::File>>>,
    pub client: WikiClient,
}

impl DstContext {
    pub fn from_env() -> Result<Self> {
        let dst_root = std::env::var("DST__ROOT")
            .map_err(|e| Error::EnvVarNotFound(format!("DST__ROOT: {}", e)))?;

        let dst_path = Path::new(&dst_root);
        if !dst_path.exists() {
            return Err(Error::DstDirNotFound(dst_root.clone()));
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
            archive: None,
            client,
        })
    }

    pub fn open_scripts_zip(&mut self) -> Result<&mut ZipArchive<BufReader<std::fs::File>>> {
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

    pub fn parse_po_file(&mut self, path: &str) -> Result<PoFile> {
        let content = self.read_zip_file(path)?;
        PoParser::parse(&content)
    }

    pub fn sources(&self) -> String {
        format!("Extract data from patch {}", self.version)
    }
}
