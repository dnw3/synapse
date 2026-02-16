mod csv_loader;
mod directory_loader;
mod json_loader;
mod text_loader;

pub use csv_loader::CsvLoader;
pub use directory_loader::DirectoryLoader;
pub use json_loader::JsonLoader;
pub use text_loader::TextLoader;

use async_trait::async_trait;
use synapse_core::SynapseError;
use synapse_retrieval::Document;

/// Trait for loading documents from various sources.
#[async_trait]
pub trait Loader: Send + Sync {
    /// Load all documents from this source.
    async fn load(&self) -> Result<Vec<Document>, SynapseError>;
}
