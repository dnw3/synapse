use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use std::time::Instant;

use dashmap::DashMap;
use synaptic::callbacks::{default_pricing, CostTrackingCallback};
use synaptic::core::{ChatModel, Tool};
use synaptic::events::EventBus;
use synaptic::memory::MemoryProvider;
use synaptic::session::SessionManager;
use tokio::sync::RwLock;

use super::auth::AuthState;
use super::canvas::CanvasEngine;
use super::rpc::{Broadcaster, RpcRouter};
use super::run_queue::AgentRunQueue;
use super::usage::UsageTracker;
use crate::agent;
use crate::agent::context_engine::{ContextEngine, SharedContextEngine};
use crate::config::SynapseConfig;
use crate::gateway::messages::ChannelRegistry;
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
    /// Multi-dimensional usage tracker with persistence.
    pub usage_tracker: Arc<UsageTracker>,
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
    /// Bootstrap token store for device pairing QR codes.
    pub bootstrap_store: Arc<RwLock<crate::gateway::nodes::BootstrapStore>>,
    /// Registry of active channel senders for outbound delivery.
    #[allow(dead_code)]
    pub channel_registry: Arc<RwLock<ChannelRegistry>>,
    /// Channel adapter lifecycle manager.
    pub channel_manager: Arc<super::channel_manager::ChannelAdapterManager>,
    /// DM pairing policy enforcer (shared with channel adapters).
    pub dm_enforcer: Arc<crate::channels::dm::FileDmPolicyEnforcer>,
    /// Registry of per-channel approval notifiers.
    pub approve_notifiers: Arc<crate::channels::dm::ApproveNotifierRegistry>,
    /// Per-session run queue to serialize concurrent agent executions.
    pub run_queue: Arc<AgentRunQueue>,
    /// Central event bus for agent lifecycle and gateway events.
    #[allow(dead_code)]
    pub event_bus: Arc<EventBus>,
    /// Canvas rendering engine for WebSocket canvas_update pipeline.
    pub canvas_engine: Arc<CanvasEngine>,
    /// Plugin registry — loaded once at startup, shared across all agent builds.
    pub plugin_registry: Arc<StdRwLock<synaptic::plugin::PluginRegistry>>,
    /// Memory provider (native LTM or Viking), built from config at startup.
    #[allow(dead_code)]
    pub memory_provider: Arc<dyn MemoryProvider>,
    /// Per-request context scopes (TTL: 30 min).  Used for variable passing
    /// across multi-step agent pipelines and sub-agent spawning.
    #[allow(dead_code)]
    pub context_engine: SharedContextEngine,
    /// Global idempotency cache: key → insertion time.
    ///
    /// Used to deduplicate messages across connections (e.g. reconnects after
    /// network failures).  Entries expire after 5 minutes and are periodically
    /// cleaned up by a background task spawned in `new()`.
    pub idempotency_cache: Arc<DashMap<String, Instant>>,
}

impl AppState {
    /// Apply a hot-reload diff to the running state.
    ///
    /// Each section is checked and the relevant in-process state is updated where
    /// safe to do so without restarting. Changes that require a restart (e.g.
    /// serve port) are logged but not applied.
    #[allow(dead_code)]
    pub fn reload_config(&mut self, new_config: SynapseConfig, diff: &crate::config::ConfigDiff) {
        if diff.agents_changed {
            tracing::info!("hot-reload: agents configuration changed — updating agent definitions");
            // Update the stored config so new agent builds use the new definitions.
            // Running sessions are not interrupted.
            self.config.agents = new_config.agents.clone();
            self.config.bindings = new_config.bindings.clone();
        }

        if diff.bindings_changed && !diff.agents_changed {
            tracing::info!("hot-reload: bindings changed — updating routing rules");
            self.config.bindings = new_config.bindings.clone();
        }

        if diff.tools_changed {
            tracing::info!("hot-reload: tool_policy changed — updating tool policy");
            self.config.tool_policy = new_config.tool_policy.clone();
        }

        if diff.memory_changed {
            tracing::info!("hot-reload: memory configuration changed");
            self.config.memory = new_config.memory.clone();
        }

        if diff.schedules_changed {
            tracing::info!(
                "hot-reload: schedule definitions changed — restart required for full effect"
            );
            self.config.schedules = new_config.schedules.clone();
        }

        if diff.auth_changed {
            tracing::info!("hot-reload: auth configuration changed — rebuilding auth state");
            self.auth = new_config
                .auth
                .as_ref()
                .map(|auth_config| Arc::new(super::auth::AuthState::new(auth_config.clone())));
            self.config.auth = new_config.auth.clone();
        }

        if diff.serve_changed {
            tracing::warn!(
                "hot-reload: serve configuration changed — host/port changes require a restart"
            );
        }

        if diff.channels_changed {
            tracing::info!(
                "hot-reload: channel configuration changed — restart required for full effect"
            );
        }

        if diff.any_changed() {
            tracing::info!(
                agents = diff.agents_changed,
                bindings = diff.bindings_changed,
                channels = diff.channels_changed,
                tools = diff.tools_changed,
                memory = diff.memory_changed,
                schedules = diff.schedules_changed,
                serve = diff.serve_changed,
                auth = diff.auth_changed,
                "config hot-reload complete"
            );
        }
    }

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

        // Multi-dimensional usage tracker with JSONL persistence
        let usage_path = super::usage::default_usage_path();
        let usage_tracker = Arc::new(UsageTracker::with_persistence(
            Arc::clone(&cost_tracker),
            usage_path,
        ));
        // Load historical records from disk
        if let Err(e) = usage_tracker.load().await {
            tracing::warn!(error = %e, "failed to load usage records from disk");
        }
        // Periodic flush every 60 seconds
        usage_tracker.spawn_periodic_flush(std::time::Duration::from_secs(60));

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

        let event_bus = Arc::new(EventBus::new());

        // Register builtin plugins so their event subscribers are wired into the
        // event bus and their manifests are visible via the plugin registry.
        let mut plugin_registry = synaptic::plugin::PluginRegistry::new(event_bus.clone());
        if let Err(e) = crate::plugin::register_builtin_plugins(
            &mut plugin_registry,
            Arc::clone(&cost_tracker),
            Arc::clone(&usage_tracker),
        ) {
            tracing::warn!(error = %e, "failed to register builtin plugins");
        }
        let plugin_registry = Arc::new(StdRwLock::new(plugin_registry));

        // Build memory provider from config (native LTM or Viking)
        let memory_provider = crate::memory::build_memory_provider(config, None);

        let idempotency_cache: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());

        // Background cleanup: remove idempotency entries older than 5 minutes every 60 seconds.
        {
            let cache = Arc::clone(&idempotency_cache);
            tokio::spawn(async move {
                let ttl = std::time::Duration::from_secs(300); // 5 minutes
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    let now = Instant::now();
                    cache.retain(|_, inserted| now.duration_since(*inserted) < ttl);
                    tracing::debug!(
                        remaining = cache.len(),
                        "idempotency cache cleanup complete"
                    );
                }
            });
        }

        let state = Self {
            config: config.clone(),
            model,
            sessions: Arc::new(session_mgr),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            auth,
            started_at: std::time::Instant::now(),
            cost_tracker,
            usage_tracker,
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
            bootstrap_store: Arc::new(RwLock::new(crate::gateway::nodes::BootstrapStore::new())),
            channel_registry: Arc::new(RwLock::new(ChannelRegistry::new())),
            channel_manager: Arc::new(super::channel_manager::ChannelAdapterManager::new()),
            dm_enforcer: {
                let pairing_dir = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".synapse")
                    .join("pairing");
                Arc::new(crate::channels::dm::FileDmPolicyEnforcer::new(
                    pairing_dir,
                    synaptic::DmPolicy::Pairing,
                    None,
                ))
            },
            approve_notifiers: Arc::new(crate::channels::dm::ApproveNotifierRegistry::default()),
            run_queue: Arc::new(AgentRunQueue::new()),
            event_bus,
            canvas_engine: Arc::new(CanvasEngine::new()),
            plugin_registry,
            memory_provider,
            context_engine: Arc::new(ContextEngine::new(std::time::Duration::from_secs(1800))),
            idempotency_cache,
        };

        Ok(state)
    }
}
