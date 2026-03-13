//! RPC infrastructure for protocol v3 WebSocket transport.
//!
//! Provides typed frames, method routing with scope-based access control,
//! connection broadcasting, and built-in health/status methods.

mod agents;
mod channels;
mod chat;
mod config_rpc;
mod debug_rpc;
mod events;
mod health;
mod identity;
mod logs_rpc;
mod models;
mod schedules;
mod sessions;
mod skills;
mod store;
mod tools_rpc;
mod usage;
mod workspace;

pub mod router;
pub mod scopes;
pub mod types;

pub use events::GATEWAY_EVENTS;
pub use router::{Broadcaster, RpcContext, RpcHandler, RpcRouter};
pub use scopes::Role;
pub use types::*;

/// Register all built-in RPC methods on the given router.
pub fn register_all(router: &mut RpcRouter) {
    // Health
    router.register(
        "health",
        Box::new(|ctx, params| Box::pin(health::handle_health(ctx, params))),
    );
    router.register(
        "status",
        Box::new(|ctx, params| Box::pin(health::handle_status(ctx, params))),
    );

    // Sessions
    router.register(
        "sessions.list",
        Box::new(|ctx, params| Box::pin(sessions::handle_list(ctx, params))),
    );
    router.register(
        "sessions.get",
        Box::new(|ctx, params| Box::pin(sessions::handle_get(ctx, params))),
    );
    router.register(
        "sessions.patch",
        Box::new(|ctx, params| Box::pin(sessions::handle_patch(ctx, params))),
    );
    router.register(
        "sessions.delete",
        Box::new(|ctx, params| Box::pin(sessions::handle_delete(ctx, params))),
    );
    router.register(
        "sessions.compact",
        Box::new(|ctx, params| Box::pin(sessions::handle_compact(ctx, params))),
    );
    router.register(
        "sessions.usage",
        Box::new(|ctx, params| Box::pin(sessions::handle_usage(ctx, params))),
    );
    router.register(
        "sessions.usage.timeseries",
        Box::new(|ctx, params| Box::pin(sessions::handle_usage_timeseries(ctx, params))),
    );
    router.register(
        "sessions.usage.logs",
        Box::new(|ctx, params| Box::pin(sessions::handle_usage_logs(ctx, params))),
    );

    // Agents
    router.register(
        "agents.list",
        Box::new(|ctx, params| Box::pin(agents::handle_list(ctx, params))),
    );
    router.register(
        "agents.create",
        Box::new(|ctx, params| Box::pin(agents::handle_create(ctx, params))),
    );
    router.register(
        "agents.update",
        Box::new(|ctx, params| Box::pin(agents::handle_update(ctx, params))),
    );
    router.register(
        "agents.delete",
        Box::new(|ctx, params| Box::pin(agents::handle_delete(ctx, params))),
    );

    // Skills
    router.register(
        "skills.status",
        Box::new(|ctx, params| Box::pin(skills::handle_status(ctx, params))),
    );
    router.register(
        "skills.bins",
        Box::new(|ctx, params| Box::pin(skills::handle_bins(ctx, params))),
    );
    router.register(
        "skills.install",
        Box::new(|ctx, params| Box::pin(skills::handle_install(ctx, params))),
    );
    router.register(
        "skills.update",
        Box::new(|ctx, params| Box::pin(skills::handle_update(ctx, params))),
    );

    // Channels
    router.register(
        "channels.status",
        Box::new(|ctx, params| Box::pin(channels::handle_status(ctx, params))),
    );
    router.register(
        "channels.logout",
        Box::new(|ctx, params| Box::pin(channels::handle_logout(ctx, params))),
    );

    // Config
    router.register(
        "config.get",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_get(ctx, params))),
    );
    router.register(
        "config.set",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_set(ctx, params))),
    );
    router.register(
        "config.schema",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_schema(ctx, params))),
    );
    router.register(
        "config.validate",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_validate(ctx, params))),
    );
    router.register(
        "config.reload",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_reload(ctx, params))),
    );

    // Schedules (cron)
    router.register(
        "cron.list",
        Box::new(|ctx, params| Box::pin(schedules::handle_list(ctx, params))),
    );
    router.register(
        "cron.add",
        Box::new(|ctx, params| Box::pin(schedules::handle_add(ctx, params))),
    );
    router.register(
        "cron.update",
        Box::new(|ctx, params| Box::pin(schedules::handle_update(ctx, params))),
    );
    router.register(
        "cron.remove",
        Box::new(|ctx, params| Box::pin(schedules::handle_remove(ctx, params))),
    );
    router.register(
        "cron.run",
        Box::new(|ctx, params| Box::pin(schedules::handle_run(ctx, params))),
    );
    router.register(
        "cron.runs",
        Box::new(|ctx, params| Box::pin(schedules::handle_runs(ctx, params))),
    );
    router.register(
        "cron.status",
        Box::new(|ctx, params| Box::pin(schedules::handle_status_toggle(ctx, params))),
    );

    // Usage
    router.register(
        "usage.status",
        Box::new(|ctx, params| Box::pin(usage::handle_status(ctx, params))),
    );
    router.register(
        "usage.cost",
        Box::new(|ctx, params| Box::pin(usage::handle_cost(ctx, params))),
    );

    // Logs
    router.register(
        "logs.tail",
        Box::new(|ctx, params| Box::pin(logs_rpc::handle_tail(ctx, params))),
    );

    // Models
    router.register(
        "models.list",
        Box::new(|ctx, params| Box::pin(models::handle_list(ctx, params))),
    );

    // Tools
    router.register(
        "tools.catalog",
        Box::new(|ctx, params| Box::pin(tools_rpc::handle_catalog(ctx, params))),
    );

    // Workspace
    router.register(
        "workspace.list",
        Box::new(|ctx, params| Box::pin(workspace::handle_list(ctx, params))),
    );
    router.register(
        "workspace.get",
        Box::new(|ctx, params| Box::pin(workspace::handle_get(ctx, params))),
    );
    router.register(
        "workspace.set",
        Box::new(|ctx, params| Box::pin(workspace::handle_set(ctx, params))),
    );
    router.register(
        "workspace.create",
        Box::new(|ctx, params| Box::pin(workspace::handle_create(ctx, params))),
    );
    router.register(
        "workspace.delete",
        Box::new(|ctx, params| Box::pin(workspace::handle_delete(ctx, params))),
    );
    router.register(
        "workspace.reset",
        Box::new(|ctx, params| Box::pin(workspace::handle_reset(ctx, params))),
    );

    // Store
    router.register(
        "store.search",
        Box::new(|ctx, params| Box::pin(store::handle_search(ctx, params))),
    );
    router.register(
        "store.list",
        Box::new(|ctx, params| Box::pin(store::handle_list(ctx, params))),
    );
    router.register(
        "store.detail",
        Box::new(|ctx, params| Box::pin(store::handle_detail(ctx, params))),
    );
    router.register(
        "store.install",
        Box::new(|ctx, params| Box::pin(store::handle_install(ctx, params))),
    );
    router.register(
        "store.status",
        Box::new(|ctx, params| Box::pin(store::handle_status(ctx, params))),
    );

    // Debug
    router.register(
        "debug.invoke",
        Box::new(|ctx, params| Box::pin(debug_rpc::handle_invoke(ctx, params))),
    );
    router.register(
        "debug.health",
        Box::new(|ctx, params| Box::pin(debug_rpc::handle_health(ctx, params))),
    );

    // Identity
    router.register(
        "agent.identity.get",
        Box::new(|ctx, params| Box::pin(identity::handle_get(ctx, params))),
    );

    // Chat / Agent control
    router.register(
        "chat.history",
        Box::new(|ctx, params| Box::pin(chat::handle_history(ctx, params))),
    );
    router.register(
        "chat.abort",
        Box::new(|ctx, params| Box::pin(chat::handle_abort(ctx, params))),
    );
    router.register(
        "chat.inject",
        Box::new(|ctx, params| Box::pin(chat::handle_inject(ctx, params))),
    );
    router.register(
        "agent.wait",
        Box::new(|ctx, params| Box::pin(chat::handle_agent_wait(ctx, params))),
    );
    router.register(
        "poll",
        Box::new(|ctx, params| Box::pin(chat::handle_poll(ctx, params))),
    );
}
