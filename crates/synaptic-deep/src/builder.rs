//! Build a complete agent from a [`SynapticAgentConfig`](synaptic_config::SynapticAgentConfig).
//!
//! This module is feature-gated behind `config-builder`.

use std::sync::Arc;

use synaptic_core::{ChatModel, SynapticError};
use synaptic_graph::{CompiledGraph, MessageState};
use synaptic_models::HttpBackend;

use crate::backend::StateBackend;
use crate::{create_deep_agent, DeepAgentOptions};

/// Build a complete deep agent from configuration.
///
/// 1. Creates the appropriate [`ChatModel`] based on `config.model.provider`
/// 2. Sets up filesystem tools if configured
/// 3. Assembles the agent with middleware stack
pub async fn build_agent_from_config(
    config: &synaptic_config::SynapticAgentConfig,
) -> Result<CompiledGraph<MessageState>, SynapticError> {
    let api_key = config.resolve_api_key().unwrap_or_default();
    let model = create_model_from_config(&config.model, &api_key)?;

    let backend = Arc::new(StateBackend::new());

    let mut options = DeepAgentOptions::new(backend);
    options.system_prompt = config.agent.system_prompt.clone();
    options.enable_filesystem = config.agent.tools.filesystem;
    options.memory_file = Some(config.paths.memory_file.clone());
    options.skills_dir = Some(config.paths.skills_dir.clone());

    create_deep_agent(model, options)
}

/// Create a [`ChatModel`] from provider configuration.
fn create_model_from_config(
    config: &synaptic_config::ModelConfig,
    api_key: &str,
) -> Result<Arc<dyn ChatModel>, SynapticError> {
    let http: Arc<dyn synaptic_models::ProviderBackend> = Arc::new(HttpBackend::new());

    match config.provider.as_str() {
        #[cfg(feature = "openai-provider")]
        "openai" => {
            let mut model_config = synaptic_openai::OpenAiConfig::new(api_key, &config.model);
            if let Some(ref url) = config.base_url {
                model_config = model_config.with_base_url(url);
            }
            if let Some(temp) = config.temperature {
                model_config = model_config.with_temperature(temp);
            }
            if let Some(max) = config.max_tokens {
                model_config = model_config.with_max_tokens(max);
            }
            Ok(Arc::new(synaptic_openai::OpenAiChatModel::new(
                model_config,
                http,
            )))
        }
        #[cfg(feature = "anthropic-provider")]
        "anthropic" => {
            let mut model_config = synaptic_anthropic::AnthropicConfig::new(api_key, &config.model);
            if let Some(ref url) = config.base_url {
                model_config = model_config.with_base_url(url);
            }
            if let Some(max) = config.max_tokens {
                model_config = model_config.with_max_tokens(max);
            }
            Ok(Arc::new(synaptic_anthropic::AnthropicChatModel::new(
                model_config,
                http,
            )))
        }
        #[cfg(feature = "gemini-provider")]
        "gemini" => {
            let mut model_config = synaptic_gemini::GeminiConfig::new(api_key, &config.model);
            if let Some(ref url) = config.base_url {
                model_config = model_config.with_base_url(url);
            }
            Ok(Arc::new(synaptic_gemini::GeminiChatModel::new(
                model_config,
                http,
            )))
        }
        #[cfg(feature = "ollama-provider")]
        "ollama" => {
            let mut model_config = synaptic_ollama::OllamaConfig::new(&config.model);
            if let Some(ref url) = config.base_url {
                model_config = model_config.with_base_url(url);
            }
            Ok(Arc::new(synaptic_ollama::OllamaChatModel::new(
                model_config,
                http,
            )))
        }
        // Default: treat as OpenAI-compatible with custom base_url
        _ => {
            #[cfg(feature = "openai-provider")]
            {
                let base_url = config.base_url.as_deref().ok_or_else(|| {
                    SynapticError::Config(format!(
                        "unknown provider '{}' requires a base_url for OpenAI-compatible mode",
                        config.provider
                    ))
                })?;
                let mut model_config = synaptic_openai::OpenAiConfig::new(api_key, &config.model);
                model_config = model_config.with_base_url(base_url);
                if let Some(temp) = config.temperature {
                    model_config = model_config.with_temperature(temp);
                }
                if let Some(max) = config.max_tokens {
                    model_config = model_config.with_max_tokens(max);
                }
                Ok(Arc::new(synaptic_openai::OpenAiChatModel::new(
                    model_config,
                    http,
                )))
            }
            #[cfg(not(feature = "openai-provider"))]
            {
                Err(SynapticError::Config(format!(
                    "provider '{}' not supported without openai-provider feature",
                    config.provider
                )))
            }
        }
    }
}
