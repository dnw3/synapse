use std::sync::Arc;

use crate::Condenser;
use async_trait::async_trait;
use synaptic_core::{Message, SynapticError};

/// Chains multiple condensers in sequence.
pub struct PipelineCondenser(pub Vec<Arc<dyn Condenser>>);

impl PipelineCondenser {
    pub fn new(condensers: Vec<Arc<dyn Condenser>>) -> Self {
        Self(condensers)
    }
}

#[async_trait]
impl Condenser for PipelineCondenser {
    async fn condense(&self, mut messages: Vec<Message>) -> Result<Vec<Message>, SynapticError> {
        for c in &self.0 {
            messages = c.condense(messages).await?;
        }
        Ok(messages)
    }
}
