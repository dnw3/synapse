use synaptic_core::SynapseError;
use synaptic_retrieval::{BM25Retriever, Document, Retriever};

#[tokio::test]
async fn bm25_ranks_by_term_relevance() -> Result<(), SynapseError> {
    let docs = vec![
        Document::new("1", "rust async runtime tokio"),
        Document::new("2", "python machine learning"),
        Document::new("3", "rust ownership borrowing lifetimes"),
    ];

    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("rust", 3).await?;

    // Both rust docs should be returned
    assert_eq!(results.len(), 2);
    let ids: Vec<&str> = results.iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"1"));
    assert!(ids.contains(&"3"));
    Ok(())
}

#[tokio::test]
async fn bm25_respects_top_k() -> Result<(), SynapseError> {
    let docs = vec![
        Document::new("1", "rust async runtime"),
        Document::new("2", "rust ownership"),
        Document::new("3", "rust lifetimes"),
    ];

    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("rust", 2).await?;

    assert_eq!(results.len(), 2);
    Ok(())
}

#[tokio::test]
async fn bm25_empty_query_returns_empty() -> Result<(), SynapseError> {
    let docs = vec![
        Document::new("1", "rust async runtime"),
        Document::new("2", "python machine learning"),
    ];

    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("", 10).await?;

    assert!(results.is_empty());
    Ok(())
}

#[tokio::test]
async fn bm25_no_matching_terms() -> Result<(), SynapseError> {
    let docs = vec![
        Document::new("1", "rust async runtime"),
        Document::new("2", "python machine learning"),
    ];

    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("javascript", 10).await?;

    assert!(results.is_empty());
    Ok(())
}

#[tokio::test]
async fn bm25_with_params_custom_k1_b() -> Result<(), SynapseError> {
    let docs = vec![
        Document::new("1", "rust rust rust async"),
        Document::new("2", "rust programming"),
    ];

    // High k1 = more weight on term frequency
    let retriever = BM25Retriever::with_params(docs, 2.0, 0.5);
    let results = retriever.retrieve("rust", 2).await?;

    assert_eq!(results.len(), 2);
    // Doc 1 has "rust" 3 times, should rank higher with high k1
    assert_eq!(results[0].id, "1");
    Ok(())
}

#[tokio::test]
async fn bm25_prefers_rare_terms() -> Result<(), SynapseError> {
    // "tokio" appears in only doc 1, while "the" appears in all docs.
    // BM25 should rank doc 1 higher for query "tokio" due to higher IDF.
    let docs = vec![
        Document::new("1", "the rust tokio runtime"),
        Document::new("2", "the python framework"),
        Document::new("3", "the java library"),
    ];

    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("tokio", 3).await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "1");
    Ok(())
}

#[tokio::test]
async fn bm25_empty_corpus() -> Result<(), SynapseError> {
    let retriever = BM25Retriever::new(vec![]);
    let results = retriever.retrieve("anything", 10).await?;

    assert!(results.is_empty());
    Ok(())
}

#[tokio::test]
async fn bm25_multi_term_query_scores_combine() -> Result<(), SynapseError> {
    let docs = vec![
        Document::new("1", "rust async tokio runtime"),
        Document::new("2", "rust programming language"),
        Document::new("3", "async javascript promises"),
    ];

    let retriever = BM25Retriever::new(docs);
    // "rust async" should favor doc 1 which has both terms
    let results = retriever.retrieve("rust async", 3).await?;

    assert!(!results.is_empty());
    assert_eq!(results[0].id, "1");
    Ok(())
}
