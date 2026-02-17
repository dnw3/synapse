mod csv_loader;
mod directory_loader;
mod file_loader;
mod json_loader;
mod markdown_loader;
mod text_loader;
mod web_loader;

pub use csv_loader::CsvLoader;
pub use directory_loader::DirectoryLoader;
pub use file_loader::FileLoader;
pub use json_loader::JsonLoader;
pub use markdown_loader::MarkdownLoader;
pub use text_loader::TextLoader;
pub use web_loader::WebBaseLoader;

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use synapse_core::SynapseError;
use synapse_retrieval::Document;

/// Trait for loading documents from various sources.
#[async_trait]
pub trait Loader: Send + Sync {
    /// Load all documents from this source.
    async fn load(&self) -> Result<Vec<Document>, SynapseError>;

    /// Stream documents lazily. Default implementation wraps load().
    fn lazy_load(&self) -> Pin<Box<dyn Stream<Item = Result<Document, SynapseError>> + Send + '_>> {
        Box::pin(async_stream::stream! {
            match self.load().await {
                Ok(docs) => {
                    for doc in docs {
                        yield Ok(doc);
                    }
                }
                Err(e) => yield Err(e),
            }
        })
    }
}
