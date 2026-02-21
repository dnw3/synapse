//! Cohere integration for Synaptic.
//!
//! This crate provides [`CohereReranker`], a reranker that uses the
//! [Cohere Rerank API](https://docs.cohere.com/reference/rerank) to
//! reorder documents by relevance to a query.
//!
//! When the `retrieval` feature is enabled, `CohereReranker` also implements
//! the [`DocumentCompressor`](synaptic_retrieval::DocumentCompressor) trait,
//! making it usable with
//! [`ContextualCompressionRetriever`](synaptic_retrieval::ContextualCompressionRetriever).
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_cohere::{CohereReranker, CohereRerankerConfig};
//! use synaptic_core::Document;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = CohereRerankerConfig::new("your-api-key")
//!     .with_top_n(3);
//! let reranker = CohereReranker::new(config);
//!
//! let docs = vec![
//!     Document::new("1", "Rust is a systems programming language"),
//!     Document::new("2", "Python is great for data science"),
//! ];
//!
//! let reranked = reranker.rerank("systems programming", docs, None).await?;
//! # Ok(())
//! # }
//! ```

mod reranker;

pub use reranker::{CohereReranker, CohereRerankerConfig};

// Re-export core types for convenience.
pub use synaptic_core::Document;
