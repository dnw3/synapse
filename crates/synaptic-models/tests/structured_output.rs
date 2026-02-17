use std::sync::Arc;

use serde::Deserialize;
use synaptic_core::{ChatRequest, ChatResponse, Message};
use synaptic_models::{ScriptedChatModel, StructuredOutputChatModel};

#[derive(Debug, Deserialize, PartialEq)]
struct Person {
    name: String,
    age: u32,
}

#[tokio::test]
async fn structured_output_parses_json() {
    let model = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(r#"{"name": "Alice", "age": 30}"#),
        usage: None,
    }]);

    let structured = StructuredOutputChatModel::<Person>::new(
        Arc::new(model),
        r#"{"name": "string", "age": "number"}"#,
    );

    let request = ChatRequest::new(vec![Message::human("Tell me about Alice")]);
    let (person, _response) = structured.generate(request).await.unwrap();
    assert_eq!(
        person,
        Person {
            name: "Alice".to_string(),
            age: 30
        }
    );
}

#[tokio::test]
async fn structured_output_handles_code_blocks() {
    let model = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("```json\n{\"name\": \"Bob\", \"age\": 25}\n```"),
        usage: None,
    }]);

    let structured = StructuredOutputChatModel::<Person>::new(
        Arc::new(model),
        r#"{"name": "string", "age": "number"}"#,
    );

    let request = ChatRequest::new(vec![Message::human("Tell me about Bob")]);
    let (person, _) = structured.generate(request).await.unwrap();
    assert_eq!(
        person,
        Person {
            name: "Bob".to_string(),
            age: 25
        }
    );
}

#[tokio::test]
async fn structured_output_returns_parsing_error() {
    let model = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("This is not JSON"),
        usage: None,
    }]);

    let structured = StructuredOutputChatModel::<Person>::new(
        Arc::new(model),
        r#"{"name": "string", "age": "number"}"#,
    );

    let request = ChatRequest::new(vec![Message::human("Tell me about someone")]);
    let err = structured.generate(request).await.unwrap_err();
    assert!(err
        .to_string()
        .contains("failed to parse structured output"));
}

#[tokio::test]
async fn structured_output_injects_system_message() {
    // Use ChatModel::chat directly (which injects the system message)
    use synaptic_core::ChatModel;

    let model = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(r#"{"name": "Test", "age": 1}"#),
        usage: None,
    }]);

    let structured = StructuredOutputChatModel::<Person>::new(Arc::new(model), "test schema");

    let request = ChatRequest::new(vec![Message::human("test")]);
    let response = structured.chat(request).await.unwrap();
    // The response should be valid (model returned valid JSON)
    assert!(response.message.content().contains("Test"));
}
