use async_trait::async_trait;
use synaptic_core::SynapticError;

use crate::Embeddings;

/// Deterministic embeddings for testing.
/// Generates vectors based on a simple hash of the input text.
pub struct FakeEmbeddings {
    dimensions: usize,
}

impl FakeEmbeddings {
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

impl Default for FakeEmbeddings {
    fn default() -> Self {
        Self::new(4)
    }
}

#[async_trait]
impl Embeddings for FakeEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        Ok(texts
            .iter()
            .map(|t| text_to_vector(t, self.dimensions))
            .collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        Ok(text_to_vector(text, self.dimensions))
    }
}

/// Generate a deterministic vector from text. Similar texts produce similar vectors.
fn text_to_vector(text: &str, dimensions: usize) -> Vec<f32> {
    let mut vec = vec![0.0f32; dimensions];
    for (i, byte) in text.bytes().enumerate() {
        vec[i % dimensions] += byte as f32;
    }
    // Normalize to unit vector
    let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for x in &mut vec {
            *x /= magnitude;
        }
    }
    vec
}
