use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use synaptic::core::{
    ChatModel, ChatRequest, ContentBlock, HeuristicTokenCounter, MemoryStore, Message, TokenCounter,
};
use synaptic::graph::{MessageState, StreamMode};
use synaptic::session::SessionManager;
use synaptic::store::FileStore;
use synaptic::DeliveryContext;
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
    AgentReply, Attachment, ChannelRegistry, InboundMessage, MessageReceivedEvent,
    MessageSentEvent, OutboundPayload,
};
use crate::gateway::rpc::Broadcaster;
use crate::memory::LongTermMemory;
use crate::router::{BindingRouter, RoutingContext};

mod broadcast;
mod execution;
mod session;

// Re-export items used by sub-modules via `use super::*`
use execution::{detect_mime_from_extension, extract_final_response};

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

// Re-export streaming types from the framework layer.
pub use synaptic::graph::streaming::{CompletionMeta, StreamingOutput, ToolCallInfo};

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
    usage_tracker: Option<Arc<crate::gateway::usage::UsageTracker>>,
    /// Per-session run queue to serialize concurrent agent executions.
    run_queue: Arc<crate::gateway::run_queue::AgentRunQueue>,
    /// Optional Outbound trait impl for the new channel trait interface.
    outbound: Option<Arc<dyn synaptic::core::Outbound>>,
    /// Optional EventBus for event-driven usage tracking and subscriber notifications.
    event_bus: Option<Arc<synaptic::events::EventBus>>,
    /// Optional PluginRegistry for plugin-registered tools.
    plugin_registry: Option<Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>>,
    /// Resolver for tool display metadata (emoji, label, detail).
    display_resolver: Arc<crate::agent::tool_display::ToolDisplayResolver>,
}

impl AgentSession {
    /// Create a new AgentSession with persistent storage and deep agent support.
    pub fn new(model: Arc<dyn ChatModel>, config: Arc<SynapseConfig>, deep_agent: bool) -> Self {
        let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
        let store = Arc::new(FileStore::new(sessions_dir));
        let session_mgr = SessionManager::new(store);
        let display_resolver = Arc::new(crate::agent::tool_display::ToolDisplayResolver::new(
            config.tool_display.clone(),
        ));

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
            outbound: None,
            event_bus: None,
            plugin_registry: None,
            display_resolver,
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

        let display_resolver2 = Arc::new(crate::agent::tool_display::ToolDisplayResolver::new(
            config.tool_display.clone(),
        ));

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
            outbound: None,
            event_bus: None,
            plugin_registry: None,
            display_resolver: display_resolver2,
        }
    }

    /// Set the EventBus for event-driven tracking in bot mode.
    pub fn with_event_bus(mut self, event_bus: Arc<synaptic::events::EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Set the PluginRegistry for plugin tools in bot mode.
    pub fn with_plugin_registry(
        mut self,
        registry: Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>,
    ) -> Self {
        self.plugin_registry = Some(registry);
        self
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
    pub fn with_usage_tracker(mut self, tracker: Arc<crate::gateway::usage::UsageTracker>) -> Self {
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

    /// Set the Outbound trait impl for the new channel trait interface.
    #[allow(dead_code)]
    pub fn with_outbound(mut self, outbound: Arc<dyn synaptic::core::Outbound>) -> Self {
        self.outbound = Some(outbound);
        self
    }

    /// Build a `DeliveryContext` from an `InboundMessage` for outbound dispatch.
    ///
    /// This is used as a fallback when the delivery routing system cannot determine
    /// a target from session state.
    pub(super) fn delivery_context_from_inbound(msg: &InboundMessage) -> DeliveryContext {
        DeliveryContext {
            channel: msg.channel.platform.clone(),
            to: msg
                .sender
                .id
                .clone()
                .map(|id| format!("{}:{}", msg.chat.chat_type, id)),
            account_id: msg.channel.account_id.clone(),
            thread_id: msg.thread.thread_id.clone(),
            meta: None,
        }
    }

    /// Build a `TurnSource` from an `InboundMessage` for cross-channel race prevention.
    fn turn_source_from_inbound(msg: &InboundMessage) -> TurnSource {
        TurnSource {
            turn_id: msg.request_id.clone(),
            channel: msg.channel.platform.clone(),
            to: msg
                .sender
                .id
                .clone()
                .map(|id| format!("{}:{}", msg.chat.chat_type, id)),
            account_id: msg.channel.account_id.clone(),
            thread_id: msg.thread.thread_id.clone(),
        }
    }

    /// Process a message through the agent pipeline.
    ///
    /// This is the unified entry point for all channels. The caller provides a
    /// `RunContext` which may carry a cancel token and a `StreamingOutputHandle`.
    /// The framework's `StreamingInterceptor` will use the handle to forward
    /// tokens automatically.
    pub async fn handle_message(
        &self,
        msg: InboundMessage,
        ctx: synaptic::core::RunContext,
    ) -> Result<AgentReply, Box<dyn std::error::Error + Send + Sync>> {
        use synaptic::deep::StreamingOutputHandle;

        let request_id = msg.request_id.clone();
        let session_key = msg.session_key.clone();
        let channel = msg.channel.platform.clone();

        let start = Instant::now();
        let span = tracing::info_span!("agent_message",
            request_id = %request_id,
            channel = %channel,
            session_key = %session_key,
        );
        let _guard = span.enter();

        // Serialize concurrent executions for the same session
        let _run_guard = self.run_queue.acquire(&session_key).await;

        // Resolve routing (single agent or broadcast)
        let route = self.resolve_route(&msg);

        // Broadcast in streaming mode: fall back to non-streaming broadcast
        if let ResolvedRoute::Broadcast {
            ref group_name,
            ref strategy,
            ref agents,
            ..
        } = route
        {
            tracing::info!(broadcast = %group_name, "dispatching broadcast (streaming fallback)");
            return self.handle_broadcast_message(&msg, agents, strategy).await;
        }

        let agent_info = match route {
            ResolvedRoute::Single(info) => info,
            _ => unreachable!(),
        };
        tracing::info!(agent = %agent_info.id, "processing channel message");

        // Load delivery state
        let mut delivery_state = self.load_delivery_state(&session_key).await;

        // Set active_turn_source (cross-channel race prevention)
        delivery_state.active_turn_source = Some(Self::turn_source_from_inbound(&msg));

        // Save delivery state for crash recovery
        self.save_delivery_state(&session_key, &delivery_state)
            .await;

        // Broadcast message.received
        if let Some(ref broadcaster) = self.broadcaster {
            let event = MessageReceivedEvent::from_inbound(&msg);
            if let Ok(payload) = serde_json::to_value(&event) {
                broadcaster.broadcast("message.received", payload).await;
            }
        }

        let sid = self.resolve_session(&session_key, &msg).await?;

        // Build content blocks from attachments
        let content_blocks = self.download_attachments(&msg.attachments).await;

        // Extract StreamingOutput from RunContext for on_complete/on_error callbacks
        let output_handle = ctx.streaming_output::<StreamingOutputHandle>();

        let result: Result<(String, u32, u32), Box<dyn std::error::Error + Send + Sync>> =
            if self.deep_agent {
                self.handle_deep_agent(
                    &sid,
                    &msg.content,
                    &content_blocks,
                    ctx,
                    &agent_info,
                    Some(&msg.request_id),
                )
                .await
            } else {
                // Simple chat doesn't support streaming, fall back
                let res = self
                    .handle_simple_chat(&sid, &msg.content, &content_blocks)
                    .await;
                if let Ok(ref response) = res {
                    if let Some(ref handle) = output_handle {
                        handle.0.on_token(response).await;
                    }
                }
                res.map(|r| (r, 0u32, 0u32))
            };

        let duration_ms = start.elapsed().as_millis() as u64;
        match &result {
            Ok((response, input_tokens, output_tokens)) => {
                if let Some(ref handle) = output_handle {
                    let meta = CompletionMeta {
                        input_tokens: *input_tokens,
                        output_tokens: *output_tokens,
                        duration_ms,
                        request_id: Some(msg.request_id.clone()),
                    };
                    handle.0.on_complete(response, Some(&meta)).await;
                }
                tracing::info!(duration_ms, "message processed");
            }
            Err(e) => {
                if let Some(ref handle) = output_handle {
                    handle.0.on_error(&e.to_string()).await;
                }
                tracing::error!(duration_ms = duration_ms, error = %e, "message failed");
            }
        }

        let (response, input_tokens, output_tokens) = result?;

        // Update session total_tokens
        let token_delta = (input_tokens + output_tokens) as u64;
        if token_delta > 0 {
            if let Ok(Some(mut info)) = self.session_mgr.get_session(&sid).await {
                info.total_tokens += token_delta;
                info.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let _ = self.session_mgr.update_session(&info).await;
            }
        }

        // Resolve delivery target via priority chain
        let fallback_delivery = Self::delivery_context_from_inbound(&msg);
        let delivery_target =
            resolve_delivery_target(&delivery_state, delivery_state.active_turn_source.as_ref())
                .unwrap_or(fallback_delivery);

        // Broadcast message.sent
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
            payloads: vec![OutboundPayload {
                text: Some(response.clone()),
                ..Default::default()
            }],
            content: response,
            delivery_target,
            turn_id: request_id,
        })
    }

    /// Download attachments and convert to ContentBlocks.
    /// Images and audio become multimodal blocks; other files become text references.
    pub(super) async fn download_attachments(
        &self,
        attachments: &[Attachment],
    ) -> Vec<ContentBlock> {
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
}

/// Simple error type for AgentSession.
#[derive(Debug)]
pub(super) struct AgentError(pub(super) String);

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AgentError {}
