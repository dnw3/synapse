use crate::Condenser;
use async_trait::async_trait;
use synaptic_core::{Message, SynapticError};

/// A no-op condenser that returns messages unchanged.
pub struct NoOpCondenser;

#[async_trait]
impl Condenser for NoOpCondenser {
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError> {
        Ok(messages)
    }
}
