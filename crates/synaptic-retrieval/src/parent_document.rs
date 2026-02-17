use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::SynapseError;
use tokio::sync::RwLock;

use crate::{Document, Retriever};

type SplitterFn = Box<dyn Fn(&str) -> Vec<String> + Send + Sync>;

/// Splits parent documents into children, stores both, and returns parent
/// documents when child chunks match a query.
///
/// Accepts a splitting function `Fn(&str) -> Vec<String>` to avoid circular
/// dependencies on `synapse-splitters`.
pub struct ParentDocumentRetriever {
    child_retriever: Arc<dyn Retriever>,
    parent_docs: Arc<RwLock<HashMap<String, Document>>>,
    child_to_parent: Arc<RwLock<HashMap<String, String>>>,
    splitter: SplitterFn,
}

impl ParentDocumentRetriever {
    pub fn new(
        child_retriever: Arc<dyn Retriever>,
        splitter: impl Fn(&str) -> Vec<String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            child_retriever,
            parent_docs: Arc::new(RwLock::new(HashMap::new())),
            child_to_parent: Arc::new(RwLock::new(HashMap::new())),
            splitter: Box::new(splitter),
        }
    }

    /// Add parent documents: splits each into children and stores mappings.
    /// Returns the child documents for indexing into the child retriever.
    pub async fn add_documents(&self, parents: Vec<Document>) -> Vec<Document> {
        let mut parent_store = self.parent_docs.write().await;
        let mut mapping = self.child_to_parent.write().await;
        let mut children = Vec::new();

        for parent in parents {
            let chunks = (self.splitter)(&parent.content);
            parent_store.insert(parent.id.clone(), parent.clone());

            for (i, chunk) in chunks.into_iter().enumerate() {
                let child_id = format!("{}-child-{i}", parent.id);
                let mut metadata = parent.metadata.clone();
                metadata.insert(
                    "parent_id".to_string(),
                    serde_json::Value::String(parent.id.clone()),
                );
                metadata.insert(
                    "chunk_index".to_string(),
                    serde_json::Value::Number(i.into()),
                );

                mapping.insert(child_id.clone(), parent.id.clone());
                children.push(Document::with_metadata(child_id, chunk, metadata));
            }
        }

        children
    }
}

#[async_trait]
impl Retriever for ParentDocumentRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        // Query child retriever for matching chunks
        let child_results = self.child_retriever.retrieve(query, top_k * 3).await?;

        // Map back to parent documents, deduplicating
        let mapping = self.child_to_parent.read().await;
        let parent_store = self.parent_docs.read().await;

        let mut seen = std::collections::HashSet::new();
        let mut parents = Vec::new();

        for child in &child_results {
            if let Some(parent_id) = mapping.get(&child.id) {
                if seen.insert(parent_id.clone()) {
                    if let Some(parent) = parent_store.get(parent_id) {
                        parents.push(parent.clone());
                        if parents.len() >= top_k {
                            break;
                        }
                    }
                }
            }
        }

        Ok(parents)
    }
}
