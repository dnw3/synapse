use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Instructions that can appear in a Prose skill body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProseInstruction {
    CallTool {
        name: String,
        args: Value,
    },
    AskUser {
        prompt: String,
    },
    Branch {
        condition: String,
        then_branch: Vec<ProseInstruction>,
        else_branch: Vec<ProseInstruction>,
    },
    Loop {
        over: String,
        body: Vec<ProseInstruction>,
    },
    SetVar {
        name: String,
        value: String,
    },
    Emit {
        event: String,
        payload: Value,
    },
    Log {
        message: String,
    },
}

/// Execution state for a prose program.
#[derive(Debug, Default)]
pub struct ProseState {
    pub variables: HashMap<String, Value>,
    pub output: Vec<String>,
    pub waiting_for_user: bool,
}

/// The VM that interprets prose instructions.
#[allow(dead_code)]
pub struct ProseVm {
    plugin_registry: Option<Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>>,
}

#[allow(dead_code)]
impl ProseVm {
    pub fn new(
        plugin_registry: Option<Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>>,
    ) -> Self {
        Self { plugin_registry }
    }

    /// Execute a list of instructions against the given state.
    pub async fn execute(
        &self,
        instructions: &[ProseInstruction],
        state: &mut ProseState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for instruction in instructions {
            match instruction {
                ProseInstruction::SetVar { name, value } => {
                    let resolved = self.resolve_template(value, &state.variables);
                    state
                        .variables
                        .insert(name.clone(), Value::String(resolved));
                }
                ProseInstruction::Log { message } => {
                    let resolved = self.resolve_template(message, &state.variables);
                    tracing::info!(prose_vm = true, "{}", resolved);
                    state.output.push(resolved);
                }
                ProseInstruction::CallTool { name, args } => {
                    if let Some(ref registry) = self.plugin_registry {
                        let tool = {
                            let reg = registry.read().await;
                            reg.tools()
                                .iter()
                                .find(|t| t.name() == name.as_str())
                                .cloned()
                        };
                        if let Some(tool) = tool {
                            tracing::info!(tool = %name, "ProseVM: calling tool via PluginRegistry");
                            match tool.call(args.clone()).await {
                                Ok(result) => {
                                    let result_str = match &result {
                                        Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                    state
                                        .variables
                                        .insert(format!("_{}_result", name), result.clone());
                                    state.output.push(result_str);
                                }
                                Err(e) => {
                                    tracing::warn!(tool = %name, error = %e, "ProseVM: tool call failed");
                                    state.output.push(format!("[tool:{} error] {}", name, e));
                                }
                            }
                        } else {
                            tracing::warn!(tool = %name, "ProseVM: tool not found in PluginRegistry");
                            state.output.push(format!("[tool:{} not found]", name));
                        }
                    } else {
                        tracing::info!(tool = %name, "ProseVM: no PluginRegistry, skipping tool call");
                        state
                            .output
                            .push(format!("[tool:{}] placeholder result", name));
                    }
                }
                ProseInstruction::AskUser { prompt } => {
                    let resolved = self.resolve_template(prompt, &state.variables);
                    state.output.push(format!("[ask_user] {}", resolved));
                    state.waiting_for_user = true;
                    return Ok(()); // Suspend execution
                }
                ProseInstruction::Branch {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let cond_val = state
                        .variables
                        .get(condition)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if cond_val {
                        Box::pin(self.execute(then_branch, state)).await?;
                    } else {
                        Box::pin(self.execute(else_branch, state)).await?;
                    }
                }
                ProseInstruction::Loop { over, body } => {
                    if let Some(Value::Array(items)) = state.variables.get(over) {
                        let items = items.clone();
                        for (i, item) in items.iter().enumerate() {
                            state.variables.insert("_item".into(), item.clone());
                            state
                                .variables
                                .insert("_index".into(), Value::Number(i.into()));
                            Box::pin(self.execute(body, state)).await?;
                        }
                    }
                }
                ProseInstruction::Emit { event, payload: _ } => {
                    tracing::info!(event = %event, "ProseVM: would emit event");
                    state.output.push(format!("[emit:{}]", event));
                }
            }
        }
        Ok(())
    }

    fn resolve_template(&self, template: &str, vars: &HashMap<String, Value>) -> String {
        let mut result = template.to_string();
        for (key, value) in vars {
            let placeholder = format!("{{{}}}", key);
            let replacement = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_var_and_log() {
        let vm = ProseVm::new(None);
        let mut state = ProseState::default();
        let instructions = vec![
            ProseInstruction::SetVar {
                name: "name".into(),
                value: "World".into(),
            },
            ProseInstruction::Log {
                message: "Hello {name}!".into(),
            },
        ];
        vm.execute(&instructions, &mut state).await.unwrap();
        assert_eq!(state.output, vec!["Hello World!"]);
    }

    #[tokio::test]
    async fn branch_true() {
        let vm = ProseVm::new(None);
        let mut state = ProseState::default();
        state.variables.insert("flag".into(), Value::Bool(true));
        let instructions = vec![ProseInstruction::Branch {
            condition: "flag".into(),
            then_branch: vec![ProseInstruction::Log {
                message: "yes".into(),
            }],
            else_branch: vec![ProseInstruction::Log {
                message: "no".into(),
            }],
        }];
        vm.execute(&instructions, &mut state).await.unwrap();
        assert_eq!(state.output, vec!["yes"]);
    }

    #[tokio::test]
    async fn ask_user_suspends() {
        let vm = ProseVm::new(None);
        let mut state = ProseState::default();
        let instructions = vec![
            ProseInstruction::AskUser {
                prompt: "What?".into(),
            },
            ProseInstruction::Log {
                message: "after".into(),
            }, // should not execute
        ];
        vm.execute(&instructions, &mut state).await.unwrap();
        assert!(state.waiting_for_user);
        assert_eq!(state.output.len(), 1); // only the ask_user output
    }

    #[tokio::test]
    async fn loop_over_array() {
        let vm = ProseVm::new(None);
        let mut state = ProseState::default();
        state
            .variables
            .insert("items".into(), serde_json::json!(["a", "b", "c"]));
        let instructions = vec![ProseInstruction::Loop {
            over: "items".into(),
            body: vec![ProseInstruction::Log {
                message: "item: {_item}".into(),
            }],
        }];
        vm.execute(&instructions, &mut state).await.unwrap();
        assert_eq!(state.output, vec!["item: a", "item: b", "item: c"]);
    }

    #[tokio::test]
    async fn call_tool_via_plugin_registry() {
        use std::sync::Arc;
        use synaptic::plugin::PluginRegistry;

        // A minimal mock tool that echoes its args back as a string.
        struct EchoTool;

        #[async_trait::async_trait]
        impl synaptic::core::Tool for EchoTool {
            fn name(&self) -> &'static str {
                "echo"
            }
            fn description(&self) -> &'static str {
                "Echoes arguments"
            }
            async fn call(&self, args: Value) -> Result<Value, synaptic::core::SynapticError> {
                Ok(Value::String(args.to_string()))
            }
        }

        let event_bus = Arc::new(synaptic::events::EventBus::new());
        let mut registry = PluginRegistry::new(event_bus);
        registry.register_tool(Arc::new(EchoTool));
        let registry = Arc::new(tokio::sync::RwLock::new(registry));

        let vm = ProseVm::new(Some(registry));
        let mut state = ProseState::default();
        let instructions = vec![ProseInstruction::CallTool {
            name: "echo".into(),
            args: serde_json::json!({"msg": "hello"}),
        }];
        vm.execute(&instructions, &mut state).await.unwrap();

        // Output should contain the echoed args string, not the placeholder.
        assert_eq!(state.output.len(), 1);
        assert!(
            state.output[0].contains("hello"),
            "expected echoed output, got: {}",
            state.output[0]
        );
        // The result should have been stored in a variable.
        assert!(state.variables.contains_key("_echo_result"));
    }
}
