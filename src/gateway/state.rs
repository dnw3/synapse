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
use crate::channels::handler::AgentSession;
use crate::config::SynapseConfig;
use crate::gateway::messages::ChannelRegistry;
use crate::gateway::rpc::wizard::WizardSession;
use crate::session::SessionWriteLock;
use synaptic::logging::LogBuffer;

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
    // ── Core config & auth ──────────────────────────────────────────────
    pub config: SynapseConfig,
    /// Authentication state (None if auth is not configured).
    pub auth: Option<Arc<AuthState>>,
    /// Server start time for health/uptime reporting.
    pub started_at: std::time::Instant,

    // ── Agent & model ───────────────────────────────────────────────────
    pub model: Arc<dyn ChatModel>,
    /// Pre-loaded MCP tools (loaded once at startup, shared across requests).
    #[allow(dead_code)]
    pub mcp_tools: Vec<Arc<dyn Tool>>,
    /// Cost and token usage tracking across all requests.
    pub cost_tracker: Arc<CostTrackingCallback>,
    /// Multi-dimensional usage tracker with persistence.
    pub usage_tracker: Arc<UsageTracker>,
    /// Memory provider (native LTM or Viking), built from config at startup.
    #[allow(dead_code)]
    pub memory_provider: Arc<dyn MemoryProvider>,
    /// Per-request context scopes (TTL: 30 min).  Used for variable passing
    /// across multi-step agent pipelines and sub-agent spawning.
    #[allow(dead_code)]
    pub context_engine: SharedContextEngine,

    // ── Session management ──────────────────────────────────────────────
    pub sessions: Arc<SessionManager>,
    /// Active agent cancel tokens, keyed by store_key (e.g. "agent:default:main").
    pub cancel_tokens: Arc<RwLock<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
    /// Per-session write locks to prevent concurrent modifications.
    pub write_lock: Arc<SessionWriteLock>,
    /// Per-session run queue to serialize concurrent agent executions.
    pub run_queue: Arc<AgentRunQueue>,
    /// Connection IDs subscribed to session change events.
    pub session_subscribers: Arc<RwLock<HashSet<String>>>,
    /// Active wizard sessions keyed by session UUID.
    pub wizard_sessions: Arc<RwLock<HashMap<String, WizardSession>>>,

    // ── RPC & networking ────────────────────────────────────────────────
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
    /// Bootstrap token store for device pairing QR codes.
    pub bootstrap_store: Arc<RwLock<crate::gateway::nodes::BootstrapStore>>,
    /// Global idempotency cache: key → insertion time.
    ///
    /// Used to deduplicate messages across connections (e.g. reconnects after
    /// network failures).  Entries expire after 5 minutes and are periodically
    /// cleaned up by a background task spawned in `new()`.
    #[allow(dead_code)]
    pub idempotency_cache: Arc<DashMap<String, Instant>>,

    // ── Exec approvals ──────────────────────────────────────────────────
    /// Exec approval manager (in-memory pending requests).
    pub exec_approval_manager: Arc<RwLock<crate::gateway::exec_approvals::ExecApprovalManager>>,
    /// Exec approvals config (persisted).
    pub exec_approvals_config: Arc<RwLock<crate::gateway::exec_approvals::ExecApprovalsConfig>>,

    // ── Channel adapters ────────────────────────────────────────────────
    /// Registry of active channel senders for outbound delivery.
    #[allow(dead_code)]
    pub channel_registry: Arc<RwLock<ChannelRegistry>>,
    /// Channel adapter lifecycle manager.
    pub channel_manager: Arc<super::channel_manager::ChannelAdapterManager>,
    /// DM pairing policy enforcer (shared with channel adapters).
    pub dm_enforcer: Arc<crate::channels::dm::FileDmPolicyEnforcer>,
    /// Registry of per-channel approval notifiers.
    pub approve_notifiers: Arc<crate::channels::dm::ApproveNotifierRegistry>,

    // ── Infrastructure & observability ──────────────────────────────────
    /// HTTP request metrics (counters, durations).
    pub request_metrics: RequestMetrics,
    /// In-memory log buffer for the /api/logs endpoint.
    pub log_buffer: LogBuffer,
    /// Central event bus for agent lifecycle and gateway events.
    #[allow(dead_code)]
    pub event_bus: Arc<EventBus>,
    /// Canvas rendering engine for WebSocket canvas_update pipeline.
    #[allow(dead_code)]
    pub canvas_engine: Arc<CanvasEngine>,
    /// Plugin registry — loaded once at startup, shared across all agent builds.
    pub plugin_registry: Arc<StdRwLock<synaptic::plugin::PluginRegistry>>,
    /// Shared AgentSession for unified message processing pipeline.
    pub agent_session: Arc<AgentSession>,
}

// ── Builder helpers ─────────────────────────────────────────────────────────
//
// These free functions break up the large `AppState::new()` constructor into
// focused initialization steps. Each returns a small bundle of related state.

/// Agent model, MCP tools, cost/usage tracking, and memory.
struct AgentBundle {
    model: Arc<dyn ChatModel>,
    mcp_tools: Vec<Arc<dyn Tool>>,
    cost_tracker: Arc<CostTrackingCallback>,
    usage_tracker: Arc<UsageTracker>,
    memory_provider: Arc<dyn MemoryProvider>,
    context_engine: SharedContextEngine,
}

async fn build_agent_bundle(
    config: &SynapseConfig,
) -> Result<AgentBundle, Box<dyn std::error::Error>> {
    let model = agent::build_model(config, None)?;
    let mcp_tools = agent::load_mcp_tools(config).await;

    let cost_tracker = Arc::new(CostTrackingCallback::new(default_pricing()));

    // Multi-dimensional usage tracker with JSONL persistence
    let usage_path = super::usage::default_usage_path();
    let usage_tracker = Arc::new(UsageTracker::with_persistence(
        Arc::clone(&cost_tracker),
        usage_path,
    ));
    if let Err(e) = usage_tracker.load().await {
        tracing::warn!(error = %e, "failed to load usage records from disk");
    }
    usage_tracker.spawn_periodic_flush(std::time::Duration::from_secs(60));

    let memory_provider = crate::memory::build_memory_provider(config, None);
    let context_engine = Arc::new(ContextEngine::new(std::time::Duration::from_secs(1800)));

    Ok(AgentBundle {
        model,
        mcp_tools,
        cost_tracker,
        usage_tracker,
        memory_provider,
        context_engine,
    })
}

/// RPC router, broadcaster, presence, node registry, pairing, bootstrap, and
/// idempotency cache (including background cleanup task).
struct RpcBundle {
    broadcaster: Arc<Broadcaster>,
    rpc_router: Arc<RpcRouter>,
    presence: Arc<RwLock<crate::gateway::presence::PresenceStore>>,
    node_registry: Arc<RwLock<crate::gateway::nodes::NodeRegistry>>,
    pairing_store: Arc<RwLock<crate::gateway::nodes::PairingStore>>,
    bootstrap_store: Arc<RwLock<crate::gateway::nodes::BootstrapStore>>,
    idempotency_cache: Arc<DashMap<String, Instant>>,
}

fn build_rpc_bundle() -> RpcBundle {
    let broadcaster = Arc::new(Broadcaster::new());
    let mut rpc_router = RpcRouter::new();
    super::rpc::register_all(&mut rpc_router);
    let rpc_router = Arc::new(rpc_router);

    let idempotency_cache: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());

    // Background cleanup: remove idempotency entries older than 5 minutes every 60 seconds.
    {
        let cache = Arc::clone(&idempotency_cache);
        tokio::spawn(async move {
            let ttl = std::time::Duration::from_secs(300);
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

    RpcBundle {
        broadcaster,
        rpc_router,
        presence: Arc::new(RwLock::new(crate::gateway::presence::PresenceStore::new())),
        node_registry: Arc::new(RwLock::new(crate::gateway::nodes::NodeRegistry::new())),
        pairing_store: Arc::new(RwLock::new(crate::gateway::nodes::PairingStore::new())),
        bootstrap_store: Arc::new(RwLock::new(crate::gateway::nodes::BootstrapStore::new())),
        idempotency_cache,
    }
}

/// Channel adapters: registry, manager, DM enforcer, approval notifiers.
struct ChannelBundle {
    channel_registry: Arc<RwLock<ChannelRegistry>>,
    channel_manager: Arc<super::channel_manager::ChannelAdapterManager>,
    dm_enforcer: Arc<crate::channels::dm::FileDmPolicyEnforcer>,
    approve_notifiers: Arc<crate::channels::dm::ApproveNotifierRegistry>,
}

fn build_channel_bundle() -> ChannelBundle {
    let pairing_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".synapse")
        .join("pairing");

    ChannelBundle {
        channel_registry: Arc::new(RwLock::new(ChannelRegistry::new())),
        channel_manager: Arc::new(super::channel_manager::ChannelAdapterManager::new()),
        dm_enforcer: Arc::new(crate::channels::dm::FileDmPolicyEnforcer::new(
            pairing_dir,
            crate::channels::dm::DmPolicy::Pairing,
            None,
        )),
        approve_notifiers: Arc::new(crate::channels::dm::ApproveNotifierRegistry::default()),
    }
}

/// Event bus and plugin registry (plugins are wired into the event bus).
struct InfraBundle {
    event_bus: Arc<EventBus>,
    plugin_registry: Arc<StdRwLock<synaptic::plugin::PluginRegistry>>,
}

fn build_infra_bundle(
    cost_tracker: &Arc<CostTrackingCallback>,
    usage_tracker: &Arc<UsageTracker>,
) -> InfraBundle {
    let event_bus = Arc::new(EventBus::new());

    let mut plugin_registry = synaptic::plugin::PluginRegistry::new(event_bus.clone());
    if let Err(e) = crate::plugin::register_builtin_plugins(
        &mut plugin_registry,
        Arc::clone(cost_tracker),
        Arc::clone(usage_tracker),
    ) {
        tracing::warn!(error = %e, "failed to register builtin plugins");
    }
    let plugin_registry = Arc::new(StdRwLock::new(plugin_registry));

    InfraBundle {
        event_bus,
        plugin_registry,
    }
}

// ── AppState impl ───────────────────────────────────────────────────────────

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
        // ── Agent & model ───────────────────────────────────────────────
        let agent = build_agent_bundle(config).await?;

        // ── Infrastructure (event bus, plugins) ─────────────────────────
        let infra = build_infra_bundle(&agent.cost_tracker, &agent.usage_tracker);

        // ── RPC & networking ────────────────────────────────────────────
        let rpc = build_rpc_bundle();

        // ── Channel adapters ────────────────────────────────────────────
        let channels = build_channel_bundle();

        // ── Session management ──────────────────────────────────────────
        let session_mgr = crate::build_session_manager(config);
        let write_lock = Arc::new(SessionWriteLock::new(std::time::Duration::from_secs(300)));

        // ── Auth ────────────────────────────────────────────────────────
        let auth = config
            .auth
            .as_ref()
            .map(|auth_config| Arc::new(AuthState::new(auth_config.clone())));

        // ── Exec approvals ──────────────────────────────────────────────
        let exec_approval_manager = Arc::new(RwLock::new(
            crate::gateway::exec_approvals::ExecApprovalManager::new(),
        ));
        let exec_approvals_config = Arc::new(RwLock::new(
            crate::gateway::exec_approvals::ExecApprovalsConfig::load(),
        ));

        // ── AgentSession for unified pipeline ──────────────────────────
        let agent_session = {
            let session = AgentSession::new(
                agent.model.clone(),
                Arc::new(config.clone()),
                true, // deep_agent
            )
            .with_channel("web")
            .with_gateway(channels.channel_registry.clone(), rpc.broadcaster.clone())
            .with_cost_tracker(agent.cost_tracker.clone())
            .with_usage_tracker(agent.usage_tracker.clone())
            .with_event_bus(infra.event_bus.clone())
            .with_plugin_registry(infra.plugin_registry.clone());
            Arc::new(session)
        };

        let state = Self {
            // Core config & auth
            config: config.clone(),
            auth,
            started_at: std::time::Instant::now(),

            // Agent & model
            model: agent.model,
            mcp_tools: agent.mcp_tools,
            cost_tracker: agent.cost_tracker,
            usage_tracker: agent.usage_tracker,
            memory_provider: agent.memory_provider,
            context_engine: agent.context_engine,

            // Session management
            sessions: Arc::new(session_mgr),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            write_lock,
            run_queue: Arc::new(AgentRunQueue::new()),
            session_subscribers: Arc::new(RwLock::new(HashSet::new())),
            wizard_sessions: Arc::new(RwLock::new(HashMap::new())),

            // RPC & networking
            broadcaster: rpc.broadcaster,
            rpc_router: rpc.rpc_router,
            presence: rpc.presence,
            node_registry: rpc.node_registry,
            pairing_store: rpc.pairing_store,
            bootstrap_store: rpc.bootstrap_store,
            idempotency_cache: rpc.idempotency_cache,

            // Exec approvals
            exec_approval_manager,
            exec_approvals_config,

            // Channel adapters
            channel_registry: channels.channel_registry,
            channel_manager: channels.channel_manager,
            dm_enforcer: channels.dm_enforcer,
            approve_notifiers: channels.approve_notifiers,

            // Infrastructure & observability
            request_metrics: RequestMetrics::default(),
            log_buffer: LogBuffer::new(config.logging.memory.capacity),
            event_bus: infra.event_bus,
            canvas_engine: Arc::new(CanvasEngine::new()),
            plugin_registry: infra.plugin_registry,

            // Unified agent session
            agent_session,
        };

        Ok(state)
    }
}
