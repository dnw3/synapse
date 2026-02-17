use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapseError};

use crate::{Document, Retriever};

/// A retriever that generates multiple query variants using a ChatModel,
/// runs each through a base retriever, and deduplicates results by document id.
pub struct MultiQueryRetriever {
    base: Arc<dyn Retriever>,
    model: Arc<dyn ChatModel>,
    num_queries: usize,
}

impl MultiQueryRetriever {
    /// Create a new MultiQueryRetriever with default num_queries (3).
    pub fn new(base: Arc<dyn Retriever>, model: Arc<dyn ChatModel>) -> Self {
        Self {
            base,
            model,
            num_queries: 3,
        }
    }

    /// Create a new MultiQueryRetriever with a custom number of query variants.
    pub fn with_num_queries(
        base: Arc<dyn Retriever>,
        model: Arc<dyn ChatModel>,
        num_queries: usize,
    ) -> Self {
        Self {
            base,
            model,
            num_queries,
        }
    }

    /// Generate alternative query variants using the ChatModel.
    async fn generate_queries(&self, query: &str) -> Result<Vec<String>, SynapseError> {
        let prompt = format!(
            "You are an AI language model assistant. Your task is to generate {} \
             different versions of the given user question to retrieve relevant documents \
             from a vector database. By generating multiple perspectives on the user question, \
             your goal is to help the user overcome some of the limitations of distance-based \
             similarity search. Provide these alternative questions separated by newlines. \
             Only output the questions, nothing else.\n\nOriginal question: {}",
            self.num_queries, query
        );

        let request = ChatRequest::new(vec![Message::human(prompt)]);
        let response = self.model.chat(request).await?;
        let content = response.message.content().to_string();

        let queries: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(queries)
    }
}

#[async_trait]
impl Retriever for MultiQueryRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        // Generate alternative queries
        let alternative_queries = self.generate_queries(query).await?;

        // Collect all queries: original + alternatives
        let mut all_queries = vec![query.to_string()];
        all_queries.extend(alternative_queries);

        // Run each query through the base retriever and deduplicate
        let mut seen_ids = HashSet::new();
        let mut results = Vec::new();

        for q in &all_queries {
            let docs = self.base.retrieve(q, top_k).await?;
            for doc in docs {
                if seen_ids.insert(doc.id.clone()) {
                    results.push(doc);
                }
            }
        }

        // Return up to top_k deduplicated results
        results.truncate(top_k);
        Ok(results)
    }
}
