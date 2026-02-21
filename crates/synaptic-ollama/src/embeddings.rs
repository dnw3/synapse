use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synaptic_core::SynapticError;
use synaptic_models::{ProviderBackend, ProviderRequest};

use synaptic_core::Embeddings;

pub struct OllamaEmbeddingsConfig {
    pub model: String,
    pub base_url: String,
}

impl OllamaEmbeddingsConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: "http://localhost:11434".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

pub struct OllamaEmbeddings {
    config: OllamaEmbeddingsConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl OllamaEmbeddings {
    pub fn new(config: OllamaEmbeddingsConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }
}

#[async_trait]
impl Embeddings for OllamaEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        let mut all_embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = self.embed_query(text).await?;
            all_embeddings.push(embedding);
        }
        Ok(all_embeddings)
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let request = ProviderRequest {
            url: format!("{}/api/embed", self.config.base_url),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: json!({
                "model": self.config.model,
                "input": text,
            }),
        };

        let response = self.backend.send(request).await?;

        if response.status != 200 {
            return Err(SynapticError::Embedding(format!(
                "Ollama API error ({}): {}",
                response.status, response.body
            )));
        }

        let embeddings = response
            .body
            .get("embeddings")
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .and_then(|e| e.as_array())
            .ok_or_else(|| SynapticError::Embedding("missing 'embeddings' field".to_string()))?;

        Ok(embeddings
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect())
    }
}
