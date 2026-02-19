use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;
use synaptic_retrieval::Document;

use crate::Loader;

/// Loads content from a URL via HTTP GET.
///
/// Uses `reqwest` to fetch the URL content and returns a single Document
/// with the URL as id and the response text as content.
/// Metadata includes `source` (the URL) and `content_type` (from the response header).
pub struct WebBaseLoader {
    url: String,
}

impl WebBaseLoader {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait]
impl Loader for WebBaseLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let response = reqwest::get(&self.url).await.map_err(|e| {
            SynapticError::Loader(format!("HTTP request failed for {}: {e}", self.url))
        })?;

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        let text = response
            .text()
            .await
            .map_err(|e| SynapticError::Loader(format!("failed to read response body: {e}")))?;

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), Value::String(self.url.clone()));
        metadata.insert("content_type".to_string(), Value::String(content_type));

        Ok(vec![Document::with_metadata(
            self.url.clone(),
            text,
            metadata,
        )])
    }
}
