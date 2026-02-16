use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use synapse_core::{
    Agent, CallbackHandler, ChatModel, ChatRequest, MemoryStore, Message, RunEvent, SynapseError,
};
use synapse_tools::SerialToolExecutor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub system_prompt: String,
    pub max_steps: usize,
}

pub struct ReActAgentExecutor {
    model: Arc<dyn ChatModel>,
    tools: Arc<SerialToolExecutor>,
    memory: Arc<dyn MemoryStore>,
    callbacks: Arc<dyn CallbackHandler>,
    config: AgentConfig,
}

impl ReActAgentExecutor {
    pub fn new(
        model: Arc<dyn ChatModel>,
        tools: Arc<SerialToolExecutor>,
        memory: Arc<dyn MemoryStore>,
        callbacks: Arc<dyn CallbackHandler>,
        config: AgentConfig,
    ) -> Self {
        Self {
            model,
            tools,
            memory,
            callbacks,
            config,
        }
    }

    fn next_run_id() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("run-{now}-{seq}")
    }
}

#[async_trait]
impl Agent for ReActAgentExecutor {
    async fn run(&self, session_id: &str, input: &str) -> Result<String, SynapseError> {
        let run_id = Self::next_run_id();
        self.callbacks
            .on_event(RunEvent::RunStarted {
                run_id: run_id.clone(),
                session_id: session_id.to_string(),
            })
            .await?;

        self.memory
            .append(session_id, Message::human(input))
            .await?;

        for step in 0..self.config.max_steps {
            self.callbacks
                .on_event(RunEvent::RunStep {
                    run_id: run_id.clone(),
                    step,
                })
                .await?;

            let mut messages = Vec::new();
            messages.push(Message::system(self.config.system_prompt.clone()));
            messages.extend(self.memory.load(session_id).await?);

            self.callbacks
                .on_event(RunEvent::LlmCalled {
                    run_id: run_id.clone(),
                    message_count: messages.len(),
                })
                .await?;

            let response = self.model.chat(ChatRequest { messages }).await?;
            self.memory
                .append(session_id, response.message.clone())
                .await?;

            if response.message.tool_calls().is_empty() {
                let output = response.message.content().to_string();
                self.callbacks
                    .on_event(RunEvent::RunFinished {
                        run_id: run_id.clone(),
                        output: output.clone(),
                    })
                    .await?;
                return Ok(output);
            }

            for call in response.message.tool_calls() {
                self.callbacks
                    .on_event(RunEvent::ToolCalled {
                        run_id: run_id.clone(),
                        tool_name: call.name.clone(),
                    })
                    .await?;

                let result = self
                    .tools
                    .execute(&call.name, call.arguments.clone())
                    .await?;
                self.memory
                    .append(session_id, Message::tool(result.to_string(), &call.id))
                    .await?;
            }
        }

        let err = SynapseError::MaxStepsExceeded {
            max_steps: self.config.max_steps,
        };
        self.callbacks
            .on_event(RunEvent::RunFailed {
                run_id,
                error: err.to_string(),
            })
            .await?;
        Err(err)
    }
}
