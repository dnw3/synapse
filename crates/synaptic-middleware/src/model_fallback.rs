use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, SynapticError};

use crate::{AgentMiddleware, BaseChatModelCaller, ModelCaller, ModelRequest, ModelResponse};

/// Falls back to alternative models when the primary model fails.
///
/// On error from the primary model call, the middleware tries each
/// fallback model in order until one succeeds.
pub struct ModelFallbackMiddleware {
    fallbacks: Vec<Arc<dyn ChatModel>>,
}

impl ModelFallbackMiddleware {
    pub fn new(fallbacks: Vec<Arc<dyn ChatModel>>) -> Self {
        Self { fallbacks }
    }
}

#[async_trait]
impl AgentMiddleware for ModelFallbackMiddleware {
    async fn wrap_model_call(
        &self,
        request: ModelRequest,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        match next.call(request.clone()).await {
            Ok(resp) => Ok(resp),
            Err(primary_err) => {
                for fallback in &self.fallbacks {
                    let caller = BaseChatModelCaller::new(fallback.clone());
                    match caller.call(request.clone()).await {
                        Ok(resp) => return Ok(resp),
                        Err(_) => continue,
                    }
                }
                // All fallbacks failed; return original error
                Err(primary_err)
            }
        }
    }
}
