use serde_json::json;
use synaptic_core::{Document, SynapticError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JinaRerankerModel {
    JinaRerankerV2BaseMultilingual,
    JinaRerankerV1BaseEn,
    Custom(String),
}

impl JinaRerankerModel {
    pub fn as_str(&self) -> &str {
        match self {
            JinaRerankerModel::JinaRerankerV2BaseMultilingual => {
                "jina-reranker-v2-base-multilingual"
            }
            JinaRerankerModel::JinaRerankerV1BaseEn => "jina-reranker-v1-base-en",
            JinaRerankerModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for JinaRerankerModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct JinaReranker {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl JinaReranker {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: JinaRerankerModel::JinaRerankerV2BaseMultilingual.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: JinaRerankerModel) -> Self {
        self.model = model.as_str().to_string();
        self
    }

    pub async fn rerank(
        &self,
        query: &str,
        documents: Vec<Document>,
        top_k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let docs: Vec<&str> = documents.iter().map(|d| d.content.as_str()).collect();
        let body = json!({
            "model": self.model,
            "query": query,
            "documents": docs,
            "top_n": top_k,
        });
        let resp = self
            .client
            .post("https://api.jina.ai/v1/rerank")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Retriever(format!("Jina rerank request: {e}")))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Retriever(format!("Jina rerank parse: {e}")))?;
        if status != 200 {
            return Err(SynapticError::Retriever(format!(
                "Jina API error ({}): {}",
                status, json
            )));
        }
        let results = json
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| SynapticError::Retriever("missing 'results' field".to_string()))?;
        let mut scored: Vec<(Document, f32)> = results
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
