use std::sync::Arc;
use std::sync::RwLock;

use async_trait::async_trait;
use synaptic::core::channel::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as ChannelMessageEnvelope, Outbound,
};
use synaptic::core::SynapticError;
use synaptic::lark::bot::session::MentionInfo;
use synaptic::lark::bot::{CardActionEvent, CardActionHandler, MessageHandler};
use synaptic::lark::{
    LarkBotClient, LarkConfig, LarkLongConnListener, LarkMessageEvent, StreamingCardOptions,
    StreamingCardWriter,
};

use synaptic::{DeliveryContext, DmPolicyEnforcer};

use crate::agent;
use crate::channels::dedup::MessageDedup;
use crate::channels::dm::FileDmPolicyEnforcer;
use crate::channels::formatter;
use crate::channels::handler::{AgentSession, StreamingOutput};
use crate::config::bot::{
    resolve_secret, DmPolicy, GroupPolicy, GroupSessionScope, LarkRenderMode,
};
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::sender::{ChannelSender, SendResult};
use crate::gateway::messages::{Attachment, MessageEnvelope};
use crate::gateway::presence::now_ms;

// ---------------------------------------------------------------------------
// ChannelSender implementation
// ---------------------------------------------------------------------------

/// Outbound sender for the Lark channel.
#[allow(dead_code)]
pub struct LarkSender {
    /// Lark bot client for making API calls.
    pub client: LarkBotClient,
}

// ---------------------------------------------------------------------------
// ApproveNotifier implementation
// ---------------------------------------------------------------------------

/// Sends a notification to a Lark user when their DM pairing is approved.
pub struct LarkApproveNotifier {
    pub client: LarkBotClient,
}

#[async_trait]
impl crate::channels::dm::ApproveNotifier for LarkApproveNotifier {
    async fn notify_approved(
        &self,
        sender_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client
            .send_text(
                "open_id",
                sender_id,
                "您的配对请求已通过，现在可以开始对话了。",
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }
}

#[async_trait]
impl ChannelSender for LarkSender {
    fn channel_id(&self) -> &str {
        "lark"
    }

    async fn send(
        &self,
        target: &DeliveryContext,
        content: &str,
        _meta: Option<&serde_json::Value>,
    ) -> Result<SendResult, Box<dyn std::error::Error + Send + Sync>> {
        let chat_id = target
            .to
            .as_deref()
            .and_then(|s| s.strip_prefix("chat:"))
            .ok_or("missing or invalid chat_id in delivery target (expected 'chat:<id>')")?;

        self.client
            .send_text("chat_id", chat_id, content)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(SendResult {
            message_id: None,
            delivered_at_ms: now_ms(),
        })
    }
}

// ---------------------------------------------------------------------------
// Streaming output adapter
// ---------------------------------------------------------------------------

struct LarkStreamingOutput {
    writer: StreamingCardWriter,
}

#[async_trait]
impl StreamingOutput for LarkStreamingOutput {
    async fn on_token(&self, token: &str) {
        self.writer.write(token).await.ok();
    }

    async fn on_tool_call(&self, tool_name: &str) {
        self.writer
            .write(&format!("\n> Using tool: {}\n", tool_name))
            .await
            .ok();
    }

    async fn on_complete(&self, _full_response: &str) {
        self.writer.finish().await.ok();
    }

    async fn on_error(&self, error: &str) {
        self.writer
            .write(&format!("\n**Error:** {}", error))
            .await
            .ok();
        self.writer.finish().await.ok();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute session key based on chat type and configured scope.
fn compute_session_key(
    event: &LarkMessageEvent,
    scope: &GroupSessionScope,
    dm_scope: &crate::config::DmSessionScope,
    account_id: &str,
) -> String {
    use crate::channels::session_key::{self, ChatType, SessionKeyParams};

    let chat_type = if event.is_dm() {
        ChatType::Dm
    } else {
        ChatType::Group
    };
    let peer_id = if event.is_dm() {
        event.sender_open_id()
    } else {
        event.chat_id()
    };
    session_key::compute(&SessionKeyParams {
        agent_id: "default", // Will be overridden by router in handler
        channel: "lark",
        account_id: Some(account_id),
        chat_type,
        peer_id,
        sender_id: Some(event.sender_open_id()),
        thread_id: event.root_id.as_deref(),
        dm_scope,
        group_scope: scope,
    })
}

/// Strip bot @mention placeholders from message text.
fn strip_bot_mention(text: &str, mentions: &[MentionInfo]) -> String {
    let mut result = text.to_string();
    for m in mentions {
        result = result.replace(&m.key, "");
    }
    result.trim().to_string()
}

/// Heuristic: does the text contain rich markdown that benefits from card rendering?
fn has_rich_content(text: &str) -> bool {
    text.contains("```")
        || text.contains("| --- |")
        || text.contains("**")
        || text.lines().any(|l| l.starts_with("# "))
}

// ---------------------------------------------------------------------------
// Card builders for device pairing
// ---------------------------------------------------------------------------

/// Build a Lark interactive card for displaying a setup code.
fn build_pair_card(setup_code: &str, gateway_url: &str) -> serde_json::Value {
    serde_json::json!({
        "config": { "wide_screen_mode": true },
        "header": {
            "title": { "tag": "plain_text", "content": "Device Pairing" },
            "template": "blue"
        },
        "elements": [
            {
                "tag": "div",
                "text": {
                    "tag": "lark_md",
                    "content": format!(
                        "**Setup Code:**\n```\n{setup_code}\n```\n\n**Gateway:** {gateway_url}\n\n_Expires in 10 minutes_"
                    )
                }
            }
        ]
    })
}

/// Build a Lark interactive card for pairing approval.
#[allow(dead_code)]
fn build_approval_card(request_id: &str, device_name: &str, platform: &str) -> serde_json::Value {
    serde_json::json!({
        "config": { "wide_screen_mode": true },
        "header": {
            "title": { "tag": "plain_text", "content": "New Device Pairing Request" },
            "template": "orange"
        },
        "elements": [
            {
                "tag": "div",
                "text": {
                    "tag": "lark_md",
                    "content": format!("**Device:** {device_name}\n**Platform:** {platform}")
                }
            },
            {
                "tag": "action",
                "actions": [
                    {
                        "tag": "button",
                        "text": { "tag": "plain_text", "content": "Approve" },
                        "type": "primary",
                        "value": format!("pair_approve:{request_id}")
                    },
                    {
                        "tag": "button",
                        "text": { "tag": "plain_text", "content": "Reject" },
                        "type": "danger",
                        "value": format!("pair_reject:{request_id}")
                    }
                ]
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// Message handler
// ---------------------------------------------------------------------------

struct LarkHandler {
    agent_session: Arc<AgentSession>,
    config: Arc<LarkHandlerConfig>,
    dedup: Arc<MessageDedup>,
    bot_open_id: String,
    account_id: String,
    enforcer: Arc<FileDmPolicyEnforcer>,
    gateway_port: u16,
    #[allow(dead_code)]
    owner_chat_id: Option<String>,
}

/// Extracted config subset for the handler.
#[allow(dead_code)]
struct LarkHandlerConfig {
    render_mode: LarkRenderMode,
    streaming: bool,
    require_mention: bool,
    typing_indicator: bool,
    reply_in_thread: bool,
    group_session_scope: GroupSessionScope,
    dm_scope: crate::config::DmSessionScope,
    dm_policy: DmPolicy,
    group_policy: GroupPolicy,
    allowlist: BotAllowlist,
    text_chunk_limit: usize,
}

impl LarkHandler {
    /// Check group access policy. Returns `true` if the message is allowed.
    fn check_group_policy(&self, event: &LarkMessageEvent) -> bool {
        match self.config.group_policy {
            GroupPolicy::Open => true,
            GroupPolicy::Disabled => false,
            GroupPolicy::Allowlist => self.config.allowlist.is_channel_allowed(event.chat_id()),
        }
    }

    /// Check DM access using the enforcer. Returns Some(reply_text) if blocked.
    async fn check_dm_access(&self, sender_id: &str) -> Option<String> {
        use synaptic::DmAccessDenied;
        match self.enforcer.check_access(sender_id, "lark").await {
            Ok(()) => None,
            Err(DmAccessDenied::NeedsPairing(challenge)) => {
                let ttl_mins = challenge.ttl_ms / 60_000;
                let ttl_desc = if ttl_mins >= 60 {
                    format!("{} 小时", ttl_mins / 60)
                } else {
                    format!("{} 分钟", ttl_mins)
                };
                Some(format!(
                    "请将以下配对码发送给管理员以完成验证：\n\n🔑 {}\n\n配对码有效期 {}。",
                    challenge.code, ttl_desc
                ))
            }
            Err(DmAccessDenied::NotAllowed) => Some("抱歉，您未获得授权使用此机器人。".to_string()),
            Err(DmAccessDenied::DmDisabled) => None, // silently ignore
        }
    }

    /// Send a plain-text reply, respecting thread mode and chunk limits.
    async fn send_reply(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        text: &str,
    ) -> Result<(), SynapticError> {
        let chunks = formatter::format_for_channel(text, "lark", self.config.text_chunk_limit);
        for chunk in chunks {
            if self.config.reply_in_thread && event.has_thread() {
                client
                    .reply_text_in_thread(event.message_id(), &chunk)
                    .await?;
            } else {
                client.reply_text(event.message_id(), &chunk).await?;
            }
        }
        Ok(())
    }

    /// Handle text/post messages with streaming or plain reply.
    async fn handle_text_message(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        session_key: &str,
        text: &str,
    ) -> Result<(), SynapticError> {
        let use_streaming = self.config.streaming
            && matches!(
                self.config.render_mode,
                LarkRenderMode::Card | LarkRenderMode::Auto
            );

        let delivery = DeliveryContext {
            channel: "lark".into(),
            to: Some(format!("chat:{}", event.chat_id())),
            account_id: Some(self.account_id.clone()),
            ..Default::default()
        };

        let build_envelope = |delivery: DeliveryContext| {
            let mut env =
                MessageEnvelope::channel(session_key.to_string(), text.to_string(), delivery);
            env.sender_id = Some(event.sender_open_id().to_string());
            env.routing.peer_kind = Some(if event.is_dm() {
                crate::config::PeerKind::Direct
            } else {
                crate::config::PeerKind::Group
            });
            env.routing.peer_id = Some(event.chat_id().to_string());
            env
        };

        if use_streaming {
            let writer = client
                .streaming_reply(
                    event.message_id(),
                    StreamingCardOptions::new().with_title("Synapse"),
                )
                .await?;

            let output: Arc<dyn StreamingOutput> = Arc::new(LarkStreamingOutput { writer });

            let envelope = build_envelope(delivery);
            self.agent_session
                .handle_message_streaming(envelope, output)
                .await
                .map_err(|e| SynapticError::Tool(e.to_string()))?;
        } else {
            let envelope = build_envelope(delivery);
            match self.agent_session.handle_message(envelope).await {
                Ok(reply) => {
                    // Auto mode: use card for rich content even without streaming
                    if matches!(self.config.render_mode, LarkRenderMode::Auto)
                        && has_rich_content(&reply.content)
                    {
                        let writer = client
                            .streaming_reply(
                                event.message_id(),
                                StreamingCardOptions::new().with_title("Synapse"),
                            )
                            .await?;
                        writer.write(&reply.content).await.ok();
                        writer.finish().await.ok();
                    } else {
                        self.send_reply(event, client, &reply.content).await?;
                    }
                }
                Err(e) => {
                    client
                        .reply_text(event.message_id(), &format!("Error: {}", e))
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Handle image messages: download and pass as attachment.
    async fn handle_image_message(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        session_key: &str,
    ) -> Result<(), SynapticError> {
        let image_key = match &event.image_key {
            Some(k) => k.clone(),
            None => {
                tracing::warn!("image message without image_key");
                return Ok(());
            }
        };

        let bytes = client.download_image(&image_key).await?;
        let tmp = std::env::temp_dir().join(format!("lark_img_{}.png", event.message_id()));
        std::fs::write(&tmp, &bytes)
            .map_err(|e| SynapticError::Tool(format!("failed to write image: {}", e)))?;

        let attachments = vec![Attachment {
            filename: "image.png".into(),
            url: format!("file://{}", tmp.display()),
            mime_type: Some("image/png".into()),
        }];

        let delivery = DeliveryContext {
            channel: "lark".into(),
            to: Some(format!("chat:{}", event.chat_id())),
            ..Default::default()
        };
        let mut envelope = MessageEnvelope::channel(
            session_key.to_string(),
            "[User sent an image]".to_string(),
            delivery,
        );
        envelope.attachments = attachments;

        match self.agent_session.handle_message(envelope).await {
            Ok(reply) => self.send_reply(event, client, &reply.content).await?,
            Err(e) => {
                client
                    .reply_text(event.message_id(), &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }

    /// Handle file messages: download and pass as attachment.
    async fn handle_file_message(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        session_key: &str,
    ) -> Result<(), SynapticError> {
        let file_key = match &event.file_key {
            Some(k) => k.clone(),
            None => {
                tracing::warn!("file message without file_key");
                return Ok(());
            }
        };

        let filename = event.file_name.as_deref().unwrap_or("file");
        let bytes = client
            .download_resource(event.message_id(), &file_key, "file")
            .await?;
        let tmp = std::env::temp_dir().join(format!("lark_file_{}", filename));
        std::fs::write(&tmp, &bytes)
            .map_err(|e| SynapticError::Tool(format!("failed to write file: {}", e)))?;

        let attachments = vec![Attachment {
            filename: filename.into(),
            url: format!("file://{}", tmp.display()),
            mime_type: None,
        }];

        let delivery = DeliveryContext {
            channel: "lark".into(),
            to: Some(format!("chat:{}", event.chat_id())),
            ..Default::default()
        };
        let mut envelope = MessageEnvelope::channel(
            session_key.to_string(),
            format!("[User sent file: {}]", filename),
            delivery,
        );
        envelope.attachments = attachments;

        match self.agent_session.handle_message(envelope).await {
            Ok(reply) => self.send_reply(event, client, &reply.content).await?,
            Err(e) => {
                client
                    .reply_text(event.message_id(), &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl MessageHandler for LarkHandler {
    async fn handle(
        &self,
        event: LarkMessageEvent,
        client: &LarkBotClient,
    ) -> Result<(), SynapticError> {
        // 1. Dedup
        if !self.dedup.check_and_mark(event.event_id()) {
            tracing::debug!(event_id = %event.event_id(), "duplicate event, skipping");
            return Ok(());
        }

        // 2. Policy
        if event.is_dm() {
            // DM policy: use enforcer
            if let Some(reply) = self.check_dm_access(event.sender_open_id()).await {
                if !reply.is_empty() {
                    client
                        .send_text("chat_id", event.chat_id(), &reply)
                        .await
                        .ok();
                }
                return Ok(());
            }
        } else if !self.check_group_policy(&event) {
            tracing::debug!(
                sender = %event.sender_open_id(),
                chat = %event.chat_id(),
                "message rejected by group policy"
            );
            return Ok(());
        }

        // 3. Mention gate (group only)
        let text = if event.is_group() && self.config.require_mention {
            if !event.mentions_bot(&self.bot_open_id) {
                return Ok(());
            }
            strip_bot_mention(event.text(), &event.mentions)
        } else {
            event.text().to_string()
        };

        if text.is_empty() && event.is_text() {
            return Ok(());
        }

        // 3.5 Command interception: /pair or 配对
        let text_trimmed = text.trim();
        if text_trimmed == "/pair" || text_trimmed == "配对" {
            let mut bootstrap = crate::gateway::nodes::BootstrapStore::new();
            let token = bootstrap.issue();
            let gateway_url = format!("ws://localhost:{}/ws", self.gateway_port);
            let setup_code =
                crate::gateway::nodes::bootstrap::encode_setup_code(&gateway_url, &token);
            let card = build_pair_card(&setup_code, &gateway_url);
            client
                .send_card("chat_id", event.chat_id(), &card)
                .await
                .ok();
            return Ok(());
        }

        // 4. Session key (using centralized computation)
        let session_key = compute_session_key(
            &event,
            &self.config.group_session_scope,
            &self.config.dm_scope,
            &self.account_id,
        );

        // 5. Typing indicator (reaction)
        let typing_reaction = if self.config.typing_indicator {
            client.add_reaction(event.message_id(), "OnIt").await.ok()
        } else {
            None
        };

        // 6. Route by message type
        let result = match event.message_type.as_str() {
            "text" | "post" => {
                self.handle_text_message(&event, client, &session_key, &text)
                    .await
            }
            "image" => {
                self.handle_image_message(&event, client, &session_key)
                    .await
            }
            "file" => self.handle_file_message(&event, client, &session_key).await,
            other => {
                tracing::debug!(message_type = %other, "unsupported message type");
                Ok(())
            }
        };

        // 7. Remove typing indicator
        if let Some(reaction_id) = typing_reaction {
            client
                .remove_reaction(event.message_id(), &reaction_id)
                .await
                .ok();
        }

        if let Err(ref e) = result {
            tracing::error!(error = %e, "lark message handling failed");
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Card action handler
// ---------------------------------------------------------------------------

struct LarkCardHandler {
    agent_session: Arc<AgentSession>,
}

#[async_trait]
impl CardActionHandler for LarkCardHandler {
    async fn handle(
        &self,
        event: CardActionEvent,
        client: &LarkBotClient,
    ) -> Result<(), SynapticError> {
        let text = event
            .action_value
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| event.action_value.get("command").and_then(|v| v.as_str()))
            .map(String::from)
            .unwrap_or_else(|| {
                format!(
                    "[Card action: {} = {}]",
                    event.action_tag, event.action_value
                )
            });

        let session_key = format!("lark:card:{}", event.chat_id);
        let delivery = DeliveryContext {
            channel: "lark".into(),
            to: Some(format!("chat:{}", event.chat_id)),
            ..Default::default()
        };
        let envelope = MessageEnvelope::channel(session_key, text, delivery);
        match self.agent_session.handle_message(envelope).await {
            Ok(reply) => {
                client
                    .send_text("chat_id", &event.chat_id, &reply.content)
                    .await?;
            }
            Err(e) => {
                client
                    .send_text("chat_id", &event.chat_id, &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// A thin adapter struct that wraps a [`LarkBotClient`] to expose the
/// synaptic channel traits without disturbing the existing event-loop code.
#[allow(dead_code)]
pub struct LarkChannelAdapter {
    client: LarkBotClient,
    status: RwLock<ChannelStatus>,
}

#[allow(dead_code)]
impl LarkChannelAdapter {
    pub fn new(client: LarkBotClient) -> Self {
        Self {
            client,
            status: RwLock::new(ChannelStatus::Disconnected),
        }
    }
}

#[async_trait]
impl ChannelAdapter for LarkChannelAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "lark".to_string(),
            name: "Lark / Feishu".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Threading,
                ChannelCap::Reactions,
                ChannelCap::Mentions,
                ChannelCap::Health,
            ],
            message_limit: Some(4096),
            supports_streaming: true,
            supports_threads: true,
            supports_reactions: true,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), SynapticError> {
        *self.status.write().unwrap() = ChannelStatus::Connecting;
        // The actual long-poll loop is managed externally via `run()`.
        // Mark as connected optimistically; the loop will surface errors.
        *self.status.write().unwrap() = ChannelStatus::Connected;
        Ok(())
    }

    async fn stop(&self) -> Result<(), SynapticError> {
        *self.status.write().unwrap() = ChannelStatus::Disconnected;
        Ok(())
    }

    fn status(&self) -> ChannelStatus {
        self.status.read().unwrap().clone()
    }
}

#[async_trait]
impl Outbound for LarkChannelAdapter {
    /// Send a text message to `envelope.channel_id` (interpreted as a Lark
    /// chat_id) with the textual content from `envelope.content`.
    async fn send(&self, envelope: &ChannelMessageEnvelope) -> Result<(), SynapticError> {
        self.client
            .send_text("chat_id", &envelope.channel_id, &envelope.content)
            .await?;
        Ok(())
    }

    /// Edit a previously sent interactive card.  `msg_id` must be the Lark
    /// card token; `content` is used as the new markdown body.
    /// Sequence is set to 0 (initial update); callers needing strict ordering
    /// should use the Lark client API directly.
    async fn edit(&self, msg_id: &str, content: &str) -> Result<(), SynapticError> {
        let card = serde_json::json!({
            "config": { "wide_screen_mode": true },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": content
                    }
                }
            ]
        });
        self.client.update_card(msg_id, 0, &card).await?;
        Ok(())
    }
}

#[async_trait]
impl ChannelHealth for LarkChannelAdapter {
    async fn health_check(&self) -> HealthStatus {
        match self.client.get_bot_info().await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy(format!("Lark API unreachable: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the Lark bot adapter.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
    status_handle: Option<Arc<dyn synaptic::ChannelStatusHandle>>,
    event_bus: Option<Arc<synaptic::events::EventBus>>,
    plugin_registry: Option<Arc<std::sync::RwLock<synaptic::plugin::PluginRegistry>>>,
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

    let card_handler = LarkCardHandler { agent_session };

    let mut listener = LarkLongConnListener::new(lark)
        .with_message_handler(msg_handler)
        .with_card_action_handler(card_handler);

    if let Some(handle) = status_handle {
        listener = listener.with_status_handle(handle);
    }

    listener.run().await?;
    Ok(())
}
