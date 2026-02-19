use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use synaptic_core::{ChatModel, Message, SynapticError, Tool};
use synaptic_graph::MessageState;
use synaptic_middleware::AgentMiddleware;

use crate::backend::Backend;

/// Definition of a custom sub-agent type available to the task tool.
#[derive(Clone)]
pub struct SubAgentDef {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub tools: Vec<Arc<dyn Tool>>,
}

/// Middleware that provides a `task` tool for spawning child agents.
///
/// The `task` tool creates a child deep agent and invokes it with the given description.
/// Recursion is bounded by `max_depth`.
pub struct SubAgentMiddleware {
    backend: Arc<dyn Backend>,
    model: Arc<dyn ChatModel>,
    max_depth: usize,
    current_depth: Arc<AtomicUsize>,
    custom_agents: Vec<SubAgentDef>,
}

impl SubAgentMiddleware {
    pub fn new(
        backend: Arc<dyn Backend>,
        model: Arc<dyn ChatModel>,
        max_depth: usize,
        custom_agents: Vec<SubAgentDef>,
    ) -> Self {
        Self {
            backend,
            model,
            max_depth,
            current_depth: Arc::new(AtomicUsize::new(0)),
            custom_agents,
        }
    }

    /// Create the `task` tool that spawns sub-agents.
    pub fn create_task_tool(&self) -> Arc<dyn Tool> {
        Arc::new(TaskTool {
            backend: self.backend.clone(),
            model: self.model.clone(),
            max_depth: self.max_depth,
            current_depth: self.current_depth.clone(),
            custom_agents: self.custom_agents.clone(),
        })
    }
}

#[async_trait]
impl AgentMiddleware for SubAgentMiddleware {}

// ---------------------------------------------------------------------------

struct TaskTool {
    backend: Arc<dyn Backend>,
    model: Arc<dyn ChatModel>,
    max_depth: usize,
    current_depth: Arc<AtomicUsize>,
    custom_agents: Vec<SubAgentDef>,
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &'static str {
        "task"
    }

    fn description(&self) -> &'static str {
        "Spawn a sub-agent to handle a complex, multi-step task autonomously"
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A detailed description of the task for the sub-agent"
                },
                "agent_type": {
                    "type": "string",
                    "description": "Type of agent to spawn (default: general-purpose)"
                }
            },
            "required": ["description"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let depth = self.current_depth.load(Ordering::Relaxed);
        if depth >= self.max_depth {
            return Err(SynapticError::Tool(format!(
                "max subagent depth ({}) exceeded",
                self.max_depth
            )));
        }

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'description' parameter".into()))?;

        let agent_type = args
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general-purpose");

        self.current_depth.fetch_add(1, Ordering::Relaxed);
        let result = self.run_subagent(description, agent_type).await;
        self.current_depth.fetch_sub(1, Ordering::Relaxed);

        result
    }
}

impl TaskTool {
    async fn run_subagent(
        &self,
        description: &str,
        agent_type: &str,
    ) -> Result<Value, SynapticError> {
        let custom = self.custom_agents.iter().find(|a| a.name == agent_type);

        let mut options = crate::DeepAgentOptions::new(self.backend.clone());
        options.enable_subagents =
            self.current_depth.load(Ordering::Relaxed) < self.max_depth;
        options.max_subagent_depth = self.max_depth;

        if let Some(def) = custom {
            options.system_prompt = Some(def.system_prompt.clone());
            options.tools = def.tools.clone();
        }

        let agent = crate::create_deep_agent(self.model.clone(), options)?;

        let state = MessageState::with_messages(vec![Message::human(description)]);
        let result = agent.invoke(state).await?;
        let final_state = result.into_state();

        let response = final_state
            .last_message()
            .map(|m| m.content().to_string())
            .unwrap_or_else(|| "Sub-agent completed with no response".to_string());

        Ok(Value::String(response))
    }
}
