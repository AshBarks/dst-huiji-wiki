use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::reader::Reader;

#[derive(Clone)]
pub struct BuildVert {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub u: f32,
    pub v: f32,
    pub w: u32,
}

#[derive(Clone)]
pub struct BuildAtlasRef {
    pub name: String,
}

#[derive(Clone)]
pub struct BuildFrame {
    pub frame_num: u32,
    #[allow(dead_code)]
    pub duration: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub verts: Vec<BuildVert>,
    pub image: Option<Arc<image::RgbaImage>>,
}

impl BuildFrame {
    pub fn image_ref(&self) -> Option<&image::RgbaImage> {
        self.image.as_ref().map(|arc| arc.as_ref())
    }
}

#[derive(Clone)]
pub struct BuildSymbol {
    pub name: String,
    pub frames: Vec<BuildFrame>,
}

#[derive(Clone)]
pub struct BuildFile {
    pub version: i32,
    pub name: String,
    pub symbols: Vec<BuildSymbol>,
    pub atlases: Vec<BuildAtlasRef>,
    pub symbol_index: HashMap<String, usize>,
}

impl BuildFile {
    pub fn build_symbol_index(&mut self) {
        self.symbol_index.clear();
        for (i, symbol) in self.symbols.iter().enumerate() {
            self.symbol_index.insert(symbol.name.to_lowercase(), i);
        }
    }
}

pub fn parse_build(data: &[u8]) -> Result<BuildFile> {
    let mut reader = Reader::new(data);
    let magic = reader.read_string(4)?;
    if magic != crate::specs::MAGIC_BILD {
        return Err(Error::InvalidMagic {
            expected: crate::specs::MAGIC_BILD.to_string(),
            actual: magic,
        });
    }
    let version = reader.read_le_i32()?;
    reader.seek(8);
    let num_symbols = reader.read_le_u32()?;
    let _total_frames = reader.read_le_u32()?;
    let name_len = reader.read_le_i32_at(16)? as usize;
    let name = reader.read_string(name_len)?;
    let num_atlases = reader.read_le_u32()?;
    let mut atlases = Vec::with_capacity(num_atlases as usize);
    for _ in 0..num_atlases {
        let atlas_name_len = reader.read_le_i32()? as usize;
        let atlas_name = reader.read_string(atlas_name_len)?;
        atlases.push(BuildAtlasRef { name: atlas_name });
    }

    let saved_cursor = reader.pos();

    for _ in 0..num_symbols {
        let frame_count = reader.read_le_u32_at(reader.pos() + 4)?;
        reader.seek(reader.pos() + frame_count as usize * 32);
    }

    let num_verts = reader.read_le_u32()?;
    let mut verts: Vec<BuildVert> = Vec::with_capacity(num_verts as usize);
    for _ in 0..num_verts {
        let x = reader.read_le_f32()?;
        let y = reader.read_le_f32()?;
        let z = reader.read_le_f32()?;
        let u = reader.read_le_f32()?;
        let v = reader.read_le_f32()?;
        let w = reader.read_le_f32()?;
        verts.push(BuildVert {
            x,
            y,
            z,
            u,
            v,
            w: w as u32,
        });
    }

    let hash_count = reader.read_le_u32()?;
    let mut hash_map: HashMap<u32, String> = HashMap::new();
    for _ in 0..hash_count {
        let hash = reader.read_le_u32()?;
        let str_len = reader.read_le_i32()? as usize;
        let s = reader.read_string(str_len)?;
        hash_map.insert(hash, s);
    }

    reader.seek(saved_cursor);
    let mut symbols = Vec::with_capacity(num_symbols as usize);
    for _ in 0..num_symbols {
        let symbol_hash = reader.read_le_u32()?;
        let frame_count = reader.read_le_u32()?;
        let symbol_name = hash_map
            .get(&symbol_hash)
            .cloned()
            .unwrap_or_else(|| symbol_hash.to_string());
        let mut frames = Vec::with_capacity(frame_count as usize);
        for _ in 0..frame_count {
            let frame_num = reader.read_le_u32()?;
            let duration = reader.read_le_u32()?;
            let pivot_x = reader.read_le_f32()?;
            let pivot_y = reader.read_le_f32()?;
            let width = reader.read_le_f32()?;
            let height = reader.read_le_f32()?;
            let vert_idx = reader.read_le_u32()? as usize;
            let vert_count = reader.read_le_u32()? as usize;
            if vert_idx + vert_count > verts.len() {
                return Err(Error::OutOfBounds {
                    pos: vert_idx + vert_count,
                    len: verts.len(),
                });
            }
            let frame_verts: Vec<BuildVert> = verts[vert_idx..vert_idx + vert_count]
                .iter()
                .map(|v| BuildVert {
                    x: v.x,
                    y: v.y,
                    z: v.z,
                    u: v.u,
                    v: v.v,
                    w: v.w,
                })
                .collect();
            frames.push(BuildFrame {
                frame_num,
                duration,
                x: pivot_x,
                y: pivot_y,
                width,
                height,
                verts: frame_verts,
                image: None,
            });
        }
        frames.sort_by_key(|f| f.frame_num);
        symbols.push(BuildSymbol {
            name: symbol_name,
            frames,
        });
    }

    let mut file = BuildFile {
        version,
        name,
        symbols,
        atlases,
        symbol_index: HashMap::new(),
    };
    file.build_symbol_index();
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_build_basic() {
        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "build.bin" {
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut buf).unwrap();
                let build = parse_build(&buf).unwrap();
                assert!(!build.symbols.is_empty());
                return;
            }
        }
        panic!("no build.bin found");
    }
}
