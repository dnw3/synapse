use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::SynapseError;

use crate::{Document, Retriever};

/// Standard RRF constant (k parameter in the RRF formula).
const RRF_K: f64 = 60.0;

/// A retriever that combines results from multiple retrievers using
/// Reciprocal Rank Fusion (RRF) with configurable weights.
pub struct EnsembleRetriever {
    retrievers: Vec<(Arc<dyn Retriever>, f64)>,
}

impl EnsembleRetriever {
    /// Create a new EnsembleRetriever with weighted retrievers.
    ///
    /// Each tuple is `(retriever, weight)`. The weight scales the RRF score
    /// contribution of that retriever.
    pub fn new(retrievers: Vec<(Arc<dyn Retriever>, f64)>) -> Self {
        Self { retrievers }
    }
}

#[async_trait]
impl Retriever for EnsembleRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        // Map from doc.id -> (rrf_score, Document)
        let mut scores: HashMap<String, (f64, Document)> = HashMap::new();

        for (retriever, weight) in &self.retrievers {
            let docs = retriever.retrieve(query, top_k).await?;

            for (rank, doc) in docs.iter().enumerate() {
                // RRF score contribution: weight / (k + rank)
                // rank is 0-based, so rank 0 = position 1
                let rrf_score = weight / (RRF_K + (rank + 1) as f64);

                scores
                    .entry(doc.id.clone())
                    .and_modify(|(existing_score, _)| {
                        *existing_score += rrf_score;
                    })
                    .or_insert_with(|| (rrf_score, doc.clone()));
            }
        }

        // Sort by RRF score descending
        let mut sorted: Vec<(f64, Document)> = scores.into_values().collect();
        sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(sorted.into_iter().take(top_k).map(|(_, doc)| doc).collect())
    }
}
