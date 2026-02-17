use std::sync::Arc;

use synaptic_core::SynapseError;
use synaptic_retrieval::{Document, EnsembleRetriever, InMemoryRetriever, Retriever};

#[tokio::test]
async fn ensemble_combines_results_from_multiple_retrievers() -> Result<(), SynapseError> {
    let retriever1 = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async tokio"),
        Document::new("2", "rust ownership"),
    ]));

    let retriever2 = Arc::new(InMemoryRetriever::new(vec![
        Document::new("3", "rust lifetimes"),
        Document::new("1", "rust async tokio"),
    ]));

    let ensemble = EnsembleRetriever::new(vec![
        (retriever1 as Arc<dyn Retriever>, 1.0),
        (retriever2 as Arc<dyn Retriever>, 1.0),
    ]);

    let results = ensemble.retrieve("rust", 10).await?;

    // Should have 3 unique documents
    assert_eq!(results.len(), 3);
    let ids: Vec<&str> = results.iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"1"));
    assert!(ids.contains(&"2"));
    assert!(ids.contains(&"3"));

    Ok(())
}

#[tokio::test]
async fn ensemble_rrf_ranks_shared_docs_higher() -> Result<(), SynapseError> {
    // Doc "1" appears in both retrievers, so it should get a higher RRF score
    let retriever1 = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async"),
        Document::new("2", "rust ownership"),
    ]));

    let retriever2 = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async"),
        Document::new("3", "rust lifetimes"),
    ]));

    let ensemble = EnsembleRetriever::new(vec![
        (retriever1 as Arc<dyn Retriever>, 1.0),
        (retriever2 as Arc<dyn Retriever>, 1.0),
    ]);

    let results = ensemble.retrieve("rust", 3).await?;

    // Doc "1" should be first since it appears in both retrievers
    assert_eq!(results[0].id, "1");

    Ok(())
}

#[tokio::test]
async fn ensemble_respects_weights() -> Result<(), SynapseError> {
    // Retriever1 has doc "A" at rank 1, weight = 0.1
    // Retriever2 has doc "B" at rank 1, weight = 10.0
    // Doc B should rank higher due to much higher weight
    let retriever1 = Arc::new(InMemoryRetriever::new(vec![Document::new(
        "A",
        "rust tokio",
    )]));

    let retriever2 = Arc::new(InMemoryRetriever::new(vec![Document::new(
        "B",
        "rust async",
    )]));

    let ensemble = EnsembleRetriever::new(vec![
        (retriever1 as Arc<dyn Retriever>, 0.1),
        (retriever2 as Arc<dyn Retriever>, 10.0),
    ]);

    let results = ensemble.retrieve("rust", 2).await?;

    assert_eq!(results.len(), 2);
    // Doc B should rank first due to higher weight
    assert_eq!(results[0].id, "B");
    assert_eq!(results[1].id, "A");

    Ok(())
}

#[tokio::test]
async fn ensemble_respects_top_k() -> Result<(), SynapseError> {
    let retriever1 = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async"),
        Document::new("2", "rust ownership"),
    ]));

    let retriever2 = Arc::new(InMemoryRetriever::new(vec![
        Document::new("3", "rust lifetimes"),
        Document::new("4", "rust macros"),
    ]));

    let ensemble = EnsembleRetriever::new(vec![
        (retriever1 as Arc<dyn Retriever>, 1.0),
        (retriever2 as Arc<dyn Retriever>, 1.0),
    ]);

    let results = ensemble.retrieve("rust", 2).await?;

    assert_eq!(results.len(), 2);

    Ok(())
}

#[tokio::test]
async fn ensemble_single_retriever() -> Result<(), SynapseError> {
    let retriever = Arc::new(InMemoryRetriever::new(vec![
        Document::new("1", "rust async"),
        Document::new("2", "python web"),
    ]));

    let ensemble = EnsembleRetriever::new(vec![(retriever as Arc<dyn Retriever>, 1.0)]);

    let results = ensemble.retrieve("rust", 10).await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "1");

    Ok(())
}

#[tokio::test]
async fn ensemble_no_results() -> Result<(), SynapseError> {
    let retriever = Arc::new(InMemoryRetriever::new(vec![Document::new(
        "1",
        "python web",
    )]));

    let ensemble = EnsembleRetriever::new(vec![(retriever as Arc<dyn Retriever>, 1.0)]);

    let results = ensemble.retrieve("rust", 10).await?;

    assert!(results.is_empty());

    Ok(())
}
