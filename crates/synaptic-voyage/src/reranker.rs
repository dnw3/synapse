use synaptic_core::{Document, SynapticError};

/// Available Voyage AI reranker models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoyageRerankerModel {
    /// `rerank-2` — highest quality (recommended)
    Rerank2,
    /// `rerank-2-lite` — faster and lower cost
    Rerank2Lite,
    /// Any Voyage model ID
    Custom(String),
}

impl VoyageRerankerModel {
    pub fn as_str(&self) -> &str {
        match self {
            VoyageRerankerModel::Rerank2 => "rerank-2",
            VoyageRerankerModel::Rerank2Lite => "rerank-2-lite",
            VoyageRerankerModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for VoyageRerankerModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Reranker using Voyage AI reranking API.
///
/// # Example
///
/// ```rust,ignore
/// use synaptic_voyage::reranker::{VoyageReranker, VoyageRerankerModel};
/// use synaptic_core::Document;
///
/// let reranker = VoyageReranker::new("pa-your-key")
///     .with_model(VoyageRerankerModel::Rerank2);
///
/// let docs = vec![
///     Document::new("Paris is the capital of France."),
///     Document::new("Berlin is the capital of Germany."),
/// ];
/// let results = reranker.rerank("capital of France", docs, 1).await?;
/// ```
pub struct VoyageReranker {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl VoyageReranker {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: VoyageRerankerModel::Rerank2.to_string(),
            base_url: "https://api.voyageai.com/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: VoyageRerankerModel) -> Self {
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
        let doc_texts: Vec<&str> = documents.iter().map(|d| d.content.as_str()).collect();
        let body = serde_json::json!({
            "model": self.model,
            "query": query,
            "documents": doc_texts,
            "top_k": top_k,
        });
        let resp = self
            .client
            .post(format!("{}/rerank", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Retriever(format!("Voyage rerank request: {e}")))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Retriever(format!("Voyage rerank parse: {e}")))?;
        if status != 200 {
            return Err(SynapticError::Retriever(format!(
                "Voyage API error ({}): {}",
                status, json
            )));
        }
        // Response: {"data": [{"index": 0, "relevance_score": 0.9}]}
        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| SynapticError::Retriever("missing 'data' field".to_string()))?;
        let mut scored: Vec<(Document, f32)> = data
            .iter()
            .filter_map(|item| {
                let idx = item.get("index")?.as_u64()? as usize;
                let score = item.get("relevance_score")?.as_f64()? as f32;
                let doc = documents.get(idx)?.clone();
                Some((doc, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().take(top_k).collect())
    }
}
