use synaptic_core::{Document, SynapticError};

/// Available BGE reranker models via HuggingFace Inference API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgeRerankerModel {
    /// `BAAI/bge-reranker-v2-m3` — multilingual cross-encoder (recommended)
    BgeRerankerV2M3,
    /// `BAAI/bge-reranker-large` — highest quality, English-focused
    BgeRerankerLarge,
    /// `BAAI/bge-reranker-base` — fast, good quality, English-focused
    BgeRerankerBase,
    /// Any HuggingFace model ID
    Custom(String),
}

impl BgeRerankerModel {
    pub fn as_str(&self) -> &str {
        match self {
            BgeRerankerModel::BgeRerankerV2M3 => "BAAI/bge-reranker-v2-m3",
            BgeRerankerModel::BgeRerankerLarge => "BAAI/bge-reranker-large",
            BgeRerankerModel::BgeRerankerBase => "BAAI/bge-reranker-base",
            BgeRerankerModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for BgeRerankerModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Reranker using HuggingFace Inference API (BGE cross-encoder models).
///
/// Calls the sentence-similarity inference endpoint with `source_sentence`/`sentences`
/// format and returns documents sorted by relevance score.
pub struct HuggingFaceReranker {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl HuggingFaceReranker {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: BgeRerankerModel::BgeRerankerV2M3.to_string(),
            base_url: "https://api-inference.huggingface.co/models".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: BgeRerankerModel) -> Self {
        self.model = model.as_str().to_string();
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Rerank documents by relevance to the query.
    ///
    /// Returns `(document, score)` pairs sorted by relevance score descending,
    /// limited to `top_k` results.
    pub async fn rerank(
        &self,
        query: &str,
        documents: Vec<Document>,
        top_k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let sentences: Vec<&str> = documents.iter().map(|d| d.content.as_str()).collect();
        let body = serde_json::json!({
            "inputs": {
                "source_sentence": query,
                "sentences": sentences,
            }
        });
        let url = format!("{}/{}", self.base_url, self.model);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("x-wait-for-model", "true")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Retriever(format!("HuggingFace rerank request: {e}")))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Retriever(format!("HuggingFace rerank parse: {e}")))?;
        if status != 200 {
            return Err(SynapticError::Retriever(format!(
                "HuggingFace API error ({}): {}",
                status, json
            )));
        }
        // Response is an array of floats, one per input sentence, in input order
        let scores = json
            .as_array()
            .ok_or_else(|| SynapticError::Retriever("expected array response".to_string()))?;
        let mut scored: Vec<(Document, f32)> = scores
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let score = v.as_f64()? as f32;
                let doc = documents.get(i)?.clone();
                Some((doc, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().take(top_k).collect())
    }
}
