use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{ChatModel, Message, SynapticError};
use synaptic::middleware::{
    AgentMiddleware, ModelFallbackMiddleware, ModelRequest, ModelResponse,
};
use tokio::sync::Mutex;

use crate::config::SynapseConfig;

use super::model::build_model_by_name;
use super::registry::ModelRegistry;

/// Detects when the agent is stuck in a loop, calling the same tool with
/// the same arguments repeatedly. After `max_repeats` consecutive identical
/// tool calls, injects a system message telling the model to try a different approach.
pub(crate) struct LoopDetectionMiddleware {
    max_repeats: usize,
    history: Mutex<Vec<u64>>,
}

impl LoopDetectionMiddleware {
    pub fn new(max_repeats: usize) -> Self {
        Self {
            max_repeats,
            history: Mutex::new(Vec::new()),
        }
    }

    fn hash_tool_calls(msg: &Message) -> Option<u64> {
        let tool_calls = msg.tool_calls();
        if tool_calls.is_empty() {
            return None;
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for tc in tool_calls {
            tc.name.hash(&mut hasher);
            tc.arguments.to_string().hash(&mut hasher);
        }
        Some(hasher.finish())
    }
}

#[async_trait]
impl AgentMiddleware for LoopDetectionMiddleware {
    async fn after_model(
        &self,
        _request: &ModelRequest,
        response: &mut ModelResponse,
    ) -> Result<(), SynapticError> {
        let is_loop = if let Some(hash) = Self::hash_tool_calls(&response.message) {
            let mut history = self.history.lock().await;

            let mut repeat_count = 0;
            for h in history.iter().rev() {
                if *h == hash {
                    repeat_count += 1;
                } else {
                    break;
                }
            }

            history.push(hash);
            let len = history.len();
            if len > 50 {
                history.drain(..len - 50);
            }

            repeat_count >= self.max_repeats
        } else {
            self.history.lock().await.clear();
            false
        };

        if is_loop {
            tracing::warn!("Loop detected — injecting correction");
            response.message = Message::ai(
                "I notice I've been repeating the same action. Let me try a different approach.",
            );
        }
        Ok(())
    }
}

/// Build ModelFallbackMiddleware from the fallback_models config.
///
/// Enhanced with registry support:
/// 1. If the primary model's provider has multi-key rotation, extra fallback instances
///    are built using different API keys (round-robin on 429/error).
/// 2. Fallback model names are resolved via the registry (supporting aliases).
pub fn build_fallback_middleware(config: &SynapseConfig) -> Option<ModelFallbackMiddleware> {
    let registry = ModelRegistry::from_config(config);
    let mut fallbacks: Vec<Arc<dyn ChatModel>> = Vec::new();

    // 1. Multi-key rotation fallbacks for the primary model
    let primary_model = &config.base.model.model;
    if let Some(key_fallbacks) = registry.rotation_fallbacks(primary_model) {
        tracing::info!(count = key_fallbacks.len(), "Key-rotation fallback(s) for primary model");
        fallbacks.extend(key_fallbacks);
    }

    // 2. Explicit fallback_models list (resolved via registry for alias support)
    if let Some(ref fallback_names) = config.fallback_models {
        for name in fallback_names {
            match build_model_by_name(config, name) {
                Ok(model) => fallbacks.push(model),
                Err(e) => {
                    tracing::warn!(model = %name, error = %e, "Failed to build fallback model");
                }
            }
        }
    }

    if fallbacks.is_empty() {
        return None;
    }

    tracing::info!(count = fallbacks.len(), "Fallback model(s) configured");
    Some(ModelFallbackMiddleware::new(fallbacks))
}
