use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::hash::dst_hash;
use crate::reader::Reader;
use crate::specs::direction_suffix;
use crate::writer::Writer;

pub struct AnimElement {
    pub z_index: f32,
    pub symbol: String,
    pub frame_num: u32,
    pub layer_name: String,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

pub struct AnimFrame {
    pub idx: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub elements: Vec<AnimElement>,
    pub events: Vec<String>,
}

pub struct AnimAnimation {
    pub name: String,
    pub frame_rate: f32,
    pub frames: Vec<AnimFrame>,
}

pub struct AnimBank {
    pub name: String,
    pub animations: Vec<AnimAnimation>,
}

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

pub fn write_anim(file: &AnimFile) -> Vec<u8> {
    let mut w = Writer::new();
    let mut hash_map: HashMap<u32, String> = HashMap::new();

    w.write_string("ANIM");
    w.write_le_i32(file.version);

    let mut total_elements = 0u32;
    let mut total_frames = 0u32;
    let mut total_events = 0u32;
    let mut total_anims = 0u32;

    for bank in &file.banks {
        total_anims += bank.animations.len() as u32;
        for anim in &bank.animations {
            total_frames += anim.frames.len() as u32;
            for frame in &anim.frames {
                total_elements += frame.elements.len() as u32;
                total_events += frame.events.len() as u32;
            }
        }
    }

    w.write_le_u32(total_elements);
    w.write_le_u32(total_frames);
    w.write_le_u32(total_events);
    w.write_le_u32(total_anims);

    for bank in &file.banks {
        let bank_hash = dst_hash(&bank.name);
        hash_map.insert(bank_hash, bank.name.clone());

        for anim in &bank.animations {
            let anim_name = anim.name.clone();
            let direction = 0u8;
            w.write_le_i32(anim_name.len() as i32);
            w.write_string(&anim_name);
            w.write_u8(direction);
            w.write_le_u32(bank_hash);
            w.write_le_f32(anim.frame_rate);
            w.write_le_u32(anim.frames.len() as u32);

            for frame in &anim.frames {
                w.write_le_f32(frame.x);
                w.write_le_f32(frame.y);
                w.write_le_f32(frame.width);
                w.write_le_f32(frame.height);
                w.write_le_u32(frame.events.len() as u32);
                for event in &frame.events {
                    let event_hash = dst_hash(event);
                    hash_map.insert(event_hash, event.clone());
                    w.write_le_u32(event_hash);
                }
                w.write_le_u32(frame.elements.len() as u32);
                for element in &frame.elements {
                    let symbol_hash = dst_hash(&element.symbol);
                    let layer_hash = dst_hash(&element.layer_name);
                    hash_map.insert(symbol_hash, element.symbol.clone());
                    hash_map.insert(layer_hash, element.layer_name.clone());
                    w.write_le_u32(symbol_hash);
                    w.write_le_u32(element.frame_num);
                    w.write_le_u32(layer_hash);
                    w.write_le_f32(element.a);
                    w.write_le_f32(element.b);
                    w.write_le_f32(element.c);
                    w.write_le_f32(element.d);
                    w.write_le_f32(element.tx);
                    w.write_le_f32(element.ty);
                    w.write_le_f32(element.z_index);
                }
            }
        }
    }

    w.write_le_u32(hash_map.len() as u32);
    for (hash, string) in &hash_map {
        w.write_le_u32(*hash);
        w.write_le_i32(string.len() as i32);
        w.write_string(string);
    }

    w.into_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_anim_empty_roundtrip() {
        let original = write_anim(&AnimFile::new());
        let parsed = parse_anim(&original).unwrap();
        assert_eq!(parsed.version, 4);
        assert!(parsed.banks.is_empty());
    }
}
