use std::sync::Arc;

use synaptic::core::ChatModel;
use synaptic::models::{HttpBackend, TokenBucketChatModel};
use synaptic::openai::{OpenAiChatModel, OpenAiConfig};

use crate::config::SynapseConfig;

use super::registry::ModelRegistry;

// Re-use BASE_URL constants from the framework's compat providers (canonical source).
use synaptic::openai::compat::{
    ark, baichuan, cohere, deepseek, fireworks, groq, huggingface, minimax, mistral, moonshot,
    openrouter, perplexity, qwen, together, xai, zhipu,
};

/// Auto-detect the base URL for a known provider name.
///
/// Returns `None` for unknown providers (falls back to the OpenAI default).
pub fn auto_detect_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "moonshot" | "kimi" => Some(moonshot::BASE_URL),
        "qwen" | "dashscope" | "tongyi" => Some(qwen::BASE_URL),
        "zhipu" | "glm" | "chatglm" => Some(zhipu::BASE_URL),
        "doubao" | "ark" => Some(ark::BASE_URL),
        "minimax" => Some(minimax::BASE_URL),
        "baichuan" => Some(baichuan::BASE_URL),
        "deepseek" => Some(deepseek::BASE_URL),
        "groq" => Some(groq::BASE_URL),
        "together" => Some(together::BASE_URL),
        "fireworks" => Some(fireworks::BASE_URL),
        "xai" | "grok" => Some(xai::BASE_URL),
        "perplexity" => Some(perplexity::BASE_URL),
        "mistral" => Some(mistral::BASE_URL),
        "cohere" => Some(cohere::BASE_URL),
        "openrouter" => Some(openrouter::BASE_URL),
        "huggingface" => Some(huggingface::BASE_URL),
        "openai" => Some("https://api.openai.com/v1"),
        "anthropic" => Some("https://api.anthropic.com/v1"),
        "gemini" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "ollama" => Some("http://localhost:11434/v1"),
        _ => None,
    }
}

/// Return a map of provider name → default base URL for all known providers.
pub fn provider_base_url_defaults() -> Vec<(&'static str, &'static str)> {
    vec![
        ("openai", "https://api.openai.com/v1"),
        ("anthropic", "https://api.anthropic.com/v1"),
        ("gemini", "https://generativelanguage.googleapis.com/v1beta"),
        ("ollama", "http://localhost:11434/v1"),
        ("moonshot", moonshot::BASE_URL),
        ("qwen", qwen::BASE_URL),
        ("zhipu", zhipu::BASE_URL),
        ("ark", ark::BASE_URL),
        ("doubao", ark::BASE_URL),
        ("minimax", minimax::BASE_URL),
        ("baichuan", baichuan::BASE_URL),
        ("deepseek", deepseek::BASE_URL),
        ("groq", groq::BASE_URL),
        ("together", together::BASE_URL),
        ("fireworks", fireworks::BASE_URL),
        ("xai", xai::BASE_URL),
        ("perplexity", perplexity::BASE_URL),
        ("mistral", mistral::BASE_URL),
        ("cohere", cohere::BASE_URL),
        ("openrouter", openrouter::BASE_URL),
        ("huggingface", huggingface::BASE_URL),
    ]
}

/// Build a ChatModel from the resolved configuration.
///
/// If `rate_limit` is configured, the model is wrapped with `TokenBucketChatModel`.
pub fn build_model(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> crate::error::Result<Arc<dyn ChatModel>> {
    let model_name = model_override.unwrap_or(&config.model_config().model);
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
) -> crate::error::Result<Arc<dyn ChatModel>> {
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
) -> crate::error::Result<Arc<dyn ChatModel>> {
    let api_key = config.resolve_api_key().unwrap_or_else(|e| {
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
    let auto_base_url =
        auto_detect_base_url(provider_prefix.unwrap_or(&config.model_config().provider));

    let mut oai_config = OpenAiConfig::new(&api_key, actual_model);

    // Priority: explicit config base_url > auto-detected > default OpenAI
    if let Some(ref url) = config.model_config().base_url {
        oai_config = oai_config.with_base_url(url);
    } else if let Some(url) = auto_base_url {
        oai_config = oai_config.with_base_url(url);
    }

    if let Some(temp) = config.model_config().temperature {
        oai_config = oai_config.with_temperature(temp);
    }
    if let Some(max) = config.model_config().max_tokens {
        oai_config = oai_config.with_max_tokens(max);
    }

    Ok(Arc::new(OpenAiChatModel::new(oai_config, http)))
}
