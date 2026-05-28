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
    use std::io::Read;

    fn read_anim_bin_from_zip(path: &str) -> Vec<u8> {
        let data = std::fs::read(path).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "anim.bin" {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).unwrap();
                return buf;
            }
        }
        panic!("no anim.bin found in {path}");
    }

    #[test]
    fn parse_anim_basic() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        assert!(!anim.banks.is_empty());
    }

    #[test]
    fn parse_anim_invalid_magic() {
        let data = b"BILD\x00\x00\x00\x00";
        match parse_anim(data) {
            Err(crate::error::Error::InvalidMagic { expected, actual }) => {
                assert_eq!(expected, "ANIM");
                assert_eq!(actual, "BILD");
            }
            _ => panic!("expected InvalidMagic error"),
        }
    }

    #[test]
    fn parse_anim_truncated_header() {
        let data = b"ANI";
        match parse_anim(data) {
            Err(crate::error::Error::OutOfBounds { .. }) => {}
            _ => panic!("expected OutOfBounds error"),
        }
    }

    #[test]
    fn parse_anim_wrong_magic_bild() {
        let data = b"BILD\x00\x00\x00\x00";
        match parse_anim(data) {
            Err(crate::error::Error::InvalidMagic { expected, actual }) => {
                assert_eq!(expected, "ANIM");
                assert_eq!(actual, "BILD");
            }
            _ => panic!("expected InvalidMagic error"),
        }
    }

    #[test]
    fn add_animation_new_bank() {
        let mut file = AnimFile::new();
        let anim1 = AnimAnimation {
            name: "idle".into(),
            frame_rate: 30.0,
            frames: Vec::new(),
        };
        let anim2 = AnimAnimation {
            name: "walk".into(),
            frame_rate: 30.0,
            frames: Vec::new(),
        };
        file.add_animation("bank_a", anim1);
        file.add_animation("bank_b", anim2);
        assert_eq!(file.banks.len(), 2);
        assert_eq!(file.banks[0].name, "bank_a");
        assert_eq!(file.banks[1].name, "bank_b");
    }

    #[test]
    fn add_animation_same_bank_merges() {
        let mut file = AnimFile::new();
        let anim1 = AnimAnimation {
            name: "idle".into(),
            frame_rate: 30.0,
            frames: Vec::new(),
        };
        let anim2 = AnimAnimation {
            name: "walk".into(),
            frame_rate: 30.0,
            frames: Vec::new(),
        };
        file.add_animation("bank_a", anim1);
        file.add_animation("bank_a", anim2);
        assert_eq!(file.banks.len(), 1);
        assert_eq!(file.banks[0].animations.len(), 2);
        assert_eq!(file.banks[0].animations[0].name, "idle");
        assert_eq!(file.banks[0].animations[1].name, "walk");
    }

    #[test]
    fn parse_anim_version() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        assert_eq!(anim.version, 4);
    }

    #[test]
    fn parse_anim_bank_name() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        assert_eq!(anim.banks[0].name, "abigail_flower");
    }

    #[test]
    fn parse_anim_animation_names() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        let names: Vec<&str> = anim.banks[0]
            .animations
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert!(names.contains(&"idle_1"), "missing idle_1: {names:?}");
        assert!(names.contains(&"idle_2"), "missing idle_2: {names:?}");
        assert!(
            names.contains(&"haunted_pre"),
            "missing haunted_pre: {names:?}"
        );
        assert!(
            names.contains(&"haunted_pst"),
            "missing haunted_pst: {names:?}"
        );
        assert!(
            names.contains(&"idle_haunted_loop"),
            "missing idle_haunted_loop: {names:?}"
        );
    }

    #[test]
    fn parse_anim_frame_counts() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        let idle1 = anim.banks[0]
            .animations
            .iter()
            .find(|a| a.name == "idle_1")
            .unwrap();
        assert_eq!(idle1.frames.len(), 5);
        let haunted_pre = anim.banks[0]
            .animations
            .iter()
            .find(|a| a.name == "haunted_pre")
            .unwrap();
        assert_eq!(haunted_pre.frames.len(), 16);
        let haunted_loop = anim.banks[0]
            .animations
            .iter()
            .find(|a| a.name == "idle_haunted_loop")
            .unwrap();
        assert_eq!(haunted_loop.frames.len(), 58);
    }

    #[test]
    fn parse_anim_frame_rate() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        for bank in &anim.banks {
            for a in &bank.animations {
                assert!(a.frame_rate > 0.0, "frame_rate should be positive");
            }
        }
    }

    #[test]
    fn parse_anim_elements_sorted_by_z() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        for bank in &anim.banks {
            for a in &bank.animations {
                for frame in &a.frames {
                    let z_values: Vec<f32> = frame.elements.iter().map(|e| e.z_index).collect();
                    let mut sorted = z_values.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    assert_eq!(z_values, sorted, "elements not sorted by z_index");
                }
            }
        }
    }

    #[test]
    fn parse_anim_frame_idx_sequential() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        let idle1 = anim.banks[0]
            .animations
            .iter()
            .find(|a| a.name == "idle_1")
            .unwrap();
        let indices: Vec<u32> = idle1.frames.iter().map(|f| f.idx).collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn parse_anim_symbol_lower_is_lowercase() {
        let buf = read_anim_bin_from_zip("data/anim/abigail_flower.zip");
        let anim = parse_anim(&buf).unwrap();
        for bank in &anim.banks {
            for a in &bank.animations {
                for frame in &a.frames {
                    for elem in &frame.elements {
                        assert_eq!(
                            elem.symbol_lower,
                            elem.symbol.to_lowercase(),
                            "symbol_lower mismatch for '{}'",
                            elem.symbol
                        );
                    }
                }
            }
        }
    }
}
