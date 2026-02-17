use std::sync::Arc;

use synaptic_core::{ChatResponse, Message, SynapseError};
use synaptic_models::ScriptedChatModel;
use synaptic_retrieval::{Document, InMemoryRetriever, MultiQueryRetriever, Retriever};

#[tokio::test]
async fn multi_query_deduplicates_results() -> Result<(), SynapseError> {
    // Base retriever with documents
    let base = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async tokio runtime"),
        Document::new("2", "python machine learning"),
        Document::new("3", "rust ownership borrowing"),
    ]));

    // ScriptedChatModel returns alternative queries that will overlap in results
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("rust programming\nasync runtime"),
        usage: None,
    }]));

    let retriever = MultiQueryRetriever::new(base, model);
    let results = retriever.retrieve("rust async", 10).await?;

    // Results should be deduplicated by doc.id
    let ids: Vec<&str> = results.iter().map(|d| d.id.as_str()).collect();
    let unique_count = ids.len();
    let deduped: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        unique_count,
        deduped.len(),
        "results should be deduplicated"
    );

    Ok(())
}

#[tokio::test]
async fn multi_query_includes_original_query() -> Result<(), SynapseError> {
    // Base retriever where only the original query "tokio" finds doc 1
    let base = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "tokio async runtime"),
        Document::new("2", "python flask web"),
    ]));

    // The model generates queries that don't match any docs
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("something unrelated\nanother unrelated"),
        usage: None,
    }]));

    let retriever = MultiQueryRetriever::new(base, model);
    let results = retriever.retrieve("tokio", 10).await?;

    // The original query "tokio" should still find doc 1
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "1");

    Ok(())
}

#[tokio::test]
async fn multi_query_with_num_queries() -> Result<(), SynapseError> {
    let base = Arc::new(InMemoryRetriever::new(vec![Document::new(
        "1",
        "rust programming language",
    )]));

    // Model generates only 1 query (matching num_queries=1)
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("rust language"),
        usage: None,
    }]));

    let retriever = MultiQueryRetriever::with_num_queries(base, model, 1);
    let results = retriever.retrieve("rust", 10).await?;

    assert!(!results.is_empty());
    assert_eq!(results[0].id, "1");

    Ok(())
}

#[tokio::test]
async fn multi_query_respects_top_k() -> Result<(), SynapseError> {
    let base = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async"),
        Document::new("2", "rust ownership"),
        Document::new("3", "rust lifetimes"),
    ]));

    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("rust programming\nrust language"),
        usage: None,
    }]));

    let retriever = MultiQueryRetriever::new(base, model);
    let results = retriever.retrieve("rust", 2).await?;

    assert_eq!(results.len(), 2);

    Ok(())
}
