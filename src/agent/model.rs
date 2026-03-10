use std::sync::Arc;

use synaptic::core::ChatModel;
use synaptic::models::{HttpBackend, TokenBucketChatModel};
use synaptic::openai::{OpenAiChatModel, OpenAiConfig};

use crate::config::SynapseConfig;

use super::registry::ModelRegistry;

/// Build a ChatModel from the resolved configuration.
///
/// If `rate_limit` is configured, the model is wrapped with `TokenBucketChatModel`.
pub fn build_model(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<Arc<dyn ChatModel>, Box<dyn std::error::Error>> {
    let model_name = model_override.unwrap_or(&config.base.model.model);
    let model = build_model_by_name(config, model_name)?;

    // Wrap with rate limiting if configured
    if let Some(ref rl) = config.rate_limit {
        tracing::info!(
            capacity = rl.capacity,
            refill_rate = rl.refill_rate,
            "Rate limiting enabled"
        );
        Ok(Arc::new(TokenBucketChatModel::new(
            model,
            rl.capacity,
            rl.refill_rate,
        )))
    } else {
        Ok(model)
    }
}

/// Build a ChatModel for a specific model name using the config's provider settings.
///
/// First tries the model registry (catalog + aliases). Falls back to hardcoded provider
/// prefix detection if the model isn't found in the catalog.
pub fn build_model_by_name(
    config: &SynapseConfig,
    model_name: &str,
) -> Result<Arc<dyn ChatModel>, Box<dyn std::error::Error>> {
    // Try registry first (catalog + aliases)
    let registry = ModelRegistry::from_config(config);
    if registry.contains(model_name) {
        return registry.resolve(model_name);
    }

    // Fallback: hardcoded provider detection
    build_model_by_name_raw(config, model_name)
}

/// Build a ChatModel using hardcoded provider prefix detection.
///
/// Recognizes provider-specific prefixes and auto-configures base URLs:
/// - `moonshot/`, `qwen/`, `zhipu/`, `doubao/`, `minimax/`, `baichuan/` — Chinese LLM providers
/// - `deepseek/`, `groq/`, `together/`, `fireworks/`, `xai/`, `perplexity/` — OpenAI-compat
/// - No prefix — uses the provider from config (default: OpenAI)
pub(crate) fn build_model_by_name_raw(
    config: &SynapseConfig,
    model_name: &str,
) -> Result<Arc<dyn ChatModel>, Box<dyn std::error::Error>> {
    let api_key = config.base.resolve_api_key().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "API key resolution failed — requests will likely fail");
        String::new()
    });

    let http = Arc::new(HttpBackend::new());

    // Check for provider prefix (e.g. "moonshot/moonshot-v1-8k")
    let (provider_prefix, actual_model) = match model_name.split_once('/') {
        Some((prefix, model)) => (Some(prefix), model),
        None => (None, model_name),
    };

    // Auto-detect base_url for known providers
    let auto_base_url = match provider_prefix.unwrap_or(&config.base.model.provider) {
        "moonshot" | "kimi" => Some("https://api.moonshot.cn/v1"),
        "qwen" | "dashscope" | "tongyi" => {
            Some("https://dashscope.aliyuncs.com/compatible-mode/v1")
        }
        "zhipu" | "glm" | "chatglm" => Some("https://open.bigmodel.cn/api/paas/v4"),
        "doubao" | "ark" => Some("https://ark.cn-beijing.volces.com/api/v3"),
        "minimax" => Some("https://api.minimax.chat/v1"),
        "baichuan" => Some("https://api.baichuan-ai.com/v1"),
        "deepseek" => Some("https://api.deepseek.com/v1"),
        "groq" => Some("https://api.groq.com/openai/v1"),
        "together" => Some("https://api.together.xyz/v1"),
        "fireworks" => Some("https://api.fireworks.ai/inference/v1"),
        "xai" | "grok" => Some("https://api.x.ai/v1"),
        "perplexity" => Some("https://api.perplexity.ai"),
        _ => None,
    };

    let mut oai_config = OpenAiConfig::new(&api_key, actual_model);

    // Priority: explicit config base_url > auto-detected > default OpenAI
    if let Some(ref url) = config.base.model.base_url {
        oai_config = oai_config.with_base_url(url);
    } else if let Some(url) = auto_base_url {
        oai_config = oai_config.with_base_url(url);
    }

    if let Some(temp) = config.base.model.temperature {
        oai_config = oai_config.with_temperature(temp);
    }
    if let Some(max) = config.base.model.max_tokens {
        oai_config = oai_config.with_max_tokens(max);
    }

    Ok(Arc::new(OpenAiChatModel::new(oai_config, http)))
}
