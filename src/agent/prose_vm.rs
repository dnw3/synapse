use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
pub struct ProseVm;

#[allow(dead_code)]
impl ProseVm {
    pub fn new() -> Self {
        Self
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
                ProseInstruction::CallTool { name, args: _ } => {
                    // TODO: wire to actual tool execution via PluginRegistry
                    tracing::info!(tool = %name, "ProseVM: would call tool");
                    state
                        .output
                        .push(format!("[tool:{}] placeholder result", name));
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
        let vm = ProseVm::new();
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
        let vm = ProseVm::new();
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
        let vm = ProseVm::new();
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
        let vm = ProseVm::new();
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
        assert_eq!(
            state.output,
            vec!["item: a", "item: b", "item: c"]
        );
    }
}
