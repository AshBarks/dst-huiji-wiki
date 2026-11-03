//! MediaWiki API 客户端：配置、对外 DTO 与 [`WikiClient`] 构造/会话状态。
//!
//! 子模块：
//! - `transport`：底层 HTTP（登录、GET/POST、CSRF）与节流/重试/退避；
//! - `endpoints`：页面/文件/分类/编辑/上传端点与响应解析。

mod endpoints;
mod transport;

pub use endpoints::*;

use crate::error::{Error, Result};
use reqwest::{redirect, Client};
use std::env;
use std::sync::{Arc, Mutex};
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

    /// 只读配置：不要求 `HUIJI__USERNAME/PASSWORD`；`HUIJI__X_AUTHKEY`
    /// 存在则仍随请求发送。供 corpus-fetch 等明确接受匿名读（可能命中
    /// 站点缓存）的只读作业，避免纯读取被迫持有登录三件套。
    pub fn from_env_readonly() -> Self {
        let username = env::var("HUIJI__USERNAME").unwrap_or_default();
        let password = env::var("HUIJI__PASSWORD").unwrap_or_default();
        let x_authkey = env::var("HUIJI__X_AUTHKEY").unwrap_or_default();
        Self::new(DEFAULT_WIKI_HOST, username, password, x_authkey)
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
    /// 登录态经 Arc 共享：克隆的 client（如 WebUI/服务层先 clone 后登录的
    /// 用法）同步可见，不再依赖“必须在登录后克隆”的人工不变量。
    logged_in: Arc<std::sync::atomic::AtomicBool>,
    /// Timestamp of the most recently reserved request slot. Shared behind an
    /// Arc so clones of the client throttle against the same global budget.
    rate: Arc<Mutex<std::time::Instant>>,
    rate_cfg: RateLimitCfg,
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
            logged_in: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

    /// 匿名只读客户端（见 [`WikiConfig::from_env_readonly`]）。
    pub fn from_env_readonly() -> Result<Self> {
        let config = WikiConfig::from_env_readonly();
        Ok(Self::new(config)?.with_rate_cfg(RateLimitCfg::from_env()))
    }

    /// Overrides the throttling/retry configuration (mainly for tests).
    pub fn with_rate_cfg(mut self, cfg: RateLimitCfg) -> Self {
        self.rate_cfg = cfg;
        self
    }

    pub fn is_logged_in(&self) -> bool {
        self.logged_in.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 写操作（edit/upload）执行前统一调用：未登录则先登录。
    /// 服务端 `assert=user` 失败仍会映射为 `Error::AuthExpired` 兜底。
    pub async fn ensure_login(&self) -> Result<()> {
        if self.is_logged_in() {
            return Ok(());
        }
        self.login().await
    }

    pub fn config(&self) -> &WikiConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clone_shares_login_state() {
        let client = WikiClient::new(WikiConfig::new(
            DEFAULT_WIKI_HOST,
            "user",
            "pass",
            "authkey",
        ))
        .unwrap();
        assert!(!client.is_logged_in());
        let clone = client.clone();
        client
            .logged_in
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(clone.is_logged_in(), "克隆必须共享登录态");
        assert!(client.is_logged_in());
    }

    #[test]
    fn readonly_config_tolerates_missing_credentials() {
        // 只读配置：无凭据也能构造（corpus-fetch 匿名读路径）。
        let config = WikiConfig::from_env_readonly();
        assert!(!config.host.is_empty());
        // 完整配置：缺任何一个凭据都报 EnvVarNotFound（读也要求登录）。
        let had = std::env::var("HUIJI__USERNAME").is_ok();
        if !had {
            std::env::remove_var("HUIJI__USERNAME");
            let result = WikiConfig::from_env();
            std::env::remove_var("HUIJI__USERNAME");
            assert!(matches!(result, Err(Error::EnvVarNotFound(_))));
        }
    }

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
}
