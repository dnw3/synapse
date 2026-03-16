use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Base URLs for the GitHub Copilot API.
const GITHUB_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const COPILOT_CHAT_URL: &str = "https://api.githubcopilot.com/chat/completions";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Provider configuration for routing LLM requests through GitHub Copilot.
///
/// Obtain a `github_token` via OAuth or a personal access token with the
/// `copilot` scope.  The first call to [`CopilotChatModel::chat`] will
/// automatically exchange this token for a short-lived Copilot session token.
#[allow(dead_code)]
pub struct CopilotProxyConfig {
    pub github_token: String,
    /// Model identifier forwarded to the Copilot API (e.g. `"gpt-4o"`).
    pub model: String,
}

#[allow(dead_code)]
impl CopilotProxyConfig {
    pub fn new(github_token: String) -> Self {
        Self {
            github_token,
            model: "gpt-4o".into(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Exchange a GitHub personal-access token for a Copilot session token.
    ///
    /// The returned token is short-lived (~30 min).  Callers should cache it
    /// and re-exchange once expired (see [`CopilotChatModel`]).
    pub async fn get_copilot_token(
        &self,
    ) -> Result<CopilotToken, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let resp = client
            .get(GITHUB_TOKEN_URL)
            .header("Authorization", format!("token {}", self.github_token))
            .header("User-Agent", "Synapse/1.0")
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GitHub token exchange failed ({status}): {body}").into());
        }

        let raw: Value = resp.json().await?;
        let token = raw
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or("missing 'token' field in GitHub API response")?
            .to_string();

        let expires_at = raw.get("expires_at").and_then(|v| v.as_u64()).unwrap_or(0);

        Ok(CopilotToken { token, expires_at })
    }
}

// ---------------------------------------------------------------------------
// Token type
// ---------------------------------------------------------------------------

/// A short-lived Copilot session token with optional expiry (Unix timestamp).
#[derive(Debug, Clone)]
pub struct CopilotToken {
    pub token: String,
    /// Unix timestamp (seconds) after which the token should be refreshed.
    /// Zero means unknown / not provided.
    pub expires_at: u64,
}

impl CopilotToken {
    /// Returns `true` when the token has expired or will expire within 60 s.
    pub fn is_expired(&self) -> bool {
        if self.expires_at == 0 {
            return false; // Unknown expiry — assume still valid.
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now + 60 >= self.expires_at
    }
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// A single message in a Copilot chat request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

/// Parameters for a Copilot chat request.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    /// Override the model from [`CopilotProxyConfig`].
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

impl ChatRequest {
    pub fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            messages,
            model: None,
            temperature: None,
            max_tokens: None,
            stream: false,
        }
    }
}

/// Parsed response from the Copilot chat API (non-streaming).
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub finish_reason: String,
    /// Raw JSON response body for callers that need additional fields.
    pub raw: Value,
}

// ---------------------------------------------------------------------------
// CopilotChatModel
// ---------------------------------------------------------------------------

/// A chat model client that proxies requests through GitHub Copilot's API.
///
/// Handles token caching and automatic re-exchange on expiry.
///
/// # Example
/// ```no_run
/// # use synapse::agent::copilot_proxy::{CopilotChatModel, ChatMessage, ChatRequest};
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let model = CopilotChatModel::new("ghp_your_token".into(), "gpt-4o".into());
/// let req = ChatRequest::new(vec![ChatMessage::user("Hello!")]);
/// let resp = model.chat(req).await?;
/// println!("{}", resp.content);
/// # Ok(())
/// # }
/// ```
pub struct CopilotChatModel {
    api_key: String,
    model: String,
    cached_token: Arc<Mutex<Option<CopilotToken>>>,
    http: reqwest::Client,
}

#[allow(dead_code)]
impl CopilotChatModel {
    /// Create a new model proxy.
    ///
    /// `api_key` is the GitHub personal-access token (PAT) with `copilot` scope.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            cached_token: Arc::new(Mutex::new(None)),
            http: reqwest::Client::builder()
                .user_agent("Synapse/1.0")
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn with_default_model(api_key: String) -> Self {
        Self::new(api_key, "gpt-4o".into())
    }

    // ------------------------------------------------------------------
    // Token management
    // ------------------------------------------------------------------

    /// Return a valid Copilot session token, refreshing if necessary.
    async fn get_or_refresh_token(
        &self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.cached_token.lock().await;

        if let Some(ref tok) = *guard {
            if !tok.is_expired() {
                return Ok(tok.token.clone());
            }
            tracing::debug!("mDNS: Copilot token expired, refreshing");
        }

        let config = CopilotProxyConfig {
            github_token: self.api_key.clone(),
            model: self.model.clone(),
        };

        let new_token = config.get_copilot_token().await?;
        tracing::info!(
            expires_at = new_token.expires_at,
            "Copilot: obtained new session token"
        );

        let token_str = new_token.token.clone();
        *guard = Some(new_token);
        Ok(token_str)
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Send a chat request and return the complete response.
    pub async fn chat(
        &self,
        req: ChatRequest,
    ) -> Result<ChatResponse, Box<dyn std::error::Error + Send + Sync>> {
        let copilot_token = self.get_or_refresh_token().await?;
        let model = req.model.as_deref().unwrap_or(&self.model);

        let mut body = json!({
            "model": model,
            "messages": req.messages,
            "stream": false,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tok) = req.max_tokens {
            body["max_tokens"] = json!(max_tok);
        }

        tracing::debug!(
            model,
            messages = req.messages.len(),
            "Copilot: sending chat request"
        );

        let resp = self
            .http
            .post(COPILOT_CHAT_URL)
            .header("Authorization", format!("Bearer {copilot_token}"))
            .header("Content-Type", "application/json")
            .header("Copilot-Integration-Id", "synapse-agent")
            .header("Editor-Version", "Synapse/1.0")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Copilot API error ({status}): {err_body}").into());
        }

        let raw: Value = resp.json().await?;
        let content = raw
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let finish_reason = raw
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let response_model = raw
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(model)
            .to_string();

        tracing::debug!(
            model = %response_model,
            finish_reason = %finish_reason,
            content_len = content.len(),
            "Copilot: received chat response"
        );

        Ok(ChatResponse {
            content,
            model: response_model,
            finish_reason,
            raw,
        })
    }

    /// Send a streaming chat request.
    ///
    /// Returns the raw SSE byte stream from the Copilot API.  Callers are
    /// responsible for parsing the `data: {...}` events.
    ///
    /// # Note
    /// A fully typed streaming wrapper is a planned enhancement.  This method
    /// provides the raw stream so callers can integrate with their own SSE
    /// parsers or forward the bytes directly to a WebSocket client.
    pub async fn stream_chat(
        &self,
        req: ChatRequest,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error + Send + Sync>> {
        let copilot_token = self.get_or_refresh_token().await?;
        let model = req.model.as_deref().unwrap_or(&self.model);

        let mut body = json!({
            "model": model,
            "messages": req.messages,
            "stream": true,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tok) = req.max_tokens {
            body["max_tokens"] = json!(max_tok);
        }

        tracing::debug!(
            model,
            messages = req.messages.len(),
            "Copilot: sending streaming chat request"
        );

        let resp = self
            .http
            .post(COPILOT_CHAT_URL)
            .header("Authorization", format!("Bearer {copilot_token}"))
            .header("Content-Type", "application/json")
            .header("Copilot-Integration-Id", "synapse-agent")
            .header("Editor-Version", "Synapse/1.0")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Copilot API streaming error ({status}): {err_body}").into());
        }

        Ok(resp)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_creates_with_defaults() {
        let config = CopilotProxyConfig::new("ghp_test123".into());
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.github_token, "ghp_test123");
    }

    #[test]
    fn config_with_model_override() {
        let config = CopilotProxyConfig::new("ghp_test".into()).with_model("gpt-4-turbo");
        assert_eq!(config.model, "gpt-4-turbo");
    }

    #[test]
    fn token_exchange_url_is_correct() {
        assert_eq!(
            GITHUB_TOKEN_URL,
            "https://api.github.com/copilot_internal/v2/token"
        );
    }

    #[test]
    fn copilot_chat_url_is_correct() {
        assert_eq!(
            COPILOT_CHAT_URL,
            "https://api.githubcopilot.com/chat/completions"
        );
    }

    #[test]
    fn token_expiry_check_unknown() {
        let tok = CopilotToken {
            token: "abc".into(),
            expires_at: 0, // unknown
        };
        assert!(
            !tok.is_expired(),
            "unknown expiry should not be treated as expired"
        );
    }

    #[test]
    fn token_expiry_check_expired() {
        let tok = CopilotToken {
            token: "abc".into(),
            expires_at: 1000, // far in the past
        };
        assert!(tok.is_expired());
    }

    #[test]
    fn token_expiry_check_future() {
        let future = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
            + 3600;
        let tok = CopilotToken {
            token: "abc".into(),
            expires_at: future,
        };
        assert!(
            !tok.is_expired(),
            "token valid for 1 hour should not be expired"
        );
    }

    #[test]
    fn chat_model_creates() {
        let model = CopilotChatModel::new("ghp_test".into(), "gpt-4o".into());
        assert_eq!(model.model, "gpt-4o");
        assert_eq!(model.api_key, "ghp_test");
    }

    #[test]
    fn chat_message_builders() {
        let sys = ChatMessage::system("You are helpful");
        assert_eq!(sys.role, "system");
        let usr = ChatMessage::user("Hi");
        assert_eq!(usr.role, "user");
        let ast = ChatMessage::assistant("Hello");
        assert_eq!(ast.role, "assistant");
    }

    #[test]
    fn chat_request_defaults_to_non_streaming() {
        let req = ChatRequest::new(vec![ChatMessage::user("test")]);
        assert!(!req.stream);
        assert!(req.model.is_none());
    }
}
