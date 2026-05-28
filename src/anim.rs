use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::reader::Reader;
use crate::specs::direction_suffix;

#[derive(Clone)]
pub struct AnimElement {
    pub z_index: f32,
    pub symbol: String,
    pub symbol_lower: String,
    pub frame_num: u32,
    pub layer_name: String,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

#[derive(Clone)]
pub struct AnimFrame {
    pub idx: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub elements: Vec<AnimElement>,
    pub events: Vec<String>,
}

#[derive(Clone)]
pub struct AnimAnimation {
    pub name: String,
    pub frame_rate: f32,
    pub frames: Vec<AnimFrame>,
}

#[derive(Clone)]
pub struct AnimBank {
    pub name: String,
    pub animations: Vec<AnimAnimation>,
}

#[derive(Clone)]
pub struct AnimFile {
    pub version: i32,
    pub banks: Vec<AnimBank>,
}

impl AnimFile {
    pub fn new() -> Self {
        Self {
            version: 4,
            banks: Vec::new(),
        }
    }

    pub fn add_animation(&mut self, bank_name: &str, animation: AnimAnimation) {
        if let Some(bank) = self.banks.iter_mut().find(|b| b.name == bank_name) {
            bank.animations.push(animation);
        } else {
            self.banks.push(AnimBank {
                name: bank_name.to_string(),
                animations: vec![animation],
            });
        }
    }
}

pub fn parse_anim(data: &[u8]) -> Result<AnimFile> {
    let mut reader = Reader::new(data);
    let magic = reader.read_string(4)?;
    if magic != crate::specs::MAGIC_ANIM {
        return Err(Error::InvalidMagic {
            expected: crate::specs::MAGIC_ANIM.to_string(),
            actual: magic,
        });
    }
    let version = reader.read_le_i32()?;
    let _total_elements = reader.read_le_u32()?;
    let _total_frames = reader.read_le_u32()?;
    let _total_events = reader.read_le_u32()?;

    reader.seek(20);
    let num_banks = reader.read_le_u32()?;

    for _ in 0..num_banks {
        let name_len = reader.read_le_u32()?;
        let num_frames = reader.read_le_u32_at(reader.pos() + 9 + name_len as usize)?;
        for _ in 0..num_frames {
            let num_events = reader.read_le_u32_at(reader.pos() + 16)?;
            let num_elements = reader.read_le_u32_at(reader.pos() + num_events as usize * 4)?;
            reader.seek(reader.pos() + 40 * num_elements as usize);
        }
    }

    let hash_count = reader.read_le_u32()?;
    let mut hash_map: HashMap<u32, String> = HashMap::new();
    for _ in 0..hash_count {
        let hash = reader.read_le_u32()?;
        let str_len = reader.read_le_i32()? as usize;
        let s = reader.read_string(str_len)?;
        hash_map.insert(hash, s);
    }

    reader.seek(24);
    let mut file = AnimFile::new();
    file.version = version;

    for _ in 0..num_banks {
        let name_len = reader.read_le_i32()? as usize;
        let anim_name = reader.read_string(name_len)?;
        let direction = reader.read_u8()?;
        let bank_hash = reader.read_le_u32()?;
        let bank_name = hash_map
            .get(&bank_hash)
            .cloned()
            .unwrap_or_else(|| bank_hash.to_string());
        let frame_rate = reader.read_le_f32()?;
        let num_frames = reader.read_le_i32()? as u32;

        let full_name = format!("{}{}", anim_name, direction_suffix(direction));
        file.add_animation(
            &bank_name,
            AnimAnimation {
                name: full_name,
                frame_rate,
                frames: Vec::with_capacity(num_frames as usize),
            },
        );

        let bank_idx = file.banks.iter().position(|b| b.name == bank_name).unwrap();
        let anim_idx = file.banks[bank_idx].animations.len() - 1;

        for frame_idx in 0..num_frames {
            let x = reader.read_le_f32()?;
            let y = reader.read_le_f32()?;
            let width = reader.read_le_f32()?;
            let height = reader.read_le_f32()?;

            let num_events = reader.read_le_u32()?;
            let mut events = Vec::with_capacity(num_events as usize);
            for _ in 0..num_events {
                let event_hash = reader.read_le_u32()?;
                events.push(
                    hash_map
                        .get(&event_hash)
                        .cloned()
                        .unwrap_or_else(|| event_hash.to_string()),
                );
            }

            let num_elements = reader.read_le_u32()?;
            let mut elements = Vec::with_capacity(num_elements as usize);
            for _ in 0..num_elements {
                let symbol_hash = reader.read_le_u32()?;
                let symbol_name = hash_map
                    .get(&symbol_hash)
                    .cloned()
                    .unwrap_or_else(|| symbol_hash.to_string());
                let frame_num = reader.read_le_u32()?;
                let layer_hash = reader.read_le_u32()?;
                let layer_name = hash_map
                    .get(&layer_hash)
                    .cloned()
                    .unwrap_or_else(|| layer_hash.to_string());
                let a = reader.read_le_f32()?;
                let b = reader.read_le_f32()?;
                let c = reader.read_le_f32()?;
                let d = reader.read_le_f32()?;
                let tx = reader.read_le_f32()?;
                let ty = reader.read_le_f32()?;
                let z_index = reader.read_le_f32()?;
                elements.push(AnimElement {
                    z_index,
                    symbol_lower: symbol_name.to_lowercase(),
                    symbol: symbol_name,
                    frame_num,
                    layer_name,
                    a,
                    b,
                    c,
                    d,
                    tx,
                    ty,
                });
            }
            elements.sort_by(|a, b| {
                a.z_index
                    .partial_cmp(&b.z_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            file.banks[bank_idx].animations[anim_idx]
                .frames
                .push(AnimFrame {
                    idx: frame_idx,
                    x,
                    y,
                    width,
                    height,
                    elements,
                    events,
                });
        }
        file.banks[bank_idx].animations[anim_idx]
            .frames
            .sort_by_key(|f| f.idx);
    }

    for bank in &mut file.banks {
        bank.animations.sort_by(|a, b| a.name.cmp(&b.name));
    }

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_anim_basic() {
        let data = std::fs::read("data/anim/abigail_flower.zip").unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "anim.bin" {
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut buf).unwrap();
                let anim = parse_anim(&buf).unwrap();
                assert!(!anim.banks.is_empty());
                return;
            }
        }
        panic!("no anim.bin found");
    }
}
