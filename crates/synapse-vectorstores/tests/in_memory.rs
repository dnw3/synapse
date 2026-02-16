use std::sync::Arc;
use synapse_embeddings::FakeEmbeddings;
use synapse_retrieval::{Document, Retriever};
use synapse_vectorstores::{InMemoryVectorStore, VectorStore, VectorStoreRetriever};

#[tokio::test]
async fn add_and_search() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let docs = vec![
        Document::new("1", "The cat sat on the mat"),
        Document::new("2", "The dog played in the park"),
        Document::new("3", "A fish swam in the ocean"),
    ];

    let ids = store.add_documents(docs, &embeddings).await.unwrap();
    assert_eq!(ids.len(), 3);

    let results = store
        .similarity_search("cat on mat", 2, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    // The most similar doc should be about the cat
    assert_eq!(results[0].id, "1");
}

#[tokio::test]
async fn search_with_scores() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    store
        .add_documents(
            vec![
                Document::new("a", "hello world"),
                Document::new("b", "goodbye world"),
            ],
            &embeddings,
        )
        .await
        .unwrap();

    let results = store
        .similarity_search_with_score("hello world", 2, &embeddings)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    // First result should have highest score
    assert!(results[0].1 >= results[1].1);
    // Exact match should have score close to 1.0
    assert!(results[0].1 > 0.9, "exact match score: {}", results[0].1);
}

#[tokio::test]
async fn delete_documents() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    store
        .add_documents(
            vec![Document::new("1", "first"), Document::new("2", "second")],
            &embeddings,
        )
        .await
        .unwrap();

    store.delete(&["1"]).await.unwrap();

    let results = store
        .similarity_search("first", 10, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "2");
}

#[tokio::test]
async fn empty_store_returns_empty() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let results = store
        .similarity_search("anything", 5, &embeddings)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn vector_store_retriever_bridge() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    store
        .add_documents(
            vec![
                Document::new("1", "rust programming"),
                Document::new("2", "python programming"),
                Document::new("3", "cooking recipes"),
            ],
            embeddings.as_ref(),
        )
        .await
        .unwrap();

    let retriever = VectorStoreRetriever::new(store, embeddings, 2);
    let results = retriever.retrieve("rust code", 2).await.unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn preserves_metadata() {
    use serde_json::Value;
    use std::collections::HashMap;

    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), Value::String("test.txt".to_string()));

    store
        .add_documents(
            vec![Document::with_metadata("1", "content", metadata)],
            &embeddings,
        )
        .await
        .unwrap();

    let results = store
        .similarity_search("content", 1, &embeddings)
        .await
        .unwrap();
    assert_eq!(results[0].metadata.get("source").unwrap(), "test.txt");
}
