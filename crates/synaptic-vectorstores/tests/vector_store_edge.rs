use std::sync::Arc;

use synaptic_embeddings::FakeEmbeddings;
use synaptic_vectorstores::{Document, InMemoryVectorStore, VectorStore};

#[tokio::test]
async fn add_and_delete_then_search_empty() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::default();

    let ids = store
        .add_documents(vec![Document::new("d1", "hello world")], &embeddings)
        .await
        .unwrap();

    store
        .delete(&ids.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .await
        .unwrap();

    let results = store
        .similarity_search("hello", 5, &embeddings)
        .await
        .unwrap();
    assert!(
        results.is_empty(),
        "store should be empty after deleting all documents"
    );
}

#[tokio::test]
async fn search_empty_store() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::default();

    let results = store
        .similarity_search("anything", 5, &embeddings)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn similarity_search_returns_at_most_k_results() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::default();

    let docs: Vec<Document> = (0..10)
        .map(|i| Document::new(format!("d{i}"), format!("document number {i}")))
        .collect();
    store.add_documents(docs, &embeddings).await.unwrap();

    let results = store
        .similarity_search("document", 3, &embeddings)
        .await
        .unwrap();
    assert_eq!(
        results.len(),
        3,
        "should return exactly k=3 results when store has more"
    );
}

#[tokio::test]
async fn from_texts_creates_searchable_store() {
    let embeddings = FakeEmbeddings::default();
    let store = InMemoryVectorStore::from_texts(
        vec![("id1", "hello world"), ("id2", "goodbye world")],
        &embeddings,
    )
    .await
    .unwrap();

    let results = store
        .similarity_search("hello", 2, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn search_with_scores_values_in_range() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(8);

    store
        .add_documents(
            vec![
                Document::new("d1", "rust programming language"),
                Document::new("d2", "python programming language"),
            ],
            &embeddings,
        )
        .await
        .unwrap();

    let results = store
        .similarity_search_with_score("rust", 2, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    // Cosine similarity of unit vectors is in [-1, 1], but for
    // FakeEmbeddings (non-negative components) scores will be >= 0.
    for (doc, score) in &results {
        assert!(
            *score >= -1.0 && *score <= 1.0,
            "score out of cosine range for doc '{}': {}",
            doc.id,
            score,
        );
    }

    // Results should be sorted descending by score
    assert!(
        results[0].1 >= results[1].1,
        "first result score ({}) should be >= second ({})",
        results[0].1,
        results[1].1,
    );
}

#[tokio::test]
async fn concurrent_add_documents() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings = Arc::new(FakeEmbeddings::default());

    let mut handles = Vec::new();
    for i in 0..5 {
        let s = store.clone();
        let e = embeddings.clone();
        handles.push(tokio::spawn(async move {
            s.add_documents(
                vec![Document::new(format!("d{i}"), format!("text {i}"))],
                e.as_ref(),
            )
            .await
            .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let results = store
        .similarity_search("text", 10, embeddings.as_ref())
        .await
        .unwrap();
    assert_eq!(
        results.len(),
        5,
        "all 5 concurrently added documents should be present"
    );
}
