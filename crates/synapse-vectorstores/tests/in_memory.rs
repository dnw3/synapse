use std::collections::HashMap;
use std::sync::Arc;
use synapse_embeddings::{Embeddings, FakeEmbeddings};
use synapse_retrieval::{Document, Retriever};
use synapse_vectorstores::{
    InMemoryVectorStore, MultiVectorRetriever, VectorStore, VectorStoreRetriever,
};

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

// --- similarity_search_by_vector ---

#[tokio::test]
async fn search_by_vector() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let docs = vec![
        Document::new("1", "The cat sat on the mat"),
        Document::new("2", "The dog played in the park"),
        Document::new("3", "A fish swam in the ocean"),
    ];

    store.add_documents(docs, &embeddings).await.unwrap();

    // Get the query embedding manually
    let query_embedding = embeddings.embed_query("cat on mat").await.unwrap();

    let results = store
        .similarity_search_by_vector(&query_embedding, 2)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "1");
}

#[tokio::test]
async fn search_by_vector_empty_store() {
    let store = InMemoryVectorStore::new();

    let results = store
        .similarity_search_by_vector(&[0.1, 0.2, 0.3, 0.4], 5)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_by_vector_matches_text_search() {
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

    // Text-based search
    let text_results = store
        .similarity_search("hello world", 2, &embeddings)
        .await
        .unwrap();

    // Vector-based search with same query
    let query_vec = embeddings.embed_query("hello world").await.unwrap();
    let vec_results = store
        .similarity_search_by_vector(&query_vec, 2)
        .await
        .unwrap();

    // Both should return the same ordering
    assert_eq!(text_results[0].id, vec_results[0].id);
    assert_eq!(text_results[1].id, vec_results[1].id);
}

// --- MMR search ---

#[tokio::test]
async fn mmr_search_returns_k_results() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let docs = vec![
        Document::new("1", "The cat sat on the mat"),
        Document::new("2", "The cat played with yarn"),
        Document::new("3", "The dog ran in the park"),
        Document::new("4", "A fish swam in the ocean"),
    ];

    store.add_documents(docs, &embeddings).await.unwrap();

    let results = store
        .max_marginal_relevance_search("cat", 2, 4, 0.5, &embeddings)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn mmr_search_promotes_diversity() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let docs = vec![
        Document::new("cat1", "The cat sat on the mat"),
        Document::new("cat2", "The cat played with the string"),
        Document::new("cat3", "The cat slept on the couch"),
        Document::new("dog1", "The dog ran in the park"),
    ];

    store.add_documents(docs, &embeddings).await.unwrap();

    // With lambda=1.0 (pure relevance), top 2 should both be cat-related
    let pure_relevance = store
        .max_marginal_relevance_search("cat on mat", 2, 4, 1.0, &embeddings)
        .await
        .unwrap();

    // With lambda=0.0 (max diversity), should get more diverse results
    let diverse = store
        .max_marginal_relevance_search("cat on mat", 2, 4, 0.0, &embeddings)
        .await
        .unwrap();

    // Pure relevance should return same top result as standard similarity
    let standard = store
        .similarity_search("cat on mat", 1, &embeddings)
        .await
        .unwrap();
    assert_eq!(pure_relevance[0].id, standard[0].id);

    assert_eq!(pure_relevance.len(), 2);
    assert_eq!(diverse.len(), 2);

    // With max diversity, the second result should differ from pure relevance
    let diverse_ids: Vec<&str> = diverse.iter().map(|d| d.id.as_str()).collect();
    let relevance_ids: Vec<&str> = pure_relevance.iter().map(|d| d.id.as_str()).collect();
    assert_ne!(
        diverse_ids, relevance_ids,
        "diverse results should differ from pure relevance"
    );
}

#[tokio::test]
async fn mmr_search_empty_store() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let results = store
        .max_marginal_relevance_search("anything", 3, 10, 0.5, &embeddings)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn mmr_search_k_zero() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    store
        .add_documents(vec![Document::new("1", "hello")], &embeddings)
        .await
        .unwrap();

    let results = store
        .max_marginal_relevance_search("hello", 0, 10, 0.5, &embeddings)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn mmr_fetch_k_limits_candidates() {
    let store = InMemoryVectorStore::new();
    let embeddings = FakeEmbeddings::new(4);

    let docs = vec![
        Document::new("1", "alpha one"),
        Document::new("2", "alpha two"),
        Document::new("3", "alpha three"),
        Document::new("4", "alpha four"),
    ];

    store.add_documents(docs, &embeddings).await.unwrap();

    // fetch_k=2 means only 2 candidates, so can only return up to 2 even if k=4
    let results = store
        .max_marginal_relevance_search("alpha", 4, 2, 0.5, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

// --- from_texts / from_documents ---

#[tokio::test]
async fn from_texts_populates_store() {
    let embeddings = FakeEmbeddings::new(16);

    let store = InMemoryVectorStore::from_texts(
        vec![
            ("1", "rust programming language systems"),
            ("2", "python scripting language interpreted"),
            ("3", "cooking recipes food kitchen"),
        ],
        &embeddings,
    )
    .await
    .unwrap();

    let results = store
        .similarity_search("rust programming language systems", 3, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 3);
    // Exact match should be first
    assert_eq!(results[0].id, "1");
}

#[tokio::test]
async fn from_documents_populates_store() {
    let embeddings = FakeEmbeddings::new(16);

    let docs = vec![
        Document::new("a", "hello world greeting salutation"),
        Document::new("b", "goodbye farewell departure leaving"),
    ];

    let store = InMemoryVectorStore::from_documents(docs, &embeddings)
        .await
        .unwrap();

    let results = store
        .similarity_search("hello world greeting salutation", 2, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    // Exact match should be first
    assert_eq!(results[0].id, "a");
}

#[tokio::test]
async fn from_documents_preserves_metadata() {
    let embeddings = FakeEmbeddings::new(4);

    let mut metadata = HashMap::new();
    metadata.insert(
        "source".to_string(),
        serde_json::Value::String("file.txt".to_string()),
    );

    let docs = vec![Document::with_metadata("1", "test content", metadata)];

    let store = InMemoryVectorStore::from_documents(docs, &embeddings)
        .await
        .unwrap();

    let results = store
        .similarity_search("test", 1, &embeddings)
        .await
        .unwrap();
    assert_eq!(results[0].metadata.get("source").unwrap(), "file.txt");
}

// --- Score threshold ---

#[tokio::test]
async fn retriever_with_score_threshold() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(32));

    store
        .add_documents(
            vec![
                Document::new("1", "hello world"),
                Document::new("2", "hello world"), // exact duplicate to guarantee a high score match
                Document::new("3", "zzz yyy xxx completely unrelated gibberish qwerty"),
            ],
            embeddings.as_ref(),
        )
        .await
        .unwrap();

    // Without threshold: returns all up to k
    let retriever = VectorStoreRetriever::new(store.clone(), embeddings.clone(), 10);
    let results = retriever.retrieve("hello world", 10).await.unwrap();
    assert_eq!(results.len(), 3);

    // Check actual scores to pick an appropriate threshold
    let scored = store
        .similarity_search_with_score("hello world", 10, embeddings.as_ref())
        .await
        .unwrap();

    // The exact match should have score ~1.0, the unrelated one should be lower.
    // Use a threshold between the highest and lowest scores.
    let max_score = scored[0].1;
    let min_score = scored.last().unwrap().1;
    assert!(
        max_score > min_score,
        "scores should differ: max={max_score}, min={min_score}"
    );

    // Use a threshold that filters the lowest-scoring document
    let threshold = (max_score + min_score) / 2.0;
    let retriever = VectorStoreRetriever::new(store.clone(), embeddings.clone(), 10)
        .with_score_threshold(threshold);
    let results = retriever.retrieve("hello world", 10).await.unwrap();
    // The threshold should filter out the dissimilar document
    assert!(
        results.len() < 3,
        "threshold should filter some results, got {} with threshold {}",
        results.len(),
        threshold,
    );
}

#[tokio::test]
async fn retriever_threshold_zero_returns_all() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    store
        .add_documents(
            vec![Document::new("1", "alpha"), Document::new("2", "beta")],
            embeddings.as_ref(),
        )
        .await
        .unwrap();

    let retriever = VectorStoreRetriever::new(store, embeddings, 10).with_score_threshold(0.0);
    let results = retriever.retrieve("alpha", 10).await.unwrap();
    assert_eq!(results.len(), 2);
}

// --- MultiVectorRetriever ---

#[tokio::test]
async fn multi_vector_retriever_basic() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    let retriever = MultiVectorRetriever::new(store, embeddings, 5);

    let parents = vec![
        Document::new(
            "parent-1",
            "Rust is a systems programming language focused on safety and performance.",
        ),
        Document::new(
            "parent-2",
            "Python is a high-level interpreted programming language.",
        ),
    ];

    let children = vec![
        Document::with_metadata(
            "child-1a",
            "Rust safety",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("parent-1".to_string()),
            )]),
        ),
        Document::with_metadata(
            "child-1b",
            "Rust performance",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("parent-1".to_string()),
            )]),
        ),
        Document::with_metadata(
            "child-2a",
            "Python interpreted",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("parent-2".to_string()),
            )]),
        ),
    ];

    retriever.add_documents(parents, children).await.unwrap();

    let results = retriever.retrieve("Rust safety", 5).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "parent-1");
    assert!(results[0].content.contains("systems programming"));
}

#[tokio::test]
async fn multi_vector_retriever_deduplication() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    let retriever = MultiVectorRetriever::new(store, embeddings, 10);

    let parents = vec![Document::new("parent-1", "Full document about Rust.")];

    let children = vec![
        Document::with_metadata(
            "child-1",
            "Rust is fast",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("parent-1".to_string()),
            )]),
        ),
        Document::with_metadata(
            "child-2",
            "Rust is safe",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("parent-1".to_string()),
            )]),
        ),
        Document::with_metadata(
            "child-3",
            "Rust is concurrent",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("parent-1".to_string()),
            )]),
        ),
    ];

    retriever.add_documents(parents, children).await.unwrap();

    // Even though multiple children match, parent should appear only once
    let results = retriever.retrieve("Rust", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "parent-1");
}

#[tokio::test]
async fn multi_vector_retriever_custom_id_key() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    let retriever = MultiVectorRetriever::new(store, embeddings, 5).with_id_key("doc_id");

    let parents = vec![Document::new("p1", "Original document content.")];

    let children = vec![Document::with_metadata(
        "c1",
        "chunk of original",
        HashMap::from([(
            "doc_id".to_string(),
            serde_json::Value::String("p1".to_string()),
        )]),
    )];

    retriever.add_documents(parents, children).await.unwrap();

    let results = retriever.retrieve("chunk original", 5).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "p1");
}

#[tokio::test]
async fn multi_vector_retriever_empty() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    let retriever = MultiVectorRetriever::new(store, embeddings, 5);

    let results = retriever.retrieve("anything", 5).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn multi_vector_retriever_missing_parent() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    let retriever = MultiVectorRetriever::new(store, embeddings, 5);

    // Add children that reference a parent that doesn't exist in docstore
    let children = vec![Document::with_metadata(
        "orphan",
        "orphan child text",
        HashMap::from([(
            "parent_id".to_string(),
            serde_json::Value::String("nonexistent".to_string()),
        )]),
    )];

    retriever.add_documents(vec![], children).await.unwrap();

    let results = retriever.retrieve("orphan", 5).await.unwrap();
    assert!(
        results.is_empty(),
        "should not return anything for missing parent"
    );
}

#[tokio::test]
async fn multi_vector_retriever_multiple_parents() {
    let store = Arc::new(InMemoryVectorStore::new());
    let embeddings: Arc<dyn synapse_embeddings::Embeddings> = Arc::new(FakeEmbeddings::new(4));

    let retriever = MultiVectorRetriever::new(store, embeddings, 10);

    let parents = vec![
        Document::new("p1", "First parent about Rust programming."),
        Document::new("p2", "Second parent about Python scripting."),
    ];

    let children = vec![
        Document::with_metadata(
            "c1",
            "Rust programming language",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("p1".to_string()),
            )]),
        ),
        Document::with_metadata(
            "c2",
            "Python scripting language",
            HashMap::from([(
                "parent_id".to_string(),
                serde_json::Value::String("p2".to_string()),
            )]),
        ),
    ];

    retriever.add_documents(parents, children).await.unwrap();

    let results = retriever
        .retrieve("programming language", 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}
