use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use synapse_core::SynapseError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

impl Document {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(
        id: impl Into<String>,
        content: impl Into<String>,
        metadata: HashMap<String, Value>,
    ) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata,
        }
    }
}

#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError>;
}

#[derive(Debug, Clone)]
pub struct InMemoryRetriever {
    documents: Vec<Document>,
}

impl InMemoryRetriever {
    pub fn new(documents: Vec<Document>) -> Self {
        Self { documents }
    }
}

#[async_trait]
impl Retriever for InMemoryRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        let query_terms = tokenize(query);
        let mut scored: Vec<(usize, &Document)> = self
            .documents
            .iter()
            .map(|doc| {
                let terms = tokenize(&doc.content);
                let score = query_terms.intersection(&terms).count();
                (score, doc)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(scored
            .into_iter()
            .filter(|(score, _)| *score > 0)
            .take(top_k)
            .map(|(_, doc)| doc.clone())
            .collect())
    }
}

fn tokenize(input: &str) -> HashSet<String> {
    input
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .collect()
}
