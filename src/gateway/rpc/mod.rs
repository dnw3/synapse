//! RPC infrastructure for protocol v3 WebSocket transport.
//!
//! Provides typed frames, method routing with scope-based access control,
//! connection broadcasting, and built-in health/status methods.

mod agent_files;
mod agents;
mod channels;
mod chat;
mod config_rpc;
mod debug_rpc;
mod devices;
mod events;
mod exec_approvals;
mod health;
mod heartbeat_rpc;
mod identity;
mod logs_rpc;
mod misc;
mod models;
mod nodes;
mod presence;
mod schedules;
mod secrets_rpc;
mod sessions;
mod skills;
mod store;
mod tools_rpc;
mod tts;
mod usage;
pub mod wizard;
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
        "sessions.create",
        Box::new(|ctx, params| Box::pin(sessions::handle_create(ctx, params))),
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
        "sessions.abort",
        Box::new(|ctx, params| Box::pin(sessions::handle_abort(ctx, params))),
    );
    router.register(
        "sessions.subscribe",
        Box::new(|ctx, params| Box::pin(sessions::handle_subscribe(ctx, params))),
    );
    router.register(
        "sessions.unsubscribe",
        Box::new(|ctx, params| Box::pin(sessions::handle_unsubscribe(ctx, params))),
    );
    router.register(
        "sessions.preview",
        Box::new(|ctx, params| Box::pin(sessions::handle_preview(ctx, params))),
    );
    router.register(
        "sessions.reset",
        Box::new(|ctx, params| Box::pin(sessions::handle_reset(ctx, params))),
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
    router.register(
        "config.patch",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_patch(ctx, params))),
    );
    router.register(
        "config.apply",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_apply(ctx, params))),
    );
    router.register(
        "config.schema.lookup",
        Box::new(|ctx, params| Box::pin(config_rpc::handle_schema_lookup(ctx, params))),
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
    router.register(
        "gateway.identity.get",
        Box::new(|ctx, params| Box::pin(identity::handle_gateway_identity(ctx, params))),
    );

    // Wizard
    router.register(
        "wizard.start",
        Box::new(|ctx, params| Box::pin(wizard::handle_start(ctx, params))),
    );
    router.register(
        "wizard.next",
        Box::new(|ctx, params| Box::pin(wizard::handle_next(ctx, params))),
    );
    router.register(
        "wizard.cancel",
        Box::new(|ctx, params| Box::pin(wizard::handle_cancel(ctx, params))),
    );
    router.register(
        "wizard.status",
        Box::new(|ctx, params| Box::pin(wizard::handle_status(ctx, params))),
    );

    // Agent files
    router.register(
        "agents.files.list",
        Box::new(|ctx, params| Box::pin(agent_files::handle_list(ctx, params))),
    );
    router.register(
        "agents.files.get",
        Box::new(|ctx, params| Box::pin(agent_files::handle_get(ctx, params))),
    );
    router.register(
        "agents.files.set",
        Box::new(|ctx, params| Box::pin(agent_files::handle_set(ctx, params))),
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
        "sessions.send",
        Box::new(|ctx, params| Box::pin(chat::handle_session_send(ctx, params))),
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

    // Presence
    router.register(
        "system-presence",
        Box::new(|ctx, params| Box::pin(presence::handle_system_presence(ctx, params))),
    );
    router.register(
        "system-event",
        Box::new(|ctx, params| Box::pin(presence::handle_system_event(ctx, params))),
    );

    // Node pairing
    router.register(
        "node.pair.request",
        Box::new(|ctx, params| Box::pin(nodes::handle_pair_request(ctx, params))),
    );
    router.register(
        "node.pair.approve",
        Box::new(|ctx, params| Box::pin(nodes::handle_pair_approve(ctx, params))),
    );
    router.register(
        "node.pair.reject",
        Box::new(|ctx, params| Box::pin(nodes::handle_pair_reject(ctx, params))),
    );
    router.register(
        "node.pair.verify",
        Box::new(|ctx, params| Box::pin(nodes::handle_pair_verify(ctx, params))),
    );
    router.register(
        "node.pair.list",
        Box::new(|ctx, params| Box::pin(nodes::handle_pair_list(ctx, params))),
    );

    // Node registry
    router.register(
        "node.list",
        Box::new(|ctx, params| Box::pin(nodes::handle_node_list(ctx, params))),
    );
    router.register(
        "node.describe",
        Box::new(|ctx, params| Box::pin(nodes::handle_node_describe(ctx, params))),
    );
    router.register(
        "node.rename",
        Box::new(|ctx, params| Box::pin(nodes::handle_node_rename(ctx, params))),
    );
    router.register(
        "node.register",
        Box::new(|ctx, params| Box::pin(nodes::handle_node_register(ctx, params))),
    );

    // Node invocation
    router.register(
        "node.invoke",
        Box::new(|ctx, params| Box::pin(nodes::handle_node_invoke(ctx, params))),
    );
    router.register(
        "node.invoke.result",
        Box::new(|ctx, params| Box::pin(nodes::handle_invoke_result(ctx, params))),
    );
    router.register(
        "node.pending.pull",
        Box::new(|ctx, params| Box::pin(nodes::handle_pending_pull(ctx, params))),
    );
    router.register(
        "node.pending.ack",
        Box::new(|ctx, params| Box::pin(nodes::handle_pending_ack(ctx, params))),
    );
    router.register(
        "node.pending.drain",
        Box::new(|ctx, params| Box::pin(nodes::handle_pending_drain(ctx, params))),
    );
    router.register(
        "node.pending.enqueue",
        Box::new(|ctx, params| Box::pin(nodes::handle_pending_enqueue(ctx, params))),
    );
    router.register(
        "node.event",
        Box::new(|ctx, params| Box::pin(nodes::handle_node_event(ctx, params))),
    );

    // Device pairing
    router.register(
        "device.pair.approve",
        Box::new(|ctx, params| Box::pin(devices::handle_pair_approve(ctx, params))),
    );
    router.register(
        "device.pair.reject",
        Box::new(|ctx, params| Box::pin(devices::handle_pair_reject(ctx, params))),
    );
    router.register(
        "device.pair.remove",
        Box::new(|ctx, params| Box::pin(devices::handle_pair_remove(ctx, params))),
    );
    router.register(
        "device.pair.list",
        Box::new(|ctx, params| Box::pin(devices::handle_pair_list(ctx, params))),
    );
    router.register(
        "device.token.rotate",
        Box::new(|ctx, params| Box::pin(devices::handle_token_rotate(ctx, params))),
    );
    router.register(
        "device.token.revoke",
        Box::new(|ctx, params| Box::pin(devices::handle_token_revoke(ctx, params))),
    );

    // Exec approvals
    router.register(
        "exec.approval.request",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_approval_request(ctx, params))),
    );
    router.register(
        "exec.approval.resolve",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_approval_resolve(ctx, params))),
    );
    router.register(
        "exec.approval.waitDecision",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_wait_decision(ctx, params))),
    );
    router.register(
        "exec.approvals.get",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_approvals_get(ctx, params))),
    );
    router.register(
        "exec.approvals.set",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_approvals_set(ctx, params))),
    );
    router.register(
        "exec.approvals.node.get",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_node_approvals_get(ctx, params))),
    );
    router.register(
        "exec.approvals.node.set",
        Box::new(|ctx, params| Box::pin(exec_approvals::handle_node_approvals_set(ctx, params))),
    );

    // TTS
    router.register(
        "tts.status",
        Box::new(|ctx, params| Box::pin(tts::handle_status(ctx, params))),
    );
    router.register(
        "tts.providers",
        Box::new(|ctx, params| Box::pin(tts::handle_providers(ctx, params))),
    );
    router.register(
        "tts.enable",
        Box::new(|ctx, params| Box::pin(tts::handle_enable(ctx, params))),
    );
    router.register(
        "tts.disable",
        Box::new(|ctx, params| Box::pin(tts::handle_disable(ctx, params))),
    );
    router.register(
        "tts.convert",
        Box::new(|ctx, params| Box::pin(tts::handle_convert(ctx, params))),
    );
    router.register(
        "tts.setProvider",
        Box::new(|ctx, params| Box::pin(tts::handle_set_provider(ctx, params))),
    );

    // Heartbeat
    router.register(
        "last-heartbeat",
        Box::new(|ctx, params| Box::pin(heartbeat_rpc::handle_last_heartbeat(ctx, params))),
    );
    router.register(
        "set-heartbeats",
        Box::new(|ctx, params| Box::pin(heartbeat_rpc::handle_set_heartbeats(ctx, params))),
    );

    // Secrets
    router.register(
        "secrets.reload",
        Box::new(|ctx, params| Box::pin(secrets_rpc::handle_reload(ctx, params))),
    );
    router.register(
        "secrets.resolve",
        Box::new(|ctx, params| Box::pin(secrets_rpc::handle_resolve(ctx, params))),
    );

    // Misc
    router.register(
        "send",
        Box::new(|ctx, params| Box::pin(misc::handle_send(ctx, params))),
    );
    router.register(
        "wake",
        Box::new(|ctx, params| Box::pin(misc::handle_wake(ctx, params))),
    );
    router.register(
        "updates.run",
        Box::new(|ctx, params| Box::pin(misc::handle_updates_run(ctx, params))),
    );
    router.register(
        "doctor.memory.status",
        Box::new(|ctx, params| Box::pin(misc::handle_doctor_memory_status(ctx, params))),
    );
}
