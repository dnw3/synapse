//! Agent tools for querying long-term memory.
//!
//! `memory_search` — semantic search over LTM.
//! `memory_get` — list or count stored memories.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};

use crate::memory::LongTermMemory;

/// Tool that allows the agent to search long-term memory.
pub struct MemorySearchTool {
    ltm: Arc<LongTermMemory>,
}

#[allow(clippy::new_ret_no_self)]
impl MemorySearchTool {
    pub fn new(ltm: Arc<LongTermMemory>) -> Arc<dyn Tool> {
        Arc::new(Self { ltm })
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &'static str {
        "memory_search"
    }

    fn description(&self) -> &'static str {
        "Search long-term memory for relevant past information. Use this when you need to recall facts, decisions, preferences, or context from previous conversations."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant memories"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of memories to return (default: 5)",
                    "default": 5
                }
            },
            "required": ["query"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing required parameter 'query'".into()))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        tracing::debug!("memory search");

        let results = self.ltm.recall_with_sources(query, limit).await;

        if results.is_empty() {
            Ok(json!("No relevant memories found."))
        } else {
            let formatted: Vec<String> = results
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let tag = if r.evergreen { " [evergreen]" } else { "" };
                    format!("{}. [{}]{} {}", i + 1, r.source_key, tag, r.content)
                })
                .collect();
            Ok(json!(formatted.join("\n")))
        }
    }
}

/// Tool that allows the agent to list or count memories.
pub struct MemoryGetTool {
    ltm: Arc<LongTermMemory>,
}

#[allow(clippy::new_ret_no_self)]
impl MemoryGetTool {
    pub fn new(ltm: Arc<LongTermMemory>) -> Arc<dyn Tool> {
        Arc::new(Self { ltm })
    }
}

#[async_trait]
impl Tool for MemoryGetTool {
    fn name(&self) -> &'static str {
        "memory_get"
    }

    fn description(&self) -> &'static str {
        "Get information about stored long-term memories. Use 'list' to see all memories or 'count' to get the total number."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "count"],
                    "description": "Action to perform: 'list' returns all memories, 'count' returns the total count"
                }
            },
            "required": ["action"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing required parameter 'action'".into()))?;

        match action {
            "count" => {
                let count = self.ltm.count().await;
                Ok(json!(format!("{} memories stored.", count)))
            }
            "list" => {
                let memories = self.ltm.list().await;
                if memories.is_empty() {
                    Ok(json!("No memories stored."))
                } else {
                    let formatted: Vec<String> = memories
                        .iter()
                        .enumerate()
                        .map(|(i, (key, content))| {
                            let preview = if content.len() > 200 {
                                format!("{}...", &content[..197])
                            } else {
                                content.clone()
                            };
                            format!("{}. [{}] {}", i + 1, key, preview)
                        })
                        .collect();
                    Ok(json!(formatted.join("\n")))
                }
            }
            _ => Err(SynapticError::Tool(format!(
                "unknown action '{}', expected 'list' or 'count'",
                action
            ))),
        }
    }
}
