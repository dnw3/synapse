use synaptic_openai::compat::*;

// ---------------------------------------------------------------------------
// Chat config base_url tests
// ---------------------------------------------------------------------------

#[test]
fn groq_config_sets_base_url() {
    let config = groq_config("key", "model");
    assert_eq!(config.base_url, "https://api.groq.com/openai/v1");
}

#[test]
fn deepseek_config_sets_base_url() {
    let config = deepseek_config("key", "model");
    assert_eq!(config.base_url, "https://api.deepseek.com/v1");
}

#[test]
fn fireworks_config_sets_base_url() {
    let config = fireworks_config("key", "model");
    assert_eq!(config.base_url, "https://api.fireworks.ai/inference/v1");
}

#[test]
fn together_config_sets_base_url() {
    let config = together_config("key", "model");
    assert_eq!(config.base_url, "https://api.together.xyz/v1");
}

#[test]
fn xai_config_sets_base_url() {
    let config = xai_config("key", "model");
    assert_eq!(config.base_url, "https://api.x.ai/v1");
}

#[test]
fn mistral_config_sets_base_url() {
    let config = mistral_config("key", "model");
    assert_eq!(config.base_url, "https://api.mistral.ai/v1");
}

#[test]
fn huggingface_config_sets_base_url() {
    let config = huggingface_config("key", "model");
    assert_eq!(config.base_url, "https://router.huggingface.co/v1");
}

#[test]
fn cohere_config_sets_base_url() {
    let config = cohere_config("key", "model");
    assert_eq!(config.base_url, "https://api.cohere.ai/compatibility/v1");
}

#[test]
fn openrouter_config_sets_base_url() {
    let config = openrouter_config("key", "model");
    assert_eq!(config.base_url, "https://openrouter.ai/api/v1");
}

// ---------------------------------------------------------------------------
// Config preserves api_key and model
// ---------------------------------------------------------------------------

#[test]
fn config_preserves_api_key_and_model() {
    let config = groq_config("my-key", "llama3-70b");
    assert_eq!(config.api_key, "my-key");
    assert_eq!(config.model, "llama3-70b");
}

// ---------------------------------------------------------------------------
// Embeddings config tests
// ---------------------------------------------------------------------------

#[test]
fn mistral_embeddings_config_sets_base_url() {
    let config = mistral_embeddings_config("key");
    assert_eq!(config.base_url, "https://api.mistral.ai/v1");
}

#[test]
fn huggingface_embeddings_config_sets_base_url() {
    let config = huggingface_embeddings_config("key");
    assert_eq!(config.base_url, "https://router.huggingface.co/v1");
}

#[test]
fn cohere_embeddings_config_sets_base_url() {
    let config = cohere_embeddings_config("key");
    assert_eq!(config.base_url, "https://api.cohere.ai/compatibility/v1");
}
