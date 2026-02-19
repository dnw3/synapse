use std::sync::Arc;
use synaptic_core::{
    ChatModel, ChatRequest, ChatResponse, Message, SynapticError, ToolCall,
};
use synaptic_deep::backend::{Backend, StateBackend};
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_graph::MessageState;

/// A scripted model that first calls write_file, then gives a final answer.
struct ScriptedDeepModel {
    call_count: std::sync::atomic::AtomicUsize,
}

impl ScriptedDeepModel {
    fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for ScriptedDeepModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        let n = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match n {
            0 => {
                // First call: request to write a file
                Ok(ChatResponse {
                    message: Message::ai_with_tool_calls(
                        "I'll write a file.",
                        vec![ToolCall {
                            id: "tc_1".to_string(),
                            name: "write_file".to_string(),
                            arguments: serde_json::json!({
                                "path": "hello.txt",
                                "content": "Hello from deep agent!"
                            }),
                        }],
                    ),
                    usage: None,
                })
            }
            _ => {
                // Second call: final answer
                Ok(ChatResponse {
                    message: Message::ai("Done! I wrote hello.txt."),
                    usage: None,
                })
            }
        }
    }
}

#[tokio::test]
async fn full_deep_agent_e2e() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(ScriptedDeepModel::new());

    let mut options = DeepAgentOptions::new(backend.clone());
    options.enable_subagents = false; // Don't need subagents for this test
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("Write hello.txt")]);
    let result = agent.invoke(state).await.unwrap();
    let final_state = result.into_state();

    // Agent should have written the file
    let content = backend.read_file("hello.txt", 0, 100).await.unwrap();
    assert_eq!(content, "Hello from deep agent!");

    // Final message should be the agent's response
    let last = final_state.last_message().unwrap();
    assert!(last.content().contains("hello.txt"));
}

#[tokio::test]
async fn deep_agent_with_custom_system_prompt() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(ScriptedDeepModel::new());

    let mut options = DeepAgentOptions::new(backend.clone());
    options.system_prompt = Some("You are a coding assistant.".to_string());
    options.enable_subagents = false;
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("Write a file")]);
    let result = agent.invoke(state).await.unwrap();
    result.into_state(); // Just verify it completes
}

#[tokio::test]
async fn deep_agent_all_features_disabled() {
    // Final-answer-only model
    struct SimpleModel;

    #[async_trait::async_trait]
    impl ChatModel for SimpleModel {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
            Ok(ChatResponse {
                message: Message::ai("Hello!"),
                usage: None,
            })
        }
    }

    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(SimpleModel);

    let mut options = DeepAgentOptions::new(backend);
    options.enable_subagents = false;
    options.enable_filesystem = false;
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("Hi")]);
    let result = agent.invoke(state).await.unwrap();
    let final_state = result.into_state();
    assert_eq!(final_state.last_message().unwrap().content(), "Hello!");
}

#[tokio::test]
async fn deep_agent_with_memory() {
    struct SimpleModel;

    #[async_trait::async_trait]
    impl ChatModel for SimpleModel {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
            Ok(ChatResponse {
                message: Message::ai("I read the memory."),
                usage: None,
            })
        }
    }

    let backend = Arc::new(StateBackend::new());
    backend
        .write_file("AGENTS.md", "# Memory\nAlways be helpful.")
        .await
        .unwrap();

    let model: Arc<dyn ChatModel> = Arc::new(SimpleModel);
    let mut options = DeepAgentOptions::new(backend);
    options.enable_subagents = false;
    options.enable_filesystem = false;
    options.enable_skills = false;
    // enable_memory = true (default)

    let agent = create_deep_agent(model, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("What do you remember?")]);
    let result = agent.invoke(state).await.unwrap();
    result.into_state();
}

#[tokio::test]
async fn deep_agent_default_options() {
    // Verify DeepAgentOptions::new sets sensible defaults
    let backend = Arc::new(StateBackend::new());
    let options = DeepAgentOptions::new(backend);

    assert_eq!(options.max_input_tokens, 128_000);
    assert!((options.summarization_threshold - 0.85).abs() < 0.01);
    assert_eq!(options.eviction_threshold, 20_000);
    assert_eq!(options.max_subagent_depth, 3);
    assert!(options.enable_subagents);
    assert!(options.enable_filesystem);
    assert!(options.enable_skills);
    assert!(options.enable_memory);
}
