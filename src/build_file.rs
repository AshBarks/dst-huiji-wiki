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

#[derive(Clone, Copy)]
pub struct RowSpan {
    pub start: usize,
    pub end: usize,
    pub opaque: bool,
}

pub struct SpriteSpans {
    pub rows: Box<[Option<RowSpan>]>,
    pub fully_opaque: bool,
}

#[derive(Clone)]
pub struct BuildFrame {
    pub frame_num: u32,
    pub duration: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub verts: Vec<BuildVert>,
    pub image: Option<Arc<image::RgbaImage>>,
    pub dest_x: i64,
    pub dest_y: i64,
    pub canvas_w: f32,
    pub canvas_h: f32,
    pub spans: Option<Arc<SpriteSpans>>,
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
    pub frame_index: HashMap<u32, usize>,
}

impl BuildSymbol {
    pub fn frame_for_anim_frame(&self, frame_num: u32) -> Option<&BuildFrame> {
        if let Some(&fi) = self.frame_index.get(&frame_num) {
            return Some(&self.frames[fi]);
        }
        let idx = self.frames.partition_point(|f| f.frame_num <= frame_num);
        self.frames[..idx]
            .iter()
            .rev()
            .find(|f| frame_num < f.frame_num + f.duration)
    }
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
    pub(crate) fn build_symbol_index(&mut self) {
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
        let symbol_name = hash_map.get(&symbol_hash).cloned().unwrap_or_else(|| {
            tracing::warn!(
                "build '{name}': symbol hash {symbol_hash} missing from hash table, \
                 using numeric fallback"
            );
            symbol_hash.to_string()
        });
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
                dest_x: 0,
                dest_y: 0,
                canvas_w: 0.0,
                canvas_h: 0.0,
                spans: None,
            });
        }
        frames.sort_by_key(|f| f.frame_num);
        let frame_index: HashMap<u32, usize> = frames
            .iter()
            .enumerate()
            .map(|(i, f)| (f.frame_num, i))
            .collect();
        symbols.push(BuildSymbol {
            name: symbol_name,
            frames,
            frame_index,
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
    use std::io::Read;

    fn read_build_bin_from_zip(path: &str) -> Vec<u8> {
        let data = std::fs::read(path).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.as_slice())).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "build.bin" {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).unwrap();
                return buf;
            }
        }
        panic!("no build.bin found in {path}");
    }

    #[test]
    fn parse_build_basic() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        assert!(!build.symbols.is_empty());
    }

    #[test]
    fn parse_build_invalid_magic() {
        let data = b"ANIM\x00\x00\x00\x00";
        match parse_build(data) {
            Err(crate::error::Error::InvalidMagic { expected, actual }) => {
                assert_eq!(expected, "BILD");
                assert_eq!(actual, "ANIM");
            }
            _ => panic!("expected InvalidMagic error"),
        }
    }

    #[test]
    fn parse_build_truncated_header() {
        let data = b"BIL";
        match parse_build(data) {
            Err(crate::error::Error::OutOfBounds { .. }) => {}
            _ => panic!("expected OutOfBounds error"),
        }
    }

    #[test]
    fn parse_build_wrong_magic_anim() {
        let data = b"ANIM\x00\x00\x00\x00";
        match parse_build(data) {
            Err(crate::error::Error::InvalidMagic { expected, actual }) => {
                assert_eq!(expected, "BILD");
                assert_eq!(actual, "ANIM");
            }
            _ => panic!("expected InvalidMagic error"),
        }
    }

    #[test]
    fn parse_build_version() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        assert_eq!(build.version, 6);
    }

    #[test]
    fn parse_build_name() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        assert_eq!(build.name, "abigail_flower");
    }

    #[test]
    fn parse_build_symbol_count() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        assert_eq!(build.symbols.len(), 10);
    }

    #[test]
    fn parse_build_symbol_names() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        let names: Vec<&str> = build.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"petal1"), "missing petal1: {names:?}");
        assert!(names.contains(&"flower1"), "missing flower1: {names:?}");
        assert!(names.contains(&"shdw"), "missing shdw: {names:?}");
    }

    #[test]
    fn parse_build_atlas_count() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        assert_eq!(build.atlases.len(), 1);
        assert_eq!(build.atlases[0].name, "atlas-0.tex");
    }

    #[test]
    fn parse_build_verts_nonempty() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        let has_verts = build
            .symbols
            .iter()
            .any(|s| s.frames.iter().any(|f| !f.verts.is_empty()));
        assert!(has_verts, "expected at least one frame with verts");
    }

    #[test]
    fn parse_build_vert_uv_range() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        for symbol in &build.symbols {
            for frame in &symbol.frames {
                for v in &frame.verts {
                    let u = v.u;
                    let vv = v.v;
                    assert!(
                        (0.0..=1.0).contains(&u),
                        "u={u} out of [0,1] in symbol '{}' frame {}",
                        symbol.name,
                        frame.frame_num
                    );
                    assert!(
                        (0.0..=1.0).contains(&vv),
                        "v={vv} out of [0,1] in symbol '{}' frame {}",
                        symbol.name,
                        frame.frame_num
                    );
                }
            }
        }
    }

    #[test]
    fn parse_build_frame_index_consistency() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        for symbol in &build.symbols {
            assert_eq!(
                symbol.frame_index.len(),
                symbol.frames.len(),
                "frame_index length mismatch for symbol '{}'",
                symbol.name
            );
            for (&frame_num, &idx) in &symbol.frame_index {
                assert!(
                    idx < symbol.frames.len(),
                    "frame_index[{frame_num}] = {idx} >= frames.len() = {}",
                    symbol.frames.len()
                );
                assert_eq!(
                    symbol.frames[idx].frame_num, frame_num,
                    "frame_index maps {frame_num} to idx {idx} but frames[{idx}].frame_num = {}",
                    symbol.frames[idx].frame_num
                );
            }
        }
    }

    #[test]
    fn frame_for_anim_frame_exact_match() {
        let symbol = BuildSymbol {
            name: "sym".into(),
            frames: vec![
                BuildFrame {
                    frame_num: 4,
                    duration: 4,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts: Vec::new(),
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                },
                BuildFrame {
                    frame_num: 20,
                    duration: 2,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts: Vec::new(),
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                },
            ],
            frame_index: HashMap::from([(4, 0), (20, 1)]),
        };
        assert_eq!(symbol.frame_for_anim_frame(4).unwrap().frame_num, 4);
        assert_eq!(symbol.frame_for_anim_frame(20).unwrap().frame_num, 20);
    }

    #[test]
    fn frame_for_anim_frame_range_match() {
        let symbol = BuildSymbol {
            name: "sym".into(),
            frames: vec![
                BuildFrame {
                    frame_num: 0,
                    duration: 4,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts: Vec::new(),
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                },
                BuildFrame {
                    frame_num: 4,
                    duration: 4,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts: Vec::new(),
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                },
                BuildFrame {
                    frame_num: 8,
                    duration: 2,
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    verts: Vec::new(),
                    image: None,
                    dest_x: 0,
                    dest_y: 0,
                    canvas_w: 10.0,
                    canvas_h: 10.0,
                    spans: None,
                },
            ],
            frame_index: HashMap::from([(0, 0), (4, 1), (8, 2)]),
        };
        assert_eq!(symbol.frame_for_anim_frame(1).unwrap().frame_num, 0);
        assert_eq!(symbol.frame_for_anim_frame(3).unwrap().frame_num, 0);
        assert_eq!(symbol.frame_for_anim_frame(5).unwrap().frame_num, 4);
        assert_eq!(symbol.frame_for_anim_frame(7).unwrap().frame_num, 4);
        assert_eq!(symbol.frame_for_anim_frame(9).unwrap().frame_num, 8);
        assert!(symbol.frame_for_anim_frame(10).is_none());
        assert!(symbol.frame_for_anim_frame(2).is_some());
    }

    #[test]
    fn frame_for_anim_frame_before_first() {
        let symbol = BuildSymbol {
            name: "sym".into(),
            frames: vec![BuildFrame {
                frame_num: 4,
                duration: 4,
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                verts: Vec::new(),
                image: None,
                dest_x: 0,
                dest_y: 0,
                canvas_w: 10.0,
                canvas_h: 10.0,
                spans: None,
            }],
            frame_index: HashMap::from([(4, 0)]),
        };
        assert!(symbol.frame_for_anim_frame(0).is_none());
        assert!(symbol.frame_for_anim_frame(3).is_none());
        assert!(symbol.frame_for_anim_frame(7).is_some());
    }

    #[test]
    fn parse_build_symbol_index() {
        let buf = read_build_bin_from_zip("data/anim/abigail_flower.zip");
        let build = parse_build(&buf).unwrap();
        for (i, symbol) in build.symbols.iter().enumerate() {
            let key = symbol.name.to_lowercase();
            assert_eq!(
                build.symbol_index.get(&key),
                Some(&i),
                "symbol_index['{key}'] mismatch"
            );
        }
    }

    #[test]
    fn parse_build_multi_atlas() {
        let buf = read_build_bin_from_zip("data/anim/abigail_shield.zip");
        let build = parse_build(&buf).unwrap();
        assert_eq!(build.atlases.len(), 2);
        let atlas_indices: std::collections::HashSet<u32> = build
            .symbols
            .iter()
            .flat_map(|s| s.frames.iter())
            .filter_map(|f| f.verts.first().map(|v| v.w))
            .collect();
        assert!(
            atlas_indices.len() >= 2,
            "expected verts referencing at least 2 atlas indices, got {atlas_indices:?}"
        );
    }
}
