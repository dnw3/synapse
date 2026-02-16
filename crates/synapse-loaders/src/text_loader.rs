use async_trait::async_trait;
use synapse_core::SynapseError;
use synapse_retrieval::Document;

use crate::Loader;

/// Wraps a string of text into a single Document.
#[derive(Debug, Clone)]
pub struct TextLoader {
    id: String,
    content: String,
}

impl TextLoader {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
        }
    }
}

#[async_trait]
impl Loader for TextLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapseError> {
        Ok(vec![Document::new(self.id.clone(), self.content.clone())])
    }
}
