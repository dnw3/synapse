use std::sync::Arc;

use async_trait::async_trait;
use synaptic_condenser::{
    Condenser, CondenserMiddleware, LlmSummarizingCondenser, NoOpCondenser, PipelineCondenser,
    RollingCondenser, TokenBudgetCondenser,
};
use synaptic_core::{
    ChatModel, ChatRequest, ChatResponse, HeuristicTokenCounter, Message, SynapticError,
};
use synaptic_middleware::{AgentMiddleware, ModelRequest};

#[tokio::test]
async fn noop_unchanged() {
    let c = NoOpCondenser;
    let msgs = vec![Message::human("hi"), Message::ai("hello")];
    let result = c.condense(msgs.clone()).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content(), "hi");
}

#[tokio::test]
async fn rolling_trims() {
    let c = RollingCondenser::new(2);
    let msgs = vec![
        Message::human("1"),
        Message::ai("2"),
        Message::human("3"),
        Message::ai("4"),
    ];
    let result = c.condense(msgs).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content(), "3");
    assert_eq!(result[1].content(), "4");
}

#[tokio::test]
async fn rolling_preserves_system() {
    let c = RollingCondenser::new(3);
    let msgs = vec![
        Message::system("system"),
        Message::human("1"),
        Message::ai("2"),
        Message::human("3"),
        Message::ai("4"),
    ];
    let result = c.condense(msgs).await.unwrap();
    assert_eq!(result.len(), 3);
    assert!(result[0].is_system());
    assert_eq!(result[1].content(), "3");
    assert_eq!(result[2].content(), "4");
}

#[tokio::test]
async fn token_budget_trims() {
    let counter = Arc::new(HeuristicTokenCounter);
    // Very small budget: only fits 1-2 short messages
    let c = TokenBudgetCondenser::new(12, counter);
    let msgs = vec![
        Message::human("first message here"),
        Message::ai("second message here"),
        Message::human("hi"),
    ];
    let result = c.condense(msgs).await.unwrap();
    // Should keep only the most recent that fit
    assert!(result.len() < 3);
    // Most recent should be last
    assert_eq!(result.last().unwrap().content(), "hi");
}

#[tokio::test]
async fn pipeline_chains() {
    let pipeline = PipelineCondenser::new(vec![
        Arc::new(RollingCondenser::new(3)),
        Arc::new(NoOpCondenser),
    ]);
    let msgs = vec![
        Message::human("1"),
        Message::ai("2"),
        Message::human("3"),
        Message::ai("4"),
    ];
    let result = pipeline.condense(msgs).await.unwrap();
    assert_eq!(result.len(), 3);
}

/// A scripted model that returns a fixed response.
struct FixedModel(String);

#[async_trait]
impl ChatModel for FixedModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Ok(ChatResponse {
            message: Message::ai(&self.0),
            usage: None,
        })
    }
}

#[tokio::test]
async fn llm_summarizing() {
    let model = Arc::new(FixedModel("This is a summary.".to_string()));
    let c = LlmSummarizingCondenser::new(model, 20, 1);

    let msgs = vec![
        Message::system("You are helpful"),
        Message::human("Tell me about rust"),
        Message::ai("Rust is a systems programming language..."),
        Message::human("What about safety?"),
        Message::ai("Rust provides memory safety without garbage collection..."),
        Message::human("Thanks!"),
    ];

    let result = c.condense(msgs).await.unwrap();
    // Should have: system + summary + last 1 message
    assert!(result.len() <= 3);
    assert!(result[0].is_system());
    // Summary should be present
    assert!(result.iter().any(|m| m.content().contains("summary")));
}

#[tokio::test]
async fn middleware_applies() {
    let condenser = Arc::new(RollingCondenser::new(2));
    let mw = CondenserMiddleware::new(condenser);

    let mut request = ModelRequest {
        messages: vec![Message::human("1"), Message::ai("2"), Message::human("3")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };

    mw.before_model(&mut request).await.unwrap();
    assert_eq!(request.messages.len(), 2);
}
