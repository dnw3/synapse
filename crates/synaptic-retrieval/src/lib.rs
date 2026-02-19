mod bm25;
mod compression;
mod ensemble;
mod multi_query;
mod parent_document;
mod self_query;

pub use bm25::BM25Retriever;
pub use compression::{ContextualCompressionRetriever, DocumentCompressor, EmbeddingsFilter};
pub use ensemble::EnsembleRetriever;
pub use multi_query::MultiQueryRetriever;
pub use parent_document::ParentDocumentRetriever;
pub use self_query::{MetadataFieldInfo, SelfQueryRetriever};

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use synaptic_core::SynapticError;

/// A document with content and metadata, used throughout the retrieval pipeline.
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

/// Trait for retrieving relevant documents given a query string.
#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapticError>;
}

/// A simple retriever that stores documents in memory and returns all of them for any query.
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
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapticError> {
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

pub(crate) fn tokenize(input: &str) -> HashSet<String> {
    input
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

/// Tokenize text into a Vec of lowercase tokens, preserving duplicates.
/// Used by BM25 which needs term frequency counts.
pub(crate) fn tokenize_to_vec(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .collect()
}
