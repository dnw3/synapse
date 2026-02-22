use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{Embeddings, SynapticError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NomicModel {
    NomicEmbedTextV1_5,
    NomicEmbedTextV1,
    Custom(String),
}

impl NomicModel {
    pub fn as_str(&self) -> &str {
        match self {
            NomicModel::NomicEmbedTextV1_5 => "nomic-embed-text-v1.5",
            NomicModel::NomicEmbedTextV1 => "nomic-embed-text-v1",
            NomicModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for NomicModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Task type for Nomic embeddings (affects how the model encodes text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NomicTaskType {
    SearchDocument,
    SearchQuery,
    Classification,
    Clustering,
}

impl NomicTaskType {
    pub fn as_str(&self) -> &str {
        match self {
            NomicTaskType::SearchDocument => "search_document",
            NomicTaskType::SearchQuery => "search_query",
            NomicTaskType::Classification => "classification",
            NomicTaskType::Clustering => "clustering",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NomicConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl NomicConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: NomicModel::NomicEmbedTextV1_5.to_string(),
            base_url: "https://api-atlas.nomic.ai/v1".to_string(),
        }
    }

    pub fn with_model(mut self, model: NomicModel) -> Self {
        self.model = model.to_string();
        self
    }
}

pub struct NomicEmbeddings {
    config: NomicConfig,
    client: reqwest::Client,
}

impl NomicEmbeddings {
    pub fn new(config: NomicConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    async fn embed_with_task(
        &self,
        texts: &[&str],
        task_type: NomicTaskType,
    ) -> Result<Vec<Vec<f32>>, SynapticError> {
        let body = json!({
            "model": self.config.model,
            "texts": texts,
            "task_type": task_type.as_str(),
        });
        let resp = self
            .client
            .post(format!("{}/embedding/text", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Embedding(format!("Nomic request: {e}")))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Embedding(format!("Nomic parse: {e}")))?;
        if status != 200 {
            return Err(SynapticError::Embedding(format!(
                "Nomic API error ({}): {}",
                status, json
            )));
        }
        let embeddings = json
            .get("embeddings")
            .and_then(|e| e.as_array())
            .ok_or_else(|| SynapticError::Embedding("missing 'embeddings' field".to_string()))?;
        let result = embeddings
            .iter()
            .map(|row| {
                row.as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                    .collect()
            })
            .collect();
        Ok(result)
    }
}

#[async_trait]
impl Embeddings for NomicEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        self.embed_with_task(texts, NomicTaskType::SearchDocument)
            .await
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let mut results = self
            .embed_with_task(&[text], NomicTaskType::SearchQuery)
            .await?;
        results
            .pop()
            .ok_or_else(|| SynapticError::Embedding("empty response".to_string()))
    }
}
