use crate::error::{Error, Result};
use reqwest::{redirect, Client, Response};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;

const DEFAULT_WIKI_HOST: &str = "dontstarve.huijiwiki.com";
const API_PATH: &str = "/api.php";

#[derive(Debug, Clone)]
pub struct WikiConfig {
    pub host: String,
    pub username: String,
    pub password: String,
    pub x_authkey: String,
}

impl WikiConfig {
    pub fn from_env() -> Result<Self> {
        let username = env::var("HUIJI__USERNAME")
            .map_err(|_| Error::EnvVarNotFound("HUIJI__USERNAME".to_string()))?;
        let password = env::var("HUIJI__PASSWORD")
            .map_err(|_| Error::EnvVarNotFound("HUIJI__PASSWORD".to_string()))?;
        let x_authkey = env::var("HUIJI__X_AUTHKEY")
            .map_err(|_| Error::EnvVarNotFound("HUIJI__X_AUTHKEY".to_string()))?;

        Ok(Self {
            host: DEFAULT_WIKI_HOST.to_string(),
            username,
            password,
            x_authkey,
        })
    }

    pub fn api_url(&self) -> String {
        format!("https://{}{}", self.host, API_PATH)
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

#[derive(Debug, Clone)]
pub struct EditResult {
    pub result: String,
    pub pageid: Option<i64>,
    pub title: Option<String>,
    pub newrevid: Option<i64>,
    pub oldrevid: Option<i64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WikiClient {
    client: Client,
    config: WikiConfig,
    logged_in: bool,
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

impl WikiClient {
    pub fn new(config: WikiConfig) -> Result<Self> {
        let client = Client::builder()
            .redirect(redirect::Policy::limited(10))
            .cookie_store(true)
            .build()?;

        Ok(Self {
            client,
            config,
            logged_in: false,
        })
    }

    pub fn from_env() -> Result<Self> {
        let config = WikiConfig::from_env()?;
        Self::new(config)
    }

    pub fn is_logged_in(&self) -> bool {
        self.logged_in
    }

    pub fn config(&self) -> &WikiConfig {
        &self.config
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
            .client
            .get(&url)
            .header("X-authkey", &self.config.x_authkey)
            .query(&params)
            .send()
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
            ("lgname", &self.config.username),
            ("lgpassword", &self.config.password),
            ("lgtoken", &token),
            ("format", "json"),
        ];

        let response = self
            .client
            .post(&url)
            .header("X-authkey", &self.config.x_authkey)
            .form(&params)
            .send()
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

    pub async fn get(&self, params: &[(&str, &str)]) -> Result<Response> {
        let url = self.config.api_url();

        let response = self
            .client
            .get(&url)
            .header("X-authkey", &self.config.x_authkey)
            .query(params)
            .send()
            .await?;

        Ok(response)
    }

    pub async fn post(&self, params: &[(&str, &str)]) -> Result<Response> {
        let url = self.config.api_url();

        let response = self
            .client
            .post(&url)
            .header("X-authkey", &self.config.x_authkey)
            .form(params)
            .send()
            .await?;

        Ok(response)
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
        let params = [
            ("action", "query"),
            ("prop", "revisions"),
            ("rvprop", "content|user|timestamp|ids"),
            ("rvlimit", "1"),
            ("titles", title),
            ("format", "json"),
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
            return Err(Error::WikiApi(format!("Page '{}' does not exist", title)));
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

    pub async fn edit_page(
        &self,
        title: &str,
        text: &str,
        summary: Option<&str>,
        minor: bool,
    ) -> Result<EditResult> {
        if !self.logged_in {
            return Err(Error::EditFailed("Not logged in".to_string()));
        }

        let csrf_token = self.get_csrf_token().await?;

        let mut params: Vec<(&str, String)> = vec![
            ("action", "edit".to_string()),
            ("title", title.to_string()),
            ("text", text.to_string()),
            ("token", csrf_token),
            ("format", "json".to_string()),
        ];

        if let Some(s) = summary {
            params.push(("summary", s.to_string()));
        }

        if minor {
            params.push(("minor", "true".to_string()));
        }

        let params_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self.post(&params_refs).await?;
        let edit_resp: EditResponse = response.json().await?;

        let edit_info = edit_resp
            .edit
            .ok_or_else(|| Error::WikiApi("No edit response received".to_string()))?;

        match edit_info.result.as_str() {
            "Success" | "success" => {
                tracing::info!(
                    "Successfully edited page '{}' (pageid: {:?}, newrevid: {:?})",
                    edit_info.title.as_deref().unwrap_or(title),
                    edit_info.pageid,
                    edit_info.newrevid
                );
                Ok(EditResult {
                    result: edit_info.result,
                    pageid: edit_info.pageid,
                    title: edit_info.title,
                    newrevid: edit_info.newrevid,
                    oldrevid: edit_info.oldrevid,
                    reason: edit_info.reason,
                })
            }
            _ => Err(Error::EditFailed(format!(
                "Edit failed: {}",
                edit_info.reason.unwrap_or_else(|| edit_info.result.clone())
            ))),
        }
    }

    pub async fn append_to_page(
        &self,
        title: &str,
        text: &str,
        summary: Option<&str>,
    ) -> Result<EditResult> {
        if !self.logged_in {
            return Err(Error::EditFailed("Not logged in".to_string()));
        }

        let csrf_token = self.get_csrf_token().await?;

        let mut params: Vec<(&str, String)> = vec![
            ("action", "edit".to_string()),
            ("title", title.to_string()),
            ("appendtext", text.to_string()),
            ("token", csrf_token),
            ("format", "json".to_string()),
        ];

        if let Some(s) = summary {
            params.push(("summary", s.to_string()));
        }

        let params_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self.post(&params_refs).await?;
        let edit_resp: EditResponse = response.json().await?;

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

    pub async fn prepend_to_page(
        &self,
        title: &str,
        text: &str,
        summary: Option<&str>,
    ) -> Result<EditResult> {
        if !self.logged_in {
            return Err(Error::EditFailed("Not logged in".to_string()));
        }

        let csrf_token = self.get_csrf_token().await?;

        let mut params: Vec<(&str, String)> = vec![
            ("action", "edit".to_string()),
            ("title", title.to_string()),
            ("prependtext", text.to_string()),
            ("token", csrf_token),
            ("format", "json".to_string()),
        ];

        if let Some(s) = summary {
            params.push(("summary", s.to_string()));
        }

        let params_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let response = self.post(&params_refs).await?;
        let edit_resp: EditResponse = response.json().await?;

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
        if result.is_err() {
            assert!(matches!(result.unwrap_err(), Error::EnvVarNotFound(_)));
        }
    }

    #[test]
    fn test_api_url() {
        let config = WikiConfig {
            host: "example.huijiwiki.com".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            x_authkey: "test".to_string(),
        };
        assert_eq!(config.api_url(), "https://example.huijiwiki.com/api.php");
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
}
