pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn pos(&self) -> usize {
        self.buf.len()
    }

    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn write_le_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_le_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_le_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_le_f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn write_string(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_u8() {
        let mut w = Writer::new();
        w.write_u8(0x42);
        assert_eq!(w.into_vec(), [0x42]);
    }

    #[test]
    fn write_le_u16() {
        let mut w = Writer::new();
        w.write_le_u16(1);
        assert_eq!(w.into_vec(), [0x01, 0x00]);
    }

    #[test]
    fn write_le_i32() {
        let mut w = Writer::new();
        w.write_le_i32(-1);
        assert_eq!(w.into_vec(), [0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn write_le_u32() {
        let mut w = Writer::new();
        w.write_le_u32(1);
        assert_eq!(w.into_vec(), [0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn write_le_f32() {
        let mut w = Writer::new();
        w.write_le_f32(1.0);
        assert_eq!(w.into_vec(), 1.0f32.to_le_bytes());
    }

    #[test]
    fn write_string() {
        let mut w = Writer::new();
        w.write_string("ANIM");
        assert_eq!(w.into_vec(), b"ANIM");
    }

    #[test]
    fn write_bytes() {
        let mut w = Writer::new();
        w.write_bytes(&[0x01, 0x02, 0x03]);
        assert_eq!(w.into_vec(), [0x01, 0x02, 0x03]);
    }

    #[test]
    fn roundtrip() {
        let mut w = Writer::new();
        w.write_le_u32(42);
        w.write_le_f32(3.14);
        w.write_string("test");
        let buf = w.into_vec();

        let mut r = crate::reader::Reader::new(&buf);
        assert_eq!(r.read_le_u32().unwrap(), 42);
        assert!((r.read_le_f32().unwrap() - 3.14).abs() < 0.001);
        assert_eq!(r.read_string(4).unwrap(), "test");
    }
}
