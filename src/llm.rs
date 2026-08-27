//! Minimal LLM API configuration and OpenAI-compatible chat-completion client.
//!
//! Built on [`async_openai`] so requests speak standard OpenAI chat protocol
//! and responses arrive as **SSE streams** — progress is observable while a
//! long completion is in flight, instead of blocking silently until the whole
//! payload returns. The endpoint is any OpenAI-compatible base URL:
//!
//! - `LLM__API_KEY` — required to enable LLM calls
//! - `LLM__BASE_URL` — default `https://api.openai.com/v1`
//! - `LLM__MODEL` — default `gpt-4o-mini`
//! - `LLM__TIMEOUT_SECS` — overall request budget including the full stream
//!   (large annotation prompts + slow reasoning models legitimately need more;
//!   default `120`)
//!
//! If `LLM__API_KEY` is absent the caller should skip LLM-dependent work
//! rather than fail; if a configured request fails at call time the error is
//! returned to the caller.

use crate::error::{Error, Result};
use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
};
use futures_util::StreamExt;
use std::time::{Duration, Instant};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Cadence of [`LlmStreamEvent::Tick`] heartbeats.
const TICK_EVERY: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

/// Observability hooks emitted while [`LlmConfig::complete_streaming`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmStreamEvent {
    /// The first content chunk arrived after `elapsed_secs`.
    FirstToken { elapsed_secs: u64 },
    /// Periodic heartbeat with the accumulated character count (`chars == 0`
    /// means still waiting for the first token).
    Tick { chars: usize },
    /// Stream finished; `chars`/`elapsed_secs` describe the full response.
    Done { chars: usize, elapsed_secs: u64 },
}

impl LlmConfig {
    /// Reads LLM configuration from the environment.
    ///
    /// Returns `None` when no API key is configured, so callers can skip
    /// LLM-dependent steps gracefully.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("LLM__API_KEY").ok()?;
        if api_key.trim().is_empty() {
            return None;
        }
        let base_url = std::env::var("LLM__BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let model = std::env::var("LLM__MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Some(Self {
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        })
    }

    fn timeout() -> Duration {
        let secs = std::env::var("LLM__TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_TIMEOUT_SECS);
        Duration::from_secs(secs)
    }

    fn client(&self) -> async_openai::Client<OpenAIConfig> {
        // Overall budget covers connect + headers + the entire SSE stream;
        // per-attempt TCP connect stays cheap so a blackholed resolved IP does
        // not eat the budget before hyper tries the next address.
        let http = reqwest::Client::builder()
            .timeout(Self::timeout())
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_default();
        let config = OpenAIConfig::new()
            .with_api_key(&self.api_key)
            .with_api_base(&self.base_url);
        async_openai::Client::<OpenAIConfig>::with_config(config).with_http_client(http)
    }

    /// Sends a chat completion request (streaming under the hood) and returns
    /// the assembled assistant message text without emitting events.
    ///
    /// Any transport/API failure is returned as an error.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String> {
        self.complete_streaming(system, user, |_| {}).await
    }

    /// Same as [`complete`](Self::complete), but surfaces progress events as
    /// SSE chunks arrive so long-running calls stay observable/debuggable.
    pub async fn complete_streaming<F>(
        &self,
        system: &str,
        user: &str,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(LlmStreamEvent),
    {
        let mut request = CreateChatCompletionRequestArgs::default();
        let system_msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(system)
            .build()
            .map_err(|e| Error::Llm(format!("system 消息构建失败：{e}")))?
            .into();
        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(user)
            .build()
            .map_err(|e| Error::Llm(format!("user 消息构建失败：{e}")))?
            .into();
        request
            .model(&self.model)
            .temperature(0.2_f32)
            .messages(vec![system_msg, user_msg]);
        let request = request
            .build()
            .map_err(|e| Error::Llm(format!("构建请求失败：{e}")))?;

        let started = Instant::now();
        let mut stream = self
            .client()
            .chat()
            .create_stream(request)
            .await
            .map_err(|e| Error::Llm(e.to_string()))?;

        let mut acc = String::new();
        let mut first_token_seen = false;
        let mut ticker = tokio::time::interval(TICK_EVERY);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await; // consume the immediate first tick

        loop {
            tokio::select! {
                biased;
                _ = ticker.tick() => {
                    on_event(LlmStreamEvent::Tick { chars: acc.len() });
                }
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { break };
                    let resp =
                        chunk.map_err(|e| Error::Llm(format!("流式响应中断：{e}")))?;
                    if let Some(delta) =
                        resp.choices.iter().find_map(|c| c.delta.content.clone())
                    {
                        if !delta.is_empty() {
                            if !first_token_seen {
                                first_token_seen = true;
                                on_event(LlmStreamEvent::FirstToken {
                                    elapsed_secs: started.elapsed().as_secs(),
                                });
                            }
                            acc.push_str(&delta);
                        }
                    }
                }
            }
        }

        on_event(LlmStreamEvent::Done {
            chars: acc.len(),
            elapsed_secs: started.elapsed().as_secs(),
        });
        Ok(acc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_absent_returns_none() {
        // Ensure these are unset for the test.
        unsafe {
            std::env::remove_var("LLM__API_KEY");
        }
        assert!(LlmConfig::from_env().is_none());
    }

    #[test]
    fn from_env_normalizes_base_url_and_defaults_model() {
        unsafe {
            std::env::set_var("LLM__API_KEY", "k");
            std::env::set_var("LLM__BASE_URL", "https://example.com/v1/");
            std::env::remove_var("LLM__MODEL");
        }
        let cfg = LlmConfig::from_env().expect("config");
        assert_eq!(cfg.base_url, "https://example.com/v1");
        assert_eq!(cfg.model, DEFAULT_MODEL);
        unsafe {
            std::env::remove_var("LLM__API_KEY");
            std::env::remove_var("LLM__BASE_URL");
        }
    }

    #[tokio::test]
    async fn complete_fails_fast_on_unreachable_endpoint() {
        let cfg = LlmConfig {
            api_key: "k".into(),
            base_url: "http://127.0.0.1:9/v1".into(), // discard port → refused fast
            model: "test-model".into(),
        };
        let err = cfg.complete("sys", "user").await.unwrap_err().to_string();
        assert!(!err.is_empty());
    }
}
