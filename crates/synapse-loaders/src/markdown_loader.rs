use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use synapse_core::SynapseError;
use synapse_retrieval::Document;

use crate::Loader;

/// Loads a markdown file, preserving original content.
///
/// Reads the markdown file and returns it as a single Document.
/// Metadata includes `source` (the file path) and `format: "markdown"`.
pub struct MarkdownLoader {
    path: PathBuf,
}

impl MarkdownLoader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl Loader for MarkdownLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapseError> {
        let content = tokio::fs::read_to_string(&self.path).await.map_err(|e| {
            SynapseError::Loader(format!("cannot read {}: {e}", self.path.display()))
        })?;

        let id = self.path.to_string_lossy().to_string();

        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            Value::String(self.path.to_string_lossy().to_string()),
        );
        metadata.insert("format".to_string(), Value::String("markdown".to_string()));

        Ok(vec![Document::with_metadata(id, content, metadata)])
    }
}
