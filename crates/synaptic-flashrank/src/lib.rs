//! Fast local cross-encoder reranker for Synaptic.
//!
//! Provides BM25-based relevance scoring that requires no external API and runs
//! entirely in-process. Ideal for development, testing, and offline deployments.
//!
//! For production neural reranking with cross-encoder models, see:
//! - [`synaptic-huggingface`] — BGE rerankers via HuggingFace Inference API
//! - [`synaptic-voyage`] — Voyage rerank-2 API
//! - [`synaptic-jina`] — Jina reranker API
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use synaptic_flashrank::{FlashRankReranker, FlashRankConfig};
//! use synaptic_core::Document;
//!
//! let reranker = FlashRankReranker::new(FlashRankConfig::default());
//! let docs = vec![
//!     Document::new("Paris is the capital of France."),
//!     Document::new("Berlin is the capital of Germany."),
//! ];
//! let results = reranker.rerank("capital of France", docs, 1).await?;
//! assert_eq!(results[0].0.content, "Paris is the capital of France.");
//! ```

use synaptic_core::{Document, SynapticError};

/// Configuration for the FlashRank BM25 reranker.
#[derive(Debug, Clone)]
pub struct FlashRankConfig {
    /// BM25 k1 parameter — term frequency saturation (default: 1.5).
    /// Higher values give more weight to repeated terms.
    pub k1: f32,
    /// BM25 b parameter — document length normalization (default: 0.75).
    /// 0 = no normalization, 1 = full normalization.
    pub b: f32,
}

impl Default for FlashRankConfig {
    fn default() -> Self {
        Self { k1: 1.5, b: 0.75 }
    }
}

impl FlashRankConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_k1(mut self, k1: f32) -> Self {
        self.k1 = k1;
        self
    }

    pub fn with_b(mut self, b: f32) -> Self {
        self.b = b;
        self
    }
}

/// Fast local reranker using BM25-based cross-encoder scoring.
///
/// Computes BM25 relevance scores between the query and each document.
/// No network calls, no API keys, runs entirely in-process.
pub struct FlashRankReranker {
    config: FlashRankConfig,
}

impl FlashRankReranker {
    pub fn new(config: FlashRankConfig) -> Self {
        Self { config }
    }

    /// Rerank documents by BM25 relevance to the query.
    ///
    /// Returns `(document, score)` pairs sorted by score descending,
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
        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            // Return first top_k docs with score 0 when query has no tokens
            return Ok(documents
                .into_iter()
                .take(top_k)
                .map(|d| (d, 0.0f32))
                .collect());
        }

        let doc_tokens: Vec<Vec<String>> = documents.iter().map(|d| tokenize(&d.content)).collect();
        let avg_dl =
            doc_tokens.iter().map(|t| t.len()).sum::<usize>() as f32 / doc_tokens.len() as f32;
        let n = documents.len() as f32;

        let mut scored: Vec<(Document, f32)> = documents
            .into_iter()
            .zip(doc_tokens.iter())
            .map(|(doc, tokens)| {
                let score = bm25_score(
                    &query_terms,
                    tokens,
                    n,
                    avg_dl,
                    self.config.k1,
                    self.config.b,
                );
                (doc, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().take(top_k).collect())
    }
}

/// Tokenize text into lowercase alphanumeric tokens of length >= 2.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_lowercase())
        .collect()
}

/// Compute BM25 score for a single query-document pair.
fn bm25_score(
    query_terms: &[String],
    doc_terms: &[String],
    n: f32,
    avg_dl: f32,
    k1: f32,
    b: f32,
) -> f32 {
    let dl = doc_terms.len() as f32;
    let mut score = 0.0f32;

    for term in query_terms {
        let tf = doc_terms.iter().filter(|t| *t == term).count() as f32;
        if tf == 0.0 {
            continue;
        }
        // IDF with smoothing: log((N - df + 0.5) / (df + 0.5) + 1)
        // Approximate df = 1 for unobserved corpus statistics
        let df = 1.0f32;
        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        let tf_norm = (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * dl / avg_dl));
        score += idf * tf_norm;
    }
    score
}
