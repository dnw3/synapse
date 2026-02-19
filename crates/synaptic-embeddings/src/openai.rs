use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synaptic_core::SynapticError;
use synaptic_models::backend::{ProviderBackend, ProviderRequest};

use crate::Embeddings;

pub struct OpenAiEmbeddingsConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl OpenAiEmbeddingsConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "text-embedding-3-small".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

pub struct OpenAiEmbeddings {
    config: OpenAiEmbeddingsConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl OpenAiEmbeddings {
    pub fn new(config: OpenAiEmbeddingsConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }

    fn build_request(&self, input: Vec<String>) -> ProviderRequest {
        ProviderRequest {
            url: format!("{}/embeddings", self.config.base_url),
            headers: vec![
                (
                    "Authorization".to_string(),
                    format!("Bearer {}", self.config.api_key),
                ),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: json!({
                "model": self.config.model,
                "input": input,
            }),
        }
    }

    fn parse_response(&self, body: &serde_json::Value) -> Result<Vec<Vec<f32>>, SynapticError> {
        let data = body.get("data").and_then(|d| d.as_array()).ok_or_else(|| {
            SynapticError::Embedding("missing 'data' field in response".to_string())
        })?;

        let mut embeddings = Vec::with_capacity(data.len());
        for item in data {
            let embedding = item
                .get("embedding")
                .and_then(|e| e.as_array())
                .ok_or_else(|| SynapticError::Embedding("missing 'embedding' field".to_string()))?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            embeddings.push(embedding);
        }

        Ok(embeddings)
    }
}

#[async_trait]
impl Embeddings for OpenAiEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        let input: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let request = self.build_request(input);
        let response = self.backend.send(request).await?;

        if response.status != 200 {
            return Err(SynapticError::Embedding(format!(
                "OpenAI API error ({}): {}",
                response.status, response.body
            )));
        }

        self.parse_response(&response.body)
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let mut results = self.embed_documents(&[text]).await?;
        results
            .pop()
            .ok_or_else(|| SynapticError::Embedding("empty response".to_string()))
    }
}
