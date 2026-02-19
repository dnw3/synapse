use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::SynapticError;
use synaptic_embeddings::Embeddings;

use crate::evaluator::{EvalResult, Evaluator};

/// Evaluator that computes cosine similarity between embeddings of prediction and reference.
pub struct EmbeddingDistanceEvaluator {
    embeddings: Arc<dyn Embeddings>,
    threshold: f64,
}

impl EmbeddingDistanceEvaluator {
    /// Create a new embedding distance evaluator.
    ///
    /// - `embeddings`: The embeddings model to use.
    /// - `threshold`: Minimum cosine similarity to pass (default suggestion: 0.8).
    pub fn new(embeddings: Arc<dyn Embeddings>, threshold: f64) -> Self {
        Self {
            embeddings,
            threshold,
        }
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    (dot / (mag_a * mag_b)) as f64
}

#[async_trait]
impl Evaluator for EmbeddingDistanceEvaluator {
    async fn evaluate(
        &self,
        prediction: &str,
        reference: &str,
        _input: &str,
    ) -> Result<EvalResult, SynapticError> {
        let pred_embedding = self.embeddings.embed_query(prediction).await?;
        let ref_embedding = self.embeddings.embed_query(reference).await?;

        let similarity = cosine_similarity(&pred_embedding, &ref_embedding);

        let passed = similarity >= self.threshold;
        let result = EvalResult {
            score: similarity,
            passed,
            reasoning: Some(format!(
                "Cosine similarity: {:.4}, threshold: {:.4}",
                similarity, self.threshold
            )),
        };

        Ok(result)
    }
}
