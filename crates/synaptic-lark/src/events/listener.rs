use crate::LarkConfig;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use synaptic_core::SynapticError;

type EventHandler = Arc<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<(), SynapticError>> + Send>> + Send + Sync,
>;

/// Webhook listener for Lark push events.
///
/// Handles Lark's URL verification challenge automatically.
/// Verifies HMAC signatures when a `verification_token` is configured.
pub struct LarkEventListener {
    #[allow(dead_code)]
    config: LarkConfig,
    /// Optional: verification token for signature checking
    verification_token: Option<String>,
    /// event_type → async handler
    handlers: HashMap<String, EventHandler>,
}

impl LarkEventListener {
    pub fn new(config: LarkConfig) -> Self {
        Self {
            config,
            verification_token: None,
            handlers: HashMap::new(),
        }
    }

    /// Set the verification token for HMAC signature checking.
    pub fn with_verification_token(mut self, token: impl Into<String>) -> Self {
        self.verification_token = Some(token.into());
        self
    }

    /// Register a handler for a specific event type.
    pub fn on<F, Fut>(mut self, event_type: &str, handler: F) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), SynapticError>> + Send + 'static,
    {
        self.handlers.insert(
            event_type.to_string(),
            Arc::new(move |v| Box::pin(handler(v))),
        );
        self
    }

    /// Dispatch a raw event payload (JSON bytes).
    ///
    /// Returns the HTTP response body to send back to Lark.
    pub async fn dispatch(
        &self,
        body_bytes: &[u8],
        timestamp: Option<&str>,
        nonce: Option<&str>,
        signature: Option<&str>,
    ) -> Result<Value, SynapticError> {
        // Signature verification
        if let (Some(ts), Some(n), Some(sig), Some(vt)) = (
            timestamp,
            nonce,
            signature,
            self.verification_token.as_deref(),
        ) {
            crate::events::verify::verify_lark_signature(ts, n, vt, body_bytes, sig)?;
        }

        let payload: Value = serde_json::from_slice(body_bytes)
            .map_err(|e| SynapticError::Config(format!("event parse: {e}")))?;

        // Handle URL verification challenge
        if payload["type"].as_str() == Some("url_verification") {
            let challenge = payload["challenge"].as_str().unwrap_or("");
            return Ok(serde_json::json!({ "challenge": challenge }));
        }

        // Extract event_type from v2.0 schema
        let event_type = payload["header"]["event_type"]
            .as_str()
            .or_else(|| payload["event"]["type"].as_str())
            .unwrap_or("");

        if let Some(handler) = self.handlers.get(event_type) {
            handler(payload).await?;
        } else {
            tracing::debug!("LarkEventListener: no handler for event_type='{event_type}'");
        }

        Ok(serde_json::json!({ "code": 0 }))
    }

    /// Registered event types.
    pub fn event_types(&self) -> Vec<&str> {
        self.handlers.keys().map(String::as_str).collect()
    }
}
