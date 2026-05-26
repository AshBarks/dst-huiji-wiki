use crate::specs::PixelFormat;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid magic: expected {expected}, got {actual}")]
    InvalidMagic { expected: String, actual: String },

    #[error("read out of bounds: pos {pos}, len {len}")]
    OutOfBounds { pos: usize, len: usize },

    #[error("unknown format: {0}")]
    UnknownFormat(String),

    #[error("unsupported pixel format: {0:?}")]
    UnsupportedPixelFormat(PixelFormat),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
