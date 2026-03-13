//! RPC infrastructure for protocol v3 WebSocket transport.
//!
//! Provides typed frames, method routing with scope-based access control,
//! connection broadcasting, and built-in health/status methods.

mod events;
mod health;
pub mod router;
pub mod scopes;
pub mod types;

pub use events::GATEWAY_EVENTS;
pub use router::{Broadcaster, RpcContext, RpcHandler, RpcRouter};
pub use scopes::Role;
pub use types::*;

/// Register all built-in RPC methods on the given router.
pub fn register_all(router: &mut RpcRouter) {
    router.register(
        "health",
        Box::new(|ctx, params| Box::pin(health::handle_health(ctx, params))),
    );
    router.register(
        "status",
        Box::new(|ctx, params| Box::pin(health::handle_status(ctx, params))),
    );
}
