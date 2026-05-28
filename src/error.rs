use crate::specs::PixelFormat;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid magic: expected {expected}, got {actual}")]
    InvalidMagic { expected: String, actual: String },

    #[error("read out of bounds: pos {pos}, len {len}")]
    OutOfBounds { pos: usize, len: usize },

    #[error("missing companion file: {0}")]
    MissingCompanion(String),

    #[error("no {0} found in archive")]
    MissingData(String),

    #[error("unsupported pixel format: {0:?}")]
    UnsupportedPixelFormat(PixelFormat),

    #[error("invalid value for {typ}: {value}")]
    InvalidValue { typ: &'static str, value: u32 },

    #[error("{0}")]
    Other(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[cfg(feature = "gif")]
    #[error("GIF error: {0}")]
    Gif(#[from] gif::EncodingError),

    #[error("UI error: {0}")]
    Ui(String),
}

pub type Result<T> = std::result::Result<T, Error>;
