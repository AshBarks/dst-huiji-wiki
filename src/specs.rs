use std::sync::LazyLock;

pub const MAGIC_ANIM: &str = "ANIM";
pub const MAGIC_BILD: &str = "BILD";
pub const MAGIC_KTEX: &str = "KTEX";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Default = 0,
    PS3 = 10,
    Xbox360 = 11,
    PC = 12,
}

impl TryFrom<u32> for Platform {
    type Error = crate::error::Error;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(Platform::Default),
            10 => Ok(Platform::PS3),
            11 => Ok(Platform::Xbox360),
            12 => Ok(Platform::PC),
            _ => Err(crate::error::Error::InvalidValue {
                typ: "platform",
                value: v,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum PixelFormat {
    DXT1 = 0,
    DXT3 = 1,
    DXT5 = 2,
    RGBA = 4,
    RGB = 5,
    UNKNOWN = 7,
}

impl TryFrom<u32> for PixelFormat {
    type Error = crate::error::Error;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(PixelFormat::DXT1),
            1 => Ok(PixelFormat::DXT3),
            2 => Ok(PixelFormat::DXT5),
            4 => Ok(PixelFormat::RGBA),
            5 => Ok(PixelFormat::RGB),
            7 => Ok(PixelFormat::UNKNOWN),
            _ => Err(crate::error::Error::InvalidValue {
                typ: "pixel_format",
                value: v,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureType {
    OneD = 0,
    TwoD = 1,
    ThreeD = 2,
    CubeMapped = 3,
}

impl TryFrom<u32> for TextureType {
    type Error = crate::error::Error;

    fn try_from(v: u32) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(TextureType::OneD),
            1 => Ok(TextureType::TwoD),
            2 => Ok(TextureType::ThreeD),
            3 => Ok(TextureType::CubeMapped),
            _ => Err(crate::error::Error::InvalidValue {
                typ: "texture_type",
                value: v,
            }),
        }
    }
}

pub const DIR_RIGHT: u8 = 1;
pub const DIR_UP: u8 = 2;
pub const DIR_LEFT: u8 = 4;
pub const DIR_DOWN: u8 = 8;
pub const DIR_UPRIGHT: u8 = 16;
pub const DIR_UPLEFT: u8 = 32;
pub const DIR_DOWNLEFT: u8 = 64;
pub const DIR_DOWNRIGHT: u8 = 128;

static DIRECTION_SUFFIX_MAP: LazyLock<std::collections::HashMap<u8, &str>> = LazyLock::new(|| {
    let mut m = std::collections::HashMap::new();
    m.insert(DIR_UP, "_up");
    m.insert(DIR_DOWN, "_down");
    m.insert(DIR_LEFT | DIR_RIGHT, "_side");
    m.insert(DIR_LEFT, "_left");
    m.insert(DIR_RIGHT, "_right");
    m.insert(DIR_UPLEFT | DIR_UPRIGHT, "_upside");
    m.insert(DIR_DOWNLEFT | DIR_DOWNRIGHT, "_downside");
    m.insert(DIR_UPLEFT, "_upleft");
    m.insert(DIR_UPRIGHT, "_upright");
    m.insert(DIR_DOWNLEFT, "_downleft");
    m.insert(DIR_DOWNRIGHT, "_downright");
    m.insert(
        DIR_UPLEFT | DIR_UPRIGHT | DIR_DOWNLEFT | DIR_DOWNRIGHT,
        "_45s",
    );
    m.insert(DIR_UP | DIR_DOWN | DIR_LEFT | DIR_RIGHT, "_90s");
    m
});

pub fn direction_suffix(flag: u8) -> &'static str {
    DIRECTION_SUFFIX_MAP.get(&flag).copied().unwrap_or("")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KtexSpec {
    pub offset_platform: u32,
    pub offset_pixel_format: u32,
    pub offset_texture_type: u32,
    pub offset_mipmap_count: u32,
    pub offset_flags: u32,
    pub offset_fill: u32,
}

pub const PRE_CAVE_SPEC: KtexSpec = KtexSpec {
    offset_platform: 0,
    offset_pixel_format: 3,
    offset_texture_type: 6,
    offset_mipmap_count: 9,
    offset_flags: 14,
    offset_fill: 15,
};

pub const POST_CAVE_SPEC: KtexSpec = KtexSpec {
    offset_platform: 0,
    offset_pixel_format: 4,
    offset_texture_type: 9,
    offset_mipmap_count: 13,
    offset_flags: 18,
    offset_fill: 20,
};

pub fn detect_spec(spec_data: u32) -> KtexSpec {
    if (spec_data >> 14) & 0x3FFFF == 0x3FFFF {
        PRE_CAVE_SPEC
    } else {
        POST_CAVE_SPEC
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_try_from() {
        assert_eq!(Platform::try_from(0).unwrap(), Platform::Default);
        assert_eq!(Platform::try_from(12).unwrap(), Platform::PC);
        assert!(Platform::try_from(99).is_err());
    }

    #[test]
    fn pixel_format_try_from() {
        assert_eq!(PixelFormat::try_from(0).unwrap(), PixelFormat::DXT1);
        assert_eq!(PixelFormat::try_from(4).unwrap(), PixelFormat::RGBA);
        assert!(PixelFormat::try_from(3).is_err());
    }

    #[test]
    fn texture_type_try_from() {
        assert_eq!(TextureType::try_from(1).unwrap(), TextureType::TwoD);
        assert!(TextureType::try_from(99).is_err());
    }

    #[test]
    fn direction_suffix_values() {
        assert_eq!(direction_suffix(DIR_UP), "_up");
        assert_eq!(direction_suffix(DIR_DOWN), "_down");
        assert_eq!(direction_suffix(DIR_LEFT | DIR_RIGHT), "_side");
        assert_eq!(direction_suffix(DIR_LEFT), "_left");
        assert_eq!(direction_suffix(DIR_RIGHT), "_right");
        assert_eq!(direction_suffix(DIR_UPLEFT | DIR_UPRIGHT), "_upside");
        assert_eq!(direction_suffix(DIR_DOWNLEFT | DIR_DOWNRIGHT), "_downside");
        assert_eq!(direction_suffix(DIR_UPLEFT), "_upleft");
        assert_eq!(direction_suffix(DIR_UPRIGHT), "_upright");
        assert_eq!(direction_suffix(DIR_DOWNLEFT), "_downleft");
        assert_eq!(direction_suffix(DIR_DOWNRIGHT), "_downright");
        assert_eq!(
            direction_suffix(DIR_UPLEFT | DIR_UPRIGHT | DIR_DOWNLEFT | DIR_DOWNRIGHT),
            "_45s"
        );
        assert_eq!(
            direction_suffix(DIR_UP | DIR_DOWN | DIR_LEFT | DIR_RIGHT),
            "_90s"
        );
        assert_eq!(direction_suffix(0), "");
    }

    #[test]
    fn detect_spec_pre_cave() {
        let spec_data = (0x3FFFF << 14) | 0x3FF;
        assert_eq!(detect_spec(spec_data), PRE_CAVE_SPEC);
    }

    #[test]
    fn detect_spec_post_cave() {
        let spec_data: u32 = 0;
        assert_eq!(detect_spec(spec_data), POST_CAVE_SPEC);
    }
}
