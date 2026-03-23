use std::collections::{HashMap, HashSet};
use std::sync::Arc;
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

// ── Sub-state structs ────────────────────────────────────────────────────────

/// Named struct replacing the (McpServerConfig, Vec<Arc<dyn Tool>>) tuple.
#[derive(Clone)]
pub struct TransientMcpServer {
    pub config: synaptic::config::McpServerConfig,
    pub tools: Vec<Arc<dyn Tool>>,
}

#[derive(Clone)]
pub struct CoreState {
    pub config: SynapseConfig,
    pub auth: Option<Arc<AuthState>>,
    pub started_at: Instant,
}

#[derive(Clone)]
pub struct AgentSubState {
    pub model: Arc<dyn ChatModel>,
    #[allow(dead_code)]
    pub mcp_tools: Vec<Arc<dyn Tool>>,
    pub transient_mcp: Arc<RwLock<HashMap<String, TransientMcpServer>>>,
    pub cost_tracker: Arc<CostTrackingCallback>,
    pub usage_tracker: Arc<UsageTracker>,
    #[allow(dead_code)]
    pub memory_provider: Arc<dyn MemoryProvider>,
    #[allow(dead_code)]
    pub context_engine: SharedContextEngine,
    pub agent_session: Arc<AgentSession>,
}

#[derive(Clone)]
pub struct SessionSubState {
    pub sessions: Arc<SessionManager>,
    pub cancel_tokens: Arc<RwLock<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
    pub write_lock: Arc<SessionWriteLock>,
    pub run_queue: Arc<AgentRunQueue>,
    pub session_subscribers: Arc<RwLock<HashSet<String>>>,
    pub wizard_sessions: Arc<RwLock<HashMap<String, WizardSession>>>,
}

#[derive(Clone)]
pub struct NetworkState {
    pub broadcaster: Arc<Broadcaster>,
    pub rpc_router: Arc<RpcRouter>,
    pub presence: Arc<RwLock<crate::gateway::presence::PresenceStore>>,
    pub node_registry: Arc<RwLock<crate::gateway::nodes::NodeRegistry>>,
    pub pairing_store: Arc<RwLock<crate::gateway::nodes::PairingStore>>,
    pub bootstrap_store: Arc<RwLock<crate::gateway::nodes::BootstrapStore>>,
    #[allow(dead_code)]
    pub idempotency_cache: Arc<DashMap<String, Instant>>,
}

#[derive(Clone)]
pub struct ChannelSubState {
    #[allow(dead_code)]
    pub channel_registry: Arc<RwLock<ChannelRegistry>>,
    pub channel_manager: Arc<super::channel_manager::ChannelAdapterManager>,
    pub dm_enforcer: Arc<crate::channels::dm::FileDmPolicyEnforcer>,
    pub approve_notifiers: Arc<crate::channels::dm::ApproveNotifierRegistry>,
    pub exec_approval_manager: Arc<RwLock<crate::gateway::exec_approvals::ExecApprovalManager>>,
    pub exec_approvals_config: Arc<RwLock<crate::gateway::exec_approvals::ExecApprovalsConfig>>,
}

#[derive(Clone)]
pub struct InfraState {
    pub request_metrics: RequestMetrics,
    pub log_buffer: LogBuffer,
    #[allow(dead_code)]
    pub event_bus: Arc<EventBus>,
    #[allow(dead_code)]
    pub canvas_engine: Arc<CanvasEngine>,
    pub plugin_registry: Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>,
    #[allow(dead_code)]
    pub bundle_skills_dirs: Vec<std::path::PathBuf>,
    #[allow(dead_code)]
    pub bundle_agent_dirs: Vec<std::path::PathBuf>,
}

// ── AppState ─────────────────────────────────────────────────────────────────

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub core: CoreState,
    pub agent: AgentSubState,
    pub session: SessionSubState,
    pub network: NetworkState,
    pub channel: ChannelSubState,
    pub infra: InfraState,
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

async fn build_agent_bundle(config: &SynapseConfig) -> crate::error::Result<AgentBundle> {
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

    // Memory provider will be set by memory plugin via PluginRegistry.memory_slot.
    // Use noop provider here — actual provider comes from infra bundle after plugin registration.
    let memory_provider: Arc<dyn MemoryProvider> =
        Arc::new(crate::memory::NativeMemoryProvider::new_noop());
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
    plugin_registry: Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>,
    /// Skills dirs contributed by plugin bundles (Claude/Codex/Cursor).
    bundle_skills_dirs: Vec<std::path::PathBuf>,
    /// Agent dirs contributed by plugin bundles.
    bundle_agent_dirs: Vec<std::path::PathBuf>,
}

async fn build_infra_bundle(
    config: &SynapseConfig,
    cost_tracker: &Arc<CostTrackingCallback>,
    usage_tracker: &Arc<UsageTracker>,
) -> InfraBundle {
    let event_bus = Arc::new(EventBus::new());
    let plugin_registry = Arc::new(tokio::sync::RwLock::new(
        synaptic::plugin::PluginRegistry::new(event_bus.clone()),
    ));

    // --- Phase 1: Register builtin event subscribers (tracing, thinking, etc.) ---
    {
        let mut reg = plugin_registry.write().await;
        if let Err(e) = crate::plugin::register_builtin_plugins(
            &mut reg,
            Arc::clone(cost_tracker),
            Arc::clone(usage_tracker),
        ) {
            tracing::warn!(error = %e, "failed to register builtin plugins");
        }
    }

    // --- Phase 2: Use PluginManager for all plugin lifecycle ---
    let mut plugin_manager = crate::plugins::manager::PluginManager::new(
        config.plugins.clone(),
        plugin_registry.clone(),
        dirs::home_dir()
            .unwrap_or_default()
            .join(".synapse/plugins"),
    );

    // Register slot-assigned plugins via factory registry (e.g., memory-viking)
    let factory_registry = crate::plugins::registry::default_registry();
    for (slot, plugin_name) in &config.plugins.slots {
        let plugin_config = config
            .plugins
            .entries
            .get(plugin_name)
            .map(|e| e.config.clone())
            .unwrap_or_default();

        let enabled = config
            .plugins
            .entries
            .get(plugin_name)
            .map(|e| e.enabled)
            .unwrap_or(true);

        if !enabled {
            tracing::info!(plugin = %plugin_name, slot = %slot, "plugin disabled in config");
            continue;
        }

        match factory_registry.create(plugin_name, plugin_config) {
            Some(plugin) => {
                plugin_manager.add_builtin(plugin);
                tracing::debug!(plugin = %plugin_name, slot = %slot, "queued slot plugin");
            }
            None => {
                tracing::warn!(
                    plugin = %plugin_name,
                    slot = %slot,
                    available = ?factory_registry.names(),
                    "unknown plugin — not found in builtin registry"
                );
            }
        }
    }

    // Load state (disabled plugins) and register all builtins
    plugin_manager.load_state();
    if let Err(e) = plugin_manager.load_all().await {
        tracing::warn!(error = %e, "failed to load builtin plugins");
    }

    // Discover and load external plugins + bundles from filesystem
    if let Err(e) = plugin_manager.discover_and_load().await {
        tracing::warn!(error = %e, "failed to discover external plugins");
    }

    // --- Phase 3: Start all plugin-managed services ---
    plugin_manager.start_services().await;

    // Collect bundle dirs before dropping plugin_manager
    let bundle_skills_dirs = plugin_manager.bundle_skills_dirs.clone();
    let bundle_agent_dirs = plugin_manager.bundle_agent_dirs.clone();

    InfraBundle {
        event_bus,
        plugin_registry,
        bundle_skills_dirs,
        bundle_agent_dirs,
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
            self.core.config.agents = new_config.agents.clone();
            self.core.config.bindings = new_config.bindings.clone();
        }

        if diff.bindings_changed && !diff.agents_changed {
            tracing::info!("hot-reload: bindings changed — updating routing rules");
            self.core.config.bindings = new_config.bindings.clone();
        }

        if diff.tools_changed {
            tracing::info!("hot-reload: tool_policy changed — updating tool policy");
            self.core.config.tool_policy = new_config.tool_policy.clone();
        }

        if diff.memory_changed {
            tracing::info!("hot-reload: memory configuration changed");
            self.core.config.memory = new_config.memory.clone();
        }

        if diff.schedules_changed {
            tracing::info!(
                "hot-reload: schedule definitions changed — restart required for full effect"
            );
            self.core.config.schedules = new_config.schedules.clone();
        }

        if diff.auth_changed {
            tracing::info!("hot-reload: auth configuration changed — rebuilding auth state");
            self.core.auth = new_config
                .auth
                .as_ref()
                .map(|auth_config| Arc::new(super::auth::AuthState::new(auth_config.clone())));
            self.core.config.auth = new_config.auth.clone();
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
    ) -> crate::error::Result<Self> {
        let mut state = Self::new(config).await?;
        state.infra.log_buffer = log_buffer;
        Ok(state)
    }

    pub async fn new(config: &SynapseConfig) -> crate::error::Result<Self> {
        // ── Agent & model ───────────────────────────────────────────────
        let agent_bundle = build_agent_bundle(config).await?;

        // ── Infrastructure (event bus, plugins) ─────────────────────────
        let infra_bundle = build_infra_bundle(
            config,
            &agent_bundle.cost_tracker,
            &agent_bundle.usage_tracker,
        )
        .await;

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
                agent_bundle.model.clone(),
                Arc::new(config.clone()),
                true, // deep_agent
            )
            .with_channel("web")
            .with_gateway(channels.channel_registry.clone(), rpc.broadcaster.clone())
            .with_tracking(
                agent_bundle.cost_tracker.clone(),
                agent_bundle.usage_tracker.clone(),
            )
            .with_plugins(
                infra_bundle.event_bus.clone(),
                infra_bundle.plugin_registry.clone(),
            );
            Arc::new(session)
        };

        // Get actual memory provider from plugin registry (set by memory plugin)
        let memory_provider = {
            let reg = infra_bundle.plugin_registry.read().await;
            reg.memory_slot()
                .cloned()
                .unwrap_or(agent_bundle.memory_provider)
        };

        let state = Self {
            core: CoreState {
                config: config.clone(),
                auth,
                started_at: std::time::Instant::now(),
            },
            agent: AgentSubState {
                model: agent_bundle.model,
                mcp_tools: agent_bundle.mcp_tools,
                transient_mcp: Arc::new(RwLock::new(HashMap::new())),
                cost_tracker: agent_bundle.cost_tracker,
                usage_tracker: agent_bundle.usage_tracker,
                memory_provider,
                context_engine: agent_bundle.context_engine,
                agent_session,
            },
            session: SessionSubState {
                sessions: Arc::new(session_mgr),
                cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
                write_lock,
                run_queue: Arc::new(AgentRunQueue::new()),
                session_subscribers: Arc::new(RwLock::new(HashSet::new())),
                wizard_sessions: Arc::new(RwLock::new(HashMap::new())),
            },
            network: NetworkState {
                broadcaster: rpc.broadcaster,
                rpc_router: rpc.rpc_router,
                presence: rpc.presence,
                node_registry: rpc.node_registry,
                pairing_store: rpc.pairing_store,
                bootstrap_store: rpc.bootstrap_store,
                idempotency_cache: rpc.idempotency_cache,
            },
            channel: ChannelSubState {
                channel_registry: channels.channel_registry,
                channel_manager: channels.channel_manager,
                dm_enforcer: channels.dm_enforcer,
                approve_notifiers: channels.approve_notifiers,
                exec_approval_manager,
                exec_approvals_config,
            },
            infra: InfraState {
                request_metrics: RequestMetrics::default(),
                log_buffer: LogBuffer::new(config.logging.memory.capacity),
                event_bus: infra_bundle.event_bus,
                canvas_engine: Arc::new(CanvasEngine::new()),
                plugin_registry: infra_bundle.plugin_registry,
                bundle_skills_dirs: infra_bundle.bundle_skills_dirs,
                bundle_agent_dirs: infra_bundle.bundle_agent_dirs,
            },
        };

        Ok(state)
    }
}
