use std::sync::Arc;

use synaptic::lark::{LarkBotClient, LarkConfig, LarkLongConnListener};

use crate::agent;
use crate::channels::dedup::MessageDedup;
use crate::channels::dm::FileDmPolicyEnforcer;
use crate::channels::handler::AgentSession;
use crate::config::bots::{resolve_secret, DmPolicy};
use crate::config::SynapseConfig;

use super::{LarkHandler, LarkHandlerConfig};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the Lark bot adapter.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
    status_handle: Option<Arc<dyn synaptic::ChannelStatusHandle>>,
    event_bus: Option<Arc<synaptic::events::EventBus>>,
    plugin_registry: Option<Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let lark_config = config
        .lark
        .first()
        .ok_or("missing [[lark]] section in config")?;

    let app_secret = resolve_secret(
        lark_config.app_secret.as_deref(),
        lark_config.app_secret_env.as_deref(),
        "Lark app secret",
    )
    .map_err(|e| e.to_string())?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let cost_tracker = Arc::new(synaptic::callbacks::CostTrackingCallback::new(
        synaptic::callbacks::default_pricing(),
    ));
    let usage_tracker = Arc::new(crate::gateway::usage::UsageTracker::with_persistence(
        Arc::clone(&cost_tracker),
        crate::gateway::usage::default_usage_path(),
    ));
    if let Err(e) = usage_tracker.load().await {
        tracing::warn!(error = %e, "failed to load usage records for lark adapter");
    }
    usage_tracker.spawn_periodic_flush(std::time::Duration::from_secs(60));

    let mut session = AgentSession::new(model, config_arc, true)
        .with_channel("lark")
        .with_cost_tracker(cost_tracker)
        .with_usage_tracker(usage_tracker);
    if let Some(eb) = event_bus {
        session = session.with_event_bus(eb);
    }
    if let Some(pr) = plugin_registry {
        session = session.with_plugin_registry(pr);
    }
    let agent_session = Arc::new(session);

    let lark = LarkConfig::new(&lark_config.app_id, &app_secret);
    let client = LarkBotClient::new(lark.clone());

    // Fetch bot info for mention detection
    let bot_info = client
        .get_bot_info()
        .await
        .map_err(|e| format!("failed to get bot info: {}", e))?;

    tracing::info!(
        channel = "lark",
        app_id = %lark_config.app_id,
        bot_name = %bot_info.app_name,
        bot_id = %bot_info.open_id,
        "adapter started"
    );

    let handler_config = Arc::new(LarkHandlerConfig {
        render_mode: lark_config.render_mode.clone(),
        streaming: lark_config.streaming,
        require_mention: lark_config.require_mention,
        typing_indicator: lark_config.typing_indicator,
        reply_in_thread: lark_config.reply_in_thread,
        group_session_scope: lark_config.group_session_scope.clone(),
        dm_scope: lark_config.dm_session_scope.clone().unwrap_or_default(),
        dm_policy: lark_config.dm_policy.clone(),
        group_policy: lark_config.group_policy.clone(),
        allowlist: lark_config.allowlist.clone(),
        text_chunk_limit: lark_config.text_chunk_limit,
        card: lark_config.card.clone(),
        bot_name: bot_info.app_name.clone(),
    });

    let pairing_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".synapse")
        .join("pairing");
    let config_allowlist = if lark_config.dm_policy == DmPolicy::Allowlist {
        Some(lark_config.allowlist.allowed_users.clone())
    } else {
        None
    };
    let pairing_ttl_ms = lark_config.pairing_ttl_secs.unwrap_or(3600) * 1000;
    let enforcer = Arc::new(FileDmPolicyEnforcer::with_ttl(
        pairing_dir,
        lark_config.dm_policy.clone(),
        config_allowlist,
        pairing_ttl_ms,
    ));

    let msg_handler = LarkHandler {
        agent_session: agent_session.clone(),
        config: handler_config,
        dedup: Arc::new(MessageDedup::new(2048)),
        bot_open_id: bot_info.open_id,
        account_id: lark_config.account_id.clone(),
        enforcer,
        gateway_port: config.serve.as_ref().and_then(|s| s.port).unwrap_or(3000),
        owner_chat_id: lark_config.owner_chat_id.clone(),
    };

    let mut listener = LarkLongConnListener::new(lark).with_event_handler(msg_handler);

    if let Some(handle) = status_handle {
        listener = listener.with_status_handle(handle);
    }

    listener.run().await?;
    Ok(())
}
