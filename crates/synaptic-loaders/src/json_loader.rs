use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;
use synaptic_retrieval::Document;

use crate::Loader;

/// Loads documents from a JSON string.
///
/// - If the JSON is an array of objects, each object becomes a Document.
///   The `content_key` field is used to extract the document content (default: "content").
///   The `id_key` field is used to extract the document id (default: "id").
/// - If the JSON is a single object, it becomes one Document.
pub struct JsonLoader {
    json: String,
    content_key: String,
    id_key: String,
}

impl JsonLoader {
    pub fn new(json: impl Into<String>) -> Self {
        Self {
            json: json.into(),
            content_key: "content".to_string(),
            id_key: "id".to_string(),
        }
    }

    pub fn with_content_key(mut self, key: impl Into<String>) -> Self {
        self.content_key = key.into();
        self
    }

    pub fn with_id_key(mut self, key: impl Into<String>) -> Self {
        self.id_key = key.into();
        self
    }
}

#[async_trait]
impl Loader for JsonLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let value: Value = serde_json::from_str(&self.json)
            .map_err(|e| SynapticError::Loader(format!("invalid JSON: {e}")))?;

        match value {
            Value::Array(arr) => {
                let mut docs = Vec::with_capacity(arr.len());
                for (i, item) in arr.iter().enumerate() {
                    let id = item
                        .get(&self.id_key)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("doc-{i}"));
                    let content = item
                        .get(&self.content_key)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| item.to_string());
                    docs.push(Document::new(id, content));
                }
                Ok(docs)
            }
            _ => {
                let content = value
                    .get(&self.content_key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| value.to_string());
                let id = value
                    .get(&self.id_key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "doc-0".to_string());
                Ok(vec![Document::new(id, content)])
            }
        }
    }
}
