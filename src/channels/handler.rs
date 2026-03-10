use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use synaptic::core::{ChatModel, ChatRequest, ContentBlock, HeuristicTokenCounter, MemoryStore, Message, TokenCounter};
use synaptic::graph::{MessageState, StreamMode};
use synaptic::session::SessionManager;
use synaptic::store::FileStore;
use futures::StreamExt;
use tokio::sync::RwLock;
use tracing;

use crate::agent;
use crate::agent::registry::ModelRegistry;
use crate::config::SynapseConfig;
use crate::logging;
use crate::memory::LongTermMemory;

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
    /// Tracks which session key maps to which session ID.
    session_map: RwLock<std::collections::HashMap<String, String>>,
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
            session_map: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Create a new AgentSession with channel-level model binding.
    ///
    /// If a `[[channel_models]]` entry matches the `channel_id`, the bound model is used.
    /// Otherwise falls back to the provided default model.
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
            session_map: RwLock::new(std::collections::HashMap::new()),
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
                if self.session_mgr.get_session(sid).await.ok().flatten().is_some() {
                    let mut map = self.session_map.write().await;
                    map.insert(session_key.to_string(), sid.to_string());
                    return Ok(sid.to_string());
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

    /// Handle a message from a chat session, returning the AI response.
    ///
    /// In deep agent mode, this runs the full agent loop with tool calling.
    /// In simple mode, this does a direct model.chat() call.
    pub async fn handle_message(
        &self,
        session_key: &str,
        text: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.handle_message_inner(session_key, text, Vec::new()).await
    }

    /// Inner handler that supports optional multimodal content blocks.
    async fn handle_message_inner(
        &self,
        session_key: &str,
        text: &str,
        content_blocks: Vec<ContentBlock>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = logging::generate_request_id();
        let start = Instant::now();
        let span = tracing::info_span!("channel_message",
            request_id = %request_id,
            session_key = %session_key,
        );
        let _guard = span.enter();

        tracing::info!("processing channel message");

        let sid = self.resolve_session(session_key).await?;

        let result = if self.deep_agent {
            self.handle_deep_agent(&sid, text, &content_blocks).await
        } else {
            self.handle_simple_chat(&sid, text, &content_blocks).await
        };

        let duration_ms = start.elapsed().as_millis();
        match &result {
            Ok(_) => tracing::info!(duration_ms = duration_ms as u64, "channel message processed"),
            Err(e) => tracing::error!(duration_ms = duration_ms as u64, error = %e, "channel message failed"),
        }

        result
    }

    /// Deep Agent mode: full tool calling loop.
    async fn handle_deep_agent(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let memory = self.session_mgr.memory();

        // Load existing messages
        let mut messages = memory.load(session_id).await.unwrap_or_default();

        // Add system prompt if this is a new conversation
        if messages.is_empty() {
            let system_prompt = self
                .config
                .base
                .agent
                .system_prompt
                .clone()
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
            None, // no cost tracking in bot mode
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
        let saved_count = memory
            .load(session_id)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        for msg in final_state.messages.iter().skip(saved_count) {
            memory.append(session_id, msg.clone()).await.ok();
        }

        // Token-aware trimming with pre-compaction LTM flush
        let mut current = memory.load(session_id).await.unwrap_or_default();
        let token_count = HeuristicTokenCounter.count_messages(&current);
        let threshold = self.config.memory.auto_compact_threshold;
        if token_count > threshold {
            // Pre-compaction flush: extract important memories before trimming
            let sessions_dir = PathBuf::from(&self.config.base.paths.sessions_dir);
            let ltm = LongTermMemory::new(
                sessions_dir.join("long_term_memory"),
                self.config.memory.clone(),
            );
            ltm.load().await.ok();

            let keep_recent = self.config.memory.keep_recent;
            let discard_end = current.len().saturating_sub(keep_recent);
            if discard_end > 0 {
                ltm.flush_before_compact(&current[..discard_end], self.model.as_ref()).await;
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

/// Attachment metadata for files sent via bot adapters.
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub url: String,
    pub mime_type: Option<String>,
}

impl AgentSession {
    /// Handle a message with file/image attachments.
    ///
    /// Downloads attachments to a temporary directory. Image and audio
    /// attachments are passed as multimodal `ContentBlock`s so the model can
    /// actually see/hear them. Other file types are appended as text paths.
    pub async fn handle_message_with_attachments(
        &self,
        session_key: &str,
        message: &str,
        attachments: &[Attachment],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if attachments.is_empty() {
            return self.handle_message(session_key, message).await;
        }

        // Download attachments to temp directory
        let tmp_dir = std::env::temp_dir().join(format!("synapse_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir)
            .map_err(|e| AgentError(format!("failed to create temp dir: {}", e)))?;

        let client = reqwest::Client::new();
        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut extra_text = String::new();

        for att in attachments {
            match client.get(&att.url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let file_path = tmp_dir.join(&att.filename);
                    match resp.bytes().await {
                        Ok(bytes) => {
                            if let Err(e) = std::fs::write(&file_path, &bytes) {
                                eprintln!("warning: failed to write attachment {}: {}", att.filename, e);
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
                                    // Non-media or unknown MIME: fall back to text reference
                                    extra_text.push_str(&format!(
                                        "\n[Attached file: {}]",
                                        file_path.display()
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("warning: failed to download attachment {}: {}", att.filename, e);
                        }
                    }
                }
                Ok(resp) => {
                    eprintln!("warning: attachment download failed with HTTP {}", resp.status());
                }
                Err(e) => {
                    eprintln!("warning: failed to fetch attachment {}: {}", att.filename, e);
                }
            }
        }

        let full_message = if extra_text.is_empty() {
            message.to_string()
        } else {
            format!("{}{}", message, extra_text)
        };

        self.handle_message_inner(session_key, &full_message, content_blocks)
            .await
    }
}

impl AgentSession {
    /// Handle a message with streaming output callbacks.
    ///
    /// Uses the deep agent in streaming mode, calling the output callbacks
    /// as tokens are generated. Falls back to non-streaming if streaming
    /// is not available (simple chat mode).
    pub async fn handle_message_streaming(
        &self,
        session_key: &str,
        text: &str,
        content_blocks: Vec<ContentBlock>,
        output: Arc<dyn StreamingOutput>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = logging::generate_request_id();
        let start = Instant::now();
        let span = tracing::info_span!("channel_message_streaming",
            request_id = %request_id,
            session_key = %session_key,
        );
        let _guard = span.enter();

        tracing::info!("processing streaming channel message");

        let sid = self.resolve_session(session_key).await?;

        let result = if self.deep_agent {
            self.handle_deep_agent_streaming(&sid, text, &content_blocks, output.clone()).await
        } else {
            // Simple chat doesn't support streaming, fall back and emit via callbacks
            let res = self.handle_simple_chat(&sid, text, &content_blocks).await;
            if let Ok(ref response) = res {
                output.on_token(response).await;
            }
            res
        };

        let duration_ms = start.elapsed().as_millis();
        match &result {
            Ok(response) => {
                output.on_complete(response).await;
                tracing::info!(duration_ms = duration_ms as u64, "streaming message processed");
            }
            Err(e) => {
                output.on_error(&e.to_string()).await;
                tracing::error!(duration_ms = duration_ms as u64, error = %e, "streaming message failed");
            }
        }

        result
    }

    /// Deep Agent mode with streaming: full tool calling loop with incremental output.
    async fn handle_deep_agent_streaming(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
        output: Arc<dyn StreamingOutput>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let memory = self.session_mgr.memory();

        // Load existing messages
        let mut messages = memory.load(session_id).await.unwrap_or_default();

        // Add system prompt if this is a new conversation
        if messages.is_empty() {
            let system_prompt = self
                .config
                .base
                .agent
                .system_prompt
                .clone()
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
        let saved_count = memory
            .load(session_id)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        for msg in final_state.messages.iter().skip(saved_count) {
            memory.append(session_id, msg.clone()).await.ok();
        }

        // Token-aware trimming with pre-compaction LTM flush
        let mut current = memory.load(session_id).await.unwrap_or_default();
        let token_count = HeuristicTokenCounter.count_messages(&current);
        let threshold = self.config.memory.auto_compact_threshold;
        if token_count > threshold {
            // Pre-compaction flush: extract important memories before trimming
            let sessions_dir = PathBuf::from(&self.config.base.paths.sessions_dir);
            let ltm = LongTermMemory::new(
                sessions_dir.join("long_term_memory"),
                self.config.memory.clone(),
            );
            ltm.load().await.ok();

            let keep_recent = self.config.memory.keep_recent;
            let discard_end = current.len().saturating_sub(keep_recent);
            if discard_end > 0 {
                ltm.flush_before_compact(&current[..discard_end], self.model.as_ref()).await;
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
