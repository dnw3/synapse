use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;

// Re-export Store trait and Item from core
pub use synaptic_core::{Embeddings, Item, Store, SynapticError};

fn namespace_key(namespace: &[&str]) -> String {
    namespace.join("::")
}

fn now_iso() -> String {
    // Simple timestamp without external chrono dependency
    format!("{:?}", std::time::SystemTime::now())
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Thread-safe in-memory implementation of `Store`.
///
/// Supports optional embedding-based semantic search via [`with_embeddings`](InMemoryStore::with_embeddings).
pub struct InMemoryStore {
    data: Arc<RwLock<HashMap<String, HashMap<String, Item>>>>,
    /// Optional embeddings model for semantic search.
    embeddings: Option<Arc<dyn Embeddings>>,
    /// Pre-computed embedding vectors, keyed by `namespace_key::item_key`.
    vectors: Arc<RwLock<HashMap<String, Vec<f32>>>>,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            embeddings: None,
            vectors: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable embedding-based semantic search.
    ///
    /// When configured, [`Store::search()`] with a query will use embedding
    /// similarity instead of substring matching. Items are ranked by cosine
    /// similarity and `Item::score` is populated.
    pub fn with_embeddings(mut self, embeddings: Arc<dyn Embeddings>) -> Self {
        self.embeddings = Some(embeddings);
        self
    }
}

#[async_trait]
impl Store for InMemoryStore {
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError> {
        let data = self.data.read().await;
        let ns_key = namespace_key(namespace);
        Ok(data.get(&ns_key).and_then(|ns| ns.get(key).cloned()))
    }

    async fn search(
        &self,
        namespace: &[&str],
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Item>, SynapticError> {
        let data = self.data.read().await;
        let ns_key = namespace_key(namespace);

        let Some(ns) = data.get(&ns_key) else {
            return Ok(vec![]);
        };

        // If embeddings are configured and a query is provided, use semantic search
        if let (Some(ref embeddings), Some(q)) = (&self.embeddings, query) {
            let query_vec = embeddings.embed_query(q).await?;
            let vectors = self.vectors.read().await;

            let mut scored: Vec<(Item, f64)> = ns
                .iter()
                .map(|(key, item)| {
                    let vec_key = format!("{}::{}", ns_key, key);
                    let score = vectors
                        .get(&vec_key)
                        .map(|v| cosine_similarity(v, &query_vec))
                        .unwrap_or(0.0);
                    let mut item = item.clone();
                    item.score = Some(score);
                    (item, score)
                })
                .collect();

            // Sort by score descending
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);

            return Ok(scored.into_iter().map(|(item, _)| item).collect());
        }

        // Fallback: substring search
        let items: Vec<Item> = ns
            .values()
            .filter(|item| {
                if let Some(q) = query {
                    // Simple substring search in key and value
                    item.key.contains(q) || item.value.to_string().contains(q)
                } else {
                    true
                }
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(items)
    }

    async fn put(&self, namespace: &[&str], key: &str, value: Value) -> Result<(), SynapticError> {
        let mut data = self.data.write().await;
        let ns_key = namespace_key(namespace);
        let ns = data.entry(ns_key.clone()).or_default();
        let now = now_iso();

        let item = if let Some(existing) = ns.get(key) {
            Item {
                namespace: namespace.iter().map(|s| s.to_string()).collect(),
                key: key.to_string(),
                value: value.clone(),
                created_at: existing.created_at.clone(),
                updated_at: now,
                score: None,
            }
        } else {
            Item {
                namespace: namespace.iter().map(|s| s.to_string()).collect(),
                key: key.to_string(),
                value: value.clone(),
                created_at: now.clone(),
                updated_at: now,
                score: None,
            }
        };

        ns.insert(key.to_string(), item);

        // If embeddings are configured, compute and store the embedding
        if let Some(ref embeddings) = self.embeddings {
            let text = value.as_str().unwrap_or(&value.to_string()).to_string();
            let vecs = embeddings.embed_documents(&[&text]).await?;
            if let Some(vec) = vecs.into_iter().next() {
                let vec_key = format!("{}::{}", ns_key, key);
                self.vectors.write().await.insert(vec_key, vec);
            }
        }

        Ok(())
    }

    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError> {
        let mut data = self.data.write().await;
        let ns_key = namespace_key(namespace);
        if let Some(ns) = data.get_mut(&ns_key) {
            ns.remove(key);
        }
        // Clean up embedding vector
        let vec_key = format!("{}::{}", ns_key, key);
        self.vectors.write().await.remove(&vec_key);
        Ok(())
    }

    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError> {
        let data = self.data.read().await;
        let prefix_str = if prefix.is_empty() {
            String::new()
        } else {
            namespace_key(prefix)
        };

        let namespaces: Vec<Vec<String>> = data
            .keys()
            .filter(|k| prefix.is_empty() || k.starts_with(&prefix_str))
            .map(|k| k.split("::").map(String::from).collect())
            .collect();

        Ok(namespaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn put_and_get() {
        let store = InMemoryStore::new();
        store
            .put(&["users", "prefs"], "theme", json!("dark"))
            .await
            .unwrap();

        let item = store
            .get(&["users", "prefs"], "theme")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(item.key, "theme");
        assert_eq!(item.value, json!("dark"));
        assert_eq!(item.namespace, vec!["users", "prefs"]);
    }

    #[tokio::test]
    async fn get_nonexistent() {
        let store = InMemoryStore::new();
        let item = store.get(&["a"], "missing").await.unwrap();
        assert!(item.is_none());
    }

    #[tokio::test]
    async fn delete_item() {
        let store = InMemoryStore::new();
        store.put(&["ns"], "k", json!(1)).await.unwrap();
        store.delete(&["ns"], "k").await.unwrap();
        assert!(store.get(&["ns"], "k").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn search_items() {
        let store = InMemoryStore::new();
        store.put(&["ns"], "a", json!("apple")).await.unwrap();
        store.put(&["ns"], "b", json!("banana")).await.unwrap();
        store.put(&["ns"], "c", json!("cherry")).await.unwrap();

        let all = store.search(&["ns"], None, 10).await.unwrap();
        assert_eq!(all.len(), 3);

        let filtered = store.search(&["ns"], Some("apple"), 10).await.unwrap();
        assert_eq!(filtered.len(), 1);
    }

    #[tokio::test]
    async fn list_namespaces_with_prefix() {
        let store = InMemoryStore::new();
        store.put(&["a", "b"], "k1", json!(1)).await.unwrap();
        store.put(&["a", "c"], "k2", json!(2)).await.unwrap();
        store.put(&["x", "y"], "k3", json!(3)).await.unwrap();

        let all = store.list_namespaces(&[]).await.unwrap();
        assert_eq!(all.len(), 3);

        let filtered = store.list_namespaces(&["a"]).await.unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[tokio::test]
    async fn upsert_preserves_created_at() {
        let store = InMemoryStore::new();
        store.put(&["ns"], "k", json!(1)).await.unwrap();
        let first = store.get(&["ns"], "k").await.unwrap().unwrap();

        store.put(&["ns"], "k", json!(2)).await.unwrap();
        let second = store.get(&["ns"], "k").await.unwrap().unwrap();

        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.value, json!(2));
    }

    /// Simple deterministic embeddings for testing semantic search.
    struct TestEmbeddings;

    #[async_trait]
    impl Embeddings for TestEmbeddings {
        async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
            Ok(texts.iter().map(|t| text_to_vec(t)).collect())
        }

        async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
            Ok(text_to_vec(text))
        }
    }

    /// Simple deterministic vector: sum of byte values in 4 dimensions.
    fn text_to_vec(text: &str) -> Vec<f32> {
        let bytes = text.as_bytes();
        let mut v = vec![0.0f32; 4];
        for (i, b) in bytes.iter().enumerate() {
            v[i % 4] += *b as f32;
        }
        v
    }

    #[tokio::test]
    async fn semantic_search_ranked_by_similarity() {
        let store = InMemoryStore::new().with_embeddings(Arc::new(TestEmbeddings));

        store
            .put(&["docs"], "a", json!("rust programming"))
            .await
            .unwrap();
        store
            .put(&["docs"], "b", json!("python programming"))
            .await
            .unwrap();
        store
            .put(&["docs"], "c", json!("cooking recipes"))
            .await
            .unwrap();

        // Search for "rust" — should rank "rust programming" highest
        let results = store.search(&["docs"], Some("rust"), 10).await.unwrap();
        assert_eq!(results.len(), 3);

        // All should have scores populated
        for item in &results {
            assert!(item.score.is_some());
        }

        // Scores should be descending
        let scores: Vec<f64> = results.iter().map(|i| i.score.unwrap()).collect();
        for w in scores.windows(2) {
            assert!(w[0] >= w[1], "scores not sorted: {:?}", scores);
        }
    }

    #[tokio::test]
    async fn semantic_search_respects_limit() {
        let store = InMemoryStore::new().with_embeddings(Arc::new(TestEmbeddings));

        for i in 0..5 {
            store
                .put(&["ns"], &format!("k{}", i), json!(format!("item {}", i)))
                .await
                .unwrap();
        }

        let results = store.search(&["ns"], Some("item"), 2).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn delete_cleans_up_embeddings() {
        let store = InMemoryStore::new().with_embeddings(Arc::new(TestEmbeddings));

        store.put(&["ns"], "k", json!("hello")).await.unwrap();
        assert!(!store.vectors.read().await.is_empty());

        store.delete(&["ns"], "k").await.unwrap();
        assert!(store.vectors.read().await.is_empty());
    }
}
