use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PO parse error: {0}")]
    PoParse(String),

    #[error("Invalid PO entry: {0}")]
    InvalidPoEntry(String),

    #[error("Environment variable not set: {0}")]
    EnvVarNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Wiki API error: {0}")]
    WikiApi(String),

    #[error("Wiki login failed: {0}")]
    LoginFailed(String),

    #[error("Wiki edit failed: {0}")]
    EditFailed(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, Error>;
