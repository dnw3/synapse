use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::SynapticError;
use synaptic_middleware::{AgentMiddleware, ModelRequest};

use crate::Condenser;

/// Middleware that applies a condenser to messages before each model call.
pub struct CondenserMiddleware {
    condenser: Arc<dyn Condenser>,
}

impl CondenserMiddleware {
    pub fn new(condenser: Arc<dyn Condenser>) -> Self {
        Self { condenser }
    }
}

#[async_trait]
impl AgentMiddleware for CondenserMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        request.messages = self.condenser.condense(request.messages.clone()).await?;
        Ok(())
    }
}
