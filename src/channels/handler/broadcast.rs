use super::*;
use crate::gateway::messages::InboundMessage;
use synaptic::core::RunContext;

impl AgentSession {
    /// Handle broadcast: fan out to multiple agents in parallel.
    ///
    /// Each agent processes the message independently with its own session/prompt/memory.
    /// Replies are collected and merged into a single response.
    pub(super) async fn handle_broadcast_message(
        &self,
        msg: &InboundMessage,
        agents: &[ResolvedAgentInfo],
        strategy: &crate::config::BroadcastStrategy,
    ) -> Result<AgentReply, Box<dyn std::error::Error + Send + Sync>> {
        use crate::config::BroadcastStrategy;

        let request_id = msg.request_id.clone();
        let session_key = msg.session_key.clone();
        let content_blocks = self.download_attachments(&msg.attachments).await;

        // Build a DeliveryContext from the inbound message for the reply
        let delivery_target = Self::delivery_context_from_inbound(msg);

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
                    let text = msg.content.clone();
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
                                None, // no event bus in broadcast mode
                                None, // no plugin registry in broadcast mode
                                None, // no channel registry in broadcast mode
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
                    payloads: vec![OutboundPayload {
                        text: Some(merged.clone()),
                        ..Default::default()
                    }],
                    content: merged,
                    delivery_target,
                    turn_id: request_id,
                })
            }
            BroadcastStrategy::Sequential => {
                // Process agents one by one, return last response
                let mut last_response = String::new();
                for agent_info in agents {
                    let sid = self.resolve_session(&session_key, msg).await?;
                    match self
                        .handle_deep_agent(
                            &sid,
                            &msg.content,
                            &content_blocks,
                            RunContext::default(),
                            agent_info,
                        )
                        .await
                    {
                        Ok((response, _, _)) => {
                            last_response = response;
                        }
                        Err(e) => {
                            tracing::warn!(agent = %agent_info.id, error = %e, "sequential broadcast agent failed");
                        }
                    }
                }
                Ok(AgentReply {
                    payloads: vec![OutboundPayload {
                        text: Some(last_response.clone()),
                        ..Default::default()
                    }],
                    content: last_response,
                    delivery_target,
                    turn_id: request_id,
                })
            }
        }
    }
}
