use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::LarkConfig;
use synaptic_core::SynapticError;

/// Cached tenant access token with automatic expiry tracking.
#[derive(Default)]
struct CachedToken {
    token: Option<String>,
    expires_at: Option<Instant>,
}

impl CachedToken {
    /// Returns `true` if the token is still valid (>300s remaining).
    fn is_valid(&self) -> bool {
        match (&self.token, &self.expires_at) {
            (Some(_), Some(exp)) => exp
                .checked_duration_since(Instant::now())
                .map(|remaining| remaining > Duration::from_secs(300))
                .unwrap_or(false),
            _ => false,
        }
    }
}

/// Thread-safe cache for a Lark tenant access token.
///
/// Automatically refreshes when fewer than 300 seconds remain before expiry.
pub struct TokenCache {
    config: Arc<LarkConfig>,
    inner: Arc<Mutex<CachedToken>>,
    client: reqwest::Client,
}

impl TokenCache {
    pub fn new(config: Arc<LarkConfig>) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(CachedToken::default())),
            client: reqwest::Client::new(),
        }
    }

    /// Return a valid access token, refreshing if necessary.
    pub async fn get_token(&self) -> Result<String, SynapticError> {
        let mut guard = self.inner.lock().await;
        if guard.is_valid() {
            return Ok(guard.token.clone().unwrap());
        }
        // Refresh
        let url = format!(
            "{}/auth/v3/tenant_access_token/internal",
            self.config.base_url
        );
        let body = serde_json::json!({
            "app_id": self.config.app_id,
            "app_secret": self.config.app_secret,
        });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Config(format!("Lark token request failed: {e}")))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Config(format!("Lark token parse failed: {e}")))?;

        if json["code"].as_i64().unwrap_or(-1) != 0 {
            return Err(SynapticError::Config(format!(
                "Lark auth error: {}",
                json["msg"].as_str().unwrap_or("unknown")
            )));
        }

        let token = json["tenant_access_token"]
            .as_str()
            .ok_or_else(|| SynapticError::Config("Lark: missing tenant_access_token".to_string()))?
            .to_string();
        let expire_secs = json["expire"].as_u64().unwrap_or(7200);
        guard.token = Some(token.clone());
        guard.expires_at = Some(Instant::now() + Duration::from_secs(expire_secs));

        tracing::debug!("Lark token refreshed, expires in {}s", expire_secs);
        Ok(token)
    }
}
