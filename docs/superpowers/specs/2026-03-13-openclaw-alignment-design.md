# OpenClaw Full Alignment Design Spec

**Date**: 2026-03-13
**Scope**: Backend + Frontend complete alignment with OpenClaw gateway protocol and features

## 1. WebSocket RPC Protocol (对齐 OpenClaw Protocol v3)

### 1.1 Frame Types

Replace current `WsEvent`/`WsCommand` with OpenClaw-compatible frames:

```rust
// === Client → Server ===
#[serde(tag = "type")]
enum WsFrame {
    #[serde(rename = "req")]
    Request {
        id: String,
        method: String,
        #[serde(default)]
        params: Value,
    },
}

// === Server → Client ===
#[serde(tag = "type")]
enum ServerFrame {
    #[serde(rename = "res")]
    Response {
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<RpcError>,
    },
    #[serde(rename = "event")]
    Event {
        event: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        state_version: Option<StateVersion>,
    },
}

struct RpcError {
    code: String,           // "INVALID_REQUEST", "NOT_PAIRED", "AUTH_REQUIRED", etc.
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_ms: Option<u64>,
}

struct StateVersion {
    presence: u64,
    health: u64,
}
```

### 1.2 Handshake Sequence

1. **Server → Client**: `connect.challenge` event with nonce + timestamp
2. **Client → Server**: `connect` request with client info, auth, capabilities
3. **Server → Client**: `hello-ok` response with methods, events, snapshot, policy

```rust
// connect.challenge payload
{ "nonce": "uuid", "ts": 1234567890123 }

// connect request params
struct ConnectParams {
    min_protocol: u32,      // 3
    max_protocol: u32,      // 3
    client: ClientInfo {
        id: String,             // "control-ui", "webchat", "node-xxx"
        display_name: Option<String>,
        version: String,
        platform: String,       // "web", "darwin", "linux", "win32"
        device_family: Option<String>,
        model_identifier: Option<String>,
        mode: String,           // "webchat" | "standalone" | "node"
        instance_id: Option<String>,
    },
    caps: Vec<String>,          // ["tool-events"]
    commands: Vec<String>,      // for node role
    role: String,               // "operator" | "node"
    scopes: Vec<String>,        // ["operator.admin", "operator.approvals"]
    permissions: Option<HashMap<String, bool>>,  // Node permissions
    path_env: Option<String>,                     // Node's PATH
    auth: Option<AuthParams> {
        token: Option<String>,          // Shared gateway token
        password: Option<String>,       // Shared password
    },
    device: Option<DeviceAuth> {
        id: String,                     // Device identifier
        public_key: String,             // Base64URL ECDSA public key
        signature: String,              // Base64URL signed payload
        signed_at: u64,                 // Timestamp ms
        nonce: String,                  // Must match connect.challenge nonce
    },
    locale: Option<String>,
    user_agent: Option<String>,
}

// hello-ok response payload
struct HelloOk {
    protocol: u32,
    server: ServerInfo {
        version: String,
        conn_id: String,
    },
    features: Features {
        methods: Vec<String>,
        events: Vec<String>,
    },
    snapshot: Snapshot {
        presence: Vec<PresenceEntry>,
        health: HealthSnapshot,
        state_version: StateVersion,
    },
    auth: Option<AuthResult> {          // Echoed back to client
        device_token: Option<String>,   // Cached for reconnect
        role: String,
        scopes: Vec<String>,
        issued_at_ms: u64,
    },
    policy: Policy {
        max_payload: usize,
        max_buffered_bytes: usize,
        tick_interval_ms: u64,
    },
}
```

### 1.3 RPC Router Architecture

```
src/gateway/
  rpc/
    mod.rs              — RpcRouter + RpcContext + method registry
    health.rs           — health, status, doctor.memory.status
    chat.rs             — chat.send, chat.history, chat.abort, agent, agent.identity.get, agent.wait
    sessions.rs         — sessions.list/preview/patch/reset/delete/compact/usage (7 methods)
    agents.rs           — agents.list/create/update/delete + agents.files.list/get/set (7 methods)
    skills.rs           — skills.status/bins/install/update (4 methods)
    channels.rs         — channels.status/logout (2 methods)
    config.rs           — config.get/set/apply/patch/schema/schema.lookup (6 methods)
    schedules.rs        — cron.list/status/add/update/remove/run/runs (7 methods)
    usage.rs            — usage.status/cost (2 methods)
    nodes.rs            — node.list/describe/invoke/invoke.result/pending.pull/pending.ack/event/rename
                          + node.pair.request/list/approve/reject/verify
                          + node.canvas.capability.refresh (14 methods)
    devices.rs          — device.pair.list/approve/reject/remove + device.token.rotate/revoke (6 methods)
    exec_approvals.rs   — exec.approval.request/waitDecision/resolve
                          + exec.approvals.get/set/node.get/node.set (7 methods)
    presence.rs         — system-presence, system-event (2 methods)
    logs.rs             — logs.tail (1 method)
    debug.rs            — debug tools (from existing dashboard)
    tts.rs              — tts.status/providers/enable/disable/convert/setProvider (6 methods)
    models.rs           — models.list (1 method)
    tools.rs            — tools.catalog (1 method)
    workspace.rs        — workspace file CRUD (from existing dashboard)
    store.rs            — skill store search/install (from existing dashboard)
    secrets.rs          — secrets.reload/resolve (2 methods)
    updates.rs          — update.run (1 method)
    send.rs             — send, wake (2 methods)
    talk.rs             — talk.config, talk.mode (2 methods)
    voicewake.rs        — voicewake.get/set (2 methods)
    wizard.rs           — wizard.start/next/cancel/status (4 methods)
    browser.rs          — browser.request (1 method)
    heartbeat.rs        — last-heartbeat, set-heartbeats (2 methods)
    web_login.rs        — web.login.start/wait (2 methods)
    push.rs             — push.test (1 method)
  ws.rs                 — Rewritten: protocol v3 frames, handshake, event broadcasting
  state.rs              — Extended with new subsystem state
```

**RpcRouter core**:

```rust
type RpcHandler = Box<dyn Fn(RpcContext, Value) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send>> + Send + Sync>;

pub struct RpcRouter {
    methods: HashMap<String, RpcHandler>,
}

impl RpcRouter {
    pub fn new() -> Self { ... }
    pub fn register(&mut self, method: &str, handler: impl ...) { ... }
    pub async fn dispatch(&self, ctx: RpcContext, method: &str, params: Value) -> ServerFrame { ... }
    pub fn method_names(&self) -> Vec<String> { ... }
}

pub struct RpcContext {
    pub state: AppState,
    pub conn_id: String,
    pub client: ClientInfo,
    pub role: String,
    pub scopes: HashSet<String>,
    pub broadcaster: Arc<Broadcaster>,
}
```

**Broadcaster** — manages all connected WS clients for event broadcasting:

```rust
pub struct Broadcaster {
    connections: RwLock<HashMap<String, mpsc::UnboundedSender<ServerFrame>>>,
    seq: AtomicU64,
}

impl Broadcaster {
    pub async fn broadcast(&self, event: &str, payload: Value) { ... }
    pub async fn send_to(&self, conn_id: &str, frame: ServerFrame) { ... }
    pub async fn register(&self, conn_id: String, tx: mpsc::UnboundedSender<ServerFrame>) { ... }
    pub async fn unregister(&self, conn_id: &str) { ... }
}
```

### 1.4 Migration Strategy for Existing REST Endpoints

- **Phase 1**: Build RPC router alongside existing REST. All new features go through RPC only.
- **Phase 2**: Migrate existing dashboard REST handlers into RPC modules (sessions, agents, skills, etc.)
- **Phase 3**: REST endpoints become thin wrappers calling RPC internally (backward compat).
- Frontend switches to WS RPC client for all operations.

### 1.5 Events Registry

```
connect.challenge        — handshake nonce (sent immediately on WS connect)
agent                    — agent streaming (text, tool_call, tool_result, status)
chat                     — chat message events
presence                 — system presence updates (full list broadcast)
tick                     — server heartbeat (configurable interval)
heartbeat                — per-client keepalive (distinct from tick)
health                   — health status changes
shutdown                 — server shutdown notice (includes restart_expected_ms)
cron                     — cron job events (started, completed, failed)
node.pair.requested      — node pairing request received
node.pair.resolved       — node pairing approved/rejected
node.invoke.request      — invoke request forwarded to node
device.pair.requested    — device pairing request
device.pair.resolved     — device pairing resolved
exec.approval.requested  — command approval needed
exec.approval.resolved   — command approval decision made
talk.mode                — voice mode changed
voicewake.changed        — voice wake word config updated
update.available         — software update available
```

### 1.6 RPC Handler Ergonomics

Use a proc-macro style registration for typed params instead of raw `Value`:

```rust
// Macro generates handler registration + param deserialization
#[rpc_method("sessions.list")]
async fn sessions_list(ctx: &RpcContext, params: SessionsListParams) -> RpcResult<SessionsListResult> {
    let sessions = ctx.state.sessions.list(params.limit).await?;
    Ok(SessionsListResult { sessions })
}

// Expands to:
// router.register("sessions.list", |ctx, raw_params| {
//     Box::pin(async move {
//         let params: SessionsListParams = serde_json::from_value(raw_params)?;
//         sessions_list(&ctx, params).await.map(|r| serde_json::to_value(r).unwrap())
//     })
// });
```

### 1.7 Rate Limiting

Per-connection rate limiting for RPC methods:

```rust
struct RpcRateLimiter {
    limits: HashMap<String, RateLimit>,  // per-method overrides
    default: RateLimit { capacity: 100, refill_per_sec: 50 },
}

// Aggressive methods get tighter limits
"agent" | "chat.send" → { capacity: 5, refill_per_sec: 1 }
"node.invoke" → { capacity: 20, refill_per_sec: 10 }
// Read methods get generous limits
READ_METHODS → { capacity: 200, refill_per_sec: 100 }
```

### 1.8 Graceful Shutdown

1. Stop accepting new WebSocket connections
2. Broadcast `shutdown` event with `{ restart_expected_ms: Option<u64> }`
3. Drain in-flight RPC calls (max 10s grace)
4. Resolve all pending node invokes with error
5. Expire all pending exec approvals
6. Close all WebSocket connections
7. Exit

---

## 2. Authentication & Authorization (对齐 OpenClaw)

### 2.1 Role-Based Access Control

```rust
enum Role {
    Operator,   // Full dashboard + chat access
    Node,       // Node-only operations (invoke, pair, presence)
}

// Scopes for operator role
const OPERATOR_SCOPES: &[&str] = &[
    "operator.admin",       // Full unrestricted access (bypasses all checks)
    "operator.read",        // Read-only dashboard access
    "operator.write",       // Can modify config, agents, skills (implies read)
    "operator.approvals",   // Can approve/reject exec requests
    "operator.pairing",     // Can approve/reject node/device pairing
];
```

### 2.2 Auth Methods (priority order)

1. **Device Token** — ECDSA signed challenge-response (future, for mobile nodes)
2. **Shared Token** — `SYNAPSE_GATEWAY_TOKEN` env var or config
3. **Password** — `auth.password` in config (existing JWT flow enhanced)
4. **None** — If auth not configured (current default)

### 2.3 Per-Method Scope Checks (Default-Deny)

```rust
// Scope classification — mirrors OpenClaw's method-scopes.ts
// DEFAULT IS DENY — unclassified methods require admin scope

const READ_METHODS: &[&str] = &[
    "health", "status", "doctor.memory.status",
    "sessions.list", "sessions.get", "sessions.preview", "sessions.resolve", "sessions.usage",
    "sessions.usage.timeseries", "sessions.usage.logs",
    "agents.list", "agents.files.list", "agents.files.get",
    "skills.status", "skills.bins",
    "channels.status",
    "config.get", "config.schema", "config.schema.lookup",
    "cron.list", "cron.status", "cron.runs",
    "usage.status", "usage.cost",
    "models.list", "tools.catalog",
    "logs.tail",
    "node.list", "node.describe",
    "node.pair.list", "device.pair.list",
    "exec.approvals.get",
    "tts.status", "tts.providers",
    "last-heartbeat",
    "agent.identity.get",
];

const WRITE_METHODS: &[&str] = &[
    "sessions.patch", "sessions.reset", "sessions.delete", "sessions.compact",
    "agents.create", "agents.update", "agents.delete", "agents.files.set",
    "skills.install", "skills.update",
    "channels.logout",
    "cron.add", "cron.update", "cron.remove", "cron.run",
    "tts.enable", "tts.disable", "tts.convert", "tts.setProvider",
    "set-heartbeats",
    "secrets.reload", "secrets.resolve",
    "node.rename",
];

const APPROVAL_METHODS: &[&str] = &[
    "exec.approval.resolve",
    "exec.approvals.set", "exec.approvals.node.get", "exec.approvals.node.set",
];

const PAIRING_METHODS: &[&str] = &[
    "node.pair.approve", "node.pair.reject",
    "device.pair.approve", "device.pair.reject", "device.pair.remove",
    "device.token.rotate", "device.token.revoke",
];

const NODE_ROLE_METHODS: &[&str] = &[
    "node.invoke.result", "node.pending.pull", "node.pending.ack",
    "node.event", "node.pair.request", "node.pair.verify",
];

// Chat methods: accessible to any authenticated connection
const CHAT_METHODS: &[&str] = &[
    "agent", "agent.wait", "chat.send", "chat.history", "chat.abort", "chat.inject",
    "send", "wake", "system-presence", "system-event",
    "exec.approval.request", "exec.approval.waitDecision",
    "poll",
];

fn check_scope(role: &Role, method: &str, scopes: &HashSet<String>) -> Result<(), RpcError> {
    if scopes.contains("operator.admin") { return Ok(()); }

    if NODE_ROLE_METHODS.contains(&method) {
        return if *role == Role::Node { Ok(()) } else { Err(rpc_error("FORBIDDEN")) };
    }
    if CHAT_METHODS.contains(&method) { return Ok(()); } // Any authenticated
    if READ_METHODS.contains(&method) {
        return require_any(scopes, &["operator.read", "operator.write"]);
    }
    if WRITE_METHODS.contains(&method) {
        return require(scopes, "operator.write");
    }
    if APPROVAL_METHODS.contains(&method) {
        return require(scopes, "operator.approvals");
    }
    if PAIRING_METHODS.contains(&method) {
        return require(scopes, "operator.pairing");
    }

    // DEFAULT DENY — unclassified methods require admin
    require(scopes, "operator.admin")
}
```

---

## 3. Nodes System

### 3.1 Data Model

```rust
// === Persistent (JSON files) ===

// ~/.synapse/pairing/nodes/pending.json
struct PendingNodeRequest {
    request_id: String,
    node_id: String,
    display_name: Option<String>,
    platform: Option<String>,       // "darwin", "linux", "win32", "ios", "android"
    version: Option<String>,
    device_family: Option<String>,  // "Mac", "iPhone", "RaspberryPi"
    model_identifier: Option<String>,
    caps: Vec<String>,              // ["exec", "canvas", "camera", "tts"]
    commands: Vec<String>,          // ["npm", "cargo", "python"]
    remote_ip: Option<String>,
    is_repair: bool,
    ts: u64,                        // created_at ms
    // TTL: 5 minutes
}

// ~/.synapse/pairing/nodes/paired.json
struct PairedNode {
    node_id: String,
    token: String,                  // Generated pairing token
    display_name: Option<String>,
    platform: Option<String>,
    version: Option<String>,
    device_family: Option<String>,
    model_identifier: Option<String>,
    caps: Vec<String>,
    commands: Vec<String>,
    remote_ip: Option<String>,
    created_at_ms: u64,
    approved_at_ms: u64,
    last_connected_at_ms: Option<u64>,
}

// === In-Memory (live connections) ===

struct NodeSession {
    node_id: String,
    conn_id: String,                // WebSocket connection ID
    display_name: Option<String>,
    platform: Option<String>,
    version: Option<String>,
    device_family: Option<String>,
    model_identifier: Option<String>,
    caps: Vec<String>,
    commands: Vec<String>,
    connected_at_ms: u64,
}

struct NodeRegistry {
    nodes_by_id: HashMap<String, NodeSession>,
    nodes_by_conn: HashMap<String, String>,     // conn_id → node_id
    pending_invokes: HashMap<String, PendingInvoke>,  // request_id → pending
}

struct PendingInvoke {
    node_id: String,
    command: String,
    tx: oneshot::Sender<NodeInvokeResult>,
    timeout: JoinHandle<()>,        // auto-cancel after 30s
}

struct NodeInvokeResult {
    ok: bool,
    payload: Option<Value>,
    error: Option<InvokeError>,
}
```

### 3.2 RPC Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `node.pair.request` | node_id, display_name, platform, caps, commands | request_id | Node requests pairing |
| `node.pair.list` | — | Vec<PairedNode> | List all paired nodes |
| `node.pair.approve` | request_id | PairedNode | Approve pending pairing |
| `node.pair.reject` | request_id | — | Reject pending pairing |
| `node.pair.verify` | node_id, token | ok | Node verifies its pairing |
| `node.list` | — | Vec<NodeSession> | List connected nodes |
| `node.describe` | node_id | NodeSession + commands | Describe node capabilities |
| `node.invoke` | node_id, command, params, timeout_ms, idempotency_key | InvokeResult | Invoke command on node |
| `node.invoke.result` | id, node_id, ok, payload, error | — | Node pushes invoke result |
| `node.pending.pull` | node_id | Vec<PendingAction> | Node pulls queued actions |
| `node.pending.ack` | ids | — | Node acknowledges pending |
| `node.event` | event, payload | — | Node pushes event |
| `node.rename` | node_id, display_name | — | Rename node |

### 3.3 Events

| Event | Payload | When |
|-------|---------|------|
| `node.pair.requested` | PendingNodeRequest | New pairing request |
| `node.pair.resolved` | { request_id, decision, node } | Pairing approved/rejected |
| `node.invoke.request` | { id, node_id, command, params } | Sent TO the target node |

---

## 4. Exec Approvals System

### 4.1 Data Model

```rust
// === Persistent: ~/.synapse/exec-approvals.json ===

struct ExecApprovalsConfig {
    version: u32,                   // 1
    socket: Option<SocketConfig> {  // Unix socket for local node IPC
        path: Option<String>,       // e.g. "~/.synapse/exec-approvals.sock"
        token: Option<String>,
    },
    defaults: ApprovalPolicy {
        security: Security,         // Deny | Allowlist | Full
        ask: AskPolicy,            // Off | OnMiss | Always
        ask_fallback: Security,    // What to do on timeout
        auto_allow_skills: bool,
    },
    agents: HashMap<String, AgentApprovalPolicy> {
        // Same as defaults + allowlist
        security: Security,
        ask: AskPolicy,
        ask_fallback: Security,
        auto_allow_skills: bool,
        allowlist: Vec<AllowlistEntry> {
            id: Option<String>,              // Stable entry ID
            pattern: String,                 // Glob: "npm", "/usr/bin/*", "*.sh"
            last_used_at: Option<u64>,
            last_used_command: Option<String>,
            last_resolved_path: Option<String>,  // Auditing: resolved binary path
        },
    },
}

enum Security { Deny, Allowlist, Full }
enum AskPolicy { Off, OnMiss, Always }

// === In-Memory: approval state machine ===

struct ExecApprovalManager {
    pending: HashMap<String, ApprovalRecord>,
}

struct ApprovalRecord {
    id: String,
    request: ApprovalRequestPayload {
        command: String,
        command_argv: Vec<String>,
        env_keys: Vec<String>,              // UI-safe env preview (no values)
        cwd: Option<String>,
        node_id: Option<String>,
        host: Option<String>,               // "sandbox" | "gateway" | "node"
        security: Option<String>,
        ask: Option<String>,
        agent_id: Option<String>,
        session_key: Option<String>,
        resolved_path: Option<String>,       // Resolved command binary path
        system_run_binding: Option<Value>,   // SystemRunApprovalBinding for argv matching
        system_run_plan: Option<Value>,      // SystemRunApprovalPlan for command plan
        turn_source_channel: Option<String>, // Which channel triggered this
        turn_source_account_id: Option<String>,
    },
    created_at_ms: u64,
    expires_at_ms: u64,
    requested_by_conn_id: Option<String>,
    resolved_at_ms: Option<u64>,
    decision: Option<ApprovalDecision>,
    resolved_by: Option<String>,
}

enum ApprovalDecision { AllowOnce, AllowAlways, Deny }

// Constants
const APPROVAL_TIMEOUT_MS: u64 = 120_000;      // 2 minutes
const RESOLVED_GRACE_MS: u64 = 15_000;          // 15s grace after resolution
```

### 4.2 RPC Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `exec.approval.request` | command, argv, env, cwd, node_id, host, agent_id | { id, status, created_at, expires_at } | Request approval |
| `exec.approval.waitDecision` | id | { id, decision } | Wait for approval decision |
| `exec.approval.resolve` | id, decision ("allow-once"\|"allow-always"\|"deny") | ok | Record decision |
| `exec.approvals.get` | — | { config, hash } | Get approval config + SHA256 |
| `exec.approvals.set` | file, base_hash | { config, hash } | Update config (CAS) |
| `exec.approvals.node.get` | node_id | config | Get node's approval config |
| `exec.approvals.node.set` | node_id, file, base_hash | config | Set node's approval config |

### 4.3 Events

| Event | Payload | When |
|-------|---------|------|
| `exec.approval.requested` | ApprovalRecord | New approval needed |
| `exec.approval.resolved` | { id, decision, resolved_by } | Decision made |

### 4.4 Approval Flow

```
Agent executes tool → SecurityCallback checks policy
  → security=deny: reject immediately
  → security=full: allow immediately
  → security=allowlist + command matches: allow
  → security=allowlist + no match + ask=on-miss:
      → exec.approval.request RPC
      → broadcast exec.approval.requested event
      → wait up to 120s for decision
      → operator resolves via exec.approval.resolve
      → broadcast exec.approval.resolved event
      → if timeout: apply ask_fallback policy
```

---

## 5. Presence System

### 5.1 Data Model

```rust
struct PresenceEntry {
    key: String,                    // Normalized lookup key
    host: Option<String>,
    ip: Option<String>,
    version: Option<String>,
    platform: Option<String>,       // "darwin 15.1", "linux", "web"
    device_family: Option<String>,  // "Mac", "iPhone", "Linux"
    model_identifier: Option<String>,
    mode: Option<String>,           // "gateway", "node", "webchat"
    reason: Option<String>,         // "self", "heartbeat", "connected", "disconnected"
    device_id: Option<String>,
    instance_id: Option<String>,
    roles: Vec<String>,             // ["gateway"], ["operator"], ["node"]
    scopes: Vec<String>,
    text: String,                   // Human-readable description
    ts: u64,                        // Last update timestamp
}

struct PresenceStore {
    entries: HashMap<String, PresenceEntry>,
    version: AtomicU64,
    // Constants
    // TTL: 5 minutes
    // MAX_ENTRIES: 200
}
```

### 5.2 RPC Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `system-presence` | text, device_id, host, ... | — | Update presence |
| `system-event` | text, device_id, ... | — | System event (also updates presence) |

### 5.3 Events

| Event | Payload | When |
|-------|---------|------|
| `presence` | { presence: Vec<PresenceEntry> } | Any presence change |

### 5.4 Presence Key Generation

Key is generated from the first available value (priority order):
1. `device_id` param
2. `instance_id` param
3. Parsed from presence text
4. `host` from text
5. `ip` from text
6. First 64 chars of text
7. `os::hostname().to_lowercase()`

All keys are normalized (trimmed + lowercased) for case-insensitive dedup.

### 5.5 Lifecycle

- Gateway registers self-presence on startup (hostname, IP, platform, version)
- Each WS client registers presence on `connect`
- Presence pruned on list (TTL 5min, max 200 entries)
- Disconnect → update presence with reason "disconnected"
- `tick` event sent periodically (configurable interval)

---

## 6. TTS/Voice System

### 6.1 Data Model

```rust
struct TtsState {
    enabled: bool,
    provider: Option<String>,       // "openai", "azure", "elevenlabs"
    providers: Vec<TtsProvider>,
}

struct TtsProvider {
    name: String,
    voices: Vec<String>,
    default_voice: String,
}
```

### 6.2 RPC Methods

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `tts.status` | — | { enabled, provider, voice } | Get TTS status |
| `tts.providers` | — | Vec<TtsProvider> | List available providers |
| `tts.enable` | provider, voice | ok | Enable TTS |
| `tts.disable` | — | ok | Disable TTS |
| `tts.convert` | text, voice | { audio_url } | Convert text to speech |
| `tts.setProvider` | provider, voice | ok | Change provider |

---

## 7. AppState Extensions

```rust
pub struct AppState {
    // === Existing ===
    pub config: SynapseConfig,
    pub model: Arc<dyn ChatModel>,
    pub sessions: Arc<SessionManager>,
    pub cancel_tokens: Arc<RwLock<HashMap<String, watch::Sender<bool>>>>,
    pub auth: Option<Arc<AuthState>>,
    pub started_at: Instant,
    pub cost_tracker: Arc<CostTrackingCallback>,
    pub request_metrics: RequestMetrics,
    pub write_lock: Arc<SessionWriteLock>,
    pub log_buffer: LogBuffer,
    pub mcp_tools: Vec<Arc<dyn Tool>>,

    // === New ===
    pub broadcaster: Arc<Broadcaster>,
    pub rpc_router: Arc<RpcRouter>,
    pub node_registry: Arc<RwLock<NodeRegistry>>,
    pub presence: Arc<RwLock<PresenceStore>>,
    pub exec_approvals: Arc<RwLock<ExecApprovalManager>>,
    pub tts: Arc<RwLock<TtsState>>,
}
```

---

## 8. Frontend Changes

### 8.1 WS RPC Client Layer

Replace REST fetch calls with WebSocket RPC:

```typescript
// web/src/lib/gateway-client.ts
class GatewayClient {
    private ws: WebSocket;
    private pending: Map<string, { resolve, reject, timeout }>;
    private seq: number = 0;
    private eventHandlers: Map<string, Set<(payload) => void>>;

    connect(url: string, auth: AuthParams): Promise<HelloOk>;
    request<T>(method: string, params?: any): Promise<T>;
    onEvent(event: string, handler: (payload) => void): () => void;
    close(): void;
}

// Usage in components
const client = useGatewayClient();
const sessions = await client.request('sessions.list', { limit: 50 });
client.onEvent('presence', (p) => setPresence(p.presence));
```

### 8.2 New Pages

**Instances Page** (`/dashboard/instances`):
- Connected instances grid (cards showing: node_id, type, platform, version, roles, scopes, last_seen)
- Refresh button
- Real-time updates via `presence` events

**Nodes Page** (`/dashboard/nodes`):
- Left: Exec Approvals config editor (security mode, ask policy, allowlist patterns)
- Right: Paired nodes list with approve/reject actions for pending requests
- Scope selector (Defaults vs per-agent)
- Save button with CAS (compare-and-swap using hash)

### 8.3 Enhanced Pages

**Chat Toolbar**:
- Focus mode toggle (hide sidebar + header)
- Thinking output toggle
- Cron session toggle

**Sessions Page**:
- Add verbose and reasoning dropdown overrides per session

**Usage Page**:
- Token/Cost view toggle
- Advanced filter syntax input

### 8.4 Sidebar Updates

```
聊天 (Chat)
  └─ 聊天

控制 (Control)
  ├─ 概览 (Overview)
  ├─ 频道 (Channels)
  ├─ 实例 (Instances)     ← NEW
  ├─ 会话 (Sessions)
  ├─ 使用情况 (Usage)
  └─ 定时任务 (Schedules)

代理 (Agent)
  ├─ 代理 (Agents)
  ├─ 技能 (Skills)
  ├─ 节点 (Nodes)         ← NEW (was Workspace, renamed)
  └─ 工作区 (Workspace)

设置 (Settings)
  ├─ 配置 (Config)
  ├─ 调试 (Debug)
  └─ 日志 (Logs)

资源 (Resources)          ← NEW
  └─ 文档 (Docs link)
```

---

## 9. ws.rs Rewrite Plan

The current `ws.rs` (1203 lines) will be restructured:

```
ws.rs (rewritten):
  - Protocol v3 frame parsing (req/res/event)
  - Handshake: connect.challenge → connect → hello-ok
  - Connection lifecycle: register in Broadcaster + PresenceStore
  - Main loop: dispatch to RpcRouter for all requests
  - Agent streaming: "agent" and "chat.send" methods stream via events
  - Approval: integrated into exec_approvals RPC module
  - Tick: periodic heartbeat event
  - Cleanup: unregister from Broadcaster + PresenceStore on disconnect
```

**Backward compatibility**: Runtime protocol negotiation — if first client frame uses old `{"type":"message",...}` format, fall back to legacy handler. Protocol v3 clients send `{"type":"req","method":"connect",...}`. This allows both old and new frontends to work with the same binary during migration.

---

## 10. File Structure Summary

### New Backend Files

```
src/gateway/
  rpc/
    mod.rs              — RpcRouter, RpcContext, Broadcaster
    health.rs           — health, status, doctor.memory.status
    chat.rs             — agent, chat.send/history/abort, agent.identity.get/wait
    sessions.rs         — sessions.* (7 methods)
    agents.rs           — agents.* (7 methods)
    skills.rs           — skills.* (4 methods)
    channels.rs         — channels.* (2 methods)
    config.rs           — config.* (6 methods)
    schedules.rs        — cron.* (7 methods)
    usage.rs            — usage.* (2 methods)
    nodes.rs            — node.* (14 methods)
    devices.rs          — device.* (6 methods)
    exec_approvals.rs   — exec.approval.* + exec.approvals.* (7 methods)
    presence.rs         — system-presence/event (2 methods)
    logs.rs             — logs.tail (1 method)
    tts.rs              — tts.* (6 methods)
    talk.rs             — talk.config/mode (2 methods)
    voicewake.rs        — voicewake.get/set (2 methods)
    models.rs           — models.list (1 method)
    tools.rs            — tools.catalog (1 method)
    workspace.rs        — workspace CRUD (migrated from dashboard)
    store.rs            — skill store (migrated from dashboard)
    secrets.rs          — secrets.reload/resolve (2 methods)
    updates.rs          — update.run (1 method)
    send.rs             — send, wake (2 methods)
    wizard.rs           — wizard.start/next/cancel/status (4 methods)
    browser.rs          — browser.request (1 method)
    heartbeat.rs        — last-heartbeat, set-heartbeats (2 methods)
    web_login.rs        — web.login.start/wait (2 methods)
    push.rs             — push.test (1 method)
  nodes/
    mod.rs              — NodeRegistry
    pairing.rs          — PendingStore + PairedStore (JSON persistence)
  presence.rs           — PresenceStore
  exec_approvals/
    mod.rs              — ExecApprovalManager (state machine)
    config.rs           — ExecApprovalsConfig (JSON persistence)
    policy.rs           — Policy evaluation (security/ask/allowlist matching)
  tts.rs                — TtsState
  ws.rs                 — Rewritten with Protocol v3
```

### New Frontend Files

```
web/src/
  lib/
    gateway-client.ts       — WS RPC client (replaces REST fetches)
  components/dashboard/
    InstancesPage.tsx        — NEW: connected instances
    NodesPage.tsx            — NEW: exec approvals + node pairing
```

### Modified Frontend Files

```
web/src/
  components/
    Sidebar.tsx              — Add Instances, Nodes, Resources section
    Dashboard.tsx            — Add new tabs + routes
    ChatPanel.tsx            — Focus mode, thinking toggle, cron toggle
  components/dashboard/
    SessionsPage.tsx         — Add verbose/reasoning overrides
    UsagePage.tsx            — Add cost toggle, advanced filters
  hooks/
    useGatewayWS.ts          — Rewrite to use new protocol
```

---

## 11. Total Method Count

| Module | Methods | Notes |
|--------|---------|-------|
| Health/System | 3 | health, status, doctor.memory.status |
| Chat/Agent | 8 | agent, agent.identity.get, agent.wait, chat.send/history/abort/inject, poll |
| Sessions | 11 | list/get/preview/resolve/patch/reset/delete/compact/usage/usage.timeseries/usage.logs |
| Agents | 7 | list/create/update/delete + files.list/get/set |
| Skills | 4 | status/bins/install/update |
| Channels | 2 | status/logout |
| Config | 6 | get/set/apply/patch/schema/schema.lookup |
| Cron | 7 | list/status/add/update/remove/run/runs |
| Usage | 2 | status/cost |
| Nodes | 14 | pair.request/list/approve/reject/verify, list/describe/invoke/invoke.result/pending.pull/pending.ack/event/rename/canvas.capability.refresh |
| Devices | 6 | pair.list/approve/reject/remove + token.rotate/revoke |
| Exec Approvals | 7 | approval.request/waitDecision/resolve + approvals.get/set/node.get/node.set |
| Presence | 2 | system-presence, system-event |
| Logs | 1 | logs.tail |
| TTS | 6 | status/providers/enable/disable/convert/setProvider |
| Talk/Voice | 4 | talk.config, talk.mode, voicewake.get, voicewake.set |
| Models | 1 | models.list |
| Tools | 1 | tools.catalog |
| Workspace | 5 | list/get/set/create/delete |
| Store | 4 | search/list/install/status |
| Secrets | 2 | reload/resolve |
| Updates | 1 | update.run |
| Send/Wake | 2 | send, wake |
| Heartbeat | 2 | last-heartbeat, set-heartbeats |
| Wizard | 4 | wizard.start/next/cancel/status |
| Browser | 1 | browser.request |
| Web Login | 2 | web.login.start, web.login.wait |
| Push | 1 | push.test |
| **Total** | **~106** | Fully aligned with OpenClaw |
