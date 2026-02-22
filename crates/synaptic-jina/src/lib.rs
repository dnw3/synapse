use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{Embeddings, SynapticError};

pub mod reranker;
pub use reranker::JinaReranker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JinaEmbeddingModel {
    JinaEmbeddingsV3,
    JinaEmbeddingsV2BaseEn,
    JinaClipV2,
    Custom(String),
}

impl JinaEmbeddingModel {
    pub fn as_str(&self) -> &str {
        match self {
            JinaEmbeddingModel::JinaEmbeddingsV3 => "jina-embeddings-v3",
            JinaEmbeddingModel::JinaEmbeddingsV2BaseEn => "jina-embeddings-v2-base-en",
            JinaEmbeddingModel::JinaClipV2 => "jina-clip-v2",
            JinaEmbeddingModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for JinaEmbeddingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct JinaConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl JinaConfig {
    pub fn new(api_key: impl Into<String>, model: JinaEmbeddingModel) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.to_string(),
            base_url: "https://api.jina.ai/v1".to_string(),
        }
    }
}

pub struct JinaEmbeddings {
    config: JinaConfig,
    client: reqwest::Client,
}

impl JinaEmbeddings {
    pub fn new(config: JinaConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        let input: Vec<serde_json::Value> = texts.iter().map(|t| json!(t)).collect();
        let body = json!({
            "model": self.config.model,
            "input": input,
        });
        let resp = self
            .client
            .post(format!("{}/embeddings", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Embedding(format!("Jina request: {e}")))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Embedding(format!("Jina parse: {e}")))?;
        if status != 200 {
            return Err(SynapticError::Embedding(format!(
                "Jina API error ({}): {}",
                status, json
            )));
        }
        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| SynapticError::Embedding("missing 'data' field".to_string()))?;
        let mut result: Vec<(usize, Vec<f32>)> = data
            .iter()
            .map(|item| {
                let idx = item.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                let emb = item
                    .get("embedding")
                    .and_then(|e| e.as_array())
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                    .collect();
                (idx, emb)
            })
            .collect();
        result.sort_by_key(|(idx, _)| *idx);
        Ok(result.into_iter().map(|(_, emb)| emb).collect())
    }
}

#[async_trait]
impl Embeddings for JinaEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        self.embed_batch(texts).await
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let mut results = self.embed_batch(&[text]).await?;
        results
            .pop()
            .ok_or_else(|| SynapticError::Embedding("empty response".to_string()))
    }
}
