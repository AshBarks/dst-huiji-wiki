use crate::error::{Error, Result};

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn check_bounds(&self, offset: usize, size: usize) -> Result<()> {
        if offset + size > self.data.len() {
            Err(Error::OutOfBounds {
                pos: offset,
                len: self.data.len(),
            })
        } else {
            Ok(())
        }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.read_u8_at(self.pos)
    }

    pub fn read_u8_at(&mut self, offset: usize) -> Result<u8> {
        self.check_bounds(offset, 1)?;
        let v = self.data[offset];
        self.pos = offset + 1;
        Ok(v)
    }

    pub fn read_le_u16(&mut self) -> Result<u16> {
        self.read_le_u16_at(self.pos)
    }

    pub fn read_le_u16_at(&mut self, offset: usize) -> Result<u16> {
        self.check_bounds(offset, 2)?;
        let v = u16::from_le_bytes([self.data[offset], self.data[offset + 1]]);
        self.pos = offset + 2;
        Ok(v)
    }

    pub fn read_le_i32(&mut self) -> Result<i32> {
        self.read_le_i32_at(self.pos)
    }

    pub fn read_le_i32_at(&mut self, offset: usize) -> Result<i32> {
        self.check_bounds(offset, 4)?;
        let v = i32::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]);
        self.pos = offset + 4;
        Ok(v)
    }

    pub fn read_le_u32(&mut self) -> Result<u32> {
        self.read_le_u32_at(self.pos)
    }

    pub fn read_le_u32_at(&mut self, offset: usize) -> Result<u32> {
        self.check_bounds(offset, 4)?;
        let v = u32::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]);
        self.pos = offset + 4;
        Ok(v)
    }

    pub fn read_le_f32(&mut self) -> Result<f32> {
        self.read_le_f32_at(self.pos)
    }

    pub fn read_le_f32_at(&mut self, offset: usize) -> Result<f32> {
        self.check_bounds(offset, 4)?;
        let bytes = [
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ];
        let v = f32::from_le_bytes(bytes);
        self.pos = offset + 4;
        Ok(v)
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        self.read_bytes_at(self.pos, len)
    }

    pub fn read_bytes_at(&mut self, offset: usize, len: usize) -> Result<&'a [u8]> {
        self.check_bounds(offset, len)?;
        let slice = &self.data[offset..offset + len];
        self.pos = offset + len;
        Ok(slice)
    }

    pub fn read_bytes_remaining(&mut self) -> Result<&'a [u8]> {
        let len = self.remaining();
        self.read_bytes(len)
    }

    pub fn read_string(&mut self, len: usize) -> Result<String> {
        self.read_string_at(self.pos, len)
    }

    pub fn read_string_at(&mut self, offset: usize, len: usize) -> Result<String> {
        let bytes = self.read_bytes_at(offset, len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| Error::UnknownFormat(format!("invalid ascii string: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u8() {
        let data: &[u8] = &[0x42, 0xFF];
        let mut r = Reader::new(data);
        assert_eq!(r.read_u8().unwrap(), 0x42);
        assert_eq!(r.pos(), 1);
        assert_eq!(r.read_u8().unwrap(), 0xFF);
        assert_eq!(r.pos(), 2);
    }

    #[test]
    fn read_le_u16() {
        let data: &[u8] = &[0x01, 0x00];
        let mut r = Reader::new(data);
        assert_eq!(r.read_le_u16().unwrap(), 1);
        assert_eq!(r.pos(), 2);
    }

    #[test]
    fn read_le_i32() {
        let data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
        let mut r = Reader::new(data);
        assert_eq!(r.read_le_i32().unwrap(), -1);
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn read_le_u32() {
        let data: &[u8] = &[0x01, 0x00, 0x00, 0x00];
        let mut r = Reader::new(data);
        assert_eq!(r.read_le_u32().unwrap(), 1);
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn read_le_f32() {
        let data: &[u8] = &1.0f32.to_le_bytes();
        let mut r = Reader::new(data);
        assert_eq!(r.read_le_f32().unwrap(), 1.0);
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn read_string() {
        let data: &[u8] = b"ANIM\x00\x00\x00\x00";
        let mut r = Reader::new(data);
        assert_eq!(r.read_string(4).unwrap(), "ANIM");
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn read_bytes() {
        let data: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05];
        let mut r = Reader::new(data);
        let bytes = r.read_bytes(3).unwrap();
        assert_eq!(bytes, [0x01, 0x02, 0x03]);
        assert_eq!(r.pos(), 3);
    }

    #[test]
    fn jump_read() {
        let data: &[u8] = &[
            0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00,
        ];
        let mut r = Reader::new(data);
        assert_eq!(r.read_le_u32_at(8).unwrap(), 10);
        assert_eq!(r.pos(), 12);
    }

    #[test]
    fn seek_and_read() {
        let data: &[u8] = &[0x01, 0x02, 0x03, 0x04];
        let mut r = Reader::new(data);
        r.seek(2);
        assert_eq!(r.read_le_u16().unwrap(), 0x0403);
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn out_of_bounds() {
        let data: &[u8] = &[0x01, 0x02];
        let mut r = Reader::new(data);
        let err = r.read_le_u32().unwrap_err();
        assert!(matches!(err, Error::OutOfBounds { pos: 0, len: 2 }));
    }

    #[test]
    fn read_bytes_remaining() {
        let data: &[u8] = &[0x01, 0x02, 0x03, 0x04];
        let mut r = Reader::new(data);
        r.seek(2);
        let bytes = r.read_bytes_remaining().unwrap();
        assert_eq!(bytes, [0x03, 0x04]);
    }
}
