use std::sync::Arc;

use synaptic_retrieval::{Document, InMemoryRetriever, ParentDocumentRetriever, Retriever};

fn simple_splitter(text: &str) -> Vec<String> {
    text.split(". ")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[tokio::test]
async fn add_and_retrieve_parent() {
    let parents = vec![
        Document::new(
            "parent-1",
            "Rust is fast. Rust is safe. Rust has zero-cost abstractions.",
        ),
        Document::new("parent-2", "Python is easy. Python is popular."),
    ];

    // Create child documents using the splitter
    let parent_retriever = ParentDocumentRetriever::new(
        Arc::new(InMemoryRetriever::new(vec![])), // placeholder
        simple_splitter,
    );
    let children = parent_retriever.add_documents(parents).await;

    // Now create a real retriever with the children
    let child_retriever = Arc::new(InMemoryRetriever::new(children));
    let parent_retriever = ParentDocumentRetriever::new(child_retriever, simple_splitter);

    // Re-add parents so parent_retriever has them in store
    let _ = parent_retriever
        .add_documents(vec![
            Document::new(
                "parent-1",
                "Rust is fast. Rust is safe. Rust has zero-cost abstractions.",
            ),
            Document::new("parent-2", "Python is easy. Python is popular."),
        ])
        .await;

    let results = parent_retriever.retrieve("Rust safe", 5).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "parent-1");
    // Should return the full parent, not a chunk
    assert!(results[0].content.contains("zero-cost abstractions"));
}

#[tokio::test]
async fn deduplication() {
    let parent_retriever =
        ParentDocumentRetriever::new(Arc::new(InMemoryRetriever::new(vec![])), simple_splitter);

    let parents = vec![Document::new(
        "doc-1",
        "Rust is fast. Rust is safe. Rust is concurrent.",
    )];

    let children = parent_retriever.add_documents(parents.clone()).await;

    // Build a new retriever with children that would match multiple chunks of same parent
    let child_retriever = Arc::new(InMemoryRetriever::new(children));
    let parent_retriever2 = ParentDocumentRetriever::new(child_retriever, simple_splitter);
    let _ = parent_retriever2.add_documents(parents).await;

    // "Rust" matches multiple child chunks, but parent should appear only once
    let results = parent_retriever2.retrieve("Rust", 5).await.unwrap();
    let unique_ids: std::collections::HashSet<_> = results.iter().map(|d| &d.id).collect();
    assert_eq!(
        unique_ids.len(),
        results.len(),
        "results should be deduplicated"
    );
}

#[tokio::test]
async fn empty_results() {
    let parent_retriever =
        ParentDocumentRetriever::new(Arc::new(InMemoryRetriever::new(vec![])), simple_splitter);

    let _ = parent_retriever
        .add_documents(vec![Document::new("doc-1", "hello world")])
        .await;

    let results = parent_retriever.retrieve("xyznotfound", 5).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn multiple_parents_returned() {
    let parents = vec![
        Document::new("p1", "Rust programming language. Systems programming."),
        Document::new("p2", "Go programming language. Concurrent programming."),
    ];

    let parent_retriever =
        ParentDocumentRetriever::new(Arc::new(InMemoryRetriever::new(vec![])), simple_splitter);
    let children = parent_retriever.add_documents(parents.clone()).await;

    let child_retriever = Arc::new(InMemoryRetriever::new(children));
    let parent_retriever2 = ParentDocumentRetriever::new(child_retriever, simple_splitter);
    let _ = parent_retriever2.add_documents(parents).await;

    let results = parent_retriever2
        .retrieve("programming language", 5)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}
