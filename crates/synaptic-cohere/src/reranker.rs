#[cfg(feature = "retrieval")]
use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{Document, SynapticError};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the Cohere Reranker.
#[derive(Debug, Clone)]
pub struct CohereRerankerConfig {
    /// Cohere API key.
    pub api_key: String,
    /// Reranker model name (default: `"rerank-v3.5"`).
    pub model: String,
    /// Maximum number of documents to return. If `None`, all documents are returned.
    pub top_n: Option<usize>,
    /// Base URL for the Cohere API (default: `"https://api.cohere.ai/v2"`).
    pub base_url: String,
}

impl CohereRerankerConfig {
    /// Create a new configuration with the given API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "rerank-v3.5".to_string(),
            top_n: None,
            base_url: "https://api.cohere.ai/v2".to_string(),
        }
    }

    /// Set the reranker model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the maximum number of results to return.
    pub fn with_top_n(mut self, top_n: usize) -> Self {
        self.top_n = Some(top_n);
        self
    }

    /// Set a custom base URL for the API.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

// ---------------------------------------------------------------------------
// CohereReranker
// ---------------------------------------------------------------------------

/// A reranker that uses the Cohere Rerank API to reorder documents by
/// relevance to a query.
///
/// Each returned document has a `relevance_score` entry added to its metadata.
pub struct CohereReranker {
    config: CohereRerankerConfig,
    client: reqwest::Client,
}

impl CohereReranker {
    /// Create a new `CohereReranker` with the given configuration.
    pub fn new(config: CohereRerankerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Create a new `CohereReranker` with a custom HTTP client.
    pub fn with_client(config: CohereRerankerConfig, client: reqwest::Client) -> Self {
        Self { config, client }
    }

    /// Rerank documents by relevance to a query.
    ///
    /// Returns documents sorted by descending relevance score. Each document's
    /// metadata will contain a `"relevance_score"` entry.
    ///
    /// # Arguments
    ///
    /// * `query` - The query to rank against.
    /// * `documents` - The documents to rerank.
    /// * `top_n` - Override the configured `top_n`. If `None`, uses the
    ///   configured value, or returns all documents.
    pub async fn rerank(
        &self,
        query: &str,
        documents: Vec<Document>,
        top_n: Option<usize>,
    ) -> Result<Vec<Document>, SynapticError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        let top_n = top_n.or(self.config.top_n).unwrap_or(documents.len());

        let doc_texts: Vec<&str> = documents.iter().map(|d| d.content.as_str()).collect();

        let body = json!({
            "model": self.config.model,
            "query": query,
            "documents": doc_texts,
            "top_n": top_n,
        });

        let response = self
            .client
            .post(format!("{}/rerank", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Model(format!("Cohere rerank request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(SynapticError::Model(format!(
                "Cohere rerank API error ({status}): {text}"
            )));
        }

        let resp_body: Value = response
            .json()
            .await
            .map_err(|e| SynapticError::Model(format!("Cohere rerank parse error: {e}")))?;

        let results = resp_body["results"]
            .as_array()
            .ok_or_else(|| SynapticError::Model("missing 'results' in response".to_string()))?;

        let mut reranked = Vec::with_capacity(results.len());
        for result in results {
            let index = result["index"].as_u64().unwrap_or(0) as usize;
            let score = result["relevance_score"].as_f64().unwrap_or(0.0);
            if index < documents.len() {
                let mut doc = documents[index].clone();
                doc.metadata
                    .insert("relevance_score".to_string(), json!(score));
                reranked.push(doc);
            }
        }

        Ok(reranked)
    }
}

// ---------------------------------------------------------------------------
// DocumentCompressor implementation (behind `retrieval` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "retrieval")]
#[async_trait]
impl synaptic_retrieval::DocumentCompressor for CohereReranker {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
    ) -> Result<Vec<Document>, SynapticError> {
        self.rerank(query, documents, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = CohereRerankerConfig::new("test-key");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.model, "rerank-v3.5");
        assert_eq!(config.base_url, "https://api.cohere.ai/v2");
        assert!(config.top_n.is_none());
    }

    #[test]
    fn config_builder() {
        let config = CohereRerankerConfig::new("key")
            .with_model("rerank-english-v3.0")
            .with_top_n(5)
            .with_base_url("https://custom.api.com");

        assert_eq!(config.model, "rerank-english-v3.0");
        assert_eq!(config.top_n, Some(5));
        assert_eq!(config.base_url, "https://custom.api.com");
    }

    #[tokio::test]
    async fn rerank_empty_documents() {
        let config = CohereRerankerConfig::new("test-key");
        let reranker = CohereReranker::new(config);

        let result = reranker.rerank("query", Vec::new(), None).await.unwrap();
        assert!(result.is_empty());
    }
}
