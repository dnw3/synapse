use std::sync::Arc;

use serde_json::{json, Value};
use synaptic_core::{ChatResponse, Message, SynapticError, ToolCall};
use synaptic_graph::{create_swarm, MessageState, SwarmAgent, SwarmOptions};
use synaptic_macros::tool;
use synaptic_models::ScriptedChatModel;

/// echoes input
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(args)
}

fn make_swarm_agent(name: &str, responses: Vec<ChatResponse>) -> SwarmAgent {
    SwarmAgent {
        name: name.to_string(),
        model: Arc::new(ScriptedChatModel::new(responses)),
        tools: vec![echo()],
        system_prompt: Some(format!("You are the {name} agent.")),
    }
}

#[test]
fn compiles_with_two_agents() {
    let agents = vec![
        make_swarm_agent(
            "triage",
            vec![ChatResponse {
                message: Message::ai("ok"),
                usage: None,
            }],
        ),
        make_swarm_agent(
            "support",
            vec![ChatResponse {
                message: Message::ai("helped"),
                usage: None,
            }],
        ),
    ];
    let result = create_swarm(agents, SwarmOptions::default());
    assert!(result.is_ok());
}

#[test]
fn entry_is_first_agent() {
    // The first agent in the list should be the entry point
    let agents = vec![
        make_swarm_agent(
            "first_agent",
            vec![ChatResponse {
                message: Message::ai("I'm first"),
                usage: None,
            }],
        ),
        make_swarm_agent(
            "second_agent",
            vec![ChatResponse {
                message: Message::ai("I'm second"),
                usage: None,
            }],
        ),
    ];
    let graph = create_swarm(agents, SwarmOptions::default()).unwrap();
    // Graph compiles successfully with first agent as entry
    assert!(!graph.is_deferred("first_agent"));
}

#[tokio::test]
async fn terminates_no_tool_calls() {
    // Agent responds without tool calls => should terminate
    let agents = vec![make_swarm_agent(
        "agent_a",
        vec![ChatResponse {
            message: Message::ai("Direct answer."),
            usage: None,
        }],
    )];

    let graph = create_swarm(agents, SwarmOptions::default()).unwrap();
    let state = MessageState::with_messages(vec![Message::human("hello")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    assert!(result.messages.len() >= 2);
    let last = result.messages.last().unwrap();
    assert!(last.is_ai());
    assert_eq!(last.content(), "Direct answer.");
}

#[tokio::test]
async fn handoff_routes_to_target() {
    // First agent calls handoff to second agent, second agent responds directly
    let agents = vec![
        make_swarm_agent(
            "triage",
            vec![ChatResponse {
                message: Message::ai_with_tool_calls(
                    "",
                    vec![ToolCall {
                        id: "h1".to_string(),
                        name: "transfer_to_support".to_string(),
                        arguments: json!({}),
                    }],
                ),
                usage: None,
            }],
        ),
        make_swarm_agent(
            "support",
            vec![ChatResponse {
                message: Message::ai("I'll help you with your issue."),
                usage: None,
            }],
        ),
    ];

    let graph = create_swarm(agents, SwarmOptions::default()).unwrap();
    let state = MessageState::with_messages(vec![Message::human("I need help")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    // Should end with support agent's response
    let last = result.messages.last().unwrap();
    assert!(last.is_ai());
    assert_eq!(last.content(), "I'll help you with your issue.");
}

#[tokio::test]
async fn non_handoff_tool_executes() {
    // Agent calls a regular tool (echo), not a handoff
    let agents = vec![make_swarm_agent(
        "worker",
        vec![
            ChatResponse {
                message: Message::ai_with_tool_calls(
                    "",
                    vec![ToolCall {
                        id: "t1".to_string(),
                        name: "echo".to_string(),
                        arguments: json!({"data": "test"}),
                    }],
                ),
                usage: None,
            },
            ChatResponse {
                message: Message::ai("Echo returned test"),
                usage: None,
            },
        ],
    )];

    let graph = create_swarm(agents, SwarmOptions::default()).unwrap();
    let state = MessageState::with_messages(vec![Message::human("echo something")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    // Should have: human, AI (tool call), tool result, AI (final)
    assert!(result.messages.len() >= 3);
    let last = result.messages.last().unwrap();
    assert!(last.is_ai());
}
