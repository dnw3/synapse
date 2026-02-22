pub mod reranker;
pub use reranker::{BgeRerankerModel, HuggingFaceReranker};

use async_trait::async_trait;
use synaptic_core::{Embeddings, SynapticError};

#[derive(Debug, Clone)]
pub struct HuggingFaceEmbeddingsConfig {
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub wait_for_model: bool,
}

impl HuggingFaceEmbeddingsConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            api_key: None,
            base_url: "https://api-inference.huggingface.co/models".to_string(),
            wait_for_model: true,
        }
    }
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
    pub fn with_wait_for_model(mut self, wait: bool) -> Self {
        self.wait_for_model = wait;
        self
    }
}

pub struct HuggingFaceEmbeddings {
    config: HuggingFaceEmbeddingsConfig,
    client: reqwest::Client,
}

impl HuggingFaceEmbeddings {
    pub fn new(config: HuggingFaceEmbeddingsConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
    pub fn with_client(config: HuggingFaceEmbeddingsConfig, client: reqwest::Client) -> Self {
        Self { config, client }
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/{}", self.config.base_url, self.config.model);
        let body = serde_json::json!({ "inputs": texts });
        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");
        if let Some(ref key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {key}"));
        }
        if self.config.wait_for_model {
            request = request.header("x-wait-for-model", "true");
        }
        let response = request
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Embedding(format!("HuggingFace request: {e}")))?;
        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            let code = status.as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(SynapticError::Embedding(format!(
                "HuggingFace API error ({code}): {text}"
            )));
        }
        let resp: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SynapticError::Embedding(format!("HuggingFace parse: {e}")))?;
        parse_hf_response(&resp)
    }
}

fn parse_hf_response(resp: &serde_json::Value) -> Result<Vec<Vec<f32>>, SynapticError> {
    let array = if let Some(arr) = resp.as_array() {
        arr
    } else if let Some(arr) = resp.get("embeddings").and_then(|e| e.as_array()) {
        arr
    } else {
        return Err(SynapticError::Embedding(
            "unexpected HuggingFace response format".to_string(),
        ));
    };
    let mut result = Vec::with_capacity(array.len());
    for item in array {
        let embedding: Vec<f32> = item
            .as_array()
            .ok_or_else(|| SynapticError::Embedding("embedding item is not array".to_string()))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        result.push(embedding);
    }
    Ok(result)
}

#[async_trait]
impl Embeddings for HuggingFaceEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        self.embed_batch(texts).await
    }
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let mut results = self.embed_batch(&[text]).await?;
        results
            .pop()
            .ok_or_else(|| SynapticError::Embedding("empty HuggingFace response".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let c = HuggingFaceEmbeddingsConfig::new("BAAI/bge-small-en-v1.5");
        assert_eq!(c.model, "BAAI/bge-small-en-v1.5");
    }

    #[test]
    fn config_builder() {
        let c = HuggingFaceEmbeddingsConfig::new("model")
            .with_api_key("hf_test")
            .with_wait_for_model(false);
        assert_eq!(c.api_key, Some("hf_test".to_string()));
    }

    #[test]
    fn parse_direct_array() {
        let resp = serde_json::json!([[0.1_f32, 0.2_f32]]);
        let result = parse_hf_response(&resp).unwrap();
        assert_eq!(result.len(), 1);
    }
}
