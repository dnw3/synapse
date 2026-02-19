use std::sync::Arc;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic_deep::backend::StateBackend;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_graph::MessageState;

/// A model that always returns a plain AI text response (no tool calls).
struct FinalAnswerModel;

#[async_trait::async_trait]
impl ChatModel for FinalAnswerModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Ok(ChatResponse {
            message: Message::ai("Done"),
            usage: None,
        })
    }
}

#[tokio::test]
async fn minimal_offline_agent() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mut options = DeepAgentOptions::new(backend);
    options.enable_subagents = false;
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();
    let state = MessageState::with_messages(vec![Message::human("hello")]);
    let result = agent.invoke(state).await.unwrap().into_state();

    let last = result.last_message().unwrap();
    assert!(last.is_ai());
    assert_eq!(last.content(), "Done");
}

#[tokio::test]
async fn filesystem_disabled_still_works() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mut options = DeepAgentOptions::new(backend);
    options.enable_filesystem = false;
    options.enable_subagents = false;
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();
    let state = MessageState::with_messages(vec![Message::human("hello")]);
    let result = agent.invoke(state).await.unwrap().into_state();

    assert!(result.last_message().unwrap().is_ai());
}

#[tokio::test]
async fn custom_system_prompt_accepted() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mut options = DeepAgentOptions::new(backend);
    options.system_prompt = Some("You are a test agent.".to_string());
    options.enable_subagents = false;
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();
    let state = MessageState::with_messages(vec![Message::human("hello")]);
    let result = agent.invoke(state).await.unwrap().into_state();

    assert!(result.last_message().unwrap().is_ai());
}

#[test]
fn options_default_values() {
    let backend = Arc::new(StateBackend::new());
    let options = DeepAgentOptions::new(backend);

    assert!(options.enable_subagents);
    assert!(options.enable_filesystem);
    assert!(options.enable_skills);
    assert!(options.enable_memory);
    assert!(options.system_prompt.is_none());
    assert!(options.tools.is_empty());
    assert!(options.middleware.is_empty());
    assert_eq!(options.max_input_tokens, 128_000);
    assert!((options.summarization_threshold - 0.85).abs() < 0.01);
    assert_eq!(options.eviction_threshold, 20_000);
    assert_eq!(options.max_subagent_depth, 3);
    assert_eq!(
        options.skills_dir,
        Some(".skills".to_string())
    );
    assert_eq!(
        options.memory_file,
        Some("AGENTS.md".to_string())
    );
    assert!(options.subagents.is_empty());
    assert!(options.checkpointer.is_none());
    assert!(options.store.is_none());
}

#[tokio::test]
async fn all_features_disabled_produces_basic_agent() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mut options = DeepAgentOptions::new(backend);
    options.enable_filesystem = false;
    options.enable_subagents = false;
    options.enable_skills = false;
    options.enable_memory = false;

    let agent = create_deep_agent(model, options).unwrap();
    let state = MessageState::with_messages(vec![Message::human("hello")]);
    let result = agent.invoke(state).await.unwrap().into_state();

    assert!(!result.messages.is_empty());
    // Should have human + AI messages
    assert!(result.messages.iter().any(|m| m.is_human()));
    assert!(result.messages.iter().any(|m| m.is_ai()));
}

#[test]
fn state_backend_new_and_default() {
    let backend = StateBackend::new();
    let _arc = Arc::new(backend);

    // Default trait also works
    let backend2 = StateBackend::default();
    let _arc2 = Arc::new(backend2);
}

#[tokio::test]
async fn custom_options_fields_mutated() {
    let backend = Arc::new(StateBackend::new());
    let mut options = DeepAgentOptions::new(backend.clone());

    options.max_input_tokens = 50_000;
    options.summarization_threshold = 0.7;
    options.eviction_threshold = 5_000;
    options.max_subagent_depth = 1;
    options.skills_dir = None;
    options.memory_file = None;
    options.enable_subagents = false;
    options.enable_skills = false;
    options.enable_memory = false;

    assert_eq!(options.max_input_tokens, 50_000);
    assert!((options.summarization_threshold - 0.7).abs() < 0.01);
    assert_eq!(options.eviction_threshold, 5_000);
    assert_eq!(options.max_subagent_depth, 1);
    assert!(options.skills_dir.is_none());
    assert!(options.memory_file.is_none());

    // Should still compile into a working agent
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let agent = create_deep_agent(model, options).unwrap();
    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = agent.invoke(state).await.unwrap().into_state();
    assert!(result.last_message().unwrap().is_ai());
}
