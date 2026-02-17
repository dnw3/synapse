use std::sync::Arc;

use synaptic_core::SynapseError;
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::{
    ContextualCompressionRetriever, Document, DocumentCompressor, EmbeddingsFilter,
    InMemoryRetriever, Retriever,
};

#[tokio::test]
async fn embeddings_filter_keeps_similar_documents() -> Result<(), SynapseError> {
    let embeddings = Arc::new(FakeEmbeddings::new(4));

    // Use a low threshold so that similar-enough docs pass
    let filter = EmbeddingsFilter::new(embeddings, 0.0);

    let docs = vec![
        Document::new("1", "rust async programming"),
        Document::new("2", "rust tokio runtime"),
        Document::new("3", "completely different topic about cooking pasta"),
    ];

    let results = filter.compress_documents(docs, "rust programming").await?;

    // With threshold 0.0, all docs with non-negative similarity should pass
    assert!(!results.is_empty());

    Ok(())
}

#[tokio::test]
async fn embeddings_filter_high_threshold_filters_dissimilar() -> Result<(), SynapseError> {
    let embeddings = Arc::new(FakeEmbeddings::new(4));

    // Use a very high threshold to filter most docs
    let filter = EmbeddingsFilter::new(embeddings, 0.99);

    let docs = vec![
        Document::new("1", "rust async programming"),
        Document::new("2", "cooking italian pasta recipes"),
    ];

    let results = filter.compress_documents(docs, "rust programming").await?;

    // With very high threshold, at most the most similar doc passes
    // The exact result depends on FakeEmbeddings behavior, but the
    // dissimilar cooking doc is likely filtered out
    for doc in &results {
        // If any doc passes, it should be the rust one
        assert_ne!(
            doc.id, "2",
            "cooking doc should be filtered at high threshold"
        );
    }

    Ok(())
}

#[tokio::test]
async fn embeddings_filter_empty_documents() -> Result<(), SynapseError> {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let filter = EmbeddingsFilter::new(embeddings, 0.5);

    let results = filter.compress_documents(vec![], "query").await?;

    assert!(results.is_empty());

    Ok(())
}

#[tokio::test]
async fn embeddings_filter_identical_text_passes() -> Result<(), SynapseError> {
    let embeddings = Arc::new(FakeEmbeddings::new(4));

    // Threshold of 0.99 - identical text should have similarity of 1.0
    let filter = EmbeddingsFilter::new(embeddings, 0.99);

    let docs = vec![Document::new("1", "rust programming")];

    let results = filter.compress_documents(docs, "rust programming").await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "1");

    Ok(())
}

#[tokio::test]
async fn contextual_compression_retriever_filters_base_results() -> Result<(), SynapseError> {
    let base = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async programming language"),
        Document::new("2", "rust ownership borrowing"),
        Document::new("3", "cooking italian pasta"),
    ]));

    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let compressor = Arc::new(EmbeddingsFilter::new(embeddings, 0.0));

    let retriever = ContextualCompressionRetriever::new(
        base as Arc<dyn Retriever>,
        compressor as Arc<dyn DocumentCompressor>,
    );

    // Query "rust" should first retrieve rust docs from base, then filter through embeddings
    let results = retriever.retrieve("rust", 10).await?;

    // Base retriever finds docs with "rust", compressor filters by embedding similarity
    // With threshold 0.0, all retrieved docs should pass
    assert!(!results.is_empty());

    Ok(())
}

#[tokio::test]
async fn contextual_compression_retriever_preserves_order() -> Result<(), SynapseError> {
    let base = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async tokio"),
        Document::new("2", "rust ownership"),
    ]));

    let embeddings = Arc::new(FakeEmbeddings::new(4));
    // Low threshold so all docs pass
    let compressor = Arc::new(EmbeddingsFilter::new(embeddings, 0.0));

    let retriever = ContextualCompressionRetriever::new(
        base as Arc<dyn Retriever>,
        compressor as Arc<dyn DocumentCompressor>,
    );

    let results = retriever.retrieve("rust", 10).await?;

    // Order should be preserved from base retriever (filtered but not reordered)
    if results.len() >= 2 {
        assert_eq!(results[0].id, "1");
        assert_eq!(results[1].id, "2");
    }

    Ok(())
}

#[tokio::test]
async fn embeddings_filter_with_default_threshold() -> Result<(), SynapseError> {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let filter = EmbeddingsFilter::with_default_threshold(embeddings);

    // Identical text should pass the default 0.75 threshold
    let docs = vec![Document::new("1", "rust programming")];
    let results = filter.compress_documents(docs, "rust programming").await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "1");

    Ok(())
}
