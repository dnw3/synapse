use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{Document, Embeddings, Retriever, SynapticError};
use tokio::sync::RwLock;

use crate::VectorStore;

/// A retriever that maps multiple child vectors back to parent documents.
///
/// Each parent document can have multiple sub-documents (e.g., summaries,
/// smaller chunks) stored in the vector store. Retrieval finds relevant
/// sub-documents, then returns the original parent documents.
pub struct MultiVectorRetriever<S: VectorStore> {
    vectorstore: Arc<S>,
    embeddings: Arc<dyn Embeddings>,
    /// Parent document store, keyed by document ID.
    docstore: Arc<RwLock<HashMap<String, Document>>>,
    /// Metadata key linking child documents to their parent.
    id_key: String,
    k: usize,
}

impl<S: VectorStore + 'static> MultiVectorRetriever<S> {
    /// Create a new `MultiVectorRetriever`.
    ///
    /// - `vectorstore`: the vector store to search for child documents.
    /// - `embeddings`: the embeddings provider for embedding child documents.
    /// - `k`: the number of child documents to retrieve for parent lookup.
    pub fn new(vectorstore: Arc<S>, embeddings: Arc<dyn Embeddings>, k: usize) -> Self {
        Self {
            vectorstore,
            embeddings,
            docstore: Arc::new(RwLock::new(HashMap::new())),
            id_key: "parent_id".to_string(),
            k,
        }
    }

    /// Set a custom metadata key linking child documents to their parent ID.
    /// Defaults to `"parent_id"`.
    pub fn with_id_key(mut self, key: impl Into<String>) -> Self {
        self.id_key = key.into();
        self
    }

    /// Add parent documents and their associated child documents.
    ///
    /// Parents are stored in the internal docstore. Children are embedded and
    /// added to the vector store. Each child document must have the `id_key`
    /// metadata field set to the parent document's ID.
    pub async fn add_documents(
        &self,
        parent_docs: Vec<Document>,
        child_docs: Vec<Document>,
    ) -> Result<(), SynapticError> {
        // Store parents in the docstore
        {
            let mut store = self.docstore.write().await;
            for doc in parent_docs {
                store.insert(doc.id.clone(), doc);
            }
        }

        // Add children to the vector store
        self.vectorstore
            .add_documents(child_docs, self.embeddings.as_ref())
            .await?;

        Ok(())
    }
}

#[async_trait]
impl<S: VectorStore + 'static> Retriever for MultiVectorRetriever<S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapticError> {
        let k = if top_k > 0 { top_k } else { self.k };

        // Search vectorstore for child documents
        let children = self
            .vectorstore
            .similarity_search(query, k, self.embeddings.as_ref())
            .await?;

        // Look up parent documents from the docstore, deduplicating
        let docstore = self.docstore.read().await;
        let mut seen = std::collections::HashSet::new();
        let mut parents = Vec::new();

        for child in &children {
            if let Some(parent_id_value) = child.metadata.get(&self.id_key) {
                if let Some(parent_id) = parent_id_value.as_str() {
                    if seen.insert(parent_id.to_string()) {
                        if let Some(parent) = docstore.get(parent_id) {
                            parents.push(parent.clone());
                        }
                    }
                }
            }
        }

        Ok(parents)
    }
}
