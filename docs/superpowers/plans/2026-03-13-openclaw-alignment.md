# OpenClaw Full Alignment Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fully align Synapse's gateway with OpenClaw's WebSocket RPC protocol v3, implementing 106 RPC methods across 6 subsystems with matching frontend.

**Architecture:** Replace current mixed REST+WS architecture with a unified WebSocket RPC protocol (Protocol v3). The RpcRouter dispatches typed handlers. Existing REST endpoints are preserved as thin wrappers during migration. Frontend switches from REST fetch to a WS RPC client class.

**Tech Stack:** Rust (Axum, tokio, serde), React 19, TypeScript, WebSocket, JSON-RPC-like protocol

**Spec:** `docs/superpowers/specs/2026-03-13-openclaw-alignment-design.md`

---

## Chunk 1: WS RPC Infrastructure (Foundation)

Everything depends on this. Build the protocol v3 frame types, RpcRouter, Broadcaster, handshake, and connection lifecycle.

### Task 1: Protocol v3 Frame Types

**Files:**
- Create: `src/gateway/rpc/mod.rs`
- Create: `src/gateway/rpc/types.rs`

- [ ] **Step 1: Create `src/gateway/rpc/types.rs` — frame types**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol version
pub const PROTOCOL_VERSION: u32 = 3;

// === Client → Server ===

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientFrame {
    #[serde(rename = "req")]
    Request {
        id: String,
        method: String,
        #[serde(default)]
        params: Value,
    },
}

// === Server → Client ===

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerFrame {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVersion {
    pub presence: u64,
    pub health: u64,
}

// === Handshake types ===

#[derive(Debug, Deserialize)]
pub struct ConnectParams {
    pub min_protocol: u32,
    pub max_protocol: u32,
    pub client: ClientInfo,
    #[serde(default)]
    pub caps: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub permissions: Option<std::collections::HashMap<String, bool>>,
    pub path_env: Option<String>,
    pub auth: Option<AuthParams>,
    pub device: Option<DeviceAuth>,
    pub locale: Option<String>,
    pub user_agent: Option<String>,
}

fn default_role() -> String { "operator".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub platform: Option<String>,
    pub device_family: Option<String>,
    pub model_identifier: Option<String>,
    pub mode: Option<String>,
    pub instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AuthParams {
    pub token: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceAuth {
    pub id: String,
    pub public_key: String,
    pub signature: String,
    pub signed_at: u64,
    pub nonce: String,
}

#[derive(Debug, Serialize)]
pub struct HelloOk {
    pub protocol: u32,
    pub server: ServerInfo,
    pub features: Features,
    pub snapshot: Snapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthResult>,
    pub policy: Policy,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub version: String,
    pub conn_id: String,
}

#[derive(Debug, Serialize)]
pub struct Features {
    pub methods: Vec<String>,
    pub events: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub presence: Vec<Value>,
    pub health: Value,
    pub state_version: StateVersion,
}

#[derive(Debug, Serialize)]
pub struct AuthResult {
    pub role: String,
    pub scopes: Vec<String>,
    pub issued_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Policy {
    pub max_payload: usize,
    pub max_buffered_bytes: usize,
    pub tick_interval_ms: u64,
}

// === Helpers ===

impl ServerFrame {
    pub fn ok(id: String, payload: Value) -> Self {
        Self::Response { id, ok: true, payload: Some(payload), error: None }
    }

    pub fn err(id: String, error: RpcError) -> Self {
        Self::Response { id, ok: false, payload: None, error: Some(error) }
    }

    pub fn event(event: &str, payload: Value) -> Self {
        Self::Event {
            event: event.to_string(),
            payload: Some(payload),
            seq: None,
            state_version: None,
        }
    }
}

impl RpcError {
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self { code: "INVALID_REQUEST".into(), message: msg.into(), details: None, retryable: None, retry_after_ms: None }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self { code: "NOT_FOUND".into(), message: msg.into(), details: None, retryable: None, retry_after_ms: None }
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self { code: "FORBIDDEN".into(), message: msg.into(), details: None, retryable: None, retry_after_ms: None }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { code: "INTERNAL_ERROR".into(), message: msg.into(), details: None, retryable: Some(true), retry_after_ms: None }
    }
    pub fn method_not_found(method: &str) -> Self {
        Self { code: "METHOD_NOT_FOUND".into(), message: format!("unknown method: {method}"), details: None, retryable: None, retry_after_ms: None }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/bytedance/code/github/synapse && cargo check --features web 2>&1 | head -20`
Expected: No errors in rpc/types.rs

- [ ] **Step 3: Commit**

```bash
git add src/gateway/rpc/
git commit -m "feat(gateway): add protocol v3 frame types"
```

### Task 2: RpcRouter + Scope Checks

**Files:**
- Create: `src/gateway/rpc/router.rs`
- Create: `src/gateway/rpc/scopes.rs`
- Modify: `src/gateway/rpc/mod.rs`

- [ ] **Step 1: Create `src/gateway/rpc/scopes.rs` — scope authorization (default-deny)**

```rust
use std::collections::HashSet;
use super::types::RpcError;

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Operator,
    Node,
}

impl Role {
    pub fn from_str(s: &str) -> Self {
        match s {
            "node" => Self::Node,
            _ => Self::Operator,
        }
    }
}

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

const CHAT_METHODS: &[&str] = &[
    "connect",
    "agent", "agent.wait", "chat.send", "chat.history", "chat.abort", "chat.inject",
    "send", "wake", "system-presence", "system-event",
    "exec.approval.request", "exec.approval.waitDecision",
    "poll",
];

pub fn check_scope(role: &Role, method: &str, scopes: &HashSet<String>) -> Result<(), RpcError> {
    // Admin bypasses all
    if scopes.contains("operator.admin") {
        return Ok(());
    }

    if NODE_ROLE_METHODS.contains(&method) {
        return if *role == Role::Node {
            Ok(())
        } else {
            Err(RpcError::forbidden("requires node role"))
        };
    }

    if CHAT_METHODS.contains(&method) {
        return Ok(()); // Any authenticated connection
    }

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

    // DEFAULT DENY
    require(scopes, "operator.admin")
}

fn require(scopes: &HashSet<String>, scope: &str) -> Result<(), RpcError> {
    if scopes.contains(scope) {
        Ok(())
    } else {
        Err(RpcError::forbidden(format!("requires scope: {scope}")))
    }
}

fn require_any(scopes: &HashSet<String>, required: &[&str]) -> Result<(), RpcError> {
    if required.iter().any(|s| scopes.contains(*s)) {
        Ok(())
    } else {
        Err(RpcError::forbidden(format!("requires one of: {}", required.join(", "))))
    }
}
```

- [ ] **Step 2: Create `src/gateway/rpc/router.rs` — RPC router with dispatch**

```rust
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::collections::HashSet;

use serde_json::Value;

use super::scopes::{Role, check_scope};
use super::types::{ClientInfo, RpcError, ServerFrame};
use crate::gateway::state::AppState;

/// Context passed to every RPC handler.
pub struct RpcContext {
    pub state: AppState,
    pub conn_id: String,
    pub client: ClientInfo,
    pub role: Role,
    pub scopes: HashSet<String>,
    pub broadcaster: Arc<Broadcaster>,
}

/// Type-erased async RPC handler.
pub type RpcHandler = Box<
    dyn Fn(Arc<RpcContext>, Value) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send>>
        + Send
        + Sync,
>;

/// Central RPC method router.
pub struct RpcRouter {
    methods: HashMap<String, RpcHandler>,
}

impl RpcRouter {
    pub fn new() -> Self {
        Self { methods: HashMap::new() }
    }

    /// Register a method handler. Handler receives (ctx, params) -> Result<Value, RpcError>.
    pub fn register<F, Fut>(&mut self, method: &str, handler: F)
    where
        F: Fn(Arc<RpcContext>, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, RpcError>> + Send + 'static,
    {
        let method = method.to_string();
        self.methods.insert(
            method,
            Box::new(move |ctx, params| Box::pin(handler(ctx, params))),
        );
    }

    /// Dispatch an RPC request. Checks scopes then invokes the handler.
    pub async fn dispatch(&self, ctx: Arc<RpcContext>, id: String, method: &str, params: Value) -> ServerFrame {
        // Scope check (default-deny)
        if let Err(e) = check_scope(&ctx.role, method, &ctx.scopes) {
            return ServerFrame::err(id, e);
        }

        match self.methods.get(method) {
            Some(handler) => match handler(ctx, params).await {
                Ok(result) => ServerFrame::ok(id, result),
                Err(e) => ServerFrame::err(id, e),
            },
            None => ServerFrame::err(id, RpcError::method_not_found(method)),
        }
    }

    /// List all registered method names (for hello-ok features).
    pub fn method_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.methods.keys().cloned().collect();
        names.sort();
        names
    }
}

/// Manages all active WebSocket connections for event broadcasting.
pub struct Broadcaster {
    connections: tokio::sync::RwLock<HashMap<String, tokio::sync::mpsc::UnboundedSender<ServerFrame>>>,
    seq: std::sync::atomic::AtomicU64,
}

impl Broadcaster {
    pub fn new() -> Self {
        Self {
            connections: tokio::sync::RwLock::new(HashMap::new()),
            seq: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub async fn register(&self, conn_id: String, tx: tokio::sync::mpsc::UnboundedSender<ServerFrame>) {
        self.connections.write().await.insert(conn_id, tx);
    }

    pub async fn unregister(&self, conn_id: &str) {
        self.connections.write().await.remove(conn_id);
    }

    /// Broadcast an event to ALL connected clients.
    pub async fn broadcast(&self, event: &str, payload: Value) {
        let seq = self.seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let frame = ServerFrame::Event {
            event: event.to_string(),
            payload: Some(payload),
            seq: Some(seq),
            state_version: None,
        };
        let conns = self.connections.read().await;
        for tx in conns.values() {
            let _ = tx.send(frame.clone());
        }
    }

    /// Send event to a specific connection.
    pub async fn send_to(&self, conn_id: &str, frame: ServerFrame) {
        let conns = self.connections.read().await;
        if let Some(tx) = conns.get(conn_id) {
            let _ = tx.send(frame);
        }
    }

    /// Number of active connections.
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}
```

- [ ] **Step 3: Create `src/gateway/rpc/mod.rs` — module root**

```rust
pub mod types;
pub mod router;
pub mod scopes;

pub use router::{Broadcaster, RpcContext, RpcRouter};
pub use types::*;
```

- [ ] **Step 4: Wire into gateway — add `pub mod rpc;` to `src/gateway/mod.rs`**

Add `pub mod rpc;` declaration. Do NOT modify the existing router setup yet — just make the module available.

- [ ] **Step 5: Verify it compiles**

Run: `cargo check --features web 2>&1 | head -20`

- [ ] **Step 6: Commit**

```bash
git add src/gateway/rpc/
git commit -m "feat(gateway): add RpcRouter, Broadcaster, scope checks"
```

### Task 3: Extend AppState with New Subsystems

**Files:**
- Modify: `src/gateway/state.rs`

- [ ] **Step 1: Add new fields to AppState**

Add to the `AppState` struct:

```rust
pub broadcaster: Arc<Broadcaster>,
pub rpc_router: Arc<RpcRouter>,
```

Import `Broadcaster` and `RpcRouter` from `super::rpc`.

- [ ] **Step 2: Initialize in `AppState::new()`**

```rust
let broadcaster = Arc::new(Broadcaster::new());
let rpc_router = Arc::new(RpcRouter::new()); // Empty for now, methods registered later
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check --features web`

- [ ] **Step 4: Commit**

```bash
git add src/gateway/state.rs
git commit -m "feat(gateway): extend AppState with Broadcaster and RpcRouter"
```

### Task 4: RPC Health Method (First Method)

**Files:**
- Create: `src/gateway/rpc/health.rs`
- Modify: `src/gateway/rpc/mod.rs`

- [ ] **Step 1: Create `src/gateway/rpc/health.rs`**

```rust
use std::sync::Arc;
use serde_json::{json, Value};
use super::router::RpcContext;
use super::types::RpcError;

pub async fn handle_health(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let uptime = ctx.state.started_at.elapsed().as_secs();
    Ok(json!({
        "ok": true,
        "uptime_secs": uptime,
        "duration": format_duration(uptime),
    }))
}

pub async fn handle_status(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let uptime = ctx.state.started_at.elapsed().as_secs();
    let ws_count = ctx.broadcaster.connection_count().await;
    let auth_enabled = ctx.state.auth.as_ref().map(|a| a.config.enabled).unwrap_or(false);

    Ok(json!({
        "status": "ok",
        "uptime_secs": uptime,
        "auth_enabled": auth_enabled,
        "connections": ws_count,
    }))
}

fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
```

- [ ] **Step 2: Add module + register methods**

In `rpc/mod.rs`, add `pub mod health;`.

Create a `pub fn register_all(router: &mut RpcRouter)` function in `rpc/mod.rs`:

```rust
pub fn register_all(router: &mut RpcRouter) {
    router.register("health", health::handle_health);
    router.register("status", health::handle_status);
}
```

- [ ] **Step 3: Call `register_all` during AppState init**

In `state.rs`, before wrapping router in Arc:

```rust
let mut rpc_router = RpcRouter::new();
super::rpc::register_all(&mut rpc_router);
let rpc_router = Arc::new(rpc_router);
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check --features web`

- [ ] **Step 5: Commit**

```bash
git add src/gateway/rpc/
git commit -m "feat(gateway): add health/status RPC methods"
```

### Task 5: Protocol v3 WebSocket Handler

**Files:**
- Modify: `src/gateway/ws.rs`

This is the biggest task. Rewrite the WS handler to support Protocol v3 with backward compatibility.

- [ ] **Step 1: Add protocol detection + v3 handshake to `handle_socket()`**

After the WebSocket connection is established:

1. Generate `conn_id` (UUID)
2. Send `connect.challenge` event with nonce
3. Wait for first client frame
4. If frame is `{"type":"req","method":"connect",...}` → Protocol v3 path
5. If frame is `{"type":"message",...}` → Legacy path (existing code)
6. On v3 connect: validate auth, build RpcContext, send hello-ok, enter v3 main loop
7. On v3 main loop: parse ClientFrame, dispatch via RpcRouter, send ServerFrame

The key change: the main loop becomes:

```rust
// Protocol v3 main loop
loop {
    tokio::select! {
        // Incoming client frames
        Some(msg) = receiver.next() => {
            if let Ok(text) = msg?.into_text() {
                match serde_json::from_str::<ClientFrame>(&text) {
                    Ok(ClientFrame::Request { id, method, params }) => {
                        // Special handling for "agent"/"chat.send" (streaming)
                        if method == "agent" || method == "chat.send" {
                            // Existing agent execution logic (adapted)
                        } else {
                            // Standard RPC dispatch
                            let frame = rpc_router.dispatch(ctx.clone(), id, &method, params).await;
                            sender.send(to_ws_msg(&frame)).await?;
                        }
                    }
                    Err(e) => {
                        // Send error frame
                    }
                }
            }
        }
        // Outgoing events from broadcaster
        Some(frame) = event_rx.recv() => {
            sender.send(to_ws_msg(&frame)).await?;
        }
    }
}
```

- [ ] **Step 2: Keep legacy path working**

The legacy path wraps the old `WsCommand` handling in a function called `handle_legacy_connection()`. The v3 path goes through `handle_v3_connection()`. Protocol detection happens on the first frame.

- [ ] **Step 3: Register connection in Broadcaster on connect, unregister on disconnect**

```rust
// On connect
state.broadcaster.register(conn_id.clone(), event_tx).await;
// On disconnect (in finally/drop)
state.broadcaster.unregister(&conn_id).await;
```

- [ ] **Step 4: Verify both protocols work**

Run: `cargo check --features web`
Manual test: Connect existing frontend (should use legacy protocol and still work).

- [ ] **Step 5: Commit**

```bash
git add src/gateway/ws.rs
git commit -m "feat(gateway): protocol v3 WebSocket handler with backward compat"
```

### Task 6: Events List Constants

**Files:**
- Create: `src/gateway/rpc/events.rs`

- [ ] **Step 1: Create events registry**

```rust
/// All events the gateway can broadcast.
pub const GATEWAY_EVENTS: &[&str] = &[
    "connect.challenge",
    "agent",
    "chat",
    "presence",
    "tick",
    "heartbeat",
    "health",
    "shutdown",
    "cron",
    "node.pair.requested",
    "node.pair.resolved",
    "node.invoke.request",
    "device.pair.requested",
    "device.pair.resolved",
    "exec.approval.requested",
    "exec.approval.resolved",
    "talk.mode",
    "voicewake.changed",
    "update.available",
];
```

- [ ] **Step 2: Use in hello-ok response**

```rust
features: Features {
    methods: rpc_router.method_names(),
    events: GATEWAY_EVENTS.iter().map(|s| s.to_string()).collect(),
}
```

- [ ] **Step 3: Commit**

```bash
git add src/gateway/rpc/events.rs
git commit -m "feat(gateway): add events registry for protocol v3"
```

---

## Chunk 2: Migrate Existing REST to RPC Methods

Migrate all existing dashboard REST endpoints into RPC handler modules. Each module extracts the handler logic from `api/dashboard.rs` into its own RPC file.

### Task 7: Sessions RPC Module

**Files:**
- Create: `src/gateway/rpc/sessions.rs`

- [ ] **Step 1: Create sessions RPC handlers**

Migrate logic from `api/dashboard.rs` session endpoints into:
- `sessions.list` — from `GET /api/dashboard/sessions`
- `sessions.get` — from `GET /api/dashboard/sessions/{id}` (NEW)
- `sessions.preview` — return last N messages for a session
- `sessions.resolve` — resolve session by key/label (NEW)
- `sessions.patch` — from `PATCH /api/dashboard/sessions/{id}`
- `sessions.reset` — clear session messages
- `sessions.delete` — from `DELETE /api/dashboard/sessions/{id}`
- `sessions.compact` — from `POST /api/dashboard/sessions/{id}/compact`
- `sessions.usage` — per-session token/cost breakdown
- `sessions.usage.timeseries` — usage over time for a session
- `sessions.usage.logs` — usage audit log entries

Each handler: `async fn handle_xxx(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError>`

- [ ] **Step 2: Register in `register_all()`**

- [ ] **Step 3: Verify compile + commit**

### Task 8: Agents RPC Module

**Files:**
- Create: `src/gateway/rpc/agents.rs`

- [ ] **Step 1: Create agents RPC handlers**

Migrate from dashboard:
- `agents.list` / `agents.create` / `agents.update` / `agents.delete`
- `agents.files.list` / `agents.files.get` / `agents.files.set` (NEW — workspace file ops per agent)

- [ ] **Step 2: Register + compile + commit**

### Task 9: Skills RPC Module

**Files:**
- Create: `src/gateway/rpc/skills.rs`

- [ ] **Step 1: Create skills RPC handlers**

- `skills.status` — list all skills with enabled/disabled state
- `skills.bins` — list skill binaries/executables
- `skills.install` — install from store
- `skills.update` — update skill

- [ ] **Step 2: Register + compile + commit**

### Task 10: Channels RPC Module

**Files:**
- Create: `src/gateway/rpc/channels.rs`

- [ ] **Step 1: Create channel RPC handlers**

- `channels.status` — list all channels with connected/configured state
- `channels.logout` — disconnect from a channel

- [ ] **Step 2: Register + compile + commit**

### Task 11: Config RPC Module

**Files:**
- Create: `src/gateway/rpc/config.rs`

- [ ] **Step 1: Create config RPC handlers**

- `config.get` / `config.set` / `config.apply` / `config.patch`
- `config.schema` / `config.schema.lookup`

Config reload: on `config.apply`, reload SynapseConfig and propagate to running subsystems (model, channels, etc.).

- [ ] **Step 2: Register + compile + commit**

### Task 12: Schedules (Cron) RPC Module

**Files:**
- Create: `src/gateway/rpc/schedules.rs`

- [ ] **Step 1: Create cron RPC handlers**

- `cron.list` / `cron.status` / `cron.add` / `cron.update` / `cron.remove` / `cron.run` / `cron.runs`

- [ ] **Step 2: Register + compile + commit**

### Task 13: Usage RPC Module

**Files:**
- Create: `src/gateway/rpc/usage.rs`

- [ ] **Step 1: Create usage RPC handlers**

- `usage.status` — aggregated usage summary
- `usage.cost` — cost breakdown by model/session

- [ ] **Step 2: Register + compile + commit**

### Task 14: Logs, Models, Tools, Workspace, Store, Debug RPC Modules

**Files:**
- Create: `src/gateway/rpc/logs.rs`
- Create: `src/gateway/rpc/models.rs`
- Create: `src/gateway/rpc/tools.rs`
- Create: `src/gateway/rpc/workspace.rs`
- Create: `src/gateway/rpc/store.rs`
- Create: `src/gateway/rpc/debug.rs`

- [ ] **Step 1: Create all remaining migration modules**

Each one extracts logic from the existing `dashboard.rs` handlers.

- `logs.tail` — stream logs from log buffer
- `models.list` — list configured model providers
- `tools.catalog` — list available tools
- `workspace.*` — workspace file CRUD (5 methods)
- `store.*` — skill store search/install (4 methods from ClawHub)
- `debug.*` — debug invoke/health

- [ ] **Step 2: Register all + compile + commit**

### Task 15: Chat/Agent RPC Module (Streaming)

**Files:**
- Create: `src/gateway/rpc/chat.rs`

This is the most complex migration — agent streaming needs special handling.

- [ ] **Step 1: Create chat RPC handlers**

- `chat.send` — sends message, streams response via events, returns final result
- `chat.history` — get message history for session
- `chat.abort` — cancel running agent
- `chat.inject` — inject message without triggering agent (admin)
- `agent` — alias for chat.send (OpenClaw compat)
- `agent.identity.get` — get agent identity (name, emoji, description)
- `agent.wait` — wait for running agent to complete
- `poll` — polling fallback for non-WS clients

For `chat.send` / `agent`: the handler starts streaming, sends `agent` events through broadcaster, and returns the final response as the RPC result.

- [ ] **Step 2: Register + compile + commit**

---

## Chunk 3: New Backend Subsystems

### Task 16: Presence System

**Files:**
- Create: `src/gateway/presence.rs`
- Create: `src/gateway/rpc/presence.rs`

- [ ] **Step 1: Create PresenceStore**

```rust
// src/gateway/presence.rs
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Serialize, Deserialize};

const TTL_MS: u64 = 5 * 60 * 1000; // 5 minutes
const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub key: String,
    pub host: Option<String>,
    pub ip: Option<String>,
    pub version: Option<String>,
    pub platform: Option<String>,
    pub device_family: Option<String>,
    pub model_identifier: Option<String>,
    pub mode: Option<String>,
    pub reason: Option<String>,
    pub device_id: Option<String>,
    pub instance_id: Option<String>,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
    pub text: String,
    pub ts: u64,
}

pub struct PresenceStore {
    entries: HashMap<String, PresenceEntry>,
    version: AtomicU64,
}

impl PresenceStore {
    pub fn new() -> Self { ... }
    pub fn upsert(&mut self, entry: PresenceEntry) -> bool { ... } // returns true if changed
    pub fn remove(&mut self, key: &str) { ... }
    pub fn list(&mut self) -> Vec<PresenceEntry> { ... } // prune + return sorted
    pub fn version(&self) -> u64 { ... }

    fn normalize_key(key: &str) -> String { key.trim().to_lowercase() }
    fn prune(&mut self) { ... } // remove expired + enforce MAX_ENTRIES
    fn generate_key(entry: &PresenceEntry) -> String { ... } // priority: device_id > instance_id > host > ip > text
}
```

- [ ] **Step 2: Create presence RPC handlers**

```rust
// src/gateway/rpc/presence.rs
pub async fn handle_system_presence(ctx, params) -> Result<Value, RpcError> { ... }
pub async fn handle_system_event(ctx, params) -> Result<Value, RpcError> { ... }
```

Both update the PresenceStore and broadcast `presence` event.

- [ ] **Step 3: Add PresenceStore to AppState, register self-presence on startup**

- [ ] **Step 4: Register in Broadcaster on WS connect, update presence on disconnect**

- [ ] **Step 5: Compile + commit**

### Task 17: Node System — Pairing Persistence

**Files:**
- Create: `src/gateway/nodes/mod.rs`
- Create: `src/gateway/nodes/pairing.rs`

- [ ] **Step 1: Create pairing persistence layer**

JSON file persistence for `~/.synapse/pairing/nodes/pending.json` and `paired.json`.

```rust
pub struct PairingStore {
    pending: Vec<PendingNodeRequest>,   // TTL 5 min
    paired: Vec<PairedNode>,
    data_dir: PathBuf,
}

impl PairingStore {
    pub fn load(data_dir: &Path) -> Self { ... }
    pub fn add_pending(&mut self, req: PendingNodeRequest) { ... }
    pub fn approve(&mut self, request_id: &str) -> Option<PairedNode> { ... }
    pub fn reject(&mut self, request_id: &str) -> bool { ... }
    pub fn verify(&self, node_id: &str, token: &str) -> bool { ... }
    pub fn list_paired(&self) -> &[PairedNode] { ... }
    pub fn list_pending(&mut self) -> Vec<&PendingNodeRequest> { ... } // prune expired
    fn save(&self) { ... } // atomic JSON write
}
```

- [ ] **Step 2: Compile + commit**

### Task 18: Node System — Live Registry + Invoke

**Files:**
- Create: `src/gateway/nodes/registry.rs`
- Modify: `src/gateway/nodes/mod.rs`

- [ ] **Step 1: Create NodeRegistry**

```rust
pub struct NodeRegistry {
    sessions: HashMap<String, NodeSession>,     // node_id → session
    by_conn: HashMap<String, String>,           // conn_id → node_id
    pending_invokes: HashMap<String, PendingInvoke>,
    pending_actions: HashMap<String, Vec<PendingAction>>, // node_id → queued
}

impl NodeRegistry {
    pub fn register(&mut self, session: NodeSession) { ... }
    pub fn unregister_by_conn(&mut self, conn_id: &str) -> Option<String> { ... }
    pub fn get(&self, node_id: &str) -> Option<&NodeSession> { ... }
    pub fn list(&self) -> Vec<&NodeSession> { ... }
    pub async fn invoke(&mut self, node_id: &str, command: &str, params: Value, timeout_ms: u64, idempotency_key: &str, broadcaster: &Broadcaster) -> Result<NodeInvokeResult, RpcError> { ... }
    pub fn handle_invoke_result(&mut self, id: &str, result: NodeInvokeResult) -> bool { ... }
    pub fn pull_pending(&mut self, node_id: &str) -> Vec<PendingAction> { ... }
    pub fn ack_pending(&mut self, ids: &[String]) { ... }
}
```

- [ ] **Step 2: Compile + commit**

### Task 19: Node RPC Methods

**Files:**
- Create: `src/gateway/rpc/nodes.rs`
- Create: `src/gateway/rpc/devices.rs`

- [ ] **Step 1: Create all 14 node RPC handlers**

Each handler operates on `NodeRegistry` (live) and `PairingStore` (persistent).

- `node.pair.request` → add to PairingStore pending + broadcast event
- `node.pair.approve` → move from pending to paired + broadcast
- `node.pair.reject` → remove from pending + broadcast
- `node.pair.verify` → check token against paired store
- `node.pair.list` → list paired nodes
- `node.list` → list live connected nodes from registry
- `node.describe` → get single node details
- `node.invoke` → registry.invoke() → send event to target node → await result
- `node.invoke.result` → registry.handle_invoke_result() (called by node)
- `node.pending.pull` → registry.pull_pending() (called by node)
- `node.pending.ack` → registry.ack_pending() (called by node)
- `node.event` → forward event + update presence
- `node.rename` → update paired store display_name

- [ ] **Step 2: Create 6 device RPC handlers** (similar pattern to node pairing)

- [ ] **Step 3: Register + compile + commit**

### Task 20: Exec Approvals System

**Files:**
- Create: `src/gateway/exec_approvals/mod.rs`
- Create: `src/gateway/exec_approvals/config.rs`
- Create: `src/gateway/exec_approvals/manager.rs`
- Create: `src/gateway/exec_approvals/policy.rs`
- Create: `src/gateway/rpc/exec_approvals.rs`

- [ ] **Step 1: Create ExecApprovalsConfig persistence**

JSON file at `~/.synapse/exec-approvals.json`. Supports CAS updates via SHA256 hash.

- [ ] **Step 2: Create ExecApprovalManager state machine**

In-memory pending map with timeout/grace period lifecycle.

```rust
impl ExecApprovalManager {
    pub fn create(&mut self, request: ApprovalRequestPayload, timeout_ms: u64) -> ApprovalRecord { ... }
    pub fn register(&mut self, record: ApprovalRecord) -> oneshot::Receiver<Option<ApprovalDecision>> { ... }
    pub fn resolve(&mut self, id: &str, decision: ApprovalDecision, resolved_by: Option<String>) -> bool { ... }
    pub fn expire(&mut self, id: &str) -> bool { ... }
    pub fn consume_allow_once(&mut self, id: &str) -> bool { ... }
    pub fn get_snapshot(&self, id: &str) -> Option<&ApprovalRecord> { ... }
}
```

- [ ] **Step 3: Create policy evaluation**

Match commands against allowlist patterns (glob), evaluate security/ask/askFallback policies.

- [ ] **Step 4: Create 7 exec approval RPC handlers**

- [ ] **Step 5: Register + compile + commit**

### Task 21: TTS/Voice/Misc RPC Modules

**Files:**
- Create: `src/gateway/rpc/tts.rs`
- Create: `src/gateway/rpc/talk.rs`
- Create: `src/gateway/rpc/voicewake.rs`
- Create: `src/gateway/rpc/secrets.rs`
- Create: `src/gateway/rpc/updates.rs`
- Create: `src/gateway/rpc/send.rs`
- Create: `src/gateway/rpc/heartbeat.rs`
- Create: `src/gateway/rpc/wizard.rs`
- Create: `src/gateway/rpc/browser.rs`
- Create: `src/gateway/rpc/web_login.rs`
- Create: `src/gateway/rpc/push.rs`
- Create: `src/gateway/tts.rs`

- [ ] **Step 1: Create TTS state + 6 handlers**

TTS is stub-ready — status returns disabled, providers returns empty list. Can be wired to OpenAI TTS API later.

- [ ] **Step 2: Create remaining stub modules**

Each returns sensible defaults:
- `talk.config/mode` → stub (voice not yet implemented)
- `voicewake.get/set` → stub
- `secrets.reload/resolve` → wire to existing secret masking
- `updates.run` → stub (check for updates placeholder)
- `send` → forward message to agent
- `wake` → wake idle agent
- `last-heartbeat/set-heartbeats` → heartbeat config
- `wizard.*` → setup wizard (stub for now)
- `browser.request` → browser automation (stub)
- `web.login.start/wait` → web login flow (stub)
- `push.test` → push notification test (stub)

- [ ] **Step 3: Register all + compile + commit**

---

## Chunk 4: Frontend — WS RPC Client + New Pages

### Task 22: GatewayClient WS RPC Class

**Files:**
- Create: `web/src/lib/gateway-client.ts`
- Create: `web/src/hooks/useGatewayClient.ts`

- [ ] **Step 1: Create `gateway-client.ts`**

```typescript
export class GatewayClient {
    private ws: WebSocket | null = null;
    private pending = new Map<string, { resolve: Function; reject: Function; timeout: number }>();
    private eventHandlers = new Map<string, Set<(payload: any) => void>>();
    private connected = false;
    private helloOk: HelloOk | null = null;

    async connect(url: string, auth?: { token?: string; password?: string }): Promise<HelloOk>;
    async request<T = any>(method: string, params?: any, timeoutMs?: number): Promise<T>;
    onEvent(event: string, handler: (payload: any) => void): () => void;
    close(): void;

    get isConnected(): boolean;
    get methods(): string[];
    get events(): string[];
}
```

- [ ] **Step 2: Create `useGatewayClient` React hook**

Manages lifecycle, reconnection with exponential backoff, provides client instance to components.

- [ ] **Step 3: Compile + verify**

Run: `cd web && npx tsc --noEmit`

- [ ] **Step 4: Commit**

### Task 23: Instances Page

**Files:**
- Create: `web/src/components/dashboard/InstancesPage.tsx`
- Modify: `web/src/components/Dashboard.tsx`
- Modify: `web/src/i18n/en.json` + `zh.json`

- [ ] **Step 1: Create InstancesPage component**

Grid of instance cards showing: instance_id, type (gateway/webchat/node), platform, version, roles, scopes, last_seen relative time. Real-time updates via `presence` events.

- [ ] **Step 2: Add tab to Dashboard + i18n keys**

- [ ] **Step 3: Commit**

### Task 24: Nodes Page

**Files:**
- Create: `web/src/components/dashboard/NodesPage.tsx`
- Modify: `web/src/components/Dashboard.tsx`
- Modify: `web/src/i18n/en.json` + `zh.json`

- [ ] **Step 1: Create NodesPage component**

Left panel: Exec Approvals config editor
- Security mode dropdown (Deny/Allowlist/Full)
- Ask policy dropdown (Off/OnMiss/Always)
- Ask fallback dropdown
- Auto-allow skills checkbox
- Allowlist pattern table with add/remove
- Scope selector tabs (Defaults, per-agent)
- Save button (CAS with hash)

Right panel: Paired nodes list
- Paired nodes table
- Pending requests with Approve/Reject buttons
- Real-time updates via `node.pair.requested/resolved` events

- [ ] **Step 2: Add tab + i18n keys + commit**

### Task 25: Sidebar + Dashboard Updates

**Files:**
- Modify: `web/src/components/Sidebar.tsx`
- Modify: `web/src/components/Dashboard.tsx`

- [ ] **Step 1: Update sidebar sections**

Add:
- "Instances" under Control section
- "Nodes" under Agent section
- "Resources" section with Docs link

- [ ] **Step 2: Update Dashboard tabs array and routing**

Add `instances` and `nodes` tab keys with their components.

- [ ] **Step 3: Add i18n keys + commit**

### Task 26: Chat Toolbar Enhancements

**Files:**
- Modify: `web/src/components/ChatPanel.tsx` (or equivalent chat area component)

- [ ] **Step 1: Add focus mode toggle**

Button that hides sidebar + header. Stores preference in localStorage.

- [ ] **Step 2: Add thinking output toggle**

Button to toggle showing assistant thinking/reasoning in chat.

- [ ] **Step 3: Add cron session toggle**

Button to show/hide scheduled task sessions in session list.

- [ ] **Step 4: Commit**

### Task 27: Sessions Page Enhancements

**Files:**
- Modify: `web/src/components/dashboard/SessionsPage.tsx`

- [ ] **Step 1: Add verbose and reasoning override dropdowns**

Per-session dropdowns (like existing thinking dropdown) for:
- Verbose: inherit / on / off
- Reasoning: inherit / off / low / medium / high

- [ ] **Step 2: Wire to `sessions.patch` RPC**

- [ ] **Step 3: Commit**

### Task 28: Usage Page Enhancements

**Files:**
- Modify: `web/src/components/dashboard/UsagePage.tsx`

- [ ] **Step 1: Add Token/Cost toggle**

Toggle button switching between token count view and cost ($) view.

- [ ] **Step 2: Add advanced filter input**

Text input supporting syntax: `key:agent:main:* model:gpt-4o has:errors minTokens:2000`

- [ ] **Step 3: Commit**

### Task 29: Migrate Existing Hooks to WS RPC

**Files:**
- Modify: `web/src/hooks/useGatewayWS.ts`
- Modify: `web/src/hooks/useConversation.ts`

- [ ] **Step 1: Update `useGatewayWS` to use Protocol v3**

Replace old frame format with `ClientFrame`/`ServerFrame`. Use `GatewayClient.request()` for RPC calls, `GatewayClient.onEvent()` for streaming.

- [ ] **Step 2: Update `useConversation` to use RPC for session operations**

Replace `fetch('/api/conversations/...')` with `client.request('sessions.list')`, etc.

- [ ] **Step 3: Update all dashboard pages to use RPC instead of REST**

Each page's data fetching changes from:
```typescript
const res = await fetch('/api/dashboard/sessions');
```
to:
```typescript
const res = await client.request('sessions.list', { limit: 50 });
```

- [ ] **Step 4: Verify frontend builds**

Run: `cd web && npm run build`

- [ ] **Step 5: Commit**

---

## Chunk 5: Integration + Polish

### Task 30: Tick Event + Heartbeat

**Files:**
- Modify: `src/gateway/ws.rs`

- [ ] **Step 1: Add tick timer to v3 connection loop**

```rust
let mut tick_interval = tokio::time::interval(Duration::from_secs(30));

loop {
    tokio::select! {
        _ = tick_interval.tick() => {
            sender.send(to_ws_msg(&ServerFrame::event("tick", json!({ "ts": now_ms() })))).await?;
        }
        // ... existing branches
    }
}
```

- [ ] **Step 2: Commit**

### Task 31: Graceful Shutdown

**Files:**
- Modify: `src/gateway/mod.rs`

- [ ] **Step 1: Add shutdown handler**

On SIGTERM/SIGINT:
1. Broadcast `shutdown` event
2. Wait 10s for in-flight RPCs
3. Close all WebSocket connections
4. Exit

- [ ] **Step 2: Commit**

### Task 32: End-to-End Integration Test

- [ ] **Step 1: Manual test**

1. Start backend: `./start.sh dev`
2. Open frontend: `http://localhost:5173`
3. Verify chat works (Protocol v3 or legacy)
4. Navigate to all dashboard pages
5. Check Instances page shows gateway self-presence
6. Check Nodes page renders (empty state)
7. Verify all existing features still work

- [ ] **Step 2: Fix any issues found**

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "feat(gateway): complete OpenClaw protocol v3 alignment — 106 RPC methods"
```
