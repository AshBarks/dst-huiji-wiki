//! Minimal LLM API configuration and OpenAI-compatible chat completion client.
//!
//! Reads configuration from the environment (`.env` is loaded by the binary):
//!
//! - `LLM__API_KEY` — required to enable LLM calls
//! - `LLM__BASE_URL` — default `https://api.openai.com/v1`
//! - `LLM__MODEL` — default `gpt-4o-mini`
//!
//! If `LLM__API_KEY` is absent the caller should skip LLM-dependent work
//! rather than fail; if a configured request fails at call time the error is
//! returned to the caller.

use crate::error::{Error, Result};
use serde::Deserialize;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
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

    fn endpoint(&self) -> String {
        if self.base_url.ends_with("/chat/completions") {
            self.base_url.clone()
        } else {
            format!("{}/chat/completions", self.base_url)
        }
    }

    /// Sends a chat completion request and returns the assistant message text.
    ///
    /// Any transport/API/parse failure is returned as an error.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "temperature": 0.2,
        });
        let resp = client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(Error::Llm(format!(
                "HTTP {} from {}: {}",
                status.as_u16(),
                self.endpoint(),
                text.chars().take(500).collect::<String>()
            )));
        }
        let value: ChatResponse = serde_json::from_str(&text)?;
        value
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| Error::Llm("LLM 响应缺少 choices[0].message.content".to_string()))
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
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
}
