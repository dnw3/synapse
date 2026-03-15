use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;
use synaptic::core::{
    ChatModel, ChatRequest, Document, Embeddings, Message, SearchOptions, Store, SynapticError,
    VectorStore,
};
use synaptic::embeddings::CacheBackedEmbeddings;
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::sqlite::SqliteVectorStore;
use synaptic::store::{FileStore, InMemoryStore};
use tokio::sync::RwLock;

use crate::config::MemoryConfig;

use super::embeddings::build_embeddings;
use super::keywords::extract_keywords;

/// Build the LTM persistence store based on configuration.
///
/// Supports "file" (default), "sqlite", "postgres", and "redis" backends.
/// Postgres and Redis require the corresponding cargo features to be enabled.
pub fn build_ltm_store(config: &MemoryConfig, base_dir: &Path) -> Arc<dyn Store> {
    match config.ltm_backend.as_str() {
        #[cfg(feature = "ltm-postgres")]
        "postgres" | "pg" => {
            let url = config
                .ltm_backend_url
                .as_deref()
                .unwrap_or("postgres://localhost/synapse");
            match synaptic::postgres::PgStore::new_blocking(url) {
                Ok(store) => {
                    tracing::info!(backend = "postgres", "LTM using PostgreSQL backend");
                    return Arc::new(store);
                }
                Err(e) => {
                    tracing::warn!(backend = "postgres", error = %e, "PostgreSQL connection failed, falling back to file");
                }
            }
        }
        #[cfg(not(feature = "ltm-postgres"))]
        "postgres" | "pg" => {
            tracing::warn!(
                backend = "postgres",
                "PostgreSQL backend requested but 'ltm-postgres' feature not enabled, using file"
            );
        }

        #[cfg(feature = "ltm-redis")]
        "redis" => {
            let url = config
                .ltm_backend_url
                .as_deref()
                .unwrap_or("redis://localhost");
            match synaptic::redis::RedisStore::new(url) {
                Ok(store) => {
                    tracing::info!(backend = "redis", "LTM using Redis backend");
                    return Arc::new(store);
                }
                Err(e) => {
                    tracing::warn!(backend = "redis", error = %e, "Redis connection failed, falling back to file");
                }
            }
        }
        #[cfg(not(feature = "ltm-redis"))]
        "redis" => {
            tracing::warn!(
                backend = "redis",
                "Redis backend requested but 'ltm-redis' feature not enabled, using file"
            );
        }

        "sqlite" => {
            let db_path = config
                .ltm_backend_url
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| base_dir.join("ltm_store.db"));
            let sqlite_cfg =
                synaptic::sqlite::SqliteStoreConfig::new(db_path.to_string_lossy().to_string());
            match synaptic::sqlite::SqliteStore::new(sqlite_cfg) {
                Ok(store) => {
                    tracing::info!(backend = "sqlite", "LTM using SQLite backend");
                    return Arc::new(store);
                }
                Err(e) => {
                    tracing::warn!(backend = "sqlite", error = %e, "SQLite store failed, falling back to file");
                }
            }
        }

        _ => {} // "file" or unknown — fall through to default
    }

    Arc::new(FileStore::new(base_dir))
}

const NAMESPACE: &[&str] = &["synapse", "long_term_memory"];
const CHUNK_SIZE: usize = 1600;
const CHUNK_OVERLAP: usize = 320;

/// A memory search result with source citation.
#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub content: String,
    pub source_key: String,
    pub evergreen: bool,
}

/// Long-term memory store with hybrid search, MMR diversity, and temporal decay.
pub struct LongTermMemory {
    base_dir: PathBuf,
    store: Arc<dyn Store>,
    hybrid_store: Arc<InMemoryStore>,
    vector_store: Arc<RwLock<SqliteVectorStore>>,
    embeddings: Arc<dyn Embeddings>,
    has_real_embeddings: bool,
    entries: Arc<RwLock<Vec<MemoryEntry>>>,
    config: MemoryConfig,
    splitter: RecursiveCharacterTextSplitter,
}

#[derive(Debug, Clone)]
struct MemoryEntry {
    key: String,
    content: String,
    keywords: Vec<String>,
    evergreen: bool,
}

impl LongTermMemory {
    pub fn new(base_dir: PathBuf, config: MemoryConfig) -> Self {
        let store = build_ltm_store(&config, &base_dir);
        Self::with_store(base_dir, config, store)
    }

    /// Create with an explicitly injected store (for testing or custom backends).
    pub fn with_store(base_dir: PathBuf, config: MemoryConfig, store: Arc<dyn Store>) -> Self {
        let (raw_embeddings, has_real) = build_embeddings(&config);

        let embeddings: Arc<dyn Embeddings> = if has_real {
            let cache_store = Arc::new(FileStore::new(base_dir.join("embedding_cache")));
            Arc::new(CacheBackedEmbeddings::new(
                raw_embeddings,
                cache_store,
                "synapse_emb",
            ))
        } else {
            raw_embeddings
        };

        let hybrid_store = if has_real && config.ltm_hybrid_search {
            InMemoryStore::new().with_hybrid_search(embeddings.clone())
        } else if has_real {
            InMemoryStore::new().with_embeddings(embeddings.clone())
        } else {
            InMemoryStore::new()
        };

        let db_path = base_dir.join("vectors.db");
        let sqlite_config =
            synaptic::sqlite::SqliteVectorStoreConfig::new(db_path.to_string_lossy());
        let vector_store = SqliteVectorStore::new(sqlite_config).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to open SQLite vector store, using in-memory fallback");
            SqliteVectorStore::new(
                synaptic::sqlite::SqliteVectorStoreConfig::new(":memory:"),
            )
            .expect("in-memory SQLite should never fail")
        });

        let splitter =
            RecursiveCharacterTextSplitter::new(CHUNK_SIZE).with_chunk_overlap(CHUNK_OVERLAP);

        Self {
            base_dir,
            store,
            hybrid_store: Arc::new(hybrid_store),
            vector_store: Arc::new(RwLock::new(vector_store)),
            embeddings,
            has_real_embeddings: has_real,
            entries: Arc::new(RwLock::new(Vec::new())),
            config,
            splitter,
        }
    }

    pub async fn load(&self) -> Result<(), SynapticError> {
        let items = self
            .store
            .search(NAMESPACE, None, self.config.ltm_max_entries)
            .await?;
        let mut entries = self.entries.write().await;
        entries.clear();

        let mut docs = Vec::new();
        for item in items {
            let content = item
                .value
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if content.is_empty() {
                continue;
            }

            self.hybrid_store
                .put(NAMESPACE, &item.key, json!(content))
                .await
                .ok();

            let evergreen = item
                .value
                .get("evergreen")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let keywords = extract_keywords(&content);
            entries.push(MemoryEntry {
                key: item.key.clone(),
                content: content.clone(),
                keywords,
                evergreen,
            });

            docs.push(Document::new(&item.key, &content));
        }

        if !docs.is_empty() {
            let vs = self.vector_store.write().await;
            if let Err(e) = vs.add_documents(docs, self.embeddings.as_ref()).await {
                tracing::warn!(error = %e, "Failed to index memories for MMR search");
            }
        }

        Ok(())
    }

    pub async fn remember(&self, content: &str) -> Result<(), SynapticError> {
        self.remember_inner(content, false).await
    }

    #[allow(dead_code)]
    pub async fn remember_evergreen(&self, content: &str) -> Result<(), SynapticError> {
        self.remember_inner(content, true).await
    }

    async fn remember_inner(&self, content: &str, evergreen: bool) -> Result<(), SynapticError> {
        let chunks = if content.len() > CHUNK_SIZE * 2 {
            self.splitter.split_text(content)
        } else {
            vec![content.to_string()]
        };

        for chunk in &chunks {
            let key = format!("mem_{}", uuid::Uuid::new_v4());
            let value = json!({
                "content": chunk,
                "timestamp": now_epoch(),
                "evergreen": evergreen,
            });

            self.store.put(NAMESPACE, &key, value).await?;
            self.hybrid_store
                .put(NAMESPACE, &key, json!(chunk))
                .await
                .ok();

            let doc = Document::new(&key, chunk);
            {
                let vs = self.vector_store.write().await;
                vs.add_documents(vec![doc], self.embeddings.as_ref())
                    .await
                    .ok();
            }

            let keywords = extract_keywords(chunk);
            let mut entries = self.entries.write().await;
            entries.push(MemoryEntry {
                key,
                content: chunk.to_string(),
                keywords,
                evergreen,
            });

            let non_evergreen_count = entries.iter().filter(|e| !e.evergreen).count();
            if entries.len() > self.config.ltm_max_entries && non_evergreen_count > 0 {
                let excess = entries.len() - self.config.ltm_max_entries;
                let mut removed = 0;
                let mut to_remove = Vec::new();
                for entry in entries.iter() {
                    if removed >= excess {
                        break;
                    }
                    if !entry.evergreen {
                        to_remove.push(entry.key.clone());
                        removed += 1;
                    }
                }
                for key in &to_remove {
                    self.store.delete(NAMESPACE, key).await.ok();
                    self.hybrid_store.delete(NAMESPACE, key).await.ok();
                    let vs = self.vector_store.write().await;
                    vs.delete(&[key.as_str()]).await.ok();
                }
                entries.retain(|e| !to_remove.contains(&e.key));
            }
        }

        Ok(())
    }

    pub async fn recall(&self, query: &str, limit: usize) -> Vec<String> {
        let limit = if limit == 0 {
            self.config.ltm_recall_limit
        } else {
            limit
        };

        if self.has_real_embeddings {
            let decay_secs = self.config.ltm_decay_half_life_days * 86400;
            let candidate_count = limit * self.config.ltm_candidate_multiplier;
            let min_score = self.config.ltm_min_score;
            let options = SearchOptions::new(candidate_count)
                .with_query(query)
                .with_decay(decay_secs)
                .with_min_score(min_score as f64);

            let hybrid_results = self
                .hybrid_store
                .search_with_options(NAMESPACE, &options)
                .await
                .unwrap_or_default();

            if !hybrid_results.is_empty() {
                let fetch_k = hybrid_results.len().min(limit * 2);
                let vs = self.vector_store.read().await;
                match vs
                    .mmr_search(
                        query,
                        limit,
                        fetch_k,
                        self.config.ltm_mmr_lambda,
                        self.embeddings.as_ref(),
                    )
                    .await
                {
                    Ok(docs) if !docs.is_empty() => {
                        return docs.into_iter().map(|d| d.content).collect();
                    }
                    _ => {
                        return hybrid_results
                            .into_iter()
                            .take(limit)
                            .filter_map(|item| item.value.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                }
            }

            let vs = self.vector_store.read().await;
            match vs
                .hybrid_search(
                    query,
                    limit,
                    self.embeddings.as_ref(),
                    self.config.ltm_vector_weight,
                )
                .await
            {
                Ok(docs) if !docs.is_empty() => {
                    return docs
                        .into_iter()
                        .filter(|(_d, score)| *score >= min_score)
                        .map(|(d, _score)| d.content)
                        .collect();
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::debug!(error = %e, "Hybrid search failed, falling back to keywords");
                }
            }
        }

        self.recall_by_keywords(query, limit).await
    }

    pub async fn recall_with_sources(&self, query: &str, limit: usize) -> Vec<MemoryResult> {
        let limit = if limit == 0 {
            self.config.ltm_recall_limit
        } else {
            limit
        };

        let contents = self.recall(query, limit).await;
        let entries = self.entries.read().await;

        contents
            .into_iter()
            .map(|content| {
                let entry = entries.iter().find(|e| e.content == content);
                MemoryResult {
                    source_key: entry.map(|e| e.key.clone()).unwrap_or_default(),
                    evergreen: entry.is_some_and(|e| e.evergreen),
                    content,
                }
            })
            .collect()
    }

    pub async fn flush_before_compact(&self, discarding: &[Message], model: &dyn ChatModel) {
        if discarding.is_empty() || !self.config.pre_compact_flush {
            return;
        }

        let has_important = discarding.iter().any(|m| Self::is_important(m.content()));
        if !has_important && discarding.len() < 10 {
            return;
        }

        let mut conversation = String::new();
        for msg in discarding {
            let content = msg.content();
            if content.len() > 200 {
                conversation.push_str(&format!("{}: {}...\n", msg.role(), &content[..200]));
            } else {
                conversation.push_str(&format!("{}: {}\n", msg.role(), content));
            }
        }
        if conversation.len() > 4000 {
            conversation.truncate(4000);
        }

        let prompt = format!(
            "Extract important facts, decisions, preferences, and key information from this \
             conversation that should be remembered long-term. Return each memory as a separate \
             line starting with '- '. Only include genuinely important information, not routine \
             exchanges. If nothing is worth remembering, respond with 'NONE'.\n\
             IMPORTANT: Preserve all identifiers exactly — IDs, hashes, commit SHAs, file paths, \
             URLs, version numbers, UUIDs, and any other specific references. Never paraphrase \
             or omit these identifiers.\n\n{}",
            conversation
        );

        let request = ChatRequest::new(vec![Message::human(prompt)]);
        match model.chat(request).await {
            Ok(response) => {
                let text = response.message.content().to_string();
                if text.trim() == "NONE" || text.is_empty() {
                    return;
                }
                for line in text.lines() {
                    let line = line.trim().trim_start_matches('-').trim();
                    if line.len() > 10 {
                        self.remember(line).await.ok();
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Pre-compact flush failed");
            }
        }
    }

    #[allow(dead_code)]
    pub async fn prune(&self) -> Result<usize, SynapticError> {
        let mut entries = self.entries.write().await;
        if entries.len() <= self.config.ltm_max_entries {
            return Ok(0);
        }

        let excess = entries.len() - self.config.ltm_max_entries;
        let mut removed = 0;
        let mut to_remove = Vec::new();
        for entry in entries.iter() {
            if removed >= excess {
                break;
            }
            if !entry.evergreen {
                to_remove.push(entry.key.clone());
                removed += 1;
            }
        }

        for key in &to_remove {
            self.store.delete(NAMESPACE, key).await.ok();
            self.hybrid_store.delete(NAMESPACE, key).await.ok();
            let vs = self.vector_store.write().await;
            vs.delete(&[key.as_str()]).await.ok();
        }
        entries.retain(|e| !to_remove.contains(&e.key));

        Ok(to_remove.len())
    }

    async fn recall_by_keywords(&self, query: &str, limit: usize) -> Vec<String> {
        let query_keywords = extract_keywords(query);
        let entries = self.entries.read().await;

        let mut scored: Vec<(usize, &MemoryEntry)> = entries
            .iter()
            .map(|entry| {
                let score = query_keywords
                    .iter()
                    .filter(|k| entry.keywords.contains(k))
                    .count();
                (score, entry)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry.content.clone())
            .collect()
    }

    pub fn is_important(content: &str) -> bool {
        let lower = content.to_lowercase();
        lower.contains("remember")
            || lower.contains("important")
            || lower.contains("note:")
            || lower.contains("decision:")
            || lower.contains("always")
            || lower.contains("never")
            || lower.contains("preference:")
            || content.len() > 500
    }

    pub async fn count(&self) -> usize {
        self.entries.read().await.len()
    }

    pub async fn forget(&self, keyword: &str) -> Result<usize, SynapticError> {
        let keyword_lower = keyword.to_lowercase();
        let mut entries = self.entries.write().await;
        let mut removed = 0;

        let to_remove: Vec<String> = entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&keyword_lower))
            .map(|e| e.key.clone())
            .collect();

        for key in &to_remove {
            self.store.delete(NAMESPACE, key).await.ok();
            self.hybrid_store.delete(NAMESPACE, key).await.ok();
            let vs = self.vector_store.write().await;
            vs.delete(&[key.as_str()]).await.ok();
            removed += 1;
        }

        entries.retain(|e| !e.content.to_lowercase().contains(&keyword_lower));
        Ok(removed)
    }

    pub async fn list(&self) -> Vec<(String, String)> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .map(|e| (e.key.clone(), e.content.clone()))
            .collect()
    }

    pub async fn clear_all(&self) -> Result<usize, SynapticError> {
        let mut entries = self.entries.write().await;
        let count = entries.len();

        for entry in entries.drain(..) {
            self.store.delete(NAMESPACE, &entry.key).await.ok();
            self.hybrid_store.delete(NAMESPACE, &entry.key).await.ok();
        }

        Ok(count)
    }

    #[allow(dead_code)]
    pub fn uses_embeddings(&self) -> bool {
        self.has_real_embeddings
    }

    pub fn watch(ltm: Arc<LongTermMemory>) -> tokio::task::JoinHandle<()> {
        use notify::{Event, EventKind, RecursiveMode, Watcher};
        use std::sync::mpsc;
        use std::time::Duration;

        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let (tx, rx) = mpsc::channel::<Event>();

            let mut watcher = match notify::recommended_watcher(move |res: Result<Event, _>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to create LTM file watcher");
                    return;
                }
            };

            let store_path = ltm.base_dir.join("synapse").join("long_term_memory");

            if store_path.exists() {
                if let Err(e) = watcher.watch(&store_path, RecursiveMode::Recursive) {
                    tracing::warn!(path = ?store_path, error = %e, "Failed to watch LTM directory");
                    return;
                }
                tracing::debug!("Watching LTM directory for changes");
            } else {
                tracing::debug!("LTM directory does not exist yet, watcher skipped");
                return;
            }

            loop {
                match rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(event) => {
                        if matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                        ) {
                            while rx.recv_timeout(Duration::from_secs(2)).is_ok() {}

                            tracing::debug!("LTM files changed, reloading");
                            handle.block_on(async { ltm.load().await.ok() });
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
    }
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
