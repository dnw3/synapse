//! RPC router: method dispatch and connection broadcasting.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{mpsc, RwLock};

use super::scopes::{self, Role};
use super::types::{RpcError, ServerFrame};
use crate::gateway::state::AppState;

// ---------------------------------------------------------------------------
// RpcContext — per-connection context passed to every handler
// ---------------------------------------------------------------------------

/// Context available to every RPC handler invocation.
#[allow(dead_code)]
pub struct RpcContext {
    /// Shared application state.
    pub state: AppState,
    /// Unique connection identifier.
    pub conn_id: String,
    /// Client information from the handshake.
    pub client: super::types::ClientInfo,
    /// Authenticated role for this connection.
    pub role: Role,
    /// Granted permission scopes.
    pub scopes: HashSet<String>,
    /// Broadcaster for pushing events to connected clients.
    pub broadcaster: Arc<Broadcaster>,
}

// ---------------------------------------------------------------------------
// RpcHandler type alias
// ---------------------------------------------------------------------------

/// An async RPC handler function.
///
/// Receives a shared context and the request params, returns a result
/// that is either a success payload or an RPC error.
pub type RpcHandler = Box<
    dyn Fn(Arc<RpcContext>, Value) -> Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send>>
        + Send
        + Sync,
>;

// ---------------------------------------------------------------------------
// RpcRouter
// ---------------------------------------------------------------------------

/// Routes RPC method calls to registered handlers, enforcing scope checks.
pub struct RpcRouter {
    handlers: HashMap<String, RpcHandler>,
}

impl RpcRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for the given method name.
    pub fn register(&mut self, method: impl Into<String>, handler: RpcHandler) {
        self.handlers.insert(method.into(), handler);
    }

    /// Dispatch a request to the appropriate handler.
    ///
    /// Performs scope checking *before* invoking the handler. Returns a
    /// `ServerFrame::Response` in all cases.
    pub async fn dispatch(
        &self,
        ctx: Arc<RpcContext>,
        id: String,
        method: &str,
        params: Value,
    ) -> ServerFrame {
        // 1. Scope check
        if let Err(reason) = scopes::check_scope(method, ctx.role, &ctx.scopes) {
            return ServerFrame::err(&id, RpcError::forbidden(reason));
        }

        // 2. Look up handler
        let handler = match self.handlers.get(method) {
            Some(h) => h,
            None => return ServerFrame::err(&id, RpcError::method_not_found(method)),
        };

        // 3. Invoke
        match handler(ctx, params).await {
            Ok(payload) => ServerFrame::ok(id, payload),
            Err(rpc_err) => ServerFrame::err(id, rpc_err),
        }
    }

    /// Return the set of registered method names.
    pub fn method_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.handlers.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for RpcRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Broadcaster — fan-out events to connected clients
// ---------------------------------------------------------------------------

/// Manages connected client channels and broadcasts server events.
pub struct Broadcaster {
    /// Connected clients: conn_id → sender.
    connections: RwLock<HashMap<String, mpsc::UnboundedSender<ServerFrame>>>,
    /// Monotonically increasing sequence counter for events.
    seq: AtomicU64,
}

impl Broadcaster {
    /// Create a new empty broadcaster.
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            seq: AtomicU64::new(1),
        }
    }

    /// Register a new connection. Returns the receiver half.
    pub async fn register(&self, conn_id: String) -> mpsc::UnboundedReceiver<ServerFrame> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.connections.write().await.insert(conn_id, tx);
        rx
    }

    /// Unregister a connection (e.g. on disconnect).
    pub async fn unregister(&self, conn_id: &str) {
        self.connections.write().await.remove(conn_id);
    }

    /// Broadcast an event to all connected clients.
    pub async fn broadcast(&self, event: impl Into<String>, payload: Value) {
        let event = event.into();
        let conns = self.connections.read().await;
        for tx in conns.values() {
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let frame = ServerFrame::event(&event, payload.clone(), seq);
            let _ = tx.send(frame);
        }
    }

    /// Send a frame to a specific connection.
    #[allow(dead_code)]
    pub async fn send_to(&self, conn_id: &str, frame: ServerFrame) -> bool {
        let conns = self.connections.read().await;
        if let Some(tx) = conns.get(conn_id) {
            tx.send(frame).is_ok()
        } else {
            false
        }
    }

    /// Return the number of active connections.
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}
