use crate::error::{Error, Result};
use reqwest::{redirect, Client, RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

const DEFAULT_WIKI_HOST: &str = "dontstarve.huijiwiki.com";
const API_PATH: &str = "/api.php";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// MediaWiki allows at most 50 titles per query for regular users.
pub const TITLES_PER_QUERY: usize = 50;

const RETRY_AFTER_CAP: Duration = Duration::from_secs(120);
const BACKOFF_CAP: Duration = Duration::from_secs(60);

/// Throttling/retry behaviour of [`WikiClient`].
///
/// Defaults are conservative because the huijiwiki gateway rejects bursts of
/// requests with intermittent HTTP 403 (observed in practice):
/// 1 request/second, up to 3 retries with exponential backoff.
#[derive(Debug, Clone)]
pub struct RateLimitCfg {
    /// Minimum interval between two outgoing requests.
    pub min_interval: Duration,
    /// Maximum number of retries per request (after the initial attempt).
    pub max_retries: u32,
    /// Base delay for exponential backoff (`base * 2^(attempt-1)`).
    pub base_delay: Duration,
}

impl Default for RateLimitCfg {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_millis(1000),
            max_retries: 3,
            base_delay: Duration::from_secs(2),
        }
    }
}

impl RateLimitCfg {
    /// Reads overrides from the environment:
    /// - `WIKI__QPS`: allowed requests per second (>0), default 1
    /// - `WIKI__MAX_RETRIES`: retry attempts (0..=10), default 3
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = env::var("WIKI__QPS") {
            match v.trim().parse::<f64>() {
                Ok(qps) if qps > 0.0 => cfg.min_interval = Duration::from_secs_f64(1.0 / qps),
                _ => tracing::warn!("invalid WIKI__QPS '{}', using default", v),
            }
        }

        if let Ok(v) = env::var("WIKI__MAX_RETRIES") {
            match v.trim().parse::<u32>() {
                Ok(n @ 0..=10) => cfg.max_retries = n,
                _ => tracing::warn!("invalid WIKI__MAX_RETRIES '{}', using default", v),
            }
        }

        cfg
    }
}

#[derive(Clone)]
pub struct WikiConfig {
    host: String,
    username: String,
    password: String,
    x_authkey: String,
}

impl WikiConfig {
    pub fn new(
        host: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        x_authkey: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            username: username.into(),
            password: password.into(),
            x_authkey: x_authkey.into(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let username = env::var("HUIJI__USERNAME")
            .map_err(|e| Error::EnvVarNotFound(format!("HUIJI__USERNAME: {}", e)))?;
        let password = env::var("HUIJI__PASSWORD")
            .map_err(|e| Error::EnvVarNotFound(format!("HUIJI__PASSWORD: {}", e)))?;
        let x_authkey = env::var("HUIJI__X_AUTHKEY")
            .map_err(|e| Error::EnvVarNotFound(format!("HUIJI__X_AUTHKEY: {}", e)))?;

        Ok(Self::new(DEFAULT_WIKI_HOST, username, password, x_authkey))
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn api_url(&self) -> String {
        format!("https://{}{}", self.host, API_PATH)
    }
}

/// Manual `Debug` so credentials never leak into logs or panic messages.
impl std::fmt::Debug for WikiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WikiConfig")
            .field("host", &self.host)
            .field("username", &self.username)
            .field("password", &"***")
            .field("x_authkey", &"***")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct PageInfo {
    pub pageid: Option<i64>,
    pub title: String,
    pub content: Option<String>,
    pub last_rev_id: Option<i64>,
    pub last_rev_user: Option<String>,
    pub last_rev_timestamp: Option<String>,
}

/// Minimal page metadata returned by [`WikiClient::get_pages_meta`].
///
/// Titles are the wiki-normalized forms; the output order does not follow the
/// input order because MediaWiki returns an unordered page map.
#[derive(Debug, Clone)]
pub struct PageBrief {
    pub title: String,
    pub pageid: Option<i64>,
    pub missing: bool,
}

/// One `File:` page returned by `prop=imageinfo&iiprop=url|size`.
///
/// `title` is the canonical wiki title (first letter upper-cased,
/// underscores displayed as spaces); for a file redirect it names the
/// redirect page while `url` points at the target file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub title: String,
    /// Direct media URL; `None` when the file is missing (or has no version).
    pub url: Option<String>,
    /// Pixel dimensions of the current file version.
    pub size: Option<(u32, u32)>,
    pub missing: bool,
}

/// One entry of a namespace enumeration (`list=allpages`).
///
/// Carries the fields required for touched-based incremental syncing.
#[derive(Debug, Clone)]
pub struct PageListingEntry {
    pub pageid: i64,
    pub ns: i64,
    pub title: String,
    pub touched: Option<String>,
    pub len: Option<u64>,
    pub is_new: bool,
    pub redirect: bool,
}

/// Current-revision wikitext plus categories for one fetched page.
///
/// A missing title yields `missing = true` with no wikitext instead of an
/// error so that callers reconciling enumerations can count gaps.
#[derive(Debug, Clone)]
pub struct PageRevisionContent {
    pub pageid: Option<i64>,
    pub title: String,
    pub wikitext: Option<String>,
    pub sha1: Option<String>,
    pub categories: Vec<String>,
    pub missing: bool,
}

/// One `list=recentchanges` row (corpus RC 增量通道,见 WIKI_CORPUS_PLAN §12)。
/// 日志事件字段按本站实测扁平化在 rc 行上(logtype/logaction/logparams)。
#[derive(Debug, Clone)]
pub struct RecentChange {
    pub rcid: i64,
    pub rc_type: String,
    pub pageid: Option<i64>,
    pub title: Option<String>,
    pub timestamp: String,
    pub user: Option<String>,
    pub sha1: Option<String>,
    pub oldlen: Option<i64>,
    pub newlen: Option<i64>,
    pub revid: Option<i64>,
    pub old_revid: Option<i64>,
    pub comment: Option<String>,
    pub bot_flag: bool,
    pub log_type: Option<String>,
    pub log_action: Option<String>,
    pub log_params: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct EditResult {
    pub result: String,
    pub pageid: Option<i64>,
    pub title: Option<String>,
    pub newrevid: Option<i64>,
    pub oldrevid: Option<i64>,
    pub reason: Option<String>,
}

/// One successful `action=upload` result.
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Canonical file name (without the `File:` prefix).
    pub filename: String,
    /// MediaWiki upload status, normally `Success`.
    pub result: String,
    /// Direct URL of the uploaded (current) file version.
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WikiClient {
    client: Client,
    config: WikiConfig,
    logged_in: bool,
    /// Timestamp of the most recently reserved request slot. Shared behind an
    /// Arc so clones of the client throttle against the same global budget.
    rate: Arc<Mutex<std::time::Instant>>,
    rate_cfg: RateLimitCfg,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    #[serde(rename = "login")]
    result: Option<LoginResult>,
}

#[derive(Debug, Deserialize)]
struct LoginResult {
    result: String,
    #[serde(rename = "lgusername")]
    lgusername: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    query: Option<TokenQuery>,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    tokens: Option<TokenTokens>,
}

#[derive(Debug, Deserialize)]
struct TokenTokens {
    #[serde(rename = "logintoken")]
    logintoken: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    query: Option<QueryPages>,
}

#[derive(Debug, Deserialize)]
struct QueryPages {
    pages: Option<HashMap<String, QueryPage>>,
}

#[derive(Debug, Deserialize)]
struct QueryPage {
    pageid: Option<i64>,
    title: String,
    revisions: Option<Vec<QueryRevision>>,
    missing: Option<bool>,
    invalid: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct QueryRevision {
    #[serde(rename = "*")]
    content: Option<String>,
    user: Option<String>,
    timestamp: Option<String>,
    revid: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UploadResponse {
    upload: Option<UploadInfo>,
}

#[derive(Debug, Deserialize)]
struct UploadInfo {
    result: Option<String>,
    filename: Option<String>,
    imageinfo: Option<Value>,
    warnings: Option<Value>,
}

/// Maps a MediaWiki upload API error code to an [`Error`].
fn map_upload_error(code: &str, info: &str, filename: &str) -> Error {
    match code {
        "ratelimited" => Error::RateLimited(info.to_string()),
        "assertuserfailed" | "notloggedin" | "mustbeloggedin" | "badtoken" => {
            Error::AuthExpired(format!("{}: {}", code, info))
        }
        other => Error::UploadFailed(format!("{}: {} ({})", other, info, filename)),
    }
}

/// Best-effort MIME type from the file extension (MediaWiki validates too).
fn mime_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Deserialize)]
struct EditResponse {
    edit: Option<EditInfo>,
}

#[derive(Debug, Deserialize)]
struct EditInfo {
    result: String,
    pageid: Option<i64>,
    title: Option<String>,
    newrevid: Option<i64>,
    oldrevid: Option<i64>,
    reason: Option<String>,
}

/// MediaWiki returns HTTP 200 even for API-level errors; this captures the
/// top-level `{"error": {"code": ..., "info": ...}}` body.
#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: String,
    #[serde(default)]
    info: String,
}

/// Maps a MediaWiki API error code to a specific [`Error`] variant so that
/// automated flows can branch on the failure kind.
fn map_api_error(code: &str, info: &str, title: &str) -> Error {
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

fn lock_rate(m: &Mutex<std::time::Instant>) -> MutexGuard<'_, std::time::Instant> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// True when a response with this status should be retried.
///
/// GET requests may be retried on any server error; POST requests are only
/// retried on WAF-level rejections (403/429) where MediaWiki definitely did
/// not process the request — a 5xx after a write might have been applied.
fn retryable_status(is_get: bool, status: StatusCode) -> bool {
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return true;
    }
    is_get && status.is_server_error()
}

/// Exponential backoff honouring an optional `Retry-After` hint.
/// `attempt` starts at 1.
fn compute_backoff(base: Duration, attempt: u32, retry_after: Option<Duration>) -> Duration {
    if let Some(ra) = retry_after {
        return ra.min(RETRY_AFTER_CAP);
    }
    let shift = attempt.saturating_sub(1).min(6);
    base.checked_mul(1u32 << shift)
        .unwrap_or(BACKOFF_CAP)
        .min(BACKOFF_CAP)
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let v = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    v.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Milliseconds since the Unix epoch, used as the `_` cache-buster parameter.
///
/// huijiwiki's CDN caches anonymous API GETs for hours and re-uploads/page
/// edits do not reliably purge those entries, so maintenance reads append a
/// unique value to get a fresh response.
fn cache_buster() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_default()
}

/// Splits titles into MediaWiki-sized chunks, preserving order and duplicates.
fn chunk_titles<'a>(titles: &[&'a str], size: usize) -> Vec<Vec<&'a str>> {
    assert!(size > 0, "chunk size must be positive");
    titles.chunks(size).map(<[&'a str]>::to_vec).collect()
}

/// MediaWiki (formatversion=1) encodes booleans as empty strings:
/// present-and-empty means `true`, absent means `false`.
fn de_bc_bool<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<Value> = Option::deserialize(deserializer)?;
    Ok(match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => b,
        // formatversion=1 marks a true flag by an (empty) string value.
        Some(Value::String(_)) => true,
        Some(_) => true,
    })
}

#[derive(Debug, Deserialize)]
struct RawInfoPage {
    title: String,
    pageid: Option<i64>,
    #[serde(default, deserialize_with = "de_bc_bool")]
    missing: bool,
}

#[derive(Debug, Deserialize)]
struct RawInfoQuery {
    pages: Option<HashMap<String, RawInfoPage>>,
}

#[derive(Debug, Deserialize)]
struct RawInfoResponse {
    query: Option<RawInfoQuery>,
}

/// Extracts [`PageBrief`]s from an `action=query&prop=info` response body.
fn parse_pages_info_response(body: &Value) -> Result<Vec<PageBrief>> {
    let parsed: RawInfoResponse =
        serde_json::from_value(body.clone()).map_err(|e| Error::WikiApi(e.to_string()))?;
    let pages = parsed
        .query
        .and_then(|q| q.pages)
        .ok_or_else(|| Error::WikiApi("No pages in response".to_string()))?;
    Ok(pages
        .into_values()
        .map(|p| PageBrief {
            title: p.title,
            pageid: p.pageid,
            missing: p.missing,
        })
        .collect())
}

/// Canonicalises a file name the way MediaWiki titles do: optional
/// `File:`/`文件:` prefix, first character upper-cased, underscores displayed
/// as spaces (`infographic_over.png` → `File:Infographic over.png`).
pub fn file_title(name: &str) -> String {
    let bare = name
        .strip_prefix("File:")
        .or_else(|| name.strip_prefix("文件:"))
        .unwrap_or(name);
    let mut chars = bare.chars();
    match chars.next() {
        Some(head) => format!(
            "File:{}{}",
            head.to_uppercase(),
            chars.as_str().replace('_', " ")
        ),
        None => "File:".to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct RawNormalizedTitle {
    from: String,
    to: String,
}

#[derive(Debug, Deserialize)]
struct RawImageInfoItem {
    url: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawFilePage {
    title: String,
    #[serde(default, deserialize_with = "de_bc_bool")]
    missing: bool,
    imageinfo: Option<Vec<RawImageInfoItem>>,
}

#[derive(Debug, Deserialize)]
struct RawFileQuery {
    #[serde(default)]
    normalized: Option<Vec<RawNormalizedTitle>>,
    pages: Option<HashMap<String, RawFilePage>>,
}

#[derive(Debug, Deserialize)]
struct RawFileResponse {
    query: Option<RawFileQuery>,
}

/// Extracts one [`FileInfo`] per requested name, in the same order.
///
/// MediaWiki answers one page per title (missing files carry `missing`) and
/// reports title normalisations (first-letter case, underscore/space) in
/// `query.normalized`; both are applied so callers can zip the result with
/// their input.
fn parse_files_info_response(body: &Value, requested: &[&str]) -> Result<Vec<FileInfo>> {
    let parsed: RawFileResponse =
        serde_json::from_value(body.clone()).map_err(|e| Error::WikiApi(e.to_string()))?;
    let query = parsed
        .query
        .ok_or_else(|| Error::WikiApi("No file query in response".to_string()))?;
    let normalized: HashMap<String, String> = query
        .normalized
        .unwrap_or_default()
        .into_iter()
        .map(|n| (n.from, n.to))
        .collect();
    let pages = query
        .pages
        .ok_or_else(|| Error::WikiApi("No pages in response".to_string()))?;
    let by_title: HashMap<String, RawFilePage> =
        pages.into_values().map(|p| (p.title.clone(), p)).collect();

    let mut out = Vec::with_capacity(requested.len());
    for name in requested {
        let sent = file_title(name);
        let canonical = normalized.get(&sent).cloned().unwrap_or(sent);
        out.push(match by_title.get(&canonical) {
            Some(page) => {
                let first = page.imageinfo.as_ref().and_then(|items| items.first());
                FileInfo {
                    title: page.title.clone(),
                    url: first.and_then(|item| item.url.clone()),
                    size: first.and_then(|item| item.width.zip(item.height)),
                    missing: page.missing || page.imageinfo.is_none(),
                }
            }
            None => FileInfo {
                title: canonical,
                url: None,
                size: None,
                missing: true,
            },
        });
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
struct RawAllpagesBatch {
    allpages: Option<Vec<RawAllpageItem>>,
}

#[derive(Debug, Deserialize)]
struct RawAllpageItem {
    title: String,
}

#[derive(Debug, Deserialize)]
struct RawContinue {
    #[serde(rename = "apcontinue")]
    ap_continue: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAllpagesResponse {
    query: Option<RawAllpagesBatch>,
    #[serde(default, rename = "continue")]
    cont: Option<RawContinue>,
}

/// Extracts `(titles, next_apcontinue)` from one `list=allpages` batch.
fn parse_allpages_batch(body: &Value) -> Result<(Vec<String>, Option<String>)> {
    let parsed: RawAllpagesResponse =
        serde_json::from_value(body.clone()).map_err(|e| Error::WikiApi(e.to_string()))?;
    let titles = parsed
        .query
        .and_then(|q| q.allpages)
        .ok_or_else(|| Error::WikiApi("No allpages in response".to_string()))?
        .into_iter()
        .map(|i| i.title)
        .collect();
    Ok((titles, parsed.cont.and_then(|c| c.ap_continue)))
}

#[derive(Debug, Deserialize)]
struct RawInfoFullPage {
    pageid: i64,
    ns: i64,
    title: String,
    touched: Option<String>,
    /// prop=info names this field `length` (list=allpages uses `len`).
    #[serde(rename = "length")]
    len: Option<u64>,
    #[serde(default, deserialize_with = "de_bc_bool")]
    new: bool,
    #[serde(default, deserialize_with = "de_bc_bool")]
    redirect: bool,
}

#[derive(Debug, Deserialize)]
struct RawInfoFullQuery {
    pages: Option<HashMap<String, RawInfoFullPage>>,
}

#[derive(Debug, Deserialize)]
struct RawGapContinue {
    #[serde(rename = "gapcontinue")]
    gap_continue: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawInfoFullResponse {
    query: Option<RawInfoFullQuery>,
    #[serde(default, rename = "continue")]
    cont: Option<RawGapContinue>,
}

/// Extracts `(entries, next_gapcontinue)` from one `generator=allpages&
/// prop=info` batch. Unlike plain `list=allpages` on this wiki's MediaWiki
/// (1.38), the info-prop page set reliably carries `touched`, `len`, `new`
/// and the redirect flag — all required for touched-based incremental sync.
/// 解析 `list=recentchanges` 响应:rc 行 + `continue.rccontinue` 续传键。
fn parse_recentchanges_response(body: &Value) -> Result<(Vec<RecentChange>, Option<String>)> {
    let rows = body
        .get("query")
        .and_then(|q| q.get("recentchanges"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let rcid = row.get("rcid").and_then(|v| v.as_i64()).unwrap_or(0);
        let timestamp = row
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if rcid == 0 || timestamp.is_empty() {
            continue;
        }
        let bot_flag = row
            .get("flags")
            .and_then(|v| v.as_str())
            .map(|f| f.contains('B'))
            .unwrap_or(false);
        out.push(RecentChange {
            rcid,
            rc_type: row
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            pageid: row.get("pageid").and_then(|v| v.as_i64()),
            title: row
                .get("title")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            timestamp,
            user: row.get("user").and_then(|v| v.as_str()).map(str::to_string),
            sha1: row.get("sha1").and_then(|v| v.as_str()).map(str::to_string),
            oldlen: row.get("oldlen").and_then(|v| v.as_i64()),
            newlen: row.get("newlen").and_then(|v| v.as_i64()),
            revid: row.get("revid").and_then(|v| v.as_i64()),
            old_revid: row.get("old_revid").and_then(|v| v.as_i64()),
            comment: row
                .get("comment")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            bot_flag,
            log_type: row
                .get("logtype")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            log_action: row
                .get("logaction")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            log_params: row.get("logparams").cloned(),
        });
    }
    let next = body
        .get("continue")
        .and_then(|c| c.get("rccontinue"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok((out, next))
}

fn parse_allpages_info_entries(body: &Value) -> Result<(Vec<PageListingEntry>, Option<String>)> {
    let parsed: RawInfoFullResponse =
        serde_json::from_value(body.clone()).map_err(|e| Error::WikiApi(e.to_string()))?;
    let pages = parsed
        .query
        .and_then(|q| q.pages)
        .ok_or_else(|| Error::WikiApi("No pages in response".to_string()))?;
    let mut entries: Vec<PageListingEntry> = pages
        .into_values()
        .map(|p| PageListingEntry {
            pageid: p.pageid,
            ns: p.ns,
            title: p.title,
            touched: p.touched,
            len: p.len,
            is_new: p.new,
            redirect: p.redirect,
        })
        .collect();
    // Page maps are unordered; sort by pageid so rounds are reproducible.
    entries.sort_by_key(|e| e.pageid);
    Ok((entries, parsed.cont.and_then(|c| c.gap_continue)))
}

#[derive(Debug, Deserialize)]
struct RawContentSlot {
    #[serde(rename = "*")]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawContentSlots {
    main: Option<RawContentSlot>,
}

#[derive(Debug, Deserialize)]
struct RawContentRevision {
    slots: Option<RawContentSlots>,
    sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCategory {
    title: String,
}

#[derive(Debug, Deserialize)]
struct RawContentPage {
    pageid: Option<i64>,
    title: String,
    #[serde(default, deserialize_with = "de_bc_bool")]
    missing: bool,
    revisions: Option<Vec<RawContentRevision>>,
    categories: Option<Vec<RawCategory>>,
}

#[derive(Debug, Deserialize)]
struct RawContentQuery {
    pages: Option<HashMap<String, RawContentPage>>,
}

#[derive(Debug, Deserialize)]
struct RawContentResponse {
    query: Option<RawContentQuery>,
}

/// Extracts fetched page contents from one `prop=revisions|categories` batch.
fn parse_pages_content_response(body: &Value) -> Result<Vec<PageRevisionContent>> {
    let parsed: RawContentResponse =
        serde_json::from_value(body.clone()).map_err(|e| Error::WikiApi(e.to_string()))?;
    let pages = parsed
        .query
        .and_then(|q| q.pages)
        .ok_or_else(|| Error::WikiApi("No pages in response".to_string()))?;

    let mut out = Vec::with_capacity(pages.len());
    for page in pages.into_values() {
        let (wikitext, sha1) = match page.revisions.as_ref().and_then(|r| r.first()) {
            Some(rev) => (
                rev.slots
                    .as_ref()
                    .and_then(|s| s.main.as_ref())
                    .and_then(|m| m.content.clone()),
                rev.sha1.clone(),
            ),
            None => (None, None),
        };
        out.push(PageRevisionContent {
            pageid: page.pageid,
            title: page.title,
            wikitext,
            sha1,
            categories: page
                .categories
                .map(|cs| cs.into_iter().map(|c| c.title).collect())
                .unwrap_or_default(),
            missing: page.missing,
        });
    }
    Ok(out)
}

impl WikiClient {
    pub fn new(config: WikiConfig) -> Result<Self> {
        let client = Client::builder()
            .redirect(redirect::Policy::limited(10))
            .cookie_store(true)
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()?;

        Ok(Self {
            client,
            config,
            logged_in: false,
            rate: Arc::new(Mutex::new(
                std::time::Instant::now() - RateLimitCfg::default().min_interval,
            )),
            rate_cfg: RateLimitCfg::default(),
        })
    }

    pub fn from_env() -> Result<Self> {
        let config = WikiConfig::from_env()?;
        Ok(Self::new(config)?.with_rate_cfg(RateLimitCfg::from_env()))
    }

    /// Overrides the throttling/retry configuration (mainly for tests).
    pub fn with_rate_cfg(mut self, cfg: RateLimitCfg) -> Self {
        self.rate_cfg = cfg;
        self
    }

    pub fn is_logged_in(&self) -> bool {
        self.logged_in
    }

    pub fn config(&self) -> &WikiConfig {
        &self.config
    }

    async fn acquire_slot(&self) {
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
    async fn send_with_retry(
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

    async fn get_login_token(&self) -> Result<String> {
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

    pub async fn login(&mut self) -> Result<()> {
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
                    self.logged_in = true;
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

    pub async fn get_page(&self, title: &str) -> Result<PageInfo> {
        let nonce = cache_buster();
        let params = [
            ("action", "query"),
            ("prop", "revisions"),
            ("rvprop", "content|user|timestamp|ids"),
            ("rvlimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("_", nonce.as_str()),
        ];

        let response = self.get(&params).await?;
        let query_resp: QueryResponse = response.json().await?;

        let pages = query_resp
            .query
            .and_then(|q| q.pages)
            .ok_or_else(|| Error::WikiApi("No pages in response".to_string()))?;

        let page = pages
            .values()
            .next()
            .ok_or_else(|| Error::WikiApi("No page found".to_string()))?;

        if page.missing.unwrap_or(false) {
            return Err(Error::PageNotFound(title.to_string()));
        }

        if page.invalid.unwrap_or(false) {
            return Err(Error::WikiApi(format!("Invalid page title: '{}'", title)));
        }

        let (content, last_rev_user, last_rev_timestamp, last_rev_id) =
            if let Some(revisions) = &page.revisions {
                if let Some(rev) = revisions.first() {
                    (
                        rev.content.clone(),
                        rev.user.clone(),
                        rev.timestamp.clone(),
                        rev.revid,
                    )
                } else {
                    (None, None, None, None)
                }
            } else {
                (None, None, None, None)
            };

        Ok(PageInfo {
            pageid: page.pageid,
            title: page.title.clone(),
            content,
            last_rev_id,
            last_rev_user,
            last_rev_timestamp,
        })
    }

    /// Fetches existence/metadata for up to N titles in batches of
    /// [`TITLES_PER_QUERY`] with throttling applied across batches.
    pub async fn get_pages_meta(&self, titles: &[&str]) -> Result<Vec<PageBrief>> {
        let mut out = Vec::with_capacity(titles.len());
        for chunk in chunk_titles(titles, TITLES_PER_QUERY) {
            let joined = chunk.join("|");
            let nonce = cache_buster();
            let params = [
                ("action", "query"),
                ("prop", "info"),
                ("titles", joined.as_str()),
                ("format", "json"),
                ("_", nonce.as_str()),
            ];
            let response = self.get(&params).await?;
            let body: Value = response.json().await?;
            out.extend(parse_pages_info_response(&body)?);
        }
        Ok(out)
    }

    /// Convenience check for whether a single page exists.
    pub async fn page_exists(&self, title: &str) -> Result<bool> {
        let pages = self.get_pages_meta(std::slice::from_ref(&title)).await?;
        Ok(pages.first().is_some_and(|p| !p.missing))
    }

    /// Batch-queries `File:` pages via `prop=imageinfo&iiprop=url|size`.
    ///
    /// Returns one [`FileInfo`] per input name, in the same order. Names may
    /// omit the `File:` prefix and may use underscores/any first-letter case:
    /// MediaWiki normalises the first letter to upper case and displays
    /// underscores as spaces (e.g. `infographic_over.png` ⇔
    /// `File:Infographic over.png`). File redirects (renamed uploads) resolve
    /// to the target file's URL. Batches of [`TITLES_PER_QUERY`] are throttled
    /// like any other read.
    ///
    /// Every batch carries a unique `_` nonce: huijiwiki's CDN caches
    /// anonymous API GETs for hours and file uploads do not purge that cache,
    /// so a plain repeated query would report the pre-upload size/URL.
    pub async fn get_files_info(&self, names: &[&str]) -> Result<Vec<FileInfo>> {
        let mut out = Vec::with_capacity(names.len());
        for chunk in chunk_titles(names, TITLES_PER_QUERY) {
            let joined = chunk
                .iter()
                .map(|name| file_title(name))
                .collect::<Vec<_>>()
                .join("|");
            let nonce = cache_buster();
            let params = [
                ("action", "query"),
                ("prop", "imageinfo"),
                ("iiprop", "url|size"),
                ("titles", joined.as_str()),
                ("format", "json"),
                ("_", nonce.as_str()),
            ];
            let response = self.get(&params).await?;
            let body: Value = response.json().await?;
            out.extend(parse_files_info_response(&body, &chunk)?);
        }
        Ok(out)
    }

    /// Convenience check for whether a single `File:` page exists.
    pub async fn file_exists(&self, name: &str) -> Result<bool> {
        Ok(self
            .get_files_info(std::slice::from_ref(&name))
            .await?
            .first()
            .is_some_and(|f| !f.missing))
    }

    /// Convenience getter for one file's direct URL (`None` if missing).
    pub async fn get_file_url(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .get_files_info(std::slice::from_ref(&name))
            .await?
            .first()
            .and_then(|f| f.url.clone()))
    }

    /// Lists page titles of a namespace via `list=allpages`, following
    /// `apcontinue` until exhausted (or `limit` titles collected).
    ///
    /// Requests are throttled by the shared rate limiter; large namespaces
    /// take one request per 500 titles.
    pub async fn list_all_pages(
        &self,
        namespace: i64,
        prefix: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        let mut titles = Vec::new();
        let mut cont: Option<String> = None;
        loop {
            let mut params: Vec<(String, String)> = vec![
                ("action".to_string(), "query".to_string()),
                ("list".to_string(), "allpages".to_string()),
                ("apnamespace".to_string(), namespace.to_string()),
                ("aplimit".to_string(), "max".to_string()),
                ("format".to_string(), "json".to_string()),
            ];
            if let Some(p) = prefix {
                params.push(("apprefix".to_string(), p.to_string()));
            }
            if let Some(c) = &cont {
                params.push(("apcontinue".to_string(), c.clone()));
            }
            let params_ref: Vec<(&str, &str)> = params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            let response = self.get(&params_ref).await?;
            let body: Value = response.json().await?;
            let (batch, next) = parse_allpages_batch(&body)?;
            titles.extend(batch);

            match next {
                Some(c) => cont = Some(c),
                None => break,
            }
            if let Some(max) = limit {
                if titles.len() >= max {
                    titles.truncate(max);
                    break;
                }
            }
        }
        Ok(titles)
    }

    /// Enumerates every page of a namespace (redirects included) via
    /// `generator=allpages` + `prop=info`, following `gapcontinue` until
    /// exhausted.
    ///
    /// One request per 500 entries under the shared throttle. Each entry
    /// carries the metadata required for touched-based incremental sync
    /// (`touched`, `len`, `new`, redirect flag), so no per-page info queries
    /// are needed before deciding what to (re)fetch. Plain `list=allpages`
    /// is deliberately avoided: on MediaWiki 1.38 its entries lack these
    /// fields, which would silently disable incremental detection.
    pub async fn enumerate_namespace(&self, namespace: i64) -> Result<Vec<PageListingEntry>> {
        let mut entries = Vec::new();
        let mut cont: Option<String> = None;
        loop {
            let mut params: Vec<(String, String)> = vec![
                ("action".to_string(), "query".to_string()),
                ("generator".to_string(), "allpages".to_string()),
                ("gapnamespace".to_string(), namespace.to_string()),
                ("gaplimit".to_string(), "max".to_string()),
                ("prop".to_string(), "info".to_string()),
                ("format".to_string(), "json".to_string()),
            ];
            if let Some(c) = &cont {
                params.push(("gapcontinue".to_string(), c.clone()));
            }
            let params_ref: Vec<(&str, &str)> = params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            let response = self.get(&params_ref).await?;
            let body: Value = response.json().await?;
            let (batch, next) = parse_allpages_info_entries(&body)?;
            entries.extend(batch);

            match next {
                Some(c) => cont = Some(c),
                None => break,
            }
        }
        Ok(entries)
    }

    /// Fetches main-namespace recent changes newer than `start` (MediaWiki
    /// timestamp), following `rccontinue` to exhaustion. RC 增量通道的 API
    /// 面:参数形态与本站实测结论见 docs/WIKI_CORPUS_PLAN.md §12.2。
    pub async fn recentchanges(&self, start: &str) -> Result<Vec<RecentChange>> {
        let mut out = Vec::new();
        let mut cont: Option<String> = None;
        loop {
            let mut params: Vec<(String, String)> = vec![
                ("action".to_string(), "query".to_string()),
                ("list".to_string(), "recentchanges".to_string()),
                ("rcdir".to_string(), "newer".to_string()),
                ("rcstart".to_string(), start.to_string()),
                ("rctype".to_string(), "edit|new|log".to_string()),
                ("rcnamespace".to_string(), "0".to_string()),
                (
                    "rcprop".to_string(),
                    "title|timestamp|ids|sizes|sha1|user|flags|comment|loginfo".to_string(),
                ),
                ("rclimit".to_string(), "max".to_string()),
                ("format".to_string(), "json".to_string()),
            ];
            if let Some(c) = &cont {
                params.push(("rccontinue".to_string(), c.clone()));
            }
            let params_ref: Vec<(&str, &str)> = params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            let response = self.get(&params_ref).await?;
            let body: Value = response.json().await?;
            let (batch, next) = parse_recentchanges_response(&body)?;
            out.extend(batch);

            match next {
                Some(c) => cont = Some(c),
                None => break,
            }
        }
        Ok(out)
    }

    /// Fetches current-revision wikitext, revision sha1 and visible category
    /// members for the given titles, in batches of [`TITLES_PER_QUERY`] with
    /// throttling applied across batches.
    ///
    /// Missing titles come back with `missing == true` and no wikitext rather
    /// than an error — callers reconciling an enumeration against local state
    /// need to count gaps, not abort on them.
    pub async fn get_pages_wikitext(&self, titles: &[&str]) -> Result<Vec<PageRevisionContent>> {
        let mut out = Vec::with_capacity(titles.len());
        for chunk in chunk_titles(titles, TITLES_PER_QUERY) {
            let joined = chunk.join("|");
            let params = [
                ("action", "query"),
                ("prop", "revisions|categories"),
                ("rvprop", "content|sha1"),
                ("rvslots", "main"),
                ("cllimit", "max"),
                ("titles", joined.as_str()),
                ("format", "json"),
            ];
            let response = self.get(&params).await?;
            let body: Value = response.json().await?;
            out.extend(parse_pages_content_response(&body)?);
        }
        Ok(out)
    }

    async fn do_edit(
        &self,
        title: &str,
        params: Vec<(&str, &str)>,
        minor: Option<bool>,
        basetimestamp: Option<&str>,
    ) -> Result<EditResult> {
        if !self.logged_in {
            return Err(Error::EditFailed("Not logged in".to_string()));
        }

        let csrf_token = self.get_csrf_token().await?;

        let mut all_params: Vec<(&str, String)> = vec![
            ("action", "edit".to_string()),
            ("title", title.to_string()),
            ("token", csrf_token),
            ("assert", "user".to_string()),
            ("format", "json".to_string()),
        ];

        for (k, v) in &params {
            all_params.push((*k, (*v).to_string()));
        }

        if let Some(ts) = basetimestamp {
            all_params.push(("basetimestamp", ts.to_string()));
        }

        if let Some(true) = minor {
            all_params.push(("minor", "true".to_string()));
        }

        let params_refs: Vec<(&str, &str)> =
            all_params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self.post(&params_refs).await?;

        // MediaWiki answers HTTP 200 even for API-level errors such as
        // editconflict, so inspect the JSON body first.
        let body: Value = response.json().await?;
        if let Some(err) = body.get("error") {
            let parsed: ApiErrorBody =
                serde_json::from_value(err.clone()).unwrap_or(ApiErrorBody {
                    code: String::new(),
                    info: "unknown error shape".to_string(),
                });
            tracing::warn!(code = %parsed.code, info = %parsed.info, page = title, "wiki edit rejected");
            return Err(map_api_error(&parsed.code, &parsed.info, title));
        }

        let edit_resp: EditResponse = serde_json::from_value(body)
            .map_err(|e| Error::WikiApi(format!("unexpected edit response: {}", e)))?;

        let edit_info = edit_resp
            .edit
            .ok_or_else(|| Error::WikiApi("No edit response received".to_string()))?;

        match edit_info.result.as_str() {
            "Success" | "success" => Ok(EditResult {
                result: edit_info.result,
                pageid: edit_info.pageid,
                title: edit_info.title,
                newrevid: edit_info.newrevid,
                oldrevid: edit_info.oldrevid,
                reason: edit_info.reason,
            }),
            _ => Err(Error::EditFailed(format!(
                "Edit failed: {}",
                edit_info.reason.unwrap_or_else(|| edit_info.result.clone())
            ))),
        }
    }

    /// Edits a page. Pass the revision timestamp obtained from
    /// [`PageInfo::last_rev_timestamp`] (fetched just before computing the new
    /// text) to enable conflict detection.
    pub async fn edit_page(
        &self,
        title: &str,
        text: &str,
        summary: Option<&str>,
        minor: bool,
        basetimestamp: Option<&str>,
    ) -> Result<EditResult> {
        let mut params: Vec<(&str, &str)> = vec![("text", text)];

        if let Some(s) = summary {
            params.push(("summary", s));
        }

        let result = self
            .do_edit(title, params, Some(minor), basetimestamp)
            .await?;

        tracing::info!(
            "Successfully edited page '{}' (pageid: {:?}, newrevid: {:?})",
            result.title.as_deref().unwrap_or(title),
            result.pageid,
            result.newrevid
        );

        Ok(result)
    }

    /// Uploads a local file via `action=upload` (multipart).
    ///
    /// `file_name` defaults to the local file name; `description` is written
    /// to the file description page (`text`) and `comment` becomes the upload
    /// log comment. Passing `ignore_warnings = true` sends `ignorewarnings=1`
    /// so an existing file can be re-uploaded without the duplicate warning.
    /// Requires a logged-in client.
    pub async fn upload_file(
        &self,
        path: &std::path::Path,
        file_name: Option<&str>,
        description: Option<&str>,
        comment: Option<&str>,
        ignore_warnings: bool,
    ) -> Result<UploadResult> {
        if !self.logged_in {
            return Err(Error::AuthExpired("upload requires login".to_string()));
        }

        let filename = match file_name {
            Some(name) => name.to_string(),
            None => path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?
                .to_string(),
        };
        let bytes = tokio::fs::read(path).await?;
        let csrf_token = self.get_csrf_token().await?;
        let mime = mime_for_path(path);
        let url = self.config.api_url();

        let response = self
            .send_with_retry(false, || {
                let part = reqwest::multipart::Part::bytes(bytes.clone())
                    .file_name(filename.clone())
                    .mime_str(mime)
                    .unwrap_or_else(|_| {
                        reqwest::multipart::Part::bytes(bytes.clone()).file_name(filename.clone())
                    });
                let mut form = reqwest::multipart::Form::new()
                    .text("action", "upload")
                    .text("format", "json")
                    .text("filename", filename.clone())
                    .text("token", csrf_token.clone())
                    .text("assert", "user")
                    .text("ignorewarnings", if ignore_warnings { "1" } else { "0" })
                    .part("file", part);
                if let Some(desc) = description {
                    form = form.text("text", desc.to_string());
                }
                if let Some(note) = comment {
                    form = form.text("comment", note.to_string());
                }
                self.client
                    .post(&url)
                    .header("X-authkey", &self.config.x_authkey)
                    .multipart(form)
            })
            .await?;

        let body: Value = response.json().await?;
        if let Some(err) = body.get("error") {
            let parsed: ApiErrorBody =
                serde_json::from_value(err.clone()).unwrap_or(ApiErrorBody {
                    code: String::new(),
                    info: "unknown error shape".to_string(),
                });
            tracing::warn!(code = %parsed.code, info = %parsed.info, file = filename, "wiki upload rejected");
            return Err(map_upload_error(&parsed.code, &parsed.info, &filename));
        }

        let upload: UploadInfo = serde_json::from_value(body)
            .ok()
            .and_then(|r: UploadResponse| r.upload)
            .ok_or_else(|| Error::WikiApi("No upload response received".to_string()))?;

        if upload.result.as_deref() != Some("Success") {
            return Err(Error::UploadFailed(format!(
                "{} {}",
                upload.result.unwrap_or_else(|| "Unknown".to_string()),
                upload.warnings.map(|w| w.to_string()).unwrap_or_default()
            )));
        }

        tracing::info!(
            "Uploaded file '{}'",
            upload.filename.as_deref().unwrap_or(&filename)
        );
        Ok(UploadResult {
            filename: upload.filename.unwrap_or(filename),
            result: upload.result.unwrap_or_default(),
            url: upload
                .imageinfo
                .as_ref()
                .and_then(|info| info.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
    }

    pub async fn append_to_page(
        &self,
        title: &str,
        text: &str,
        summary: Option<&str>,
        basetimestamp: Option<&str>,
    ) -> Result<EditResult> {
        let mut params: Vec<(&str, &str)> = vec![("appendtext", text)];

        if let Some(s) = summary {
            params.push(("summary", s));
        }

        self.do_edit(title, params, None, basetimestamp).await
    }

    pub async fn prepend_to_page(
        &self,
        title: &str,
        text: &str,
        summary: Option<&str>,
        basetimestamp: Option<&str>,
    ) -> Result<EditResult> {
        let mut params: Vec<(&str, &str)> = vec![("prependtext", text)];

        if let Some(s) = summary {
            params.push(("summary", s));
        }

        self.do_edit(title, params, None, basetimestamp).await
    }

    pub async fn get_json_data(&self, title: &str) -> Result<serde_json::Value> {
        let page = self.get_page(title).await?;

        let content = page
            .content
            .ok_or_else(|| Error::WikiApi(format!("Page '{}' has no content", title)))?;

        let json_str = content.trim();
        serde_json::from_str(json_str).map_err(|e| {
            Error::WikiApi(format!("Failed to parse JSON from page '{}': {}", title, e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PAGE: &str = "用户讨论:2199AshBark";

    #[test]
    fn test_config_from_env_missing() {
        dotenvy::dotenv().ok();
        let result = WikiConfig::from_env();
        if let Err(e) = result {
            assert!(matches!(e, Error::EnvVarNotFound(_)));
        }
    }

    #[test]
    fn test_api_url() {
        let config = WikiConfig::new("example.huijiwiki.com", "test", "test", "test");
        assert_eq!(config.api_url(), "https://example.huijiwiki.com/api.php");
        assert_eq!(config.host(), "example.huijiwiki.com");
        assert_eq!(config.username(), "test");
    }

    #[test]
    fn test_config_debug_masks_credentials() {
        let config = WikiConfig::new("h", "user", "s3cret-password", "authkey-123");
        let debug = format!("{:?}", config);
        assert!(!debug.contains("s3cret-password"));
        assert!(!debug.contains("authkey-123"));
        assert!(debug.contains("***"));
        assert!(debug.contains("user"));
    }

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
    fn test_parse_pages_info_response() {
        let body = serde_json::json!({
            "batchcomplete": "",
            "query": {
                "pages": {
                    "123": {"pageid": 123, "ns": 0, "title": "火腿棒"},
                    "-1": {"ns": 0, "title": "不存在的页", "missing": ""}
                }
            }
        });
        let briefs = parse_pages_info_response(&body).unwrap();
        assert_eq!(briefs.len(), 2);
        let existing = briefs.iter().find(|b| b.title == "火腿棒").unwrap();
        assert_eq!(existing.pageid, Some(123));
        assert!(!existing.missing);
        let missing = briefs.iter().find(|b| b.title == "不存在的页").unwrap();
        assert!(missing.missing);
        assert!(missing.pageid.is_none());
    }

    #[test]
    fn test_mime_for_path() {
        assert_eq!(mime_for_path(std::path::Path::new("a.PNG")), "image/png");
        assert_eq!(mime_for_path(std::path::Path::new("a.jpg")), "image/jpeg");
        assert_eq!(mime_for_path(std::path::Path::new("a.jpeg")), "image/jpeg");
        assert_eq!(mime_for_path(std::path::Path::new("a.gif")), "image/gif");
        assert_eq!(
            mime_for_path(std::path::Path::new("a.bin")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_map_upload_error_codes() {
        assert!(matches!(
            map_upload_error("ratelimited", "slow", "a.png"),
            Error::RateLimited(_)
        ));
        assert!(matches!(
            map_upload_error("mustbeloggedin", "x", "a.png"),
            Error::AuthExpired(_)
        ));
        assert!(matches!(
            map_upload_error("fileexists-no-change", "dup", "a.png"),
            Error::UploadFailed(_)
        ));
    }

    #[test]
    fn test_file_title_normalizes() {
        assert_eq!(
            file_title("infographic_over.png"),
            "File:Infographic over.png"
        );
        assert_eq!(file_title("File:Frame.png"), "File:Frame.png");
        assert_eq!(file_title("文件:Frame.png"), "File:Frame.png");
        assert_eq!(file_title("Wortox_scales"), "File:Wortox scales");
        assert_eq!(file_title(""), "File:");
    }

    #[test]
    fn test_parse_files_info_response() {
        let body = serde_json::json!({
            "batchcomplete": "",
            "query": {
                "normalized": [
                    {"from": "File:infographic_over.png", "to": "File:Infographic over.png"}
                ],
                "pages": {
                    "100": {
                        "pageid": 100, "ns": 6, "title": "File:Infographic over.png",
                        "imagerepository": "local",
                        "imageinfo": [{
                            "url": "https://example.com/Infographic_over.png",
                            "width": 64, "height": 64
                        }]
                    },
                    "-1": {"ns": 6, "title": "File:Not uploaded.png", "missing": ""}
                }
            }
        });
        let infos =
            parse_files_info_response(&body, &["File:infographic_over.png", "not_uploaded.png"])
                .unwrap();
        assert_eq!(infos.len(), 2);
        // Canonical title + direct URL, aligned with the input order.
        assert_eq!(infos[0].title, "File:Infographic over.png");
        assert_eq!(
            infos[0].url.as_deref(),
            Some("https://example.com/Infographic_over.png")
        );
        assert_eq!(infos[0].size, Some((64, 64)));
        assert!(!infos[0].missing);
        // Missing files carry the BC empty-string flag and no URL.
        assert!(infos[1].missing);
        assert!(infos[1].url.is_none());
        assert!(infos[1].size.is_none());
        assert_eq!(infos[1].title, "File:Not uploaded.png");
        // Duplicate inputs still yield one aligned entry per input.
        let dup = parse_files_info_response(
            &body,
            &["File:infographic_over.png", "File:infographic_over.png"],
        )
        .unwrap();
        assert_eq!(dup.len(), 2);
        assert!(!dup[1].missing);
    }

    #[test]
    fn test_parse_allpages_batch_with_continuation() {
        let body = serde_json::json!({
            "batchcomplete": "",
            "continue": {"apcontinue": "模块:下一段", "continue": "-||"},
            "query": {
                "allpages": [
                    {"pageid": 1, "ns": 828, "title": "模块:A"},
                    {"pageid": 2, "ns": 828, "title": "模块:B"}
                ]
            }
        });
        let (titles, cont) = parse_allpages_batch(&body).unwrap();
        assert_eq!(titles, vec!["模块:A", "模块:B"]);
        assert_eq!(cont.as_deref(), Some("模块:下一段"));

        let done = serde_json::json!({
            "batchcomplete": "",
            "query": {"allpages": [{"pageid": 3, "ns": 828, "title": "模块:C"}]}
        });
        let (titles, cont) = parse_allpages_batch(&done).unwrap();
        assert_eq!(titles.len(), 1);
        assert!(cont.is_none());
    }

    #[test]
    fn test_rate_limit_cfg_from_env() {
        env::set_var("WIKI__QPS", "4");
        env::set_var("WIKI__MAX_RETRIES", "0");
        let cfg = RateLimitCfg::from_env();
        assert_eq!(cfg.min_interval, Duration::from_millis(250));
        assert_eq!(cfg.max_retries, 0);

        env::set_var("WIKI__QPS", "not-a-number");
        env::set_var("WIKI__MAX_RETRIES", "99");
        let cfg = RateLimitCfg::from_env();
        // Invalid values fall back to defaults.
        assert_eq!(cfg.min_interval, Duration::from_millis(1000));
        assert_eq!(cfg.max_retries, 3);

        env::remove_var("WIKI__QPS");
        env::remove_var("WIKI__MAX_RETRIES");
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

    #[test]
    fn test_parse_allpages_info_entries() {
        // formatversion=1 shape: BC booleans as empty strings; page map keyed
        // by pageid (unordered by design); continue carries gapcontinue;
        // prop=info names the byte length field `length`.
        let body: Value = serde_json::json!({
            "continue": {"gapcontinue": "下一页|2"},
            "query": {
                "pages": {
                    "99": {
                        "pageid": 99, "ns": 0, "title": "猎犬",
                        "touched": "2026-08-01T00:00:00Z", "length": 5650,
                        "new": ""
                    },
                    "7": {
                        "pageid": 7, "ns": 0, "title": "野狗",
                        "touched": "2026-07-01T00:00:00Z", "length": 30,
                        "redirect": ""
                    }
                }
            }
        });
        let (entries, next) = parse_allpages_info_entries(&body).unwrap();
        assert_eq!(next.as_deref(), Some("下一页|2"));
        // Sorted by pageid for reproducible rounds.
        assert_eq!(entries[0].pageid, 7);
        assert!(entries[0].redirect, "BC empty string means true");
        assert_eq!(entries[1].pageid, 99);
        assert!(!entries[1].redirect);
        assert_eq!(entries[1].len, Some(5650));
        assert!(entries[1].is_new);
    }

    #[test]
    fn test_parse_allpages_info_entries_no_continue() {
        let body: Value = serde_json::json!({
            "query": {"pages": {}}
        });
        let (entries, next) = parse_allpages_info_entries(&body).unwrap();
        assert!(entries.is_empty());
        assert!(next.is_none());
    }

    #[test]
    fn test_parse_pages_content_response() {
        let body: Value = serde_json::json!({
            "query": {
                "pages": {
                    "13857": {
                        "pageid": 13857, "ns": 0, "title": "猎犬",
                        "revisions": [{
                            "slots": {"main": {"contentmodel": "wikitext", "*": "{{实体信息框/自动|dst|hound}}"}},
                            "sha1": "deadbeef"
                        }],
                        "categories": [{"ns": 14, "title": "分类:联机版"}]
                    },
                    "-1": {"ns": 0, "title": "不存在页", "missing": ""}
                }
            }
        });
        let mut pages = parse_pages_content_response(&body).unwrap();
        pages.sort_by(|a, b| a.title.cmp(&b.title));
        assert_eq!(pages.len(), 2);

        let hound = pages.iter().find(|p| p.title == "猎犬").unwrap();
        assert_eq!(
            hound.wikitext.as_deref(),
            Some("{{实体信息框/自动|dst|hound}}")
        );
        assert_eq!(hound.sha1.as_deref(), Some("deadbeef"));
        assert_eq!(hound.categories, vec!["分类:联机版".to_string()]);
        assert!(!hound.missing);

        let missing = pages.iter().find(|p| p.title == "不存在页").unwrap();
        assert!(missing.missing);
        assert!(missing.wikitext.is_none());
    }

    #[tokio::test]
    async fn test_get_page() {
        dotenvy::dotenv().ok();

        let client = match WikiClient::from_env() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Skipping test: environment variables not set");
                return;
            }
        };

        let result = client.get_page(TEST_PAGE).await;
        match result {
            Ok(page) => {
                assert_eq!(page.title, TEST_PAGE);
                println!(
                    "Page content length: {:?}",
                    page.content.as_ref().map(|c| c.len())
                );
            }
            Err(e) => {
                eprintln!("Error getting page: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_login_and_get_page() {
        dotenvy::dotenv().ok();

        let mut client = match WikiClient::from_env() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Skipping test: environment variables not set");
                return;
            }
        };

        match client.login().await {
            Ok(_) => {
                println!("Login successful");
                assert!(client.is_logged_in());

                let page = client.get_page(TEST_PAGE).await.unwrap();
                println!("Page title: {}", page.title);
            }
            Err(e) => {
                eprintln!("Login failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_files_info_live() {
        dotenvy::dotenv().ok();

        let client = match WikiClient::from_env() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Skipping test: environment variables not set");
                return;
            }
        };

        // `Frame.png` 长期存在于站内；下划线/大小写归一化应能命中它。
        match client
            .get_files_info(&["File:frame.png", "File:肯定不存在的图片XYZ.png"])
            .await
        {
            Ok(infos) => {
                assert_eq!(infos.len(), 2);
                assert!(!infos[0].missing);
                assert!(infos[0].url.as_deref().unwrap_or("").starts_with("http"));
                assert!(infos[0].size.is_some(), "iiprop=size should be parsed");
                assert!(infos[1].missing);
                assert!(infos[1].url.is_none());
                assert!(!client.file_exists("肯定不存在的图片XYZ.png").await.unwrap());
                assert!(client
                    .get_file_url("Frame.png")
                    .await
                    .unwrap()
                    .is_some_and(|u| u.starts_with("http")));
            }
            Err(e) => eprintln!("Error querying file info: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_get_pages_meta_and_exists() {
        dotenvy::dotenv().ok();

        let client = match WikiClient::from_env() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Skipping test: environment variables not set");
                return;
            }
        };

        let briefs = client
            .get_pages_meta(&[TEST_PAGE, "Data:肯定不存在的页面XYZ.tabx"])
            .await
            .expect("query should succeed");
        assert_eq!(briefs.len(), 2);
        let existing = briefs.iter().find(|b| b.title == TEST_PAGE).unwrap();
        assert!(!existing.missing);
        let missing = briefs.iter().find(|b| b.missing).unwrap();
        assert!(missing.pageid.is_none());

        assert!(client.page_exists(TEST_PAGE).await.unwrap_or(false));
    }
}
