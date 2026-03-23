# R1: Structure Slimming — God Object Decomposition

**Date:** 2026-03-23
**Status:** Approved
**Scope:** synapse (business) + synaptic (framework)
**Breaking Changes:** Yes (allowed)

## Overview

Decompose 4 God Object structs into domain-specific sub-structs using nested grouping. This is the root cause fix for 5 of the 10 identified architectural issues (#1, #2, #4, #5, #9).

Strategy: compiler-driven migration — nest fields into sub-structs, fix all access paths until build passes. No behavior changes.

## 1. AppState (37 fields → 6 groups)

Verified field-by-field against `src/gateway/state.rs`. All 37 fields accounted for.

### Sub-state structs

```rust
#[derive(Clone)]
pub struct CoreState {
    pub config: SynapseConfig,
    pub auth: Option<Arc<AuthState>>,
    pub started_at: Instant,
}

#[derive(Clone)]
pub struct AgentState {
    pub model: Arc<dyn ChatModel>,
    pub mcp_tools: Vec<Arc<dyn Tool>>,
    pub transient_mcp: Arc<RwLock<HashMap<String, TransientMcpServer>>>,
    pub cost_tracker: Arc<CostTrackingCallback>,
    pub usage_tracker: Arc<UsageTracker>,
    pub memory_provider: Arc<dyn MemoryProvider>,
    pub context_engine: SharedContextEngine,
    pub agent_session: Arc<AgentSession>,
}

#[derive(Clone)]
pub struct SessionState {
    pub sessions: Arc<SessionManager>,
    pub cancel_tokens: Arc<RwLock<HashMap<String, watch::Sender<bool>>>>,
    pub write_lock: Arc<SessionWriteLock>,
    pub run_queue: Arc<AgentRunQueue>,
    pub session_subscribers: Arc<RwLock<HashSet<String>>>,
    pub wizard_sessions: Arc<RwLock<HashMap<String, WizardSession>>>,
}

#[derive(Clone)]
pub struct NetworkState {
    pub broadcaster: Arc<Broadcaster>,
    pub rpc_router: Arc<RpcRouter>,
    pub presence: Arc<RwLock<PresenceStore>>,
    pub node_registry: Arc<RwLock<NodeRegistry>>,
    pub pairing_store: Arc<RwLock<PairingStore>>,
    pub bootstrap_store: Arc<RwLock<BootstrapStore>>,
    pub idempotency_cache: Arc<DashMap<String, Instant>>,
}

#[derive(Clone)]
pub struct ChannelState {
    pub channel_registry: Arc<RwLock<ChannelRegistry>>,
    pub channel_manager: Arc<ChannelAdapterManager>,
    pub dm_enforcer: Arc<FileDmPolicyEnforcer>,
    pub approve_notifiers: Arc<ApproveNotifierRegistry>,
    pub exec_approval_manager: Arc<RwLock<ExecApprovalManager>>,
    pub exec_approvals_config: Arc<RwLock<ExecApprovalsConfig>>,
}

#[derive(Clone)]
pub struct InfraState {
    pub request_metrics: RequestMetrics,
    pub log_buffer: LogBuffer,
    pub event_bus: Arc<EventBus>,
    pub canvas_engine: Arc<CanvasEngine>,
    pub plugin_registry: Arc<RwLock<PluginRegistry>>,
    pub bundle_skills_dirs: Vec<PathBuf>,
    pub bundle_agent_dirs: Vec<PathBuf>,
}
```

### Slim AppState

```rust
#[derive(Clone)]
pub struct AppState {
    pub core: CoreState,
    pub agent: AgentState,
    pub session: SessionState,
    pub network: NetworkState,
    pub channel: ChannelState,
    pub infra: InfraState,
}
```

### TransientMcpServer (replaces tuple)

```rust
pub struct TransientMcpServer {
    pub config: McpServerConfig,
    pub tools: Vec<Arc<dyn Tool>>,
}
```

### Migration rule

All field access paths change mechanically:
- `state.config` → `state.core.config`
- `state.model` → `state.agent.model`
- `state.sessions` → `state.session.sessions`
- `state.broadcaster` → `state.network.broadcaster`
- `state.channel_manager` → `state.channel.channel_manager`
- `state.plugin_registry` → `state.infra.plugin_registry`
- etc.

## 2. DeepAgentOptions (38 fields → 7 core + 5 groups)

**Location:** `synaptic-deep/src/lib.rs` (framework layer)

Verified field-by-field against the actual struct (lines 72-151). All 38 fields accounted for.

### Sub-config structs

```rust
/// Filesystem and execution environment options.
#[derive(Default)]
pub struct FilesystemOptions {
    pub backend: Option<Arc<dyn Backend>>,  // required at build time, Option for Default
    pub enable_filesystem: bool,            // default true
    pub path_guard: Option<Arc<PathGuard>>,
}

/// Skills middleware configuration.
#[derive(Default)]
pub struct SkillsOptions {
    pub enable_skills: bool,                // default true
    pub skills_dirs: Vec<String>,
    pub skill_description_budget: usize,    // default 16000
    pub skill_overrides: HashMap<String, SkillOverride>,
    pub command_executor: Option<Arc<dyn CommandExecutor>>,
    pub hooks_executor: Option<Arc<dyn SkillHooksExecutor>>,
}

/// Sub-agent spawning configuration.
#[derive(Default)]
pub struct SubagentOptions {
    pub enable_subagents: bool,             // default true
    pub max_subagent_depth: usize,          // default 3
    pub max_concurrent_subagents: usize,    // default 3
    pub max_children_per_agent: usize,      // default 0 (unlimited)
    pub tool_profiles: HashMap<String, Vec<String>>,
    pub subagents: Vec<SubAgentDef>,
    pub model_resolver: Option<Arc<dyn ModelResolver>>,
}

/// Context injection and memory options.
#[derive(Default)]
pub struct ContextOptions {
    pub system_prompt: Option<String>,
    pub environment: Option<EnvironmentInfo>,
    pub self_section: Option<String>,
    pub memory_file: Option<String>,        // default "AGENTS.md"
    pub enable_memory: bool,                // default true
    pub session_id: Option<String>,
}

/// Token management and summarization thresholds.
#[derive(Default)]
pub struct CondenserOptions {
    pub max_input_tokens: usize,            // default 128000
    pub summarization_threshold: f64,       // default 0.85
    pub eviction_threshold: usize,          // default 20000
    pub max_iterations: Option<usize>,      // default 100
}

/// Observability, events, and reflection.
#[derive(Default)]
pub struct ObservabilityOptions {
    pub event_bus: Option<Arc<EventBus>>,
    pub model_name: Option<String>,
    pub provider_name: Option<String>,
    pub channel: Option<String>,
    pub agent_id: Option<String>,
    pub reflection_model: Option<Arc<dyn ChatModel>>,
    pub reflection_config: Option<ReflectionConfig>,
}
```

### Slim DeepAgentOptions

```rust
pub struct DeepAgentOptions {
    // Core (ungroupable)
    pub tools: Vec<Arc<dyn Tool>>,
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    pub checkpointer: Option<Arc<dyn Checkpointer>>,
    pub store: Option<Arc<dyn Store>>,
    pub parallel_tools: bool,

    // Domain groups
    pub filesystem: FilesystemOptions,
    pub skills: SkillsOptions,
    pub subagent: SubagentOptions,
    pub context: ContextOptions,
    pub condenser: CondenserOptions,
    pub observability: ObservabilityOptions,
}
```

**Field accounting (38 total):**
- Core: tools, interceptors, checkpointer, store, parallel_tools = **5**
- FilesystemOptions: backend, enable_filesystem, path_guard = **3**
- SkillsOptions: enable_skills, skills_dirs, skill_description_budget, skill_overrides, command_executor, hooks_executor = **6**
- SubagentOptions: enable_subagents, max_subagent_depth, max_concurrent_subagents, max_children_per_agent, tool_profiles, subagents, model_resolver = **7**
- ContextOptions: system_prompt, environment, self_section, memory_file, enable_memory, session_id = **6**
- CondenserOptions: max_input_tokens, summarization_threshold, eviction_threshold, max_iterations = **4**
- ObservabilityOptions: event_bus, model_name, provider_name, channel, agent_id, reflection_model, reflection_config = **7**
- **Total: 5 + 3 + 6 + 7 + 6 + 4 + 7 = 38** ✓

## 3. SynapseConfig (22 bot Vecs → dynamic channels map)

Verified field-by-field against `src/config/mod.rs` (lines 32-214). 22 bot platform fields + ~25 non-bot fields.

### ChannelAccountConfig

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct ChannelAccountConfig {
    pub enabled: Option<bool>,
    pub account_id: Option<String>,
    #[serde(flatten)]
    pub settings: serde_json::Value,
}
```

### Slim SynapseConfig

22 bot platform `Vec<XxxBotConfig>` fields → `channels: HashMap<String, Vec<ChannelAccountConfig>>`.

All non-bot fields are **retained as-is** (no grouping changes for non-bot fields in this spec):

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct SynapseConfig {
    #[serde(flatten)]
    pub base: SynapticAgentConfig,

    // ── Model & providers ────────────────────────────────────
    pub fallback_models: Option<Vec<String>>,
    #[serde(rename = "models")]
    pub model_catalog: Option<Vec<ModelEntry>>,
    #[serde(rename = "providers")]
    pub provider_catalog: Option<Vec<ProviderEntry>>,
    #[serde(rename = "channel_models")]
    pub channel_model_bindings: Option<Vec<ChannelModelBinding>>,

    // ── 22 platform fields → 1 dynamic map ───────────────────
    // TOML: [[channels.lark]], [[channels.telegram]], etc.
    #[serde(default)]
    pub channels: HashMap<String, Vec<ChannelAccountConfig>>,

    // ── Server & auth ────────────────────────────────────────
    pub serve: Option<ServeConfig>,
    pub docker: Option<DockerConfig>,
    #[cfg(feature = "sandbox")]
    pub sandbox: Option<SandboxConfig>,
    pub auth: Option<AuthConfig>,

    // ── Scheduling & voice ───────────────────────────────────
    #[serde(rename = "schedule")]
    pub schedules: Option<Vec<ScheduleEntry>>,
    pub voice: Option<VoiceConfig>,

    // ── Multi-agent ──────────────────────────────────────────
    pub agents: Option<AgentsConfig>,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub broadcasts: Vec<AgentBroadcastGroup>,
    #[serde(rename = "agent_routes")]
    pub agent_routes: Option<Vec<AgentRouteConfig>>,

    // ── Security & policy ────────────────────────────────────
    pub rate_limit: Option<RateLimitConfig>,
    pub secrets: Option<SecretsConfig>,
    pub security: Option<SecurityConfig>,
    #[serde(default)]
    pub tool_policy: ToolPolicyConfig,

    // ── Commands & broadcast groups ──────────────────────────
    #[serde(rename = "command")]
    pub commands: Option<Vec<CustomCommand>>,
    #[serde(rename = "broadcast_group")]
    pub broadcast_groups: Option<Vec<BroadcastGroup>>,

    // ── Infrastructure ───────────────────────────────────────
    pub gateway: Option<GatewayConfig>,
    pub hub: Option<HubConfig>,
    pub workspace: Option<String>,
    #[serde(default)]
    pub logging: synaptic::logging::LogConfig,

    // ── Agent behavior ───────────────────────────────────────
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub subagent: SubAgentConfig,
    #[serde(default)]
    pub skill_overrides: HashMap<String, SkillOverrideConfig>,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub heartbeat: HeartbeatConfig,
    #[serde(default)]
    pub tool_display: ToolDisplayConfig,
    #[serde(default)]
    pub reflection: ReflectionSynapseConfig,
    #[serde(default)]
    pub session_reset: ResetConfig,

    // ── MCP ──────────────────────────────────────────────────
    pub mcp: Option<Vec<McpServerConfig>>,
}
```

### Channel adapter initialization

Per-platform `XxxBotConfig` structs are preserved. Adapters deserialize from `ChannelAccountConfig` at startup:

```rust
let lark_configs: Vec<LarkBotConfig> = config.channels
    .get("lark")
    .map(|accounts| accounts.iter()
        .filter_map(|a| serde_json::from_value(a.settings.clone()).ok())
        .collect())
    .unwrap_or_default();
```

### TOML format change

Old format (removed):
```toml
[[lark]]
enabled = true
app_id = "cli_xxx"
```

New format:
```toml
[[channels.lark]]
enabled = true
app_id = "cli_xxx"
```

## 4. AgentSession (16 fields → 9 required + 3 capabilities)

**Location:** `src/channels/handler/mod.rs`

Verified field-by-field against the actual struct (lines 76-105). All 16 fields accounted for.

### Capability groups

```rust
#[derive(Clone)]
pub struct GatewayCapability {
    pub broadcaster: Arc<Broadcaster>,
    pub channel_registry: Arc<RwLock<ChannelRegistry>>,
    pub router: Option<Arc<BindingRouter>>,
    pub outbound: Option<Arc<dyn Outbound>>,
}

#[derive(Clone)]
pub struct TrackingCapability {
    pub cost_tracker: Arc<CostTrackingCallback>,
    pub usage_tracker: Arc<UsageTracker>,
}

#[derive(Clone)]
pub struct PluginCapability {
    pub event_bus: Arc<EventBus>,
    pub plugin_registry: Arc<RwLock<PluginRegistry>>,
}
```

### Slim AgentSession

```rust
pub struct AgentSession {
    // Required (all channels have these)
    pub model: Arc<dyn ChatModel>,
    pub config: Arc<SynapseConfig>,
    pub session_mgr: SessionManager,
    pub deep_agent: bool,
    pub channel: String,
    pub session_map: RwLock<HashMap<String, String>>,
    pub run_queue: Arc<AgentRunQueue>,
    pub display_resolver: Arc<ToolDisplayResolver>,

    // Optional capabilities (wired up per channel type)
    pub gateway: Option<GatewayCapability>,   // web gateway has, REPL doesn't
    pub tracking: Option<TrackingCapability>,  // web/bot have, REPL optional
    pub plugins: Option<PluginCapability>,     // when plugin system active
}
```

**Field accounting (16 total):**
- Required: model, config, session_mgr, deep_agent, channel, session_map, run_queue, display_resolver = **8**
- GatewayCapability: broadcaster, channel_registry, router, outbound = **4**
- TrackingCapability: cost_tracker, usage_tracker = **2**
- PluginCapability: event_bus, plugin_registry = **2**
- **Total: 8 + 4 + 2 + 2 = 16** ✓

### Builder convergence

```rust
session
    .with_gateway(broadcaster, channel_registry)  // also sets router, outbound via chaining
    .with_tracking(cost_tracker, usage_tracker)
    .with_plugins(event_bus, plugin_registry)
```

## File Impact Summary

### Synapse (business layer)

| File | Change |
|------|--------|
| `src/gateway/state.rs` | AppState → 6 sub-state structs + TransientMcpServer |
| `src/config/mod.rs` | 22 bot Vec fields → `channels: HashMap` |
| `src/config/bots.rs` | Remove 22 per-platform config fields from SynapseConfig |
| `src/channels/handler/mod.rs` | AgentSession → 3 capabilities |
| `src/channels/handler/session.rs` | Update capability access |
| `src/channels/handler/execution.rs` | Update capability access |
| `src/channels/handler/broadcast.rs` | Update capability access |
| `src/channels/adapters/*.rs` | 22 adapter files: config access + AgentSession construction |
| `src/agent/builder.rs` | Update state access paths |
| `src/gateway/ws/v3.rs` | Update state access paths |
| `src/gateway/api/dashboard/*.rs` | Update state access paths |
| `src/gateway/rpc/*.rs` | Update state access paths |
| `src/gateway/mod.rs` | Channel spawn → dynamic map iteration |
| `src/gateway/auth.rs` | Update `state.auth` → `state.core.auth` |
| `src/gateway/metrics.rs` | Update `state.request_metrics` → `state.infra.request_metrics` |
| `src/gateway/messages/reply.rs` | Update AgentSession access |
| `src/acp/server.rs` | Update state access paths |
| All other files referencing AppState | Mechanical path updates |

### Synaptic (framework layer)

| File | Change |
|------|--------|
| `crates/synaptic-deep/src/lib.rs` | DeepAgentOptions → 5 sub-option structs |
| `crates/synaptic-deep/src/agent.rs` | Update options access paths |
| `crates/synaptic-deep/src/middleware/*.rs` | Update options access paths |
| `crates/synaptic-deep/src/middleware/subagent/task_tool.rs` | Update SubagentOptions access |
| `crates/synaptic-deep/src/config.rs` | Update builder to use sub-options |

## Verification

Pure refactoring — no behavior changes. Verification:
1. `cargo check --features full` passes (framework)
2. `cargo check --features web,plugins,bot-lark` passes (business)
3. `cargo clippy -- -D warnings` passes
4. `cd web && npx tsc --noEmit` passes (frontend unchanged)
5. Existing tests pass

## Future Work (not in this spec)

- Axum `FromRef` extractors for handlers to take only needed sub-state
- `SynapseError` typed error enum (R3)
- Plugin system unification (R2)
