use serde::Deserialize;

/// Memory and context management configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct MemoryConfig {
    /// Whether auto-compaction is enabled when token count exceeds threshold.
    #[serde(default = "default_true")]
    pub auto_compact: bool,
    /// Token threshold for auto-compaction (default: 80000, ~80% of 128k context).
    #[serde(default = "default_auto_compact_threshold")]
    pub auto_compact_threshold: usize,
    /// Compaction strategy: "truncate" or "summarize".
    #[serde(default = "default_compact_strategy")]
    pub compact_strategy: String,
    /// Number of recent messages to keep after compaction.
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,

    /// Whether long-term memory is enabled.
    #[serde(default = "default_true")]
    pub ltm_enabled: bool,
    /// Maximum LTM entries (oldest pruned when exceeded).
    #[serde(default = "default_ltm_max_entries")]
    pub ltm_max_entries: usize,
    /// Number of memories to recall per query.
    #[serde(default = "default_ltm_recall_limit")]
    pub ltm_recall_limit: usize,
    /// Temporal decay half-life in days (older memories score lower).
    #[serde(default = "default_ltm_decay_half_life_days")]
    pub ltm_decay_half_life_days: u64,
    /// MMR lambda for recall diversity (0.0=pure diversity, 1.0=pure relevance).
    #[serde(default = "default_ltm_mmr_lambda")]
    pub ltm_mmr_lambda: f32,
    /// Whether to use hybrid search (BM25 + embedding).
    #[serde(default = "default_true")]
    pub ltm_hybrid_search: bool,

    /// Vector weight for hybrid search (0.0=pure BM25, 1.0=pure vector). Default: 0.7.
    #[serde(default = "default_ltm_vector_weight")]
    pub ltm_vector_weight: f32,

    /// Minimum score threshold for recall results (0.0–1.0). Results below this are discarded.
    /// Default: 0.35 (matching OpenClaw's query.minScore).
    #[serde(default = "default_ltm_min_score")]
    pub ltm_min_score: f32,

    /// Candidate multiplier for hybrid search — retrieve `limit * multiplier` candidates
    /// before re-ranking. Default: 4 (matching OpenClaw's candidateMultiplier).
    #[serde(default = "default_ltm_candidate_multiplier")]
    pub ltm_candidate_multiplier: usize,

    /// Whether to flush important memories before compaction.
    #[serde(default = "default_true")]
    pub pre_compact_flush: bool,

    /// Auto-prune sessions older than this many days (0 = disabled).
    #[serde(default = "default_session_prune_days")]
    pub session_prune_days: u64,

    /// Maximum characters per tool result before pruning (0 = disabled).
    #[serde(default = "default_max_tool_result_chars")]
    pub max_tool_result_chars: usize,

    /// Threshold for hard-clear: tool results exceeding this are replaced entirely
    /// with a placeholder. Must be >= max_tool_result_chars. Default: 32000.
    #[serde(default = "default_hard_clear_chars")]
    pub hard_clear_chars: usize,

    /// Head chars to keep in soft-trim (0 = auto 50/50 split). Default: 0.
    #[serde(default)]
    pub soft_trim_head_chars: usize,

    /// Tail chars to keep in soft-trim (0 = auto 50/50 split). Default: 0.
    #[serde(default)]
    pub soft_trim_tail_chars: usize,

    /// Number of most-recent assistant turns whose tool results are protected from pruning.
    /// Default: 0 (disabled).
    #[serde(default)]
    pub keep_last_assistants: usize,

    /// Tool names to exclude from pruning (allow list). Supports trailing `*` wildcard.
    #[serde(default)]
    pub prune_allow_tools: Vec<String>,

    /// Tool names to always prune (deny list, takes precedence over allow). Supports trailing `*`.
    #[serde(default)]
    pub prune_deny_tools: Vec<String>,

    /// LTM storage backend: "file" (default), "sqlite", "postgres", "redis".
    #[serde(default = "default_ltm_backend")]
    pub ltm_backend: String,
    /// Connection URL for ltm_backend (e.g. postgres://..., redis://..., path for sqlite).
    #[serde(default)]
    pub ltm_backend_url: Option<String>,

    /// Embedding provider: "auto", "openai", "ollama", "fake". Default: "auto".
    #[serde(default = "default_embedding_provider")]
    pub embedding_provider: String,

    /// Ollama embedding server URL (default: "http://localhost:11434").
    #[serde(default = "default_ollama_embedding_url")]
    pub ollama_embedding_url: String,

    /// Ollama embedding model name (default: "nomic-embed-text").
    #[serde(default = "default_ollama_embedding_model")]
    pub ollama_embedding_model: String,
}

fn default_auto_compact_threshold() -> usize {
    80000
}
fn default_compact_strategy() -> String {
    "summarize".to_string()
}
fn default_keep_recent() -> usize {
    10
}
fn default_ltm_max_entries() -> usize {
    1000
}
fn default_ltm_recall_limit() -> usize {
    5
}
fn default_ltm_decay_half_life_days() -> u64 {
    30
}
fn default_ltm_mmr_lambda() -> f32 {
    0.7
}
fn default_session_prune_days() -> u64 {
    90
}
fn default_max_tool_result_chars() -> usize {
    8000
}
fn default_hard_clear_chars() -> usize {
    32000
}
fn default_ltm_vector_weight() -> f32 {
    0.7
}
fn default_ltm_min_score() -> f32 {
    0.35
}
fn default_ltm_candidate_multiplier() -> usize {
    4
}
fn default_ltm_backend() -> String {
    "file".to_string()
}
fn default_embedding_provider() -> String {
    "auto".to_string()
}
fn default_ollama_embedding_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

pub(crate) fn default_true() -> bool {
    true
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            auto_compact: true,
            auto_compact_threshold: default_auto_compact_threshold(),
            compact_strategy: default_compact_strategy(),
            keep_recent: default_keep_recent(),
            ltm_enabled: true,
            ltm_max_entries: default_ltm_max_entries(),
            ltm_recall_limit: default_ltm_recall_limit(),
            ltm_decay_half_life_days: default_ltm_decay_half_life_days(),
            ltm_mmr_lambda: default_ltm_mmr_lambda(),
            ltm_hybrid_search: true,
            ltm_vector_weight: default_ltm_vector_weight(),
            ltm_min_score: default_ltm_min_score(),
            ltm_candidate_multiplier: default_ltm_candidate_multiplier(),
            pre_compact_flush: true,
            session_prune_days: default_session_prune_days(),
            max_tool_result_chars: default_max_tool_result_chars(),
            hard_clear_chars: default_hard_clear_chars(),
            soft_trim_head_chars: 0,
            soft_trim_tail_chars: 0,
            keep_last_assistants: 0,
            prune_allow_tools: Vec::new(),
            prune_deny_tools: Vec::new(),
            ltm_backend: default_ltm_backend(),
            ltm_backend_url: None,
            embedding_provider: default_embedding_provider(),
            ollama_embedding_url: default_ollama_embedding_url(),
            ollama_embedding_model: default_ollama_embedding_model(),
        }
    }
}

/// Context injection configuration — controls bootstrap file truncation.
#[derive(Debug, Clone, Deserialize)]
pub struct ContextConfig {
    /// Maximum characters per injected file (0 = unlimited). Default: 20000.
    #[serde(default = "default_max_chars_per_file")]
    pub max_chars_per_file: usize,
    /// Total maximum characters for all injected files combined (0 = unlimited). Default: 100000.
    #[serde(default = "default_total_max_chars")]
    pub total_max_chars: usize,
}

fn default_max_chars_per_file() -> usize {
    20000
}
fn default_total_max_chars() -> usize {
    100000
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_chars_per_file: default_max_chars_per_file(),
            total_max_chars: default_total_max_chars(),
        }
    }
}

/// Session management configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionConfig {
    /// Whether to auto-reset the session when it spans a new day. Default: false.
    #[serde(default)]
    pub daily_reset: bool,
    /// Minutes of idle time before auto-resetting the session. 0 = disabled.
    #[serde(default)]
    pub idle_reset_minutes: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            daily_reset: false,
            idle_reset_minutes: 0,
        }
    }
}
