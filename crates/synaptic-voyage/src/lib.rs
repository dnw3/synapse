pub mod reranker;
pub use reranker::{VoyageReranker, VoyageRerankerModel};

use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{Embeddings, SynapticError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoyageModel {
    Voyage3Large,
    Voyage3,
    Voyage3Lite,
    VoyageCode3,
    VoyageFinance2,
    Custom(String),
}

impl VoyageModel {
    pub fn as_str(&self) -> &str {
        match self {
            VoyageModel::Voyage3Large => "voyage-3-large",
            VoyageModel::Voyage3 => "voyage-3",
            VoyageModel::Voyage3Lite => "voyage-3-lite",
            VoyageModel::VoyageCode3 => "voyage-code-3",
            VoyageModel::VoyageFinance2 => "voyage-finance-2",
            VoyageModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for VoyageModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct VoyageConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub input_type: Option<String>,
}

impl VoyageConfig {
    pub fn new(api_key: impl Into<String>, model: VoyageModel) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.to_string(),
            base_url: "https://api.voyageai.com/v1".to_string(),
            input_type: None,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_input_type(mut self, t: impl Into<String>) -> Self {
        self.input_type = Some(t.into());
        self
    }
}

pub struct VoyageEmbeddings {
    config: VoyageConfig,
    client: reqwest::Client,
}

impl VoyageEmbeddings {
    pub fn new(config: VoyageConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    async fn embed_batch(
        &self,
        texts: &[&str],
        input_type: Option<&str>,
    ) -> Result<Vec<Vec<f32>>, SynapticError> {
        let mut body = json!({
            "model": self.config.model,
            "input": texts,
        });
        let itype = input_type.or(self.config.input_type.as_deref());
        if let Some(t) = itype {
            body["input_type"] = json!(t);
        }
        let resp = self
            .client
            .post(format!("{}/embeddings", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Embedding(format!("Voyage request: {e}")))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Embedding(format!("Voyage parse: {e}")))?;
        if status != 200 {
            return Err(SynapticError::Embedding(format!(
                "Voyage API error ({}): {}",
                status, json
            )));
        }
        parse_voyage_response(&json)
    }
}

fn parse_voyage_response(body: &serde_json::Value) -> Result<Vec<Vec<f32>>, SynapticError> {
    let data = body
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| SynapticError::Embedding("missing 'data' field".to_string()))?;
    let mut result = Vec::with_capacity(data.len());
    for item in data {
        let emb = item
            .get("embedding")
            .and_then(|e| e.as_array())
            .ok_or_else(|| SynapticError::Embedding("missing 'embedding' field".to_string()))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        result.push(emb);
    }
    Ok(result)
}

#[async_trait]
impl Embeddings for VoyageEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        self.embed_batch(texts, Some("document")).await
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let mut results = self.embed_batch(&[text], Some("query")).await?;
        results
            .pop()
            .ok_or_else(|| SynapticError::Embedding("empty response".to_string()))
    }
}
