use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::SynapseError;
use synapse_embeddings::Embeddings;
use tokio::sync::RwLock;

use crate::FewShotExample;

/// Trait for selecting examples for few-shot prompting.
#[async_trait]
pub trait ExampleSelector: Send + Sync {
    /// Select examples most relevant to the input.
    async fn select_examples(&self, input: &str) -> Result<Vec<FewShotExample>, SynapseError>;

    /// Add a new example to the selector's pool.
    async fn add_example(&self, example: FewShotExample) -> Result<(), SynapseError>;
}

/// Selects examples based on semantic similarity using embeddings.
pub struct SemanticSimilarityExampleSelector {
    examples: Arc<RwLock<Vec<(FewShotExample, Vec<f32>)>>>,
    embeddings: Arc<dyn Embeddings>,
    k: usize,
}

impl SemanticSimilarityExampleSelector {
    /// Create a new selector that returns the top-k most similar examples.
    pub fn new(embeddings: Arc<dyn Embeddings>, k: usize) -> Self {
        Self {
            examples: Arc::new(RwLock::new(Vec::new())),
            embeddings,
            k,
        }
    }
}

#[async_trait]
impl ExampleSelector for SemanticSimilarityExampleSelector {
    async fn add_example(&self, example: FewShotExample) -> Result<(), SynapseError> {
        let embedding = self.embeddings.embed_query(&example.input).await?;
        let mut examples = self.examples.write().await;
        examples.push((example, embedding));
        Ok(())
    }

    async fn select_examples(&self, input: &str) -> Result<Vec<FewShotExample>, SynapseError> {
        let query_embedding = self.embeddings.embed_query(input).await?;
        let examples = self.examples.read().await;

        if examples.is_empty() {
            return Ok(Vec::new());
        }

        // Compute similarities and collect with indices
        let mut scored: Vec<(usize, f32)> = examples
            .iter()
            .enumerate()
            .map(|(i, (_, emb))| (i, cosine_similarity(&query_embedding, emb)))
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k
        let result = scored
            .iter()
            .take(self.k)
            .map(|(i, _)| examples[*i].0.clone())
            .collect();

        Ok(result)
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0];
        let b = vec![0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
