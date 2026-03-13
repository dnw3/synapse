use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use synaptic::callbacks::{default_pricing, CostTrackingCallback};
use synaptic::core::{ChatModel, Tool};
use synaptic::session::SessionManager;
use tokio::sync::RwLock;

use super::auth::AuthState;
use super::rpc::{Broadcaster, RpcRouter};
use crate::agent;
use crate::config::SynapseConfig;
use crate::gateway::rpc::wizard::WizardSession;
use crate::logging::LogBuffer;
use crate::session::SessionWriteLock;

/// Request counter key: (method, path, status).
type RequestKey = (String, String, u16);
/// Duration bucket key: (method, path) or model name.
type DurationEntry = (u64, f64);

/// HTTP request metrics for Prometheus exposition.
#[derive(Clone, Default)]
pub struct RequestMetrics {
    /// (method, path, status) → count
    pub requests: Arc<RwLock<HashMap<RequestKey, u64>>>,
    /// (method, path) → (count, sum_seconds)
    pub durations: Arc<RwLock<HashMap<(String, String), DurationEntry>>>,
    /// (model) → (count, sum_seconds) for LLM call durations
    pub llm_durations: Arc<RwLock<HashMap<String, DurationEntry>>>,
}

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub config: SynapseConfig,
    pub model: Arc<dyn ChatModel>,
    pub sessions: Arc<SessionManager>,
    /// Active agent cancel tokens, keyed by conversation_id.
    pub cancel_tokens: Arc<RwLock<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
    /// Authentication state (None if auth is not configured).
    pub auth: Option<Arc<AuthState>>,
    /// Server start time for health/uptime reporting.
    pub started_at: std::time::Instant,
    /// Cost and token usage tracking across all requests.
    pub cost_tracker: Arc<CostTrackingCallback>,
    /// HTTP request metrics (counters, durations).
    pub request_metrics: RequestMetrics,
    /// Per-session write locks to prevent concurrent modifications.
    pub write_lock: Arc<SessionWriteLock>,
    /// In-memory log buffer for the /api/logs endpoint.
    pub log_buffer: LogBuffer,
    /// Pre-loaded MCP tools (loaded once at startup, shared across requests).
    pub mcp_tools: Vec<Arc<dyn Tool>>,
    /// RPC event broadcaster for connected clients.
    pub broadcaster: Arc<Broadcaster>,
    /// RPC method router.
    pub rpc_router: Arc<RpcRouter>,
    /// Presence tracking for connected clients.
    pub presence: Arc<RwLock<crate::gateway::presence::PresenceStore>>,
    /// Live node registry.
    pub node_registry: Arc<RwLock<crate::gateway::nodes::NodeRegistry>>,
    /// Node pairing store (persisted).
    pub pairing_store: Arc<RwLock<crate::gateway::nodes::PairingStore>>,
    /// Exec approval manager (in-memory pending requests).
    pub exec_approval_manager: Arc<RwLock<crate::gateway::exec_approvals::ExecApprovalManager>>,
    /// Exec approvals config (persisted).
    pub exec_approvals_config: Arc<RwLock<crate::gateway::exec_approvals::ExecApprovalsConfig>>,
    /// Connection IDs subscribed to session change events.
    pub session_subscribers: Arc<RwLock<HashSet<String>>>,
    /// Active wizard sessions keyed by session UUID.
    pub wizard_sessions: Arc<RwLock<HashMap<String, WizardSession>>>,
}

impl AppState {
    pub async fn with_log_buffer(
        config: &SynapseConfig,
        log_buffer: LogBuffer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut state = Self::new(config).await?;
        state.log_buffer = log_buffer;
        Ok(state)
    }

    pub async fn new(config: &SynapseConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let model = agent::build_model(config, None)?;
        let session_mgr = crate::build_session_manager(config);

        // Set up auth if configured
        let auth = config
            .auth
            .as_ref()
            .map(|auth_config| Arc::new(AuthState::new(auth_config.clone())));

        let cost_tracker = Arc::new(CostTrackingCallback::new(default_pricing()));

        // Default write-lock timeout: 5 minutes
        let write_lock = Arc::new(SessionWriteLock::new(std::time::Duration::from_secs(300)));

        // In-memory log buffer (capacity from config)
        let log_buffer = LogBuffer::new(config.logging.memory.capacity);

        // Load MCP tools once at startup
        let mcp_tools = agent::load_mcp_tools(config).await;

        // RPC infrastructure
        let broadcaster = Arc::new(Broadcaster::new());
        let mut rpc_router = RpcRouter::new();
        super::rpc::register_all(&mut rpc_router);
        let rpc_router = Arc::new(rpc_router);

        // Presence, nodes, exec approvals
        let presence = Arc::new(RwLock::new(crate::gateway::presence::PresenceStore::new()));
        let node_registry = Arc::new(RwLock::new(crate::gateway::nodes::NodeRegistry::new()));
        let pairing_store = Arc::new(RwLock::new(crate::gateway::nodes::PairingStore::new()));
        let exec_approval_manager = Arc::new(RwLock::new(
            crate::gateway::exec_approvals::ExecApprovalManager::new(),
        ));
        let exec_approvals_config = Arc::new(RwLock::new(
            crate::gateway::exec_approvals::ExecApprovalsConfig::load(),
        ));

        Ok(Self {
            config: config.clone(),
            model,
            sessions: Arc::new(session_mgr),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            auth,
            started_at: std::time::Instant::now(),
            cost_tracker,
            request_metrics: RequestMetrics::default(),
            write_lock,
            log_buffer,
            mcp_tools,
            broadcaster,
            rpc_router,
            presence,
            node_registry,
            pairing_store,
            exec_approval_manager,
            exec_approvals_config,
            session_subscribers: Arc::new(RwLock::new(HashSet::new())),
            wizard_sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}
