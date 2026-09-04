//! AnimFile/BuildFile → 可序列化规范 DTO。
//!
//! 规则：
//! - 忽略 `symbol_lower`、`symbol_index`、`image`、`spans` 等派生字段；
//! - bank/animation/frame/symbol 排序；
//! - f32 输出固定 6 位小数，避免浮点噪声。

use dst_anim_tool::anim::{AnimAnimation, AnimElement, AnimFile, AnimFrame};
use dst_anim_tool::build_file::{BuildFile, BuildFrame, BuildSymbol, BuildVert};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimFileDto {
    pub version: i32,
    pub banks: Vec<AnimBankDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimBankDto {
    pub name: String,
    pub animations: Vec<AnimAnimationDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimAnimationDto {
    pub name: String,
    pub frame_rate: f64,
    pub frames: Vec<AnimFrameDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimFrameDto {
    pub idx: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub events: Vec<String>,
    pub elements: Vec<AnimElementDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimElementDto {
    pub z_index: f64,
    pub symbol: String,
    pub frame_num: u32,
    pub layer_name: String,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuildFileDto {
    pub version: i32,
    pub name: String,
    pub atlases: Vec<String>,
    pub symbols: Vec<BuildSymbolDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuildSymbolDto {
    pub name: String,
    pub frames: Vec<BuildFrameDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuildFrameDto {
    pub frame_num: u32,
    pub duration: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub verts: Vec<BuildVertDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuildVertDto {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub u: f64,
    pub v: f64,
    pub w: u32,
}

pub fn normalize_anim(anim: &AnimFile) -> AnimFileDto {
    let mut banks: Vec<AnimBankDto> = anim
        .banks
        .iter()
        .map(|b| AnimBankDto {
            name: b.name.clone(),
            animations: normalize_animations(&b.animations),
        })
        .collect();
    banks.sort_by(|a, b| a.name.cmp(&b.name));
    AnimFileDto {
        version: anim.version,
        banks,
    }
}

fn normalize_animations(animations: &[AnimAnimation]) -> Vec<AnimAnimationDto> {
    let mut out: Vec<AnimAnimationDto> = animations
        .iter()
        .map(|a| AnimAnimationDto {
            name: a.name.clone(),
            frame_rate: round_f32(a.frame_rate),
            frames: normalize_frames(&a.frames),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn normalize_frames(frames: &[AnimFrame]) -> Vec<AnimFrameDto> {
    let mut out: Vec<AnimFrameDto> = frames
        .iter()
        .map(|f| AnimFrameDto {
            idx: f.idx,
            x: round_f32(f.x),
            y: round_f32(f.y),
            width: round_f32(f.width),
            height: round_f32(f.height),
            events: f.events.clone(),
            elements: normalize_elements(&f.elements),
        })
        .collect();
    out.sort_by_key(|f| f.idx);
    out
}

fn normalize_elements(elements: &[AnimElement]) -> Vec<AnimElementDto> {
    let mut out: Vec<AnimElementDto> = elements
        .iter()
        .map(|e| AnimElementDto {
            z_index: round_f32(e.z_index),
            symbol: e.symbol.clone(),
            frame_num: e.frame_num,
            layer_name: e.layer_name.clone(),
            a: round_f32(e.a),
            b: round_f32(e.b),
            c: round_f32(e.c),
            d: round_f32(e.d),
            tx: round_f32(e.tx),
            ty: round_f32(e.ty),
        })
        .collect();
    out.sort_by(|a, b| {
        a.z_index
            .partial_cmp(&b.z_index)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.symbol.cmp(&b.symbol))
            .then_with(|| a.layer_name.cmp(&b.layer_name))
            .then_with(|| a.frame_num.cmp(&b.frame_num))
    });
    out
}

pub fn normalize_build(build: &BuildFile) -> BuildFileDto {
    let mut symbols: Vec<BuildSymbolDto> = build.symbols.iter().map(normalize_symbol).collect();
    symbols.sort_by(|a, b| a.name.cmp(&b.name));
    BuildFileDto {
        version: build.version,
        name: build.name.clone(),
        atlases: build.atlases.iter().map(|a| a.name.clone()).collect(),
        symbols,
    }
}

fn normalize_symbol(symbol: &BuildSymbol) -> BuildSymbolDto {
    let mut frames: Vec<BuildFrameDto> = symbol.frames.iter().map(normalize_frame).collect();
    frames.sort_by_key(|f| f.frame_num);
    BuildSymbolDto {
        name: symbol.name.clone(),
        frames,
    }
}

fn normalize_frame(frame: &BuildFrame) -> BuildFrameDto {
    BuildFrameDto {
        frame_num: frame.frame_num,
        duration: frame.duration,
        x: round_f32(frame.x),
        y: round_f32(frame.y),
        width: round_f32(frame.width),
        height: round_f32(frame.height),
        verts: frame.verts.iter().map(normalize_vert).collect(),
    }
}

fn normalize_vert(vert: &BuildVert) -> BuildVertDto {
    BuildVertDto {
        x: round_f32(vert.x),
        y: round_f32(vert.y),
        z: round_f32(vert.z),
        u: round_f32(vert.u),
        v: round_f32(vert.v),
        w: vert.w,
    }
}

/// 将 f32 规范化为 6 位小数的 f64。
fn round_f32(v: f32) -> f64 {
    let scaled = (v as f64 * 1_000_000.0).round() / 1_000_000.0;
    if scaled == 0.0 {
        0.0
    } else {
        scaled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_float_normalizes() {
        assert_eq!(round_f32(1.0000004), 1.0);
        assert_eq!(round_f32(-0.0), 0.0);
    }
}
