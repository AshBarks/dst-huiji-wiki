pub fn dst_hash(s: &str) -> u32 {
    let mut hash: u64 = 0;
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase() as u64;
        hash = (c + (hash << 6) + (hash << 16) - hash) & 0xFFFFFFFF;
    }
    hash as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert_eq!(dst_hash(""), 0);
    }

    #[test]
    fn single_char() {
        let expected = (('a' as u64) + (0u64 << 6) + (0u64 << 16) - 0u64) & 0xFFFFFFFF;
        assert_eq!(dst_hash("a"), expected as u32);
        assert_eq!(dst_hash("A"), expected as u32);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(dst_hash("hello"), dst_hash("HELLO"));
        assert_eq!(dst_hash("test"), dst_hash("Test"));
    }

    #[test]
    fn known_hash() {
        let h = dst_hash("swap_object");
        assert_ne!(h, 0);
    }
}
