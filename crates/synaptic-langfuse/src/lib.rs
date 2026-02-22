//! Langfuse observability integration for Synaptic.
//!
//! Langfuse is an open-source LLM observability platform. This crate provides a
//! CallbackHandler that records all Synaptic run events to Langfuse traces.

use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use synaptic_core::{CallbackHandler, RunEvent, SynapticError};
use tokio::sync::Mutex;

/// Configuration for the Langfuse callback.
#[derive(Debug, Clone)]
pub struct LangfuseConfig {
    /// Langfuse public key (pk-lf-...)
    pub public_key: String,
    /// Langfuse secret key (sk-lf-...)
    pub secret_key: String,
    /// Langfuse host (default: https://cloud.langfuse.com)
    pub host: String,
    /// How many events to buffer before flushing (default: 20)
    pub flush_batch_size: usize,
}

impl LangfuseConfig {
    /// Create a new Langfuse config for Langfuse Cloud.
    pub fn new(public_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self {
            public_key: public_key.into(),
            secret_key: secret_key.into(),
            host: "https://cloud.langfuse.com".to_string(),
            flush_batch_size: 20,
        }
    }

    /// Use a self-hosted Langfuse instance.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the number of events to buffer before flushing.
    pub fn with_flush_batch_size(mut self, size: usize) -> Self {
        self.flush_batch_size = size;
        self
    }
}

/// A single Langfuse event for the ingestion API.
#[derive(Debug, Serialize, Deserialize)]
pub struct LangfuseEvent {
    pub id: String,
    pub r#type: String,
    pub timestamp: String,
    pub body: Value,
}

/// Callback handler that sends Synaptic run events to Langfuse.
pub struct LangfuseCallback {
    config: LangfuseConfig,
    client: reqwest::Client,
    event_queue: Arc<Mutex<Vec<LangfuseEvent>>>,
}

impl LangfuseCallback {
    /// Create a new Langfuse callback.
    pub async fn new(config: LangfuseConfig) -> Result<Self, SynapticError> {
        let client = reqwest::Client::new();
        Ok(Self {
            config,
            client,
            event_queue: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Flush buffered events to the Langfuse ingestion API.
    pub async fn flush(&self) -> Result<(), SynapticError> {
        let mut queue = self.event_queue.lock().await;
        if queue.is_empty() {
            return Ok(());
        }
        let batch: Vec<LangfuseEvent> = queue.drain(..).collect();
        drop(queue);

        let body = serde_json::json!({ "batch": batch });
        let credentials = base64::engine::general_purpose::STANDARD.encode(format!(
            "{}:{}",
            self.config.public_key, self.config.secret_key
        ));
        let url = format!("{}/api/public/ingestion", self.config.host);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Basic {}", credentials))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Callback(format!("Langfuse flush: {}", e)))?;
        let status = resp.status().as_u16();
        if status >= 400 {
            let text = resp.text().await.unwrap_or_default();
            return Err(SynapticError::Callback(format!(
                "Langfuse API error ({}): {}",
                status, text
            )));
        }
        Ok(())
    }

    /// Queue a Langfuse event. Flushes automatically when batch size is reached.
    async fn queue_event(&self, event: LangfuseEvent) -> Result<(), SynapticError> {
        let mut queue = self.event_queue.lock().await;
        queue.push(event);
        let should_flush = queue.len() >= self.config.flush_batch_size;
        drop(queue);
        if should_flush {
            self.flush().await?;
        }
        Ok(())
    }

    fn now_iso8601() -> String {
        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = dur.as_secs();
        let millis = dur.subsec_millis();
        let year = 1970 + secs / 31_536_000;
        format!("{:04}-01-01T00:00:{:02}.{:03}Z", year, secs % 60, millis)
    }
}

#[async_trait]
impl CallbackHandler for LangfuseCallback {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
        let event_type = format!("{:?}", event);
        let event_name = event_type
            .split_whitespace()
            .next()
            .unwrap_or("Unknown")
            .to_string();
        let ts = Self::now_iso8601();
        let langfuse_event = LangfuseEvent {
            id: uuid::Uuid::new_v4().to_string(),
            r#type: "span-create".to_string(),
            timestamp: ts.clone(),
            body: serde_json::json!({
                "name": event_name,
                "startTime": ts,
            }),
        };
        self.queue_event(langfuse_event).await
    }
}
