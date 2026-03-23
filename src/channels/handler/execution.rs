use std::time::Duration;

use synaptic::core::RunContext;
use synaptic::deep::StreamingOutputHandle;

use super::*;

impl AgentSession {
    /// Deep Agent mode: full tool calling loop with streaming via RunContext.
    ///
    /// Uses `stream_with_context` to propagate the RunContext (which carries
    /// the StreamingOutputHandle and cancel token) through the graph execution,
    /// enabling the framework's `StreamingInterceptor` to forward tokens
    /// automatically.
    ///
    /// When no `StreamingOutputHandle` is present in the RunContext, streaming
    /// callbacks are no-ops (guarded by `if let Some(ref handle) = output_handle`).
    pub(super) async fn handle_deep_agent(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
        ctx: RunContext,
        agent_info: &ResolvedAgentInfo,
        request_id: Option<&str>,
    ) -> Result<(String, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
        let memory = self.session_mgr.memory();

        // Load existing messages
        let mut messages = memory.load(session_id).await.unwrap_or_default();

        // Add system prompt if this is a new conversation
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
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let human_msg = if content_blocks.is_empty() {
            Message::human(text)
        } else {
            Message::human(text).with_content_blocks(content_blocks.to_vec())
        }
        .with_additional_kwarg("timestamp", serde_json::json!(now_ms));
        let human_msg = if let Some(rid) = request_id {
            human_msg
                .with_additional_kwarg("request_id", serde_json::Value::String(rid.to_string()))
        } else {
            human_msg
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
            self.event_bus.clone(),
            self.plugin_registry.clone(),
            None, // no channel registry in bot mode
            crate::agent::SessionKind::Full,
            &[], // TODO: pass bundle_skills_dirs from gateway
        )
        .await
        .map_err(|e| AgentError(format!("failed to build agent: {}", e)))?;

        // Stream agent execution using RunContext
        let initial_state = MessageState::with_messages(messages);
        let mut stream = agent.stream_with_context(initial_state, StreamMode::Values, ctx.clone());

        let mut last_content_len = 0;
        let mut final_state = None;
        let mut seen_tool_calls: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut total_input_tokens: u32 = 0;
        let mut total_output_tokens: u32 = 0;
        let mut counted_ai_messages: usize = 0;

        // Heartbeat timer — fires every 15 seconds
        let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
        heartbeat.tick().await; // skip first immediate tick

        // Extract StreamingOutput from RunContext for tool_call/token notifications
        // that are based on graph state diffs (kept alongside interceptor-based streaming).
        let output_handle = ctx.streaming_output::<StreamingOutputHandle>();

        loop {
            tokio::select! {
                event_result = stream.next() => {
                    match event_result {
                        Some(Ok(event)) => {
                            match event.node.as_str() {
                                "agent" => {
                                    let ai_messages: Vec<_> =
                                        event.state.messages.iter().filter(|m| m.is_ai()).collect();
                                    for ai_msg in ai_messages.iter().skip(counted_ai_messages) {
                                        if let Some(usage) = ai_msg.response_metadata().get("usage") {
                                            total_input_tokens +=
                                                usage["input_tokens"].as_u64().unwrap_or(0) as u32;
                                            total_output_tokens +=
                                                usage["output_tokens"].as_u64().unwrap_or(0) as u32;
                                        }
                                    }
                                    counted_ai_messages = ai_messages.len();

                                    if let Some(ref handle) = output_handle {
                                        if let Some(last_ai) =
                                            event.state.messages.iter().rev().find(|m| m.is_ai())
                                        {
                                            for tc in last_ai.tool_calls() {
                                                if seen_tool_calls.insert(tc.id.clone()) {
                                                    let display = self.display_resolver.resolve(
                                                        &tc.name,
                                                        &tc.arguments,
                                                    );
                                                    handle.0
                                                        .on_tool_call(&super::ToolCallInfo {
                                                            name: tc.name.clone(),
                                                            id: tc.id.clone(),
                                                            args: tc.arguments.to_string(),
                                                            display: Some(display),
                                                        })
                                                        .await;
                                                }
                                            }
                                        }

                                        let current_content = extract_final_response(&event.state.messages);
                                        if current_content.len() > last_content_len {
                                            // Token streaming is handled by StreamingInterceptor,
                                            // but update last_content_len to track progress.
                                            last_content_len = current_content.len();
                                        }
                                    }
                                }
                                "tools" => {
                                    for msg in event.state.messages.iter().rev() {
                                        if !msg.is_tool() {
                                            break;
                                        }
                                        let content = msg.content();
                                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
                                            if let Some(stats) = v.get("stats") {
                                                total_input_tokens +=
                                                    stats["input_tokens"].as_u64().unwrap_or(0) as u32;
                                                total_output_tokens +=
                                                    stats["output_tokens"].as_u64().unwrap_or(0) as u32;
                                            }
                                        }
                                        // Emit tool results via StreamingOutput
                                        if let Some(ref handle) = output_handle {
                                            let tool_name = "tool"; // generic; tool name extraction from graph state is best-effort
                                            handle.0.on_tool_result(tool_name, &msg.content()[..std::cmp::min(msg.content().len(), 500)]).await;
                                        }
                                    }
                                }
                                _ => {}
                            }

                            final_state = Some(event.state);
                        }
                        Some(Err(e)) => {
                            return Err(Box::new(AgentError(format!("agent stream error: {}", e))));
                        }
                        None => {
                            break;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if let Some(ref handle) = output_handle {
                        handle.0.on_heartbeat().await;
                    }
                }
            }
        }

        let final_state =
            final_state.ok_or_else(|| AgentError("no output from agent stream".into()))?;
        let response = extract_final_response(&final_state.messages);

        // Save new messages to history, injecting request_id + timestamp
        let saved_count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
        let save_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        for msg in final_state.messages.iter().skip(saved_count) {
            let mut m = msg
                .clone()
                .with_additional_kwarg("timestamp", serde_json::json!(save_ts));
            if let Some(rid) = request_id {
                m = m.with_additional_kwarg(
                    "request_id",
                    serde_json::Value::String(rid.to_string()),
                );
            }
            memory.append(session_id, m).await.ok();
        }

        // Token-aware trimming with pre-compaction LTM flush
        let mut current = memory.load(session_id).await.unwrap_or_default();
        let token_count = HeuristicTokenCounter.count_messages(&current);
        let threshold = self.config.memory.auto_compact_threshold;
        if token_count > threshold {
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

            let opts = crate::tools::PruningOptions::from_config(&self.config.memory);
            crate::tools::prune_tool_results_with_options(&mut current, &opts);

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

        Ok((response, total_input_tokens, total_output_tokens))
    }

    /// Simple chat mode: direct model.chat() call without tools.
    pub(super) async fn handle_simple_chat(
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
pub(super) fn extract_final_response(messages: &[Message]) -> String {
    // Walk backwards to find the last AI message with text content
    for msg in messages.iter().rev() {
        if msg.is_ai() {
            let content = msg.content();
            if !content.is_empty() {
                return content.to_string();
            }
        }
    }
    String::new()
}

/// Detect MIME type from a filename extension. Returns `None` for unknown types.
pub(super) fn detect_mime_from_extension(filename: &str) -> Option<&'static str> {
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
