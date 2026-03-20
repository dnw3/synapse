use super::*;

impl AgentSession {
    /// Deep Agent mode: full tool calling loop.
    pub(super) async fn handle_deep_agent(
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
            self.event_bus.clone(),
            self.plugin_registry.clone(),
            None, // no channel registry in bot mode
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

    /// Deep Agent mode with streaming: full tool calling loop with incremental output.
    pub(super) async fn handle_deep_agent_streaming(
        &self,
        session_id: &str,
        text: &str,
        content_blocks: &[ContentBlock],
        output: Arc<dyn StreamingOutput>,
        agent_info: &ResolvedAgentInfo,
    ) -> Result<(String, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
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
            self.event_bus.clone(),
            self.plugin_registry.clone(),
            None, // no channel registry in bot mode
        )
        .await
        .map_err(|e| AgentError(format!("failed to build agent: {}", e)))?;

        // Stream agent execution — show thinking indicator
        output.on_token("💭 *Thinking...*\n").await;

        let initial_state = MessageState::with_messages(messages);
        let mut stream = agent.stream(initial_state, StreamMode::Values);

        let mut last_content_len = 0;
        let mut final_state = None;
        let mut seen_tool_calls: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut total_input_tokens: u32 = 0;
        let mut total_output_tokens: u32 = 0;
        let mut counted_ai_messages: usize = 0; // track how many AI messages we've counted usage for

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    match event.node.as_str() {
                        "agent" => {
                            // Agent node: model call completed
                            // Accumulate token usage from ALL new AI messages (not just the last)
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

                            // Check for tool calls first (intermediate turns)
                            if let Some(last_ai) =
                                event.state.messages.iter().rev().find(|m| m.is_ai())
                            {
                                for tc in last_ai.tool_calls() {
                                    if seen_tool_calls.insert(tc.id.clone()) {
                                        output
                                            .on_tool_call(&super::ToolCallInfo {
                                                name: tc.name.clone(),
                                                id: tc.id.clone(),
                                                args: tc.arguments.to_string(),
                                            })
                                            .await;
                                    }
                                }
                            }

                            // Check for new text content (final turn)
                            let current_content = extract_final_response(&event.state.messages);
                            if current_content.len() > last_content_len {
                                let new_text = &current_content[last_content_len..];
                                output.on_token(new_text).await;
                                last_content_len = current_content.len();
                            }
                        }
                        "tools" => {
                            // Tools node: extract subagent token usage from tool results
                            for msg in event.state.messages.iter().rev() {
                                if !msg.is_tool() {
                                    break;
                                }
                                // TaskTool results contain {"stats": {"input_tokens": N, ...}}
                                let content = msg.content();
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
                                    if let Some(stats) = v.get("stats") {
                                        total_input_tokens +=
                                            stats["input_tokens"].as_u64().unwrap_or(0) as u32;
                                        total_output_tokens +=
                                            stats["output_tokens"].as_u64().unwrap_or(0) as u32;
                                    }
                                }
                            }
                        }
                        _ => {}
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
