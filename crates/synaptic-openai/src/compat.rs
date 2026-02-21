//! Convenience constructors for OpenAI-compatible providers.
//!
//! Each provider uses the same wire format as OpenAI but with a different
//! base URL. The functions in this module pre-configure `OpenAiConfig` and
//! `OpenAiEmbeddingsConfig` with the correct endpoint.

use std::sync::Arc;

use synaptic_models::ProviderBackend;

use crate::{OpenAiChatModel, OpenAiConfig, OpenAiEmbeddings, OpenAiEmbeddingsConfig};

// ---------------------------------------------------------------------------
// Groq
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for Groq.
pub fn groq_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://api.groq.com/openai/v1")
}

/// Create a Groq chat model.
pub fn groq_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(groq_config(api_key, model), backend)
}

// ---------------------------------------------------------------------------
// DeepSeek
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for DeepSeek.
pub fn deepseek_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://api.deepseek.com/v1")
}

/// Create a DeepSeek chat model.
pub fn deepseek_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(deepseek_config(api_key, model), backend)
}

// ---------------------------------------------------------------------------
// Fireworks
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for Fireworks AI.
pub fn fireworks_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://api.fireworks.ai/inference/v1")
}

/// Create a Fireworks AI chat model.
pub fn fireworks_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(fireworks_config(api_key, model), backend)
}

// ---------------------------------------------------------------------------
// Together
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for Together AI.
pub fn together_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://api.together.xyz/v1")
}

/// Create a Together AI chat model.
pub fn together_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(together_config(api_key, model), backend)
}

// ---------------------------------------------------------------------------
// xAI
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for xAI.
pub fn xai_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://api.x.ai/v1")
}

/// Create an xAI chat model.
pub fn xai_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(xai_config(api_key, model), backend)
}

// ---------------------------------------------------------------------------
// MistralAI  (+Embeddings)
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for Mistral AI.
pub fn mistral_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://api.mistral.ai/v1")
}

/// Create a Mistral AI chat model.
pub fn mistral_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(mistral_config(api_key, model), backend)
}

/// Create an OpenAI-compatible embeddings config for Mistral AI.
pub fn mistral_embeddings_config(api_key: impl Into<String>) -> OpenAiEmbeddingsConfig {
    OpenAiEmbeddingsConfig::new(api_key).with_base_url("https://api.mistral.ai/v1")
}

/// Create Mistral AI embeddings.
pub fn mistral_embeddings(
    api_key: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiEmbeddings {
    OpenAiEmbeddings::new(mistral_embeddings_config(api_key), backend)
}

// ---------------------------------------------------------------------------
// HuggingFace  (+Embeddings)
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for HuggingFace Inference API.
pub fn huggingface_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://router.huggingface.co/v1")
}

/// Create a HuggingFace chat model.
pub fn huggingface_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(huggingface_config(api_key, model), backend)
}

/// Create an OpenAI-compatible embeddings config for HuggingFace.
pub fn huggingface_embeddings_config(api_key: impl Into<String>) -> OpenAiEmbeddingsConfig {
    OpenAiEmbeddingsConfig::new(api_key).with_base_url("https://router.huggingface.co/v1")
}

/// Create HuggingFace embeddings.
pub fn huggingface_embeddings(
    api_key: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiEmbeddings {
    OpenAiEmbeddings::new(huggingface_embeddings_config(api_key), backend)
}

// ---------------------------------------------------------------------------
// Cohere  (+Embeddings)
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for Cohere.
pub fn cohere_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model)
        .with_base_url("https://api.cohere.ai/compatibility/v1")
}

/// Create a Cohere chat model.
pub fn cohere_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(cohere_config(api_key, model), backend)
}

/// Create an OpenAI-compatible embeddings config for Cohere.
pub fn cohere_embeddings_config(api_key: impl Into<String>) -> OpenAiEmbeddingsConfig {
    OpenAiEmbeddingsConfig::new(api_key)
        .with_base_url("https://api.cohere.ai/compatibility/v1")
}

/// Create Cohere embeddings.
pub fn cohere_embeddings(
    api_key: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiEmbeddings {
    OpenAiEmbeddings::new(cohere_embeddings_config(api_key), backend)
}

// ---------------------------------------------------------------------------
// OpenRouter
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible config for OpenRouter.
pub fn openrouter_config(api_key: impl Into<String>, model: impl Into<String>) -> OpenAiConfig {
    OpenAiConfig::new(api_key, model).with_base_url("https://openrouter.ai/api/v1")
}

/// Create an OpenRouter chat model.
pub fn openrouter_chat_model(
    api_key: impl Into<String>,
    model: impl Into<String>,
    backend: Arc<dyn ProviderBackend>,
) -> OpenAiChatModel {
    OpenAiChatModel::new(openrouter_config(api_key, model), backend)
}
