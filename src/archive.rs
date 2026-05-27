use std::collections::HashMap;
use std::io::Read;

use crate::anim::{AnimFile, parse_anim};
use crate::build_file::{BuildFile, parse_build};
use crate::error::Result;
use crate::xor::xor_decrypt;

pub enum ParsedEntry {
    Anim(AnimFile),
    Build(BuildFile),
}

pub struct ParsedArchive {
    pub anim: Option<AnimFile>,
    pub build: Option<BuildFile>,
    pub tex_files: HashMap<String, Vec<u8>>,
    pub raw_files: HashMap<String, Vec<u8>>,
}

pub fn parse_zip(data: &[u8]) -> Result<ParsedArchive> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))?;
    parse_zip_archive(&mut archive)
}

pub fn parse_dyn(data: &[u8]) -> Result<ParsedArchive> {
    let decrypted = xor_decrypt(data);
    parse_zip(&decrypted)
}

fn parse_zip_archive(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<ParsedArchive> {
    let mut anim: Option<AnimFile> = None;
    let mut build: Option<BuildFile> = None;
    let mut tex_files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut raw_files: HashMap<String, Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        raw_files.insert(name.clone(), buf.clone());

        if name == "anim.bin" {
            anim = Some(parse_anim(&buf)?);
        } else if name == "build.bin" {
            build = Some(parse_build(&buf)?);
        } else if name.ends_with(".tex") {
            tex_files.insert(name, buf);
        }
    }

    Ok(ParsedArchive {
        anim,
        build,
        tex_files,
        raw_files,
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
        assert!(!result.tex_files.is_empty());
    }
}
