use std::collections::HashMap;

use async_trait::async_trait;
use synaptic_core::SynapticError;

use crate::{tokenize_to_vec, Document, Retriever};

/// BM25 (Best Matching 25) retriever using Okapi BM25 scoring.
///
/// Pre-computes term frequencies, document lengths, and inverse document
/// frequencies at construction time for efficient retrieval.
#[derive(Debug, Clone)]
pub struct BM25Retriever {
    documents: Vec<Document>,
    /// Term frequency per document: doc_term_freqs[doc_index][term] = count
    doc_term_freqs: Vec<HashMap<String, usize>>,
    /// Token count per document
    doc_lengths: Vec<usize>,
    /// Average document length across the corpus
    avg_doc_length: f64,
    /// Number of documents containing each term
    doc_freq: HashMap<String, usize>,
    /// Term saturation parameter (default 1.5)
    k1: f64,
    /// Length normalization parameter (default 0.75)
    b: f64,
}

impl BM25Retriever {
    /// Create a new BM25Retriever with default parameters (k1=1.5, b=0.75).
    pub fn new(documents: Vec<Document>) -> Self {
        Self::with_params(documents, 1.5, 0.75)
    }

    /// Create a new BM25Retriever with custom k1 and b parameters.
    pub fn with_params(documents: Vec<Document>, k1: f64, b: f64) -> Self {
        let mut doc_term_freqs = Vec::with_capacity(documents.len());
        let mut doc_lengths = Vec::with_capacity(documents.len());
        let mut doc_freq: HashMap<String, usize> = HashMap::new();

        for doc in &documents {
            let tokens = tokenize_to_vec(&doc.content);
            let mut term_freq: HashMap<String, usize> = HashMap::new();

            for token in &tokens {
                *term_freq.entry(token.clone()).or_insert(0) += 1;
            }

            // Each unique term in this doc increments its document frequency
            for term in term_freq.keys() {
                *doc_freq.entry(term.clone()).or_insert(0) += 1;
            }

            doc_term_freqs.push(term_freq);
            doc_lengths.push(tokens.len());
        }

        let avg_doc_length = if documents.is_empty() {
            0.0
        } else {
            doc_lengths.iter().sum::<usize>() as f64 / documents.len() as f64
        };

        Self {
            documents,
            doc_term_freqs,
            doc_lengths,
            avg_doc_length,
            doc_freq,
            k1,
            b,
        }
    }

    /// Compute BM25 score for a single document given query terms.
    fn score(&self, doc_idx: usize, query_terms: &[String]) -> f64 {
        let n = self.documents.len() as f64;
        let doc_len = self.doc_lengths[doc_idx] as f64;
        let term_freqs = &self.doc_term_freqs[doc_idx];

        let mut score = 0.0;

        for term in query_terms {
            let tf = *term_freqs.get(term).unwrap_or(&0) as f64;
            let df = *self.doc_freq.get(term).unwrap_or(&0) as f64;

            if df == 0.0 || tf == 0.0 {
                continue;
            }

            // IDF: ln((N - df + 0.5) / (df + 0.5) + 1)
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

            // BM25 term score: idf * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl / avgdl))
            let numerator = tf * (self.k1 + 1.0);
            let denominator =
                tf + self.k1 * (1.0 - self.b + self.b * doc_len / self.avg_doc_length);

            score += idf * numerator / denominator;
        }

        score
    }
}

#[async_trait]
impl Retriever for BM25Retriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapticError> {
        let query_terms = tokenize_to_vec(query);

        if query_terms.is_empty() {
            return Ok(vec![]);
        }

        let mut scored: Vec<(f64, usize)> = self
            .documents
            .iter()
            .enumerate()
            .map(|(idx, _)| (self.score(idx, &query_terms), idx))
            .filter(|(score, _)| *score > 0.0)
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored
            .into_iter()
            .take(top_k)
            .map(|(_, idx)| self.documents[idx].clone())
            .collect())
    }
}
