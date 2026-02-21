use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;
use crate::Document;

use crate::Loader;

/// Loads content from a file on disk.
///
/// Reads the file contents via `tokio::fs::read_to_string` and returns a single
/// Document with the file path as id and the file contents as content.
/// The `source` metadata key is set to the file path.
pub struct FileLoader {
    path: PathBuf,
}

impl FileLoader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl Loader for FileLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let content = tokio::fs::read_to_string(&self.path).await.map_err(|e| {
            SynapticError::Loader(format!("cannot read {}: {e}", self.path.display()))
        })?;

        let id = self.path.to_string_lossy().to_string();

        let mut metadata = HashMap::new();
        metadata.insert(
            "source".to_string(),
            Value::String(self.path.to_string_lossy().to_string()),
        );

        Ok(vec![Document::with_metadata(id, content, metadata)])
    }
}
