use std::sync::Arc;

use futures::StreamExt;
use serde_json::json;
use synaptic_core::{ChatModel, ChatRequest, Message, ToolDefinition};
use synaptic_models::{FakeBackend, GeminiChatModel, GeminiConfig, ProviderResponse};

fn setup(backend: Arc<FakeBackend>) -> GeminiChatModel {
    let config = GeminiConfig::new("test-key", "gemini-2.0-flash");
    GeminiChatModel::new(config, backend)
}

#[tokio::test]
async fn chat_parses_text_response() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello!"}],
                    "role": "model"
                }
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
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
async fn chat_parses_function_call() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "search",
                            "args": {"q": "rust"}
                        }
                    }],
                    "role": "model"
                }
            }],
            "usageMetadata": null
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("search")]);
    let response = model.chat(request).await.unwrap();

    assert_eq!(response.message.tool_calls().len(), 1);
    assert_eq!(response.message.tool_calls()[0].name, "search");
    assert_eq!(
        response.message.tool_calls()[0].arguments,
        json!({"q": "rust"})
    );
}

#[tokio::test]
async fn chat_with_system_instruction() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "ok"}],
                    "role": "model"
                }
            }]
        }),
    });

    let model = setup(backend);
    let messages = vec![Message::system("Be helpful"), Message::human("Hello")];
    let request = ChatRequest::new(messages);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "ok");
}

#[tokio::test]
async fn chat_with_function_declarations() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "ok"}],
                    "role": "model"
                }
            }]
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
async fn chat_handles_rate_limit() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: json!({"error": {"message": "quota exceeded"}}),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("rate limit"));
}

#[tokio::test]
async fn stream_chat_parses_sse() {
    let backend = Arc::new(FakeBackend::new());

    let sse = [
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" world\"}]}}]}\n\n",
    ];

    backend.push_stream_chunks(sse.iter().map(|s| bytes::Bytes::from(*s)).collect());

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let stream = model.stream_chat(request);

    let chunks: Vec<_> = stream
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].content, "Hello");
    assert_eq!(chunks[1].content, " world");
}
