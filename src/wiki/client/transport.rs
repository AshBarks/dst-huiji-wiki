//! 底层 HTTP：登录、GET/POST、CSRF token，以及节流/重试/退避策略。

use crate::error::{Error, Result};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use std::sync::{MutexGuard, PoisonError};
use std::time::Duration;

use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct LoginResponse {
    #[serde(rename = "login")]
    result: Option<LoginResult>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LoginResult {
    result: String,
    #[serde(rename = "lgusername")]
    lgusername: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenResponse {
    query: Option<TokenQuery>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenQuery {
    tokens: Option<TokenTokens>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenTokens {
    #[serde(rename = "logintoken")]
    logintoken: Option<String>,
}

/// MediaWiki returns HTTP 200 even for API-level errors; this captures the
/// top-level `{"error": {"code": ..., "info": ...}}` body.
#[derive(Debug, Deserialize)]
pub(super) struct ApiErrorBody {
    pub(super) code: String,
    #[serde(default)]
    pub(super) info: String,
}

/// Maps a MediaWiki API error code to a specific [`Error`] variant so that
/// automated flows can branch on the failure kind.
pub(super) fn map_api_error(code: &str, info: &str, title: &str) -> Error {
    match code {
        "editconflict" => Error::EditConflict(title.to_string()),
        "assertuserfailed" | "notloggedin" | "mustbeloggedin" | "badtoken" => {
            Error::AuthExpired(format!("{}: {}", code, info))
        }
        "ratelimited" => Error::RateLimited(info.to_string()),
        "" => Error::WikiApi("No edit response received".to_string()),
        other => Error::EditFailed(format!("{}: {}", other, info)),
    }
}

pub(super) fn lock_rate(m: &Mutex<std::time::Instant>) -> MutexGuard<'_, std::time::Instant> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// True when a response with this status should be retried.
///
/// GET requests may be retried on any server error; POST requests are only
/// retried on WAF-level rejections (403/429) where MediaWiki definitely did
/// not process the request — a 5xx after a write might have been applied.
pub(super) fn retryable_status(is_get: bool, status: StatusCode) -> bool {
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return true;
    }
    is_get && status.is_server_error()
}

/// Exponential backoff honouring an optional `Retry-After` hint.
/// `attempt` starts at 1.
pub(super) fn compute_backoff(
    base: Duration,
    attempt: u32,
    retry_after: Option<Duration>,
) -> Duration {
    if let Some(ra) = retry_after {
        return ra.min(RETRY_AFTER_CAP);
    }
    let shift = attempt.saturating_sub(1).min(6);
    base.checked_mul(1u32 << shift)
        .unwrap_or(BACKOFF_CAP)
        .min(BACKOFF_CAP)
}

pub(super) fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let v = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    v.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Milliseconds since the Unix epoch, used as the `_` cache-buster parameter.
///
/// huijiwiki's CDN caches anonymous API GETs for hours and re-uploads/page
/// edits do not reliably purge those entries, so maintenance reads append a
/// unique value to get a fresh response.
pub(super) fn cache_buster() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_default()
}

/// Splits titles into MediaWiki-sized chunks, preserving order and duplicates.
pub(super) fn chunk_titles<'a>(titles: &[&'a str], size: usize) -> Vec<Vec<&'a str>> {
    assert!(size > 0, "chunk size must be positive");
    titles.chunks(size).map(<[&'a str]>::to_vec).collect()
}

impl WikiClient {
    pub(super) async fn acquire_slot(&self) {
        let wait = {
            let mut last = lock_rate(&self.rate);
            let now = std::time::Instant::now();
            let earliest = *last + self.rate_cfg.min_interval;
            let target = if earliest > now { earliest } else { now };
            *last = target;
            target.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    /// Sends a request with global throttling and bounded retries.
    ///
    /// `build` must construct a fresh builder on every call because a
    /// `RequestBuilder` cannot be reused after `.send()` consumes it.
    ///
    /// Retry policy:
    /// - GET: transport errors (connect/timeout), 403/429, and 5xx;
    /// - POST: only 403/429 (WAF rejections happen *before* MediaWiki
    ///   processes the request, so replaying is safe; a 5xx after a write is
    ///   ambiguous and must not be replayed blindly).
    pub(super) async fn send_with_retry(
        &self,
        is_get: bool,
        build: impl Fn() -> RequestBuilder,
    ) -> Result<Response> {
        let mut attempt: u32 = 0;
        loop {
            self.acquire_slot().await;
            let response = match build().send().await {
                Ok(resp) => resp,
                Err(e) => {
                    let retryable = is_get && (e.is_connect() || e.is_timeout());
                    if retryable && attempt < self.rate_cfg.max_retries {
                        attempt += 1;
                        let delay = compute_backoff(self.rate_cfg.base_delay, attempt, None);
                        tracing::warn!(
                            error = %e,
                            attempt,
                            "wiki request transport error, retrying in {:?}",
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(Error::Http(e));
                }
            };

            let status = response.status();
            if !retryable_status(is_get, status) {
                return Ok(response);
            }
            if attempt >= self.rate_cfg.max_retries {
                let detail = format!(
                    "HTTP {} persisted after {} attempt(s)",
                    status.as_u16(),
                    attempt + 1
                );
                return Err(
                    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
                        Error::RateLimited(detail)
                    } else {
                        Error::WikiApi(detail)
                    },
                );
            }

            attempt += 1;
            let retry_after = parse_retry_after(response.headers());
            let delay = compute_backoff(self.rate_cfg.base_delay, attempt, retry_after);
            tracing::warn!(
                status = status.as_u16(),
                attempt,
                "wiki api rejected request, retrying in {:?}",
                delay
            );
            drop(response);
            tokio::time::sleep(delay).await;
        }
    }

    pub(super) async fn get_login_token(&self) -> Result<String> {
        let url = self.config.api_url();
        let params = [
            ("action", "query"),
            ("meta", "tokens"),
            ("type", "login"),
            ("format", "json"),
        ];

        let response = self
            .send_with_retry(true, || {
                self.client
                    .get(&url)
                    .header("X-authkey", &self.config.x_authkey)
                    .query(&params)
            })
            .await?;

        let token_resp: TokenResponse = response.json().await?;

        token_resp
            .query
            .and_then(|q| q.tokens)
            .and_then(|t| t.logintoken)
            .ok_or_else(|| Error::WikiApi("Failed to get login token".to_string()))
    }

    pub async fn login(&self) -> Result<()> {
        let token = self.get_login_token().await?;

        let url = self.config.api_url();
        let params = [
            ("action", "login"),
            ("lgname", self.config.username.as_str()),
            ("lgpassword", self.config.password.as_str()),
            ("lgtoken", token.as_str()),
            ("format", "json"),
        ];

        let response = self
            .send_with_retry(false, || {
                self.client
                    .post(&url)
                    .header("X-authkey", &self.config.x_authkey)
                    .form(&params)
            })
            .await?;

        let login_resp: LoginResponse = response.json().await?;

        if let Some(result) = login_resp.result {
            match result.result.as_str() {
                "Success" | "success" => {
                    self.logged_in
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    tracing::info!(
                        "Logged in as user: {}",
                        result.lgusername.unwrap_or_default()
                    );
                    Ok(())
                }
                "NeedToken" | "Failed" | "WrongPass" | "WrongPluginPass" | "NotExists"
                | "EmptyPass" | "CreateBlocked" | "Throttled" | "Blocked" => {
                    let reason = result.reason.unwrap_or_else(|| result.result.clone());
                    Err(Error::LoginFailed(reason))
                }
                _ => Err(Error::LoginFailed(format!(
                    "Unknown result: {}",
                    result.result
                ))),
            }
        } else {
            Err(Error::LoginFailed("No login response received".to_string()))
        }
    }

    /// GET against the wiki API with throttling and retries applied.
    pub async fn get(&self, params: &[(&str, &str)]) -> Result<Response> {
        let url = self.config.api_url();

        self.send_with_retry(true, || {
            self.client
                .get(&url)
                .header("X-authkey", &self.config.x_authkey)
                .query(params)
        })
        .await
    }

    /// POST against the wiki API with throttling and WAF-rejection retries.
    pub async fn post(&self, params: &[(&str, &str)]) -> Result<Response> {
        let url = self.config.api_url();

        self.send_with_retry(false, || {
            self.client
                .post(&url)
                .header("X-authkey", &self.config.x_authkey)
                .form(params)
        })
        .await
    }

    pub async fn get_csrf_token(&self) -> Result<String> {
        let params = [("action", "query"), ("meta", "tokens"), ("format", "json")];

        let response = self.get(&params).await?;

        #[derive(Debug, Deserialize)]
        struct CsrfResponse {
            query: Option<CsrfQuery>,
        }

        #[derive(Debug, Deserialize)]
        struct CsrfQuery {
            tokens: Option<CsrfTokens>,
        }

        #[derive(Debug, Deserialize)]
        struct CsrfTokens {
            #[serde(rename = "csrftoken")]
            csrftoken: Option<String>,
        }

        let csrf_resp: CsrfResponse = response.json().await?;

        csrf_resp
            .query
            .and_then(|q| q.tokens)
            .and_then(|t| t.csrftoken)
            .ok_or_else(|| Error::WikiApi("Failed to get CSRF token".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_titles() {
        let names: Vec<String> = (0..120).map(|i| format!("t{}", i)).collect();
        let titles: Vec<&str> = names.iter().map(String::as_str).collect();
        let chunks = chunk_titles(&titles, TITLES_PER_QUERY);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 50);
        assert_eq!(chunks[2].len(), 20);
        assert_eq!(chunks[0][0], "t0");

        assert!(chunk_titles(&[], 50).is_empty());

        let one = vec!["a"];
        assert_eq!(chunk_titles(&one, 50), vec![vec!["a"]]);
    }

    #[test]
    fn test_compute_backoff_exponential_and_caps() {
        let base = Duration::from_secs(2);
        assert_eq!(compute_backoff(base, 1, None), Duration::from_secs(2));
        assert_eq!(compute_backoff(base, 2, None), Duration::from_secs(4));
        assert_eq!(compute_backoff(base, 3, None), Duration::from_secs(8));
        // Caps regardless of attempt count.
        assert_eq!(compute_backoff(base, 30, None), BACKOFF_CAP);
        // Retry-After wins and is capped.
        assert_eq!(
            compute_backoff(base, 1, Some(Duration::from_secs(5))),
            Duration::from_secs(5)
        );
        assert_eq!(
            compute_backoff(base, 1, Some(Duration::from_secs(999))),
            RETRY_AFTER_CAP
        );
    }

    #[test]
    fn test_parse_retry_after_header() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "7".parse().unwrap());
        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(7)));

        let empty = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&empty), None);
    }

    #[test]
    fn test_map_api_error_codes() {
        assert!(matches!(
            map_api_error("editconflict", "x", "P"),
            Error::EditConflict(_)
        ));
        assert!(matches!(
            map_api_error("assertuserfailed", "x", "P"),
            Error::AuthExpired(_)
        ));
        assert!(matches!(
            map_api_error("badtoken", "x", "P"),
            Error::AuthExpired(_)
        ));
        assert!(matches!(
            map_api_error("ratelimited", "slow down", "P"),
            Error::RateLimited(_)
        ));
        assert!(matches!(map_api_error("", "x", "P"), Error::WikiApi(_)));
        assert!(matches!(
            map_api_error("unknowncode", "x", "P"),
            Error::EditFailed(_)
        ));
    }
}
