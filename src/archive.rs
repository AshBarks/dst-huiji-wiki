use std::cell::OnceCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use crate::anim::{AnimFile, parse_anim};
use crate::build_file::{BuildFile, parse_build};
use crate::error::{Error, Result};
use crate::specs::{MAGIC_ANIM, MAGIC_BILD};
use crate::xor::xor_decrypt;

#[derive(Clone)]
pub struct TexSource {
    pub source_name: String,
    pub tex_files: HashMap<String, Arc<Vec<u8>>>,
}

pub struct ParsedArchive {
    pub anim: Option<AnimFile>,
    pub build: Option<BuildFile>,
    pub tex_sources: Vec<TexSource>,
    pub raw_files: HashMap<String, Arc<Vec<u8>>>,
    merged_tex_cache: OnceCell<HashMap<String, Arc<Vec<u8>>>>,
}

impl ParsedArchive {
    pub fn tex_files(&self) -> &HashMap<String, Arc<Vec<u8>>> {
        self.merged_tex_cache.get_or_init(|| {
            let mut merged = HashMap::new();
            for source in &self.tex_sources {
                for (k, v) in &source.tex_files {
                    merged.insert(k.clone(), v.clone());
                }
            }
            merged
        })
    }

    pub fn merge(&mut self, other: ParsedArchive) {
        if other.anim.is_some() {
            self.anim = other.anim;
        }
        if other.build.is_some() {
            self.build = other.build;
        }
        for source in other.tex_sources {
            self.tex_sources.push(source);
        }
        for (k, v) in other.raw_files {
            self.raw_files.insert(k, v);
        }
        self.merged_tex_cache = OnceCell::new();
    }
}

pub fn parse_zip(data: &[u8]) -> Result<ParsedArchive> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))?;
    parse_zip_archive(&mut archive)
}

pub fn parse_dyn(data: &[u8]) -> Result<ParsedArchive> {
    let decrypted = xor_decrypt(data);
    parse_zip(&decrypted)
}

pub fn parse_anim_bin(data: &[u8]) -> Result<ParsedArchive> {
    let anim = parse_anim(data)?;
    Ok(ParsedArchive {
        anim: Some(anim),
        build: None,
        tex_sources: Vec::new(),
        raw_files: HashMap::new(),
        merged_tex_cache: OnceCell::new(),
    })
}

pub fn parse_build_bin(data: &[u8]) -> Result<ParsedArchive> {
    let build = parse_build(data)?;
    Ok(ParsedArchive {
        anim: None,
        build: Some(build),
        tex_sources: Vec::new(),
        raw_files: HashMap::new(),
        merged_tex_cache: OnceCell::new(),
    })
}

pub fn detect_bin_type(data: &[u8]) -> BinType {
    if data.len() < 4 {
        return BinType::Unknown;
    }
    let magic = &data[0..4];
    if magic == MAGIC_ANIM.as_bytes() {
        BinType::Anim
    } else if magic == MAGIC_BILD.as_bytes() {
        BinType::Build
    } else {
        BinType::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinType {
    Anim,
    Build,
    Unknown,
}

pub fn parse_file_by_path(path: &Path, data: &[u8]) -> Result<ParsedArchive> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "zip" => parse_zip(data),
        "dyn" => parse_dyn(data),
        "bin" => match detect_bin_type(data) {
            BinType::Anim => parse_anim_bin(data),
            BinType::Build => parse_build_bin(data),
            BinType::Unknown => Err(Error::Other(
                "unknown .bin magic, expected ANIM or BILD".to_string(),
            )),
        },
        _ => Err(Error::Other(format!("unsupported file extension: .{ext}"))),
    }
}

pub fn load_archives(paths: &[std::path::PathBuf]) -> Result<ParsedArchive> {
    if paths.is_empty() {
        return Err(Error::Other("no input files".to_string()));
    }

    let first_data = std::fs::read(&paths[0])?;
    let mut merged = parse_file_by_path(&paths[0], &first_data)?;
    let first_name = paths[0]
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    for source in &mut merged.tex_sources {
        if source.source_name.is_empty() {
            source.source_name = first_name.to_string();
        }
    }

    for path in &paths[1..] {
        let data = std::fs::read(path)?;
        let mut archive = parse_file_by_path(path, &data)?;
        let source_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        for source in &mut archive.tex_sources {
            if source.source_name.is_empty() {
                source.source_name = source_name.to_string();
            }
        }
        merged.merge(archive);
    }

    Ok(merged)
}

fn parse_zip_archive(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<ParsedArchive> {
    let mut anim: Option<AnimFile> = None;
    let mut build: Option<BuildFile> = None;
    let mut tex_files: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    let mut raw_files: HashMap<String, Arc<Vec<u8>>> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        let buf_arc = Arc::new(buf);
        raw_files.insert(name.clone(), buf_arc.clone());

        if name == "anim.bin" {
            anim = Some(parse_anim(&buf_arc)?);
        } else if name == "build.bin" {
            build = Some(parse_build(&buf_arc)?);
        } else if name.ends_with(".tex") {
            tex_files.insert(name, buf_arc);
        }
    }

    let tex_sources = if !tex_files.is_empty() {
        vec![TexSource {
            source_name: String::new(),
            tex_files,
        }]
    } else {
        Vec::new()
    };

    Ok(ParsedArchive {
        anim,
        build,
        tex_sources,
        raw_files,
        merged_tex_cache: OnceCell::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_abigail_flower_zip() {
        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let result = parse_zip(&data).unwrap();
        assert!(result.anim.is_some());
        assert!(result.build.is_some());
        let anim = result.anim.unwrap();
        assert!(!anim.banks.is_empty());
        let build = result.build.unwrap();
        assert!(!build.symbols.is_empty());
    }

    #[test]
    fn parse_abigail_ice_dyn() {
        let data = std::fs::read("data/anim/dynamic/abigail_ice.dyn").unwrap();
        let result = parse_dyn(&data).unwrap();
        assert!(!result.tex_sources.is_empty());
        assert!(!result.tex_sources[0].tex_files.is_empty());
    }

    #[test]
    fn detect_anim_bin_type() {
        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "anim.bin" {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).unwrap();
                assert_eq!(detect_bin_type(&buf), BinType::Anim);
            }
        }
    }

    #[test]
    fn detect_build_bin_type() {
        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "build.bin" {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).unwrap();
                assert_eq!(detect_bin_type(&buf), BinType::Build);
            }
        }
    }

    #[test]
    fn merge_archives() {
        let zip_data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let dyn_data = std::fs::read("data/anim/dynamic/abigail_ice.dyn").unwrap();

        let mut archive = parse_zip(&zip_data).unwrap();
        assert!(!archive.tex_sources.is_empty());
        let dyn_archive = parse_dyn(&dyn_data).unwrap();
        assert!(!dyn_archive.tex_sources.is_empty());

        let zip_tex_count = archive.tex_sources[0].tex_files.len();
        let dyn_tex_count = dyn_archive.tex_sources[0].tex_files.len();
        let zip_tex_names: std::collections::HashSet<String> =
            archive.tex_sources[0].tex_files.keys().cloned().collect();
        let dyn_tex_names: std::collections::HashSet<String> = dyn_archive.tex_sources[0]
            .tex_files
            .keys()
            .cloned()
            .collect();
        let has_new = dyn_tex_names.difference(&zip_tex_names).count() > 0;

        let orig_source_count = archive.tex_sources.len();
        archive.merge(dyn_archive);

        assert!(archive.tex_sources.len() > orig_source_count);
        assert!(archive.tex_sources[0].tex_files.len() == zip_tex_count);
        assert!(archive.tex_sources[1].tex_files.len() == dyn_tex_count);
        if has_new {
            let merged_tex = archive.tex_files();
            assert!(merged_tex.len() >= zip_tex_count);
        }
    }

    #[test]
    fn load_archives_multi() {
        let zip_path = std::path::PathBuf::from("data/anim/abigail_flower.zip");
        let result = load_archives(&[zip_path]).unwrap();
        assert!(result.anim.is_some());
        assert!(result.build.is_some());
    }
}
