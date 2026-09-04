//! 从 `.zip` / `.dyn` 中解析 anim/build，并计算条目级 hash。
//!
//! 使用当前项目已有的 `zip` 依赖读取条目，只取出需要的 `anim.bin` /
//! `build.bin` 和 tex 哈希；不直接调用 `dst_anim_tool::archive::parse_zip`，
//! 避免把 atlas tex 全部读进内存。

use crate::error::Result;
use crate::scripts_sync::anim::history::hash_bytes;
use crate::scripts_sync::anim::normalize::{
    normalize_anim, normalize_build, AnimFileDto, BuildFileDto,
};
use crate::scripts_sync::anim::snapshot::FileEntry;
use dst_anim_tool::xor::xor_decrypt;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

/// 一个动画包解析后的结构化数据。
#[derive(Debug, Clone)]
pub struct ParsedArchiveData {
    pub anim: Option<AnimFileDto>,
    pub build: Option<BuildFileDto>,
    pub anim_sha256: Option<String>,
    pub build_sha256: Option<String>,
    pub tex_hashes: BTreeMap<String, String>,
}

impl ParsedArchiveData {
    pub fn is_empty(&self) -> bool {
        self.anim.is_none() && self.build.is_none() && self.tex_hashes.is_empty()
    }
}

/// 解析磁盘上的动画文件。
pub fn parse_archive_file(entry: &FileEntry) -> Result<ParsedArchiveData> {
    let raw = std::fs::read(&entry.path)?;
    let bytes = if entry.is_dyn() {
        xor_decrypt(&raw)
    } else {
        raw
    };
    parse_archive_bytes(&bytes)
}

/// 解析 zip（或已解密的 dyn）字节。
pub fn parse_archive_bytes(bytes: &[u8]) -> Result<ParsedArchiveData> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut data = ParsedArchiveData {
        anim: None,
        build: None,
        anim_sha256: None,
        build_sha256: None,
        tex_hashes: BTreeMap::new(),
    };

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().replace('\\', "/");
        if name == "anim.bin" {
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            data.anim_sha256 = Some(hash_bytes(&buf));
            let anim = dst_anim_tool::archive::parse_anim_bin(&buf)?;
            if let Some(anim) = anim.anim {
                data.anim = Some(normalize_anim(&anim));
            } else {
                return Err(crate::error::Error::Config(
                    "parse_anim_bin returned no anim".to_string(),
                ));
            }
        } else if name == "build.bin" {
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            data.build_sha256 = Some(hash_bytes(&buf));
            let build = dst_anim_tool::archive::parse_build_bin(&buf)?;
            if let Some(build) = build.build {
                data.build = Some(normalize_build(&build));
            } else {
                return Err(crate::error::Error::Config(
                    "parse_build_bin returned no build".to_string(),
                ));
            }
        } else if name.ends_with(".tex") {
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            data.tex_hashes.insert(name, hash_bytes(&buf));
        }
    }

    Ok(data)
}
