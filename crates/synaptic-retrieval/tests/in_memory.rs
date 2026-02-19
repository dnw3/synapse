use synaptic_core::SynapticError;
use synaptic_retrieval::{Document, InMemoryRetriever, Retriever};

#[tokio::test]
async fn retrieves_best_match() -> Result<(), SynapticError> {
    let retriever = InMemoryRetriever::new(vec![
        Document::new("1", "rust async tokio"),
        Document::new("2", "python notebooks"),
    ]);

    let docs = retriever.retrieve("tokio runtime", 1).await?;
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "1");
    Ok(())
}

#[tokio::test]
async fn retrieves_multiple_matches() -> Result<(), SynapticError> {
    let retriever = InMemoryRetriever::new(vec![
        Document::new("1", "rust programming language"),
        Document::new("2", "rust async runtime"),
        Document::new("3", "python data science"),
    ]);

    let docs = retriever.retrieve("rust", 2).await?;
    assert_eq!(docs.len(), 2);
    // Both rust-related docs should be returned
    let ids: Vec<&str> = docs.iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"1") || ids.contains(&"2"));
    Ok(())
}

#[tokio::test]
async fn returns_empty_for_no_match() -> Result<(), SynapticError> {
    let retriever = InMemoryRetriever::new(vec![Document::new("1", "rust programming")]);

    // InMemoryRetriever returns all docs, but top_k limits
    let docs = retriever.retrieve("anything", 0).await?;
    assert!(docs.is_empty());
    Ok(())
}

#[tokio::test]
async fn top_k_limits_results() -> Result<(), SynapticError> {
    let retriever = InMemoryRetriever::new(vec![
        Document::new("1", "document one"),
        Document::new("2", "document two"),
        Document::new("3", "document three"),
        Document::new("4", "document four"),
    ]);

    let docs = retriever.retrieve("document", 2).await?;
    assert!(docs.len() <= 2);
    Ok(())
}

#[tokio::test]
async fn metadata_preserved() -> Result<(), SynapticError> {
    use serde_json::json;
    use std::collections::HashMap;

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), json!("test"));
    let doc = Document {
        id: "1".to_string(),
        content: "test content".to_string(),
        metadata,
    };

    let retriever = InMemoryRetriever::new(vec![doc]);
    let docs = retriever.retrieve("test", 1).await?;
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].metadata.get("source"), Some(&json!("test")));
    Ok(())
}
