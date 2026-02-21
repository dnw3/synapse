use std::sync::Arc;

use futures::StreamExt;
use serde_json::json;
use synaptic_core::{ChatModel, ChatRequest, Message, ToolDefinition};
use synaptic_models::{FakeBackend, ProviderResponse};
use synaptic_ollama::{OllamaChatModel, OllamaConfig};

fn setup(backend: Arc<FakeBackend>) -> OllamaChatModel {
    let config = OllamaConfig::new("llama3");
    OllamaChatModel::new(config, backend)
}

#[tokio::test]
async fn chat_parses_text_response() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "message": {
                "role": "assistant",
                "content": "Hello!"
            },
            "prompt_eval_count": 10,
            "eval_count": 5
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let response = model.chat(request).await.unwrap();

    assert_eq!(response.message.content(), "Hello!");
    let usage = response.usage.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}

#[tokio::test]
async fn chat_parses_tool_calls() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "search",
                        "arguments": {"q": "rust"}
                    }
                }]
            }
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("search")]);
    let response = model.chat(request).await.unwrap();

    assert_eq!(response.message.tool_calls().len(), 1);
    assert_eq!(response.message.tool_calls()[0].name, "search");
}

#[tokio::test]
async fn chat_with_tool_definitions() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "message": {"role": "assistant", "content": "ok"}
        }),
    });

    let model = setup(backend);
    let tools = vec![ToolDefinition {
        name: "search".to_string(),
        description: "Search".to_string(),
        parameters: json!({"type": "object"}),
        extras: None,
    }];
    let request = ChatRequest::new(vec![Message::human("hi")]).with_tools(tools);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "ok");
}

#[tokio::test]
async fn chat_handles_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 500,
        body: json!({"error": "model not found"}),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("Ollama API error"));
}

#[tokio::test]
async fn stream_chat_parses_ndjson() {
    let backend = Arc::new(FakeBackend::new());

    let ndjson = format!(
        "{}\n{}\n{}\n",
        json!({"message":{"role":"assistant","content":"Hello"},"done":false}),
        json!({"message":{"role":"assistant","content":" world"},"done":false}),
        json!({"message":{"role":"assistant","content":""},"done":true,"prompt_eval_count":10,"eval_count":8}),
    );

    backend.push_stream_chunks(vec![bytes::Bytes::from(ndjson)]);

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let stream = model.stream_chat(request);

    let chunks: Vec<_> = stream
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].content, "Hello");
    assert_eq!(chunks[1].content, " world");
    assert!(chunks[2].usage.is_some());
    assert_eq!(chunks[2].usage.as_ref().unwrap().output_tokens, 8);
}
