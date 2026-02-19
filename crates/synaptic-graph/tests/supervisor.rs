use std::sync::Arc;

use serde_json::{json, Value};
use synaptic_core::{ChatResponse, Message, SynapticError, Tool, ToolCall};
use synaptic_graph::{
    create_react_agent, create_supervisor, MessageState, SupervisorOptions,
};
use synaptic_macros::tool;
use synaptic_models::ScriptedChatModel;

/// echoes input
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(args)
}


fn make_sub_agent(
    name: &str,
    response: &str,
) -> (String, synaptic_graph::CompiledGraph<MessageState>) {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(response),
        usage: None,
    }]));
    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let graph = create_react_agent(model, tools).unwrap();
    (name.to_string(), graph)
}

#[test]
fn compiles_with_two_agents() {
    let supervisor_model = Arc::new(ScriptedChatModel::new(vec![]));
    let agents = vec![
        make_sub_agent("researcher", "research done"),
        make_sub_agent("writer", "writing done"),
    ];
    let result = create_supervisor(supervisor_model, agents, SupervisorOptions::default());
    assert!(result.is_ok());
}

#[tokio::test]
async fn routes_to_correct_agent() {
    // Supervisor calls handoff to "researcher", then researcher responds, then supervisor ends
    let supervisor_model = Arc::new(ScriptedChatModel::new(vec![
        // Supervisor's first response: call handoff to researcher
        ChatResponse {
            message: Message::ai_with_tool_calls(
                "",
                vec![ToolCall {
                    id: "hc-1".to_string(),
                    name: "transfer_to_researcher".to_string(),
                    arguments: json!({}),
                }],
            ),
            usage: None,
        },
        // Supervisor's response after researcher returns
        ChatResponse {
            message: Message::ai("Here is the research result."),
            usage: None,
        },
    ]));

    let agents = vec![
        make_sub_agent("researcher", "I found the answer"),
        make_sub_agent("writer", "I wrote the doc"),
    ];

    let graph = create_supervisor(supervisor_model, agents, SupervisorOptions::default()).unwrap();
    let state = MessageState::with_messages(vec![Message::human("research AI")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    // Should have messages from the full flow
    assert!(result.messages.len() >= 2);
    // Final message should be from the supervisor
    let last = result.messages.last().unwrap();
    assert!(last.is_ai());
}

#[tokio::test]
async fn terminates_when_no_tool_call() {
    // Supervisor responds without tool calls => terminates
    let supervisor_model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("I can answer directly."),
        usage: None,
    }]));

    let agents = vec![make_sub_agent("agent1", "response")];
    let graph = create_supervisor(supervisor_model, agents, SupervisorOptions::default()).unwrap();

    let state = MessageState::with_messages(vec![Message::human("simple question")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    assert!(result.messages.len() >= 2);
    assert_eq!(
        result.messages.last().unwrap().content(),
        "I can answer directly."
    );
}

#[test]
fn custom_system_prompt() {
    let supervisor_model = Arc::new(ScriptedChatModel::new(vec![]));
    let agents = vec![make_sub_agent("agent1", "ok")];
    let options = SupervisorOptions {
        system_prompt: Some("You are a supervisor managing agents.".to_string()),
        ..Default::default()
    };
    let result = create_supervisor(supervisor_model, agents, options);
    assert!(result.is_ok());
}
