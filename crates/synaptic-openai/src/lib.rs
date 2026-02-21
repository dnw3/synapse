pub(crate) mod chat_model;
pub(crate) mod embeddings;
pub mod compat;
mod azure;

pub use azure::{
    AzureOpenAiChatModel, AzureOpenAiConfig, AzureOpenAiEmbeddings, AzureOpenAiEmbeddingsConfig,
};
pub use chat_model::{OpenAiChatModel, OpenAiConfig};
pub use embeddings::{OpenAiEmbeddings, OpenAiEmbeddingsConfig};
