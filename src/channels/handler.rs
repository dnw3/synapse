use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use synaptic::core::{
    ChatModel, ChatRequest, ContentBlock, HeuristicTokenCounter, MemoryStore, Message, TokenCounter,
};
use synaptic::graph::{MessageState, StreamMode};
use synaptic::session::SessionManager;
use synaptic::store::FileStore;
use tokio::sync::RwLock;
use tracing;

use crate::agent;
use crate::agent::registry::ModelRegistry;
use crate::config::AgentDef;
use crate::config::SynapseConfig;
use crate::gateway::messages::routing::{
    resolve_delivery_target, update_last_route, SessionDeliveryState, TurnSource,
};
use crate::gateway::messages::{
    AgentReply, Attachment, ChannelRegistry, MessageEnvelope, MessageReceivedEvent,
    MessageSentEvent,
};
use crate::gateway::rpc::Broadcaster;
use crate::memory::LongTermMemory;
use crate::router::{BindingRouter, RoutingContext};

/// Resolved agent info passed through the message handling pipeline.
struct ResolvedAgentInfo {
    /// Agent ID.
    id: String,
    /// Model override (if any).
    #[allow(dead_code)]
    model_override: Option<String>,
    /// System prompt override (if any).
    prompt_override: Option<String>,
    /// Full agent definition (if routed to a defined agent).
    #[allow(dead_code)]
    def: Option<AgentDef>,
}

/// Result of routing: single agent or broadcast to multiple.
#[allow(clippy::large_enum_variant)]
enum ResolvedRoute {
    Single(ResolvedAgentInfo),
    Broadcast {
        group_name: String,
        strategy: crate::config::BroadcastStrategy,
        agents: Vec<ResolvedAgentInfo>,
        #[allow(dead_code)]
        timeout_secs: u64,
    },
}

/// Callback for streaming token output to bot adapters.
///
/// Implementors receive incremental updates as the agent generates a response,
/// enabling real-time message editing in chat platforms (e.g. Lark, Telegram).
#[async_trait]
pub trait StreamingOutput: Send + Sync {
    /// Called when new text content is generated (incremental delta).
    async fn on_token(&self, token: &str);
    /// Called when the agent invokes a tool.
    async fn on_tool_call(&self, tool_name: &str);
    /// Called when the agent finishes successfully.
    async fn on_complete(&self, full_response: &str);
    /// Called on error.
    async fn on_error(&self, error: &str);
}

/// Shared agent session handler for all bot adapters.
///
/// Supports two modes:
/// - **Deep Agent mode** (default): full tool calling, file operations, MCP, streaming.
///   Uses `build_deep_agent()` for each invocation, with persistent sessions via `SessionManager`.
/// - **Simple chat mode** (fallback): direct `model.chat()` for lightweight deployments.
///
/// All sessions are persisted to disk via `FileStore` and survive restarts.
pub struct AgentSession {
    model: Arc<dyn ChatModel>,
    config: Arc<SynapseConfig>,
    session_mgr: SessionManager,
    deep_agent: bool,
    /// The channel name (e.g. "lark", "slack", "telegram") for self-awareness context.
    channel: String,
    /// Tracks which session key maps to which session ID.
    session_map: RwLock<std::collections::HashMap<String, String>>,
    /// Optional channel registry for outbound dispatch (available in gateway mode).
    channel_registry: Option<Arc<RwLock<ChannelRegistry>>>,
    /// Optional broadcaster for message events (available in gateway mode).
    broadcaster: Option<Arc<Broadcaster>>,
    /// Optional binding router for multi-agent dispatch.
    router: Option<Arc<BindingRouter>>,
    /// Optional cost tracker for usage statistics.
    cost_tracker: Option<Arc<synaptic::callbacks::CostTrackingCallback>>,
    /// Optional multi-dimensional usage tracker.
    usage_tracker: Option<Arc<crate::usage::UsageTracker>>,
    /// Per-session run queue to serialize concurrent agent executions.
    run_queue: Arc<crate::gateway::run_queue::AgentRunQueue>,
}

impl AgentSession {
    /// Create a new AgentSession with persistent storage and deep agent support.
    pub fn new(model: Arc<dyn ChatModel>, config: Arc<SynapseConfig>, deep_agent: bool) -> Self {
        let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
        let store = Arc::new(FileStore::new(sessions_dir));
        let session_mgr = SessionManager::new(store);

        Self {
            model,
            config,
            session_mgr,
            deep_agent,
            channel: "unknown".to_string(),
            session_map: RwLock::new(std::collections::HashMap::new()),
            channel_registry: None,
            broadcaster: None,
            router: None,
            cost_tracker: None,
            usage_tracker: None,
            run_queue: Arc::new(crate::gateway::run_queue::AgentRunQueue::new()),
        }
    }

    /// Create a new AgentSession with channel-level model binding.
    ///
    /// If a `[[channel_models]]` entry matches the `channel_id`, the bound model is used.
    /// Otherwise falls back to the provided default model.
    #[allow(dead_code)]
    pub fn new_for_channel(
        default_model: Arc<dyn ChatModel>,
        config: Arc<SynapseConfig>,
        deep_agent: bool,
        channel_id: &str,
    ) -> Self {
        let registry = ModelRegistry::from_config(&config);
        let model = match registry.resolve_for_channel(channel_id) {
            Some(Ok(m)) => m,
            Some(Err(e)) => {
                eprintln!(
                    "warning: channel model binding for '{}' failed: {}; using default",
                    channel_id, e
                );
                default_model
            }
            None => default_model,
        };

        let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
        let store = Arc::new(FileStore::new(sessions_dir));
        let session_mgr = SessionManager::new(store);

        Self {
            model,
            config,
            session_mgr,
            deep_agent,
            channel: "unknown".to_string(),
            session_map: RwLock::new(std::collections::HashMap::new()),
            channel_registry: None,
            broadcaster: None,
            router: None,
            cost_tracker: None,
            usage_tracker: None,
            run_queue: Arc::new(crate::gateway::run_queue::AgentRunQueue::new()),
        }
    }

    /// Set the cost tracker for usage statistics.
    #[allow(dead_code)]
    pub fn with_cost_tracker(
        mut self,
        tracker: Arc<synaptic::callbacks::CostTrackingCallback>,
    ) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }

    /// Set the multi-dimensional usage tracker.
    #[allow(dead_code)]
    pub fn with_usage_tracker(mut self, tracker: Arc<crate::usage::UsageTracker>) -> Self {
        self.usage_tracker = Some(tracker);
        self
    }

    /// Set the binding router for multi-agent dispatch.
    #[allow(dead_code)]
    pub fn with_router(mut self, router: Arc<BindingRouter>) -> Self {
        self.router = Some(router);
        self
    }

    /// Set the channel name for self-awareness context (e.g. "lark", "slack", "web").
    pub fn with_channel(mut self, channel: &str) -> Self {
        self.channel = channel.to_string();
        self
    }

    /// Set channel registry and broadcaster for gateway mode.
    #[allow(dead_code)]
    pub fn with_gateway(
        mut self,
        channel_registry: Arc<RwLock<ChannelRegistry>>,
        broadcaster: Arc<Broadcaster>,
    ) -> Self {
        self.channel_registry = Some(channel_registry);
        self.broadcaster = Some(broadcaster);
        self
    }

    /// Build a routing context from a message envelope.
    fn routing_context(envelope: &MessageEnvelope) -> RoutingContext {
        RoutingContext {
            channel: Some(envelope.delivery.channel.clone()),
            account_id: envelope.delivery.account_id.clone(),
            peer_kind: envelope.routing.peer_kind.clone(),
            peer_id: envelope.routing.peer_id.clone(),
            sender_id: envelope.sender_id.clone(),
            guild_id: envelope.routing.guild_id.clone(),
            team_id: envelope.routing.team_id.clone(),
            roles: envelope.routing.roles.clone(),
            message: Some(envelope.content.clone()),
        }
    }

    /// Resolve the routing for this envelope via the binding router.
    fn resolve_route(&self, envelope: &MessageEnvelope) -> ResolvedRoute {
        if let Some(ref router) = self.router {
            let ctx = Self::routing_context(envelope);
            match router.resolve(&ctx) {
                crate::router::RouteResult::Single(resolved) => {
                    let agent_id = resolved.def.id.clone();
                    tracing::info!(
                        agent = %agent_id,
                        binding = ?resolved.binding.map(|b| &b.agent),
                        "routed to agent"
                    );
                    ResolvedRoute::Single(ResolvedAgentInfo {
                        id: agent_id,
                        model_override: resolved.def.model.clone(),
                        prompt_override: resolved.def.system_prompt.clone(),
                        def: Some(resolved.def.clone()),
                    })
                }
                crate::router::RouteResult::Broadcast { group, agents } => {
                    tracing::info!(
                        broadcast_group = %group.name,
                        strategy = ?group.strategy,
                        agent_count = agents.len(),
                        "broadcast match"
                    );
                    let infos: Vec<_> = agents
                        .iter()
                        .map(|r| ResolvedAgentInfo {
                            id: r.def.id.clone(),
                            model_override: r.def.model.clone(),
                            prompt_override: r.def.system_prompt.clone(),
                            def: Some(r.def.clone()),
                        })
                        .collect();
                    ResolvedRoute::Broadcast {
                        group_name: group.name.clone(),
                        strategy: group.strategy.clone(),
                        agents: infos,
                        timeout_secs: group.timeout_secs,
                    }
                }
            }
        } else {
            ResolvedRoute::Single(ResolvedAgentInfo {
                id: "default".into(),
                model_override: None,
                prompt_override: None,
                def: None,
            })
        }
    }

    /// Load delivery state from session metadata store.
    async fn load_delivery_state(&self, session_key: &str) -> SessionDeliveryState {
        let store = self.session_mgr.store();
        let ns = &["delivery_state"];
        match store.get(ns, session_key).await {
            Ok(Some(item)) => serde_json::from_value(item.value).unwrap_or_default(),
            _ => SessionDeliveryState::default(),
        }
    }

    /// Save delivery state to session metadata store.
    async fn save_delivery_state(&self, session_key: &str, state: &SessionDeliveryState) {
        let store = self.session_mgr.store();
        let ns = &["delivery_state"];
        if let Ok(value) = serde_json::to_value(state) {
            let _ = store.put(ns, session_key, value).await;
        }
    }

    /// Resolve or create a persistent session ID for a given chat key.
    async fn resolve_session(&self, session_key: &str) -> Result<String, AgentError> {
        // Check if we already have a mapping
        {
            let map = self.session_map.read().await;
            if let Some(sid) = map.get(session_key) {
                return Ok(sid.clone());
            }
        }

        // Try to find an existing session for this key by searching the store
        // Convention: we store the session_key → session_id mapping under a special namespace
        let store = self.session_mgr.store();
        let ns = &["bot_sessions"];
        if let Ok(Some(item)) = store.get(ns, session_key).await {
            if let Some(sid) = item.value.as_str() {
                // Verify the session still exists
                if self
                    .session_mgr
                    .get_session(sid)
                    .await
                    .ok()
                    .flatten()
                    .is_some()
                {
                    let mut map = self.session_map.write().await;
                    map.insert(session_key.to_string(), sid.to_string());
                    return Ok(sid.to_string());
                }
            }
        }

        // Legacy key fallback: try stripping the "agent:default:" prefix
        // Old keys look like "lark:dm:xxx", new keys like "agent:default:lark:dm:xxx"
        if let Some(legacy_key) = session_key.strip_prefix("agent:default:") {
            if let Ok(Some(item)) = store.get(ns, legacy_key).await {
                if let Some(sid) = item.value.as_str() {
                    if self
                        .session_mgr
                        .get_session(sid)
                        .await
                        .ok()
                        .flatten()
                        .is_some()
                    {
                        // Migrate: save under new key, cache
                        let _ = store
                            .put(ns, session_key, serde_json::Value::String(sid.to_string()))
                            .await;
                        let mut map = self.session_map.write().await;
                        map.insert(session_key.to_string(), sid.to_string());
                        tracing::info!(old_key = %legacy_key, new_key = %session_key, "migrated legacy session key");
                        return Ok(sid.to_string());
                    }
                }
            }
        }

        // Create a new session
        let sid = self
            .session_mgr
            .create_session()
            .await
            .map_err(|e| AgentError(format!("failed to create session: {}", e)))?;

        // Persist the mapping
        let _ = store
            .put(ns, session_key, serde_json::Value::String(sid.clone()))
            .await;

        let mut map = self.session_map.write().await;
        map.insert(session_key.to_string(), sid.clone());
        Ok(sid)
    }

    /// Process a message through the agent pipeline.
    /// This is the unified entry point for all channels.
    pub async fn handle_message(
        &self,
        envelope: MessageEnvelope,
    ) -> Result<AgentReply, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = envelope.request_id.clone();
        let session_key = envelope.session_key.clone();
        let channel = envelope.delivery.channel.clone();

        let start = Instant::now();
        let span = tracing::info_span!("agent_message",
            request_id = %request_id,
            channel = %channel,
            session_key = %session_key,
            provenance = ?envelope.provenance.kind,
        );
        let _guard = span.enter();

        // Serialize concurrent executions for the same session
        let _run_guard = self.run_queue.acquire(&session_key).await;

        // Resolve routing (single agent or broadcast)
        let route = self.resolve_route(&envelope);

        // Handle broadcast: fan out to multiple agents
        if let ResolvedRoute::Broadcast {
            ref group_name,
            ref strategy,
            ref agents,
            ..
        } = route
        {
            tracing::info!(broadcast = %group_name, "dispatching broadcast");
            return self
                .handle_broadcast_message(&envelope, agents, strategy)
                .await;
        }

        // Single agent path
        let agent_info = match route {
            ResolvedRoute::Single(info) => info,
            _ => unreachable!(),
        };
        tracing::info!(agent = %agent_info.id, "processing channel message");

        // Load delivery state
        let mut delivery_state = self.load_delivery_state(&session_key).await;

        // Set active_turn_source (cross-channel race prevention)
        delivery_state.active_turn_source = Some(TurnSource {
            turn_id: request_id.clone(),
            channel: envelope.delivery.channel.clone(),
            to: envelope.delivery.to.clone(),
            account_id: envelope.delivery.account_id.clone(),
            thread_id: envelope.delivery.thread_id.clone(),
        });

        // Save delivery state for crash recovery
        self.save_delivery_state(&session_key, &delivery_state)
            .await;

        // Broadcast message.received
        if let Some(ref broadcaster) = self.broadcaster {
            let event = MessageReceivedEvent::from_envelope(&envelope);
            if let Ok(payload) = serde_json::to_value(&event) {
                broadcaster.broadcast("message.received", payload).await;
            }
        }

        let sid = self.resolve_session(&session_key).await?;

        // Build content blocks from attachments
        let content_blocks = self.download_attachments(&envelope.attachments).await;

        // Snapshot cost tracker before agent call for usage diff
        let pre_snap = if let Some(ref tracker) = self.usage_tracker {
            Some(tracker.framework_tracker.snapshot().await)
        } else {
            None
        };

        let result = if self.deep_agent {
            self.handle_deep_agent(&sid, &envelope.content, &content_blocks, &agent_info)
                .await
        } else {
            self.handle_simple_chat(&sid, &envelope.content, &content_blocks)
                .await
        };

        let duration_ms = start.elapsed().as_millis();
        match &result {
            Ok(_) => tracing::info!(
                agent = %agent_info.id,
                duration_ms = duration_ms as u64,
                "channel message processed"
            ),
            Err(e) => {
                tracing::error!(agent = %agent_info.id, duration_ms = duration_ms as u64, error = %e, "channel message failed")
            }
        }

        // Record usage with dimensional metadata (even on error — we still used tokens)
        if let (Some(ref tracker), Some(pre)) = (&self.usage_tracker, pre_snap) {
            let post = tracker.framework_tracker.snapshot().await;
            let model_name = self.model.profile().map(|p| p.name).unwrap_or_default();
            let provider = self.model.profile().map(|p| p.provider).unwrap_or_default();
            tracker
                .record(crate::usage::UsageRecord {
                    model: model_name,
                    provider,
                    channel: channel.clone(),
                    agent_id: agent_info.id.clone(),
                    session_key: session_key.clone(),
                    input_tokens: post
                        .total_input_tokens
                        .saturating_sub(pre.total_input_tokens),
                    output_tokens: post
                        .total_output_tokens
                        .saturating_sub(pre.total_output_tokens),
                    total_tokens: (post.total_input_tokens + post.total_output_tokens)
                        .saturating_sub(pre.total_input_tokens + pre.total_output_tokens),
                    cost_usd: (post.estimated_cost_usd - pre.estimated_cost_usd).max(0.0),
                    latency_ms: duration_ms as u64,
                    timestamp_ms: crate::gateway::presence::now_ms(),
                })
                .await;
        }

        let response = result?;

        // Resolve delivery target via priority chain
        let delivery_target =
            resolve_delivery_target(&delivery_state, delivery_state.active_turn_source.as_ref())
                .unwrap_or_else(|_| envelope.delivery.clone());

        // Dispatch outbound (for non-webchat channels only)
        if delivery_target.channel != "webchat" {
            if let Some(ref registry) = self.channel_registry {
                let sender = {
                    let reg = registry.read().await;
                    reg.get(&delivery_target.channel).cloned()
                };
                if let Some(sender) = sender {
                    match sender
                        .send(&delivery_target, &response, delivery_target.meta.as_ref())
                        .await
                    {
                        Ok(send_result) => {
                            if let Some(ref broadcaster) = self.broadcaster {
                                let sent_event = MessageSentEvent {
                                    request_id: request_id.clone(),
                                    channel: delivery_target.channel.clone(),
                                    to: delivery_target.to.clone(),
                                    timestamp_ms: send_result.delivered_at_ms,
                                    message_id: send_result.message_id,
                                };
                                if let Ok(payload) = serde_json::to_value(&sent_event) {
                                    broadcaster.broadcast("message.sent", payload).await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "failed to dispatch to {}: {}",
                                delivery_target.channel,
                                e
                            );
                        }
                    }
                }
            }
        }

        // Update last_* fields
        update_last_route(&mut delivery_state, &delivery_target);

        // Clear active_turn_source
        delivery_state.active_turn_source = None;

        // Save delivery state
        self.save_delivery_state(&session_key, &delivery_state)
            .await;

        Ok(AgentReply {
            content: response,
            delivery_target,
            turn_id: request_id,
        })
    }

    /// Process a message with real-time streaming output.
    pub async fn handle_message_streaming(
        &self,
        envelope: MessageEnvelope,
        output: Arc<dyn StreamingOutput>,
    ) -> Result<AgentReply, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = envelope.request_id.clone();
        let session_key = envelope.session_key.clone();
        let channel = envelope.delivery.channel.clone();

        let start = Instant::now();
        let span = tracing::info_span!("agent_message",
            request_id = %request_id,
            channel = %channel,
            session_key = %session_key,
            provenance = ?envelope.provenance.kind,
        );
        let _guard = span.enter();

        // Serialize concurrent executions for the same session
        let _run_guard = self.run_queue.acquire(&session_key).await;

        // Resolve routing (single agent or broadcast)
        let route = self.resolve_route(&envelope);

        // Broadcast in streaming mode: fall back to non-streaming broadcast
        if let ResolvedRoute::Broadcast {
            ref group_name,
            ref strategy,
            ref agents,
            ..
        } = route
        {
            tracing::info!(broadcast = %group_name, "dispatching broadcast (streaming fallback)");
            return self
                .handle_broadcast_message(&envelope, agents, strategy)
                .await;
        }

        let agent_info = match route {
            ResolvedRoute::Single(info) => info,
            _ => unreachable!(),
        };
        tracing::info!(agent = %agent_info.id, "processing streaming channel message");

        // Load delivery state
        let mut delivery_state = self.load_delivery_state(&session_key).await;

        // Set active_turn_source (cross-channel race prevention)
        delivery_state.active_turn_source = Some(TurnSource {
            turn_id: request_id.clone(),
            channel: envelope.delivery.channel.clone(),
            to: envelope.delivery.to.clone(),
            account_id: envelope.delivery.account_id.clone(),
            thread_id: envelope.delivery.thread_id.clone(),
        });

        // Save delivery state for crash recovery
        self.save_delivery_state(&session_key, &delivery_state)
            .await;

        // Broadcast message.received
        if let Some(ref broadcaster) = self.broadcaster {
            let event = MessageReceivedEvent::from_envelope(&envelope);
            if let Ok(payload) = serde_json::to_value(&event) {
                broadcaster.broadcast("message.received", payload).await;
            }
        }

        let sid = self.resolve_session(&session_key).await?;

        // Build content blocks from attachments
        let content_blocks = self.download_attachments(&envelope.attachments).await;

        // Snapshot cost tracker before agent call for usage diff
        let pre_snap = if let Some(ref tracker) = self.usage_tracker {
            Some(tracker.framework_tracker.snapshot().await)
        } else {
            None
        };

        let result = if self.deep_agent {
            self.handle_deep_agent_streaming(
                &sid,
                &envelope.content,
                &content_blocks,
                output.clone(),
                &agent_info,
            )
            .await
        } else {
            // Simple chat doesn't support streaming, fall back and emit via callbacks
            let res = self
                .handle_simple_chat(&sid, &envelope.content, &content_blocks)
                .await;
            if let Ok(ref response) = res {
                output.on_token(response).await;
            }
            res
        };

        let duration_ms = start.elapsed().as_millis();
        match &result {
            Ok(response) => {
                output.on_complete(response).await;
                tracing::info!(
                    duration_ms = duration_ms as u64,
                    "streaming message processed"
                );
            }
            Err(e) => {
                output.on_error(&e.to_string()).await;
                tracing::error!(duration_ms = duration_ms as u64, error = %e, "streaming message failed");
            }
        }

        // Record usage with dimensional metadata (even on error — we still used tokens)
        if let (Some(ref tracker), Some(pre)) = (&self.usage_tracker, pre_snap) {
            let post = tracker.framework_tracker.snapshot().await;
            let model_name = self.model.profile().map(|p| p.name).unwrap_or_default();
            let provider = self.model.profile().map(|p| p.provider).unwrap_or_default();
            tracker
                .record(crate::usage::UsageRecord {
                    model: model_name,
                    provider,
                    channel: channel.clone(),
                    agent_id: agent_info.id.clone(),
                    session_key: session_key.clone(),
                    input_tokens: post
                        .total_input_tokens
                        .saturating_sub(pre.total_input_tokens),
                    output_tokens: post
                        .total_output_tokens
                        .saturating_sub(pre.total_output_tokens),
                    total_tokens: (post.total_input_tokens + post.total_output_tokens)
                        .saturating_sub(pre.total_input_tokens + pre.total_output_tokens),
                    cost_usd: (post.estimated_cost_usd - pre.estimated_cost_usd).max(0.0),
                    latency_ms: duration_ms as u64,
                    timestamp_ms: crate::gateway::presence::now_ms(),
                })
                .await;
        }

        let response = result?;

        // Resolve delivery target via priority chain
        let delivery_target =
            resolve_delivery_target(&delivery_state, delivery_state.active_turn_source.as_ref())
                .unwrap_or_else(|_| envelope.delivery.clone());

        // For webchat streaming, actual delivery is via WebSocket (caller handles it).
        // For other channels, streaming is not typically used for dispatch,
        // but broadcast message.sent to keep event bus consistent.
        if let Some(ref broadcaster) = self.broadcaster {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let sent_event = MessageSentEvent {
                request_id: request_id.clone(),
                channel: delivery_target.channel.clone(),
                to: delivery_target.to.clone(),
                timestamp_ms: now_ms,
                message_id: None,
            };
            if let Ok(payload) = serde_json::to_value(&sent_event) {
                broadcaster.broadcast("message.sent", payload).await;
            }
        }

        // Update last_* fields
        update_last_route(&mut delivery_state, &delivery_target);

        // Clear active_turn_source
        delivery_state.active_turn_source = None;

        // Save delivery state
        self.save_delivery_state(&session_key, &delivery_state)
            .await;

        Ok(AgentReply {
            content: response,
            delivery_target,
            turn_id: request_id,
        })
    }

    /// Download attachments and convert to ContentBlocks.
    /// Images and audio become multimodal blocks; other files become text references.
    async fn download_attachments(&self, attachments: &[Attachment]) -> Vec<ContentBlock> {
        if attachments.is_empty() {
            return Vec::new();
        }

        let tmp_dir = std::env::temp_dir().join(format!("synapse_{}", uuid::Uuid::new_v4()));
        if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
            tracing::warn!("failed to create temp dir: {}", e);
            return Vec::new();
        }

        let client = reqwest::Client::new();
        let mut content_blocks = Vec::new();

        for att in attachments {
            match client.get(&att.url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let file_path = tmp_dir.join(&att.filename);
                    match resp.bytes().await {
                        Ok(bytes) => {
                            if let Err(e) = std::fs::write(&file_path, &bytes) {
                                tracing::warn!(
                                    "failed to write attachment {}: {}",
                                    att.filename,
                                    e
                                );
                                continue;
                            }
                            let file_url = format!("file://{}", file_path.display());
                            let mime = att
                                .mime_type
                                .as_deref()
                                .or_else(|| detect_mime_from_extension(&att.filename));

                            match mime {
                                Some(m) if m.starts_with("image/") => {
                                    content_blocks.push(ContentBlock::Image {
                                        url: file_url,
                                        detail: None,
                                    });
                                }
                                Some(m) if m.starts_with("audio/") => {
                                    content_blocks.push(ContentBlock::Audio { url: file_url });
                                }
                                _ => {
                                    content_blocks.push(ContentBlock::Text {
                                        text: format!("[Attached file: {}]", file_path.display()),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("failed to download attachment {}: {}", att.filename, e);
                        }
                    }
                }
                Ok(resp) => {
                    tracing::warn!("attachment download failed with HTTP {}", resp.status());
                }
                Err(e) => {
                    tracing::warn!("failed to fetch attachment {}: {}", att.filename, e);
                }
            }
        }

        content_blocks
    }

    /// Deep Agent mode: full tool calling loop.
    async fn handle_deep_agent(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
        agent_info: &ResolvedAgentInfo,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let memory = self.session_mgr.memory();

        // Load existing messages
        let mut messages = memory.load(session_id).await.unwrap_or_default();

        // Add system prompt if this is a new conversation
        // Priority: agent route override > global config > default
        if messages.is_empty() {
            let system_prompt = agent_info
                .prompt_override
                .clone()
                .or_else(|| self.config.base.agent.system_prompt.clone())
                .unwrap_or_else(|| {
                    "You are Synapse, a helpful AI assistant. You can read and write files, \
                     execute commands, and help with complex tasks. Keep responses concise \
                     for chat messages."
                        .to_string()
                });
            messages.push(Message::system(&system_prompt));
        }

        // Append user message (with multimodal content blocks if present)
        let human_msg = if content_blocks.is_empty() {
            Message::human(text)
        } else {
            Message::human(text).with_content_blocks(content_blocks.to_vec())
        };
        memory
            .append(session_id, human_msg.clone())
            .await
            .map_err(|e| AgentError(format!("failed to save message: {}", e)))?;
        messages.push(human_msg);

        // Build deep agent
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let checkpointer = Arc::new(self.session_mgr.checkpointer());
        let mcp_tools = agent::load_mcp_tools(&self.config).await;

        let agent = agent::build_deep_agent_with_callback(
            self.model.clone(),
            &self.config,
            &cwd,
            checkpointer,
            mcp_tools,
            None,
            Some(Arc::new(agent::BotSafetyCallback)),
            None, // no LTM tools in bot mode
            None, // no session tools in bot mode
            None, // no session overrides in bot mode
            self.cost_tracker.clone(),
            &self.channel,
            None, // agent routing resolved at higher level
        )
        .await
        .map_err(|e| AgentError(format!("failed to build agent: {}", e)))?;

        // Invoke agent (non-streaming for bot replies)
        let initial_state = MessageState::with_messages(messages);
        let result = agent
            .invoke(initial_state)
            .await
            .map_err(|e| AgentError(format!("agent error: {}", e)))?;

        let final_state = result.into_state();

        // Extract final AI response text from the last messages
        let response = extract_final_response(&final_state.messages);

        // Save new messages to history (skip the ones we already had)
        let saved_count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
        for msg in final_state.messages.iter().skip(saved_count) {
            memory.append(session_id, msg.clone()).await.ok();
        }

        // Token-aware trimming with pre-compaction LTM flush
        let mut current = memory.load(session_id).await.unwrap_or_default();
        let token_count = HeuristicTokenCounter.count_messages(&current);
        let threshold = self.config.memory.auto_compact_threshold;
        if token_count > threshold {
            // Pre-compaction flush: extract important memories before trimming
            // Use per-agent memory dir if routed to a named agent
            let ltm_dir = if agent_info.id != "default" {
                crate::config::agent_memory_dir(&agent_info.id)
            } else {
                PathBuf::from(&self.config.base.paths.sessions_dir).join("long_term_memory")
            };
            let ltm = LongTermMemory::new(ltm_dir, self.config.memory.clone());
            ltm.load().await.ok();

            let keep_recent = self.config.memory.keep_recent;
            let discard_end = current.len().saturating_sub(keep_recent);
            if discard_end > 0 {
                ltm.flush_before_compact(&current[..discard_end], self.model.as_ref())
                    .await;
            }

            // Prune tool results before trimming
            let opts = crate::tools::PruningOptions::from_config(&self.config.memory);
            crate::tools::prune_tool_results_with_options(&mut current, &opts);

            // Truncate: keep system + last N messages
            memory.clear(session_id).await.ok();
            let system = current.iter().find(|m| m.is_system()).cloned();
            let keep_from = current.len().saturating_sub(keep_recent);
            if let Some(sys) = system {
                memory.append(session_id, sys).await.ok();
            }
            for msg in current.iter().skip(keep_from) {
                if !msg.is_system() {
                    memory.append(session_id, msg.clone()).await.ok();
                }
            }
        }

        Ok(response)
    }

    /// Handle broadcast: fan out to multiple agents in parallel.
    ///
    /// Each agent processes the message independently with its own session/prompt/memory.
    /// Replies are collected and merged into a single response.
    async fn handle_broadcast_message(
        &self,
        envelope: &MessageEnvelope,
        agents: &[ResolvedAgentInfo],
        strategy: &crate::config::BroadcastStrategy,
    ) -> Result<AgentReply, Box<dyn std::error::Error + Send + Sync>> {
        use crate::config::BroadcastStrategy;

        let request_id = envelope.request_id.clone();
        let session_key = envelope.session_key.clone();
        let content_blocks = self.download_attachments(&envelope.attachments).await;

        match strategy {
            BroadcastStrategy::Parallel | BroadcastStrategy::Aggregated => {
                // Spawn all agents concurrently
                let mut set = tokio::task::JoinSet::new();
                for agent_info in agents {
                    let _sid_key = format!(
                        "agent:{}:{}",
                        agent_info.id,
                        session_key.trim_start_matches("agent:default:")
                    );
                    let text = envelope.content.clone();
                    let blocks = content_blocks.clone();
                    let agent_id = agent_info.id.clone();
                    let prompt = agent_info.prompt_override.clone();

                    // Create agent info for the spawned task
                    let info = ResolvedAgentInfo {
                        id: agent_id.clone(),
                        model_override: agent_info.model_override.clone(),
                        prompt_override: prompt,
                        def: agent_info.def.clone(),
                    };

                    let memory_store = self.session_mgr.memory();
                    let model = self.model.clone();
                    let config = self.config.clone();
                    let deep = self.deep_agent;

                    let checkpointer = Arc::new(self.session_mgr.checkpointer());

                    set.spawn(async move {
                        // Use a unique session for each broadcast agent
                        let memory = memory_store;
                        let sid = uuid::Uuid::new_v4().to_string();

                        // Build messages
                        let mut messages = memory.load(&sid).await.unwrap_or_default();
                        if messages.is_empty() {
                            let sys_prompt = info
                                .prompt_override
                                .clone()
                                .or_else(|| config.base.agent.system_prompt.clone())
                                .unwrap_or_else(|| "You are a helpful AI assistant.".into());
                            messages.push(Message::system(&sys_prompt));
                        }
                        let human_msg = if blocks.is_empty() {
                            Message::human(&text)
                        } else {
                            Message::human(&text).with_content_blocks(blocks)
                        };
                        memory.append(&sid, human_msg.clone()).await.ok();
                        messages.push(human_msg);

                        if deep {
                            let cwd =
                                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                            let mcp_tools = agent::load_mcp_tools(&config).await;

                            let agent = agent::build_deep_agent_with_callback(
                                model,
                                &config,
                                &cwd,
                                checkpointer,
                                mcp_tools,
                                None,
                                Some(Arc::new(agent::BotSafetyCallback)),
                                None,
                                None,
                                None,
                                None,
                                "broadcast",
                                None,
                            )
                            .await
                            .map_err(|e| AgentError(format!("agent build: {}", e)))?;

                            let initial_state = MessageState::with_messages(messages);
                            let result = agent
                                .invoke(initial_state)
                                .await
                                .map_err(|e| AgentError(format!("agent error: {}", e)))?;
                            let response = extract_final_response(&result.into_state().messages);
                            Ok::<(String, String), Box<dyn std::error::Error + Send + Sync>>((
                                agent_id, response,
                            ))
                        } else {
                            let req = ChatRequest::new(messages);
                            let resp = model
                                .chat(req)
                                .await
                                .map_err(|e| AgentError(format!("chat error: {}", e)))?;
                            let text = resp.message.content().to_string();
                            Ok((agent_id, text))
                        }
                    });
                }

                // Collect results
                let mut replies: Vec<(String, String)> = Vec::new();
                while let Some(result) = set.join_next().await {
                    match result {
                        Ok(Ok((agent_id, response))) => {
                            tracing::info!(agent = %agent_id, "broadcast agent completed");
                            replies.push((agent_id, response));
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(error = %e, "broadcast agent failed");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "broadcast task panicked");
                        }
                    }
                }

                // Merge responses
                let merged = if replies.len() == 1 {
                    replies.into_iter().next().unwrap().1
                } else {
                    replies
                        .iter()
                        .map(|(agent_id, response)| format!("**[{}]**\n\n{}", agent_id, response))
                        .collect::<Vec<_>>()
                        .join("\n\n---\n\n")
                };

                Ok(AgentReply {
                    content: merged,
                    delivery_target: envelope.delivery.clone(),
                    turn_id: request_id,
                })
            }
            BroadcastStrategy::Sequential => {
                // Process agents one by one, return last response
                let mut last_response = String::new();
                for agent_info in agents {
                    let sid = self.resolve_session(&session_key).await?;
                    match self
                        .handle_deep_agent(&sid, &envelope.content, &content_blocks, agent_info)
                        .await
                    {
                        Ok(response) => {
                            last_response = response;
                        }
                        Err(e) => {
                            tracing::warn!(agent = %agent_info.id, error = %e, "sequential broadcast agent failed");
                        }
                    }
                }
                Ok(AgentReply {
                    content: last_response,
                    delivery_target: envelope.delivery.clone(),
                    turn_id: request_id,
                })
            }
        }
    }

    /// Simple chat mode: direct model.chat() call without tools.
    async fn handle_simple_chat(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let memory = self.session_mgr.memory();

        // Load existing messages
        let mut messages = memory.load(session_id).await.unwrap_or_default();

        // Add system prompt if new conversation
        if messages.is_empty() {
            if let Some(ref prompt) = self.config.base.agent.system_prompt {
                messages.push(Message::system(prompt));
            }
        }

        // Append user message (with multimodal content blocks if present)
        let human_msg = if content_blocks.is_empty() {
            Message::human(text)
        } else {
            Message::human(text).with_content_blocks(content_blocks.to_vec())
        };
        memory.append(session_id, human_msg.clone()).await.ok();
        messages.push(human_msg);

        // Call model
        let request = ChatRequest::new(messages.clone());
        let response = self
            .model
            .chat(request)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        let content = response.message.content().to_string();

        // Save AI response
        let ai_msg = Message::ai(&content);
        memory.append(session_id, ai_msg).await.ok();

        // Token-aware trimming
        let current = memory.load(session_id).await.unwrap_or_default();
        let token_count = HeuristicTokenCounter.count_messages(&current);
        let threshold = self.config.memory.auto_compact_threshold;
        if token_count > threshold {
            let keep_recent = self.config.memory.keep_recent;
            memory.clear(session_id).await.ok();
            let system = current.iter().find(|m| m.is_system()).cloned();
            let keep_from = current.len().saturating_sub(keep_recent);
            if let Some(sys) = system {
                memory.append(session_id, sys).await.ok();
            }
            for msg in current.iter().skip(keep_from) {
                if !msg.is_system() {
                    memory.append(session_id, msg.clone()).await.ok();
                }
            }
        }

        Ok(content)
    }

    /// Deep Agent mode with streaming: full tool calling loop with incremental output.
    async fn handle_deep_agent_streaming(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
        output: Arc<dyn StreamingOutput>,
        agent_info: &ResolvedAgentInfo,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let memory = self.session_mgr.memory();

        // Load existing messages
        let mut messages = memory.load(session_id).await.unwrap_or_default();

        // Add system prompt if this is a new conversation
        // Priority: agent route override > global config > default
        if messages.is_empty() {
            let system_prompt = agent_info
                .prompt_override
                .clone()
                .or_else(|| self.config.base.agent.system_prompt.clone())
                .unwrap_or_else(|| {
                    "You are Synapse, a helpful AI assistant. You can read and write files, \
                     execute commands, and help with complex tasks. Keep responses concise \
                     for chat messages."
                        .to_string()
                });
            messages.push(Message::system(&system_prompt));
        }

        // Append user message (with multimodal content blocks if present)
        let human_msg = if content_blocks.is_empty() {
            Message::human(text)
        } else {
            Message::human(text).with_content_blocks(content_blocks.to_vec())
        };
        memory
            .append(session_id, human_msg.clone())
            .await
            .map_err(|e| AgentError(format!("failed to save message: {}", e)))?;
        messages.push(human_msg);

        // Build deep agent
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let checkpointer = Arc::new(self.session_mgr.checkpointer());
        let mcp_tools = agent::load_mcp_tools(&self.config).await;

        let agent = agent::build_deep_agent_with_callback(
            self.model.clone(),
            &self.config,
            &cwd,
            checkpointer,
            mcp_tools,
            None,
            Some(Arc::new(agent::BotSafetyCallback)),
            None,
            None,
            None,
            None,
            &self.channel,
            None, // agent routing resolved at higher level
        )
        .await
        .map_err(|e| AgentError(format!("failed to build agent: {}", e)))?;

        // Stream agent execution
        let initial_state = MessageState::with_messages(messages);
        let mut stream = agent.stream(initial_state, StreamMode::Values);

        let mut last_content_len = 0;
        let mut final_state = None;

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    // Check if there's new AI content in this state snapshot
                    let current_content = extract_final_response(&event.state.messages);
                    if current_content.len() > last_content_len {
                        let new_text = &current_content[last_content_len..];
                        output.on_token(new_text).await;
                        last_content_len = current_content.len();
                    }

                    // Detect tool call nodes (heuristic: node name contains "tool")
                    if event.node.contains("tool") {
                        output.on_tool_call(&event.node).await;
                    }

                    final_state = Some(event.state);
                }
                Err(e) => {
                    return Err(Box::new(AgentError(format!("agent stream error: {}", e))));
                }
            }
        }

        let final_state =
            final_state.ok_or_else(|| AgentError("no output from agent stream".into()))?;
        let response = extract_final_response(&final_state.messages);

        // Save new messages to history (skip the ones we already had)
        let saved_count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
        for msg in final_state.messages.iter().skip(saved_count) {
            memory.append(session_id, msg.clone()).await.ok();
        }

        // Token-aware trimming with pre-compaction LTM flush
        let mut current = memory.load(session_id).await.unwrap_or_default();
        let token_count = HeuristicTokenCounter.count_messages(&current);
        let threshold = self.config.memory.auto_compact_threshold;
        if token_count > threshold {
            // Pre-compaction flush: extract important memories before trimming
            // Use per-agent memory dir if routed to a named agent
            let ltm_dir = if agent_info.id != "default" {
                crate::config::agent_memory_dir(&agent_info.id)
            } else {
                PathBuf::from(&self.config.base.paths.sessions_dir).join("long_term_memory")
            };
            let ltm = LongTermMemory::new(ltm_dir, self.config.memory.clone());
            ltm.load().await.ok();

            let keep_recent = self.config.memory.keep_recent;
            let discard_end = current.len().saturating_sub(keep_recent);
            if discard_end > 0 {
                ltm.flush_before_compact(&current[..discard_end], self.model.as_ref())
                    .await;
            }

            // Prune tool results before trimming
            let opts = crate::tools::PruningOptions::from_config(&self.config.memory);
            crate::tools::prune_tool_results_with_options(&mut current, &opts);

            // Truncate: keep system + last N messages
            memory.clear(session_id).await.ok();
            let system = current.iter().find(|m| m.is_system()).cloned();
            let keep_from = current.len().saturating_sub(keep_recent);
            if let Some(sys) = system {
                memory.append(session_id, sys).await.ok();
            }
            for msg in current.iter().skip(keep_from) {
                if !msg.is_system() {
                    memory.append(session_id, msg.clone()).await.ok();
                }
            }
        }

        Ok(response)
    }
}

/// Extract the final AI response text from the message list.
///
/// In a deep agent loop, the last AI message with non-empty content
/// (that isn't just tool calls) is the final response.
fn extract_final_response(messages: &[Message]) -> String {
    // Walk backwards to find the last AI message with text content
    for msg in messages.iter().rev() {
        if msg.is_ai() {
            let content = msg.content();
            if !content.is_empty() {
                return content.to_string();
            }
        }
    }
    "I processed your request but have no text response.".to_string()
}

/// Detect MIME type from a filename extension. Returns `None` for unknown types.
fn detect_mime_from_extension(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        // Images
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "tiff" | "tif" => Some("image/tiff"),
        "ico" => Some("image/x-icon"),
        "heic" | "heif" => Some("image/heic"),
        // Audio
        "mp3" => Some("audio/mpeg"),
        "wav" => Some("audio/wav"),
        "ogg" | "oga" => Some("audio/ogg"),
        "flac" => Some("audio/flac"),
        "aac" => Some("audio/aac"),
        "m4a" => Some("audio/mp4"),
        "weba" => Some("audio/webm"),
        "opus" => Some("audio/opus"),
        // Video (treated as non-media for now)
        // Documents / other — return None to fall back to text
        _ => None,
    }
}

/// Simple error type for AgentSession.
#[derive(Debug)]
struct AgentError(String);

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AgentError {}
