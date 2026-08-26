use thiserror::Error;

#[non_exhaustive]
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

    #[error("Lua parse error: {}", .0.iter().map(|e| e.error_message().into_owned()).collect::<Vec<_>>().join(", "))]
    LuaParse(Vec<full_moon::Error>),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Wiki API error: {0}")]
    WikiApi(String),

    /// The wiki (or its WAF/gateway) rejected the request with 403/429.
    /// The request was NOT processed by MediaWiki, so retrying later is safe.
    #[error("Wiki rate limited: {0}")]
    RateLimited(String),

    /// The queried page does not exist (normal business condition).
    #[error("Page not found: {0}")]
    PageNotFound(String),

    /// The login session expired or was never established for a privileged
    /// operation (e.g. MediaWiki returned `assertuserfailed`/`badtoken`).
    #[error("Wiki session expired or unauthorized: {0}")]
    AuthExpired(String),

    /// Edit conflict: the page was modified by someone else after our base
    /// revision (`basetimestamp`). Re-fetch the page and re-apply.
    #[error("Edit conflict on page: {0}")]
    EditConflict(String),

    #[error("Wiki login failed: {0}")]
    LoginFailed(String),

    #[error("Wiki edit failed: {0}")]
    EditFailed(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Zip archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("File not found in archive: {0}")]
    ArchiveFileNotFound(String),

    #[error("DST directory does not exist: {0}")]
    DstDirNotFound(String),

    #[error("Invalid path (non-UTF-8): {0}")]
    InvalidPath(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("LLM error: {0}")]
    Llm(String),
}

impl Error {
    /// Whether retrying the failed operation later can plausibly succeed.
    ///
    /// Used by automated flows to decide between "retry queue" and
    /// "report and move on". Transport errors (connect/timeout) and
    /// rate-limit rejections are retryable; logic errors are not.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::RateLimited(_) => true,
            Error::Http(e) => {
                e.is_connect() || e.is_timeout() || e.status().is_some_and(|s| s.is_server_error())
            }
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_parse_error_display() {
        let errors = vec![];
        let err = Error::LuaParse(errors);
        let display = err.to_string();
        assert!(display.contains("Lua parse error"));
    }

    #[test]
    fn test_lua_parse_error_with_errors() {
        // Test that LuaParse variant can be constructed and displayed
        // without panicking even with actual parse errors
        let source = "local x = ";
        let errors = full_moon::parse(source).unwrap_err();
        let err = Error::LuaParse(errors);
        let display = err.to_string();
        assert!(display.contains("Lua parse error"));
    }

    #[test]
    fn test_new_variant_displays() {
        assert!(Error::RateLimited("403 after 3 attempts".into())
            .to_string()
            .contains("rate limited"));
        assert!(Error::PageNotFound("Data:X.tabx".into())
            .to_string()
            .contains("not found"));
        assert!(Error::AuthExpired("assertuserfailed".into())
            .to_string()
            .contains("expired"));
        assert!(Error::EditConflict("模块:Constants/Tech".into())
            .to_string()
            .contains("conflict"));
    }

    #[test]
    fn test_is_retryable_classification() {
        assert!(Error::RateLimited("waf".into()).is_retryable());
        // Logic errors are not retryable.
        assert!(!Error::PageNotFound("x".into()).is_retryable());
        assert!(!Error::AuthExpired("x".into()).is_retryable());
        assert!(!Error::EditConflict("x".into()).is_retryable());
        assert!(!Error::WikiApi("boom".into()).is_retryable());
    }
}
