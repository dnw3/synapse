use std::sync::Arc;

use synaptic::core::Embeddings;
use synaptic::embeddings::FakeEmbeddings;
use synaptic::models::HttpBackend;
use synaptic::ollama::{OllamaEmbeddings, OllamaEmbeddingsConfig};
use synaptic::openai::{OpenAiEmbeddings, OpenAiEmbeddingsConfig};

use crate::config::MemoryConfig;

/// Build an embedding provider based on config.
///
/// Priority order depends on `embedding_provider` setting:
/// - "auto": OPENAI_API_KEY → MISTRAL_API_KEY → VOYAGE_API_KEY → JINA_API_KEY
///            → COHERE_API_KEY → NOMIC_API_KEY → Ollama → fake
/// - "openai" / "mistral" / "voyage" / "jina" / "cohere" / "nomic" / "ollama" / "fake"
pub fn build_embeddings(config: &MemoryConfig) -> (Arc<dyn Embeddings>, bool) {
    let provider = config.embedding_provider.as_str();

    match provider {
        "openai" => build_openai_embeddings(),
        "mistral" => build_mistral_embeddings(),
        "voyage" => build_voyage_embeddings(),
        "jina" => build_jina_embeddings(),
        "cohere" => build_cohere_embeddings(),
        "nomic" => build_nomic_embeddings(),
        "ollama" => build_ollama_embeddings(config),
        "fake" => (Arc::new(FakeEmbeddings::new(384)), false),
        _ => {
            // "auto" — try providers in priority order by env key availability
            for builder in [
                build_openai_embeddings as fn() -> (Arc<dyn Embeddings>, bool),
                build_mistral_embeddings,
                build_voyage_embeddings,
                build_jina_embeddings,
                build_cohere_embeddings,
                build_nomic_embeddings,
            ] {
                let (emb, real) = builder();
                if real {
                    return (emb, true);
                }
            }
            let (emb, real) = build_ollama_embeddings(config);
            if real {
                return (emb, true);
            }
            (Arc::new(FakeEmbeddings::new(384)), false)
        }
    }
}

fn build_openai_embeddings() -> (Arc<dyn Embeddings>, bool) {
    match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let cfg = OpenAiEmbeddingsConfig::new(&key);
            let backend = Arc::new(HttpBackend::new());
            (Arc::new(OpenAiEmbeddings::new(cfg, backend)), true)
        }
        _ => (Arc::new(FakeEmbeddings::new(384)), false),
    }
}

fn build_mistral_embeddings() -> (Arc<dyn Embeddings>, bool) {
    match std::env::var("MISTRAL_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let backend = Arc::new(HttpBackend::new());
            let emb = synaptic::openai::compat::mistral::embeddings(
                key,
                "mistral-embed",
                backend,
            );
            (Arc::new(emb), true)
        }
        _ => (Arc::new(FakeEmbeddings::new(384)), false),
    }
}

fn build_voyage_embeddings() -> (Arc<dyn Embeddings>, bool) {
    use synaptic::voyage::{VoyageConfig, VoyageEmbeddings, VoyageModel};
    match std::env::var("VOYAGE_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let cfg = VoyageConfig::new(key, VoyageModel::Voyage3);
            (Arc::new(VoyageEmbeddings::new(cfg)), true)
        }
        _ => (Arc::new(FakeEmbeddings::new(384)), false),
    }
}

fn build_jina_embeddings() -> (Arc<dyn Embeddings>, bool) {
    use synaptic::jina::{JinaConfig, JinaEmbeddingModel, JinaEmbeddings};
    match std::env::var("JINA_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let cfg = JinaConfig::new(key, JinaEmbeddingModel::JinaEmbeddingsV3);
            (Arc::new(JinaEmbeddings::new(cfg)), true)
        }
        _ => (Arc::new(FakeEmbeddings::new(384)), false),
    }
}

fn build_cohere_embeddings() -> (Arc<dyn Embeddings>, bool) {
    use synaptic::cohere::{CohereEmbeddings, CohereEmbeddingsConfig};
    match std::env::var("COHERE_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let cfg = CohereEmbeddingsConfig::new(key);
            (Arc::new(CohereEmbeddings::new(cfg)), true)
        }
        _ => (Arc::new(FakeEmbeddings::new(384)), false),
    }
}

fn build_nomic_embeddings() -> (Arc<dyn Embeddings>, bool) {
    use synaptic::nomic::{NomicConfig, NomicEmbeddings};
    match std::env::var("NOMIC_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let cfg = NomicConfig::new(key);
            (Arc::new(NomicEmbeddings::new(cfg)), true)
        }
        _ => (Arc::new(FakeEmbeddings::new(384)), false),
    }
}

fn build_ollama_embeddings(config: &MemoryConfig) -> (Arc<dyn Embeddings>, bool) {
    let url = &config.ollama_embedding_url;
    let model = &config.ollama_embedding_model;

    let cfg = OllamaEmbeddingsConfig::new(model).with_base_url(url);
    let backend = Arc::new(HttpBackend::new());
    (Arc::new(OllamaEmbeddings::new(cfg, backend)), true)
}
