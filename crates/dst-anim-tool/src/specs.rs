use std::sync::LazyLock;

pub const MAGIC_ANIM: &str = "ANIM";
pub const MAGIC_BILD: &str = "BILD";
pub(crate) const DIR_RIGHT: u8 = 1;
pub(crate) const DIR_UP: u8 = 2;
pub(crate) const DIR_LEFT: u8 = 4;
pub(crate) const DIR_DOWN: u8 = 8;
pub(crate) const DIR_UPRIGHT: u8 = 16;
pub(crate) const DIR_UPLEFT: u8 = 32;
pub(crate) const DIR_DOWNLEFT: u8 = 64;
pub(crate) const DIR_DOWNRIGHT: u8 = 128;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
