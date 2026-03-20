use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic::core::SynapticError;
use synaptic::memory::{CommitResult, MemoryProvider, MemoryResult};

/// Configuration for the VikingMemoryProvider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VikingConfig {
    /// Base URL of the OpenViking REST API (default: http://127.0.0.1:1933).
    pub url: String,
    /// Optional API key sent as a Bearer token.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Automatically recall memories before each agent turn.
    #[serde(default = "default_true")]
    pub auto_recall: bool,
    /// Maximum number of memories to recall per query.
    #[serde(default = "default_recall_limit")]
    pub recall_limit: usize,
    /// Minimum relevance score threshold for recalled memories.
    #[serde(default = "default_threshold")]
    pub recall_score_threshold: f64,
    /// Automatically capture conversation turns for later extraction.
    #[serde(default = "default_true")]
    pub auto_capture: bool,
    /// Commit the session buffer to long-term memory on session reset.
    #[serde(default = "default_true")]
    pub capture_on_reset: bool,
}

fn default_true() -> bool {
    true
}
fn default_recall_limit() -> usize {
    6
}
fn default_threshold() -> f64 {
    0.01
}

impl Default for VikingConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:1933".into(),
            api_key: None,
            auto_recall: true,
            recall_limit: 6,
            recall_score_threshold: 0.01,
            auto_capture: true,
            capture_on_reset: true,
        }
    }
}

/// MemoryProvider implementation backed by the OpenViking REST API.
pub struct VikingMemoryProvider {
    client: reqwest::Client,
    config: VikingConfig,
}

impl VikingMemoryProvider {
    /// Create a new provider with the given configuration.
    pub fn new(config: VikingConfig) -> Self {
        let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
        Self {
            client: builder.build().unwrap_or_default(),
            config,
        }
    }

    /// Build the full URL for the given API path.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.url.trim_end_matches('/'), path)
    }

    /// Attach optional API key as a Bearer authorization header.
    fn auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.config.api_key {
            builder.bearer_auth(key)
        } else {
            builder
        }
    }

    /// Map a reqwest error into SynapticError::Tool.
    fn map_err(e: reqwest::Error) -> SynapticError {
        SynapticError::Tool(format!("VikingMemoryProvider HTTP error: {e}"))
    }

    /// Map a non-success HTTP status into SynapticError::Tool.
    async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, SynapticError> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp)
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(SynapticError::Tool(format!(
                "VikingMemoryProvider HTTP {status}: {body}"
            )))
        }
    }
}

// ── Viking API response types ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct VikingMemoryItem {
    #[serde(default)]
    uri: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    layer: Option<String>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

impl From<VikingMemoryItem> for MemoryResult {
    fn from(item: VikingMemoryItem) -> Self {
        Self {
            uri: item.uri,
            content: item.content,
            score: item.score,
            category: item.category,
            layer: item.layer,
            metadata: item.metadata.unwrap_or(serde_json::Value::Null),
        }
    }
}

#[derive(Debug, Deserialize)]
struct VikingSearchResponse {
    #[serde(default)]
    results: Vec<VikingMemoryItem>,
}

#[derive(Debug, Deserialize)]
struct VikingCommitResponse {
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    memories_extracted: usize,
    #[serde(default)]
    memories_merged: usize,
    #[serde(default)]
    memories_skipped: usize,
}

#[derive(Debug, Deserialize)]
struct VikingFilesystemReadResponse {
    #[serde(default)]
    content: Option<String>,
}

// ── MemoryProvider impl ────────────────────────────────────────────────────

#[async_trait]
impl MemoryProvider for VikingMemoryProvider {
    /// Append a single conversation turn to the session buffer.
    async fn add_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
    ) -> Result<(), SynapticError> {
        let url = self.url(&format!(
            "/api/v1/sessions/{}/messages",
            urlencoding::encode(session_key)
        ));
        let body = serde_json::json!({ "role": role, "content": content });
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;
        Self::check_status(resp).await?;
        Ok(())
    }

    /// Record that the session consumed particular context or skill URIs.
    async fn record_usage(
        &self,
        session_key: &str,
        context_uris: &[String],
        skill_uris: &[String],
    ) -> Result<(), SynapticError> {
        let url = self.url(&format!(
            "/api/v1/sessions/{}/used",
            urlencoding::encode(session_key)
        ));
        let body = serde_json::json!({
            "context_uris": context_uris,
            "skill_uris": skill_uris,
        });
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;
        Self::check_status(resp).await?;
        Ok(())
    }

    /// Retrieve the most relevant memories for `query` across all sessions.
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryResult>, SynapticError> {
        let url = self.url("/api/v1/search/find");
        let body = serde_json::json!({ "query": query, "limit": limit });
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;
        let resp = Self::check_status(resp).await?;
        let data: VikingSearchResponse = resp.json().await.map_err(Self::map_err)?;
        Ok(data.results.into_iter().map(MemoryResult::from).collect())
    }

    /// Search memories, optionally scoped to a specific session.
    async fn search(
        &self,
        query: &str,
        session_key: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryResult>, SynapticError> {
        let url = self.url("/api/v1/search/search");
        let mut body = serde_json::json!({ "query": query, "limit": limit });
        if let Some(sid) = session_key {
            body["session_id"] = serde_json::Value::String(sid.to_string());
        }
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;
        let resp = Self::check_status(resp).await?;
        let data: VikingSearchResponse = resp.json().await.map_err(Self::map_err)?;
        Ok(data.results.into_iter().map(MemoryResult::from).collect())
    }

    /// Commit the session buffer to long-term storage, extracting and merging memories.
    async fn commit(&self, session_key: &str) -> Result<CommitResult, SynapticError> {
        let url = self.url(&format!(
            "/api/v1/sessions/{}/commit",
            urlencoding::encode(session_key)
        ));
        let body = serde_json::json!({ "wait": true });
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;
        let resp = Self::check_status(resp).await?;
        let data: VikingCommitResponse = resp.json().await.map_err(Self::map_err)?;
        Ok(CommitResult {
            archived: data.archived,
            memories_extracted: data.memories_extracted,
            memories_merged: data.memories_merged,
            memories_skipped: data.memories_skipped,
        })
    }

    /// Index an external resource so it can be recalled in future searches.
    async fn add_resource(&self, uri: &str) -> Result<(), SynapticError> {
        let url = self.url("/api/v1/resources/add");
        let body = serde_json::json!({ "url": uri });
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;
        Self::check_status(resp).await?;
        Ok(())
    }

    /// Retrieve a human-readable profile summary for the given user.
    async fn get_profile(&self, user_id: &str) -> Result<Option<String>, SynapticError> {
        let url = self.url("/api/v1/filesystem/read");
        let uri = format!("viking://user/{}/memories/profile.md", user_id);
        let body = serde_json::json!({ "uri": uri });
        let resp = self
            .auth(self.client.post(&url).json(&body))
            .send()
            .await
            .map_err(Self::map_err)?;

        // A 404 means no profile exists yet — return None instead of an error.
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let resp = Self::check_status(resp).await?;
        let data: VikingFilesystemReadResponse = resp.json().await.map_err(Self::map_err)?;
        Ok(data.content.filter(|s| !s.is_empty()))
    }
}
