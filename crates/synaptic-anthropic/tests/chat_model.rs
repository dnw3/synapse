use std::sync::Arc;

use futures::StreamExt;
use serde_json::json;
use synaptic_core::{ChatModel, ChatRequest, Message, ToolCall, ToolDefinition};
use synaptic_anthropic::{AnthropicChatModel, AnthropicConfig};
use synaptic_models::{FakeBackend, ProviderResponse};

fn setup(backend: Arc<FakeBackend>) -> AnthropicChatModel {
    let config =
        AnthropicConfig::new("test-key", "claude-sonnet-4-5-20250929").with_max_tokens(1024);
    AnthropicChatModel::new(config, backend)
}

#[tokio::test]
async fn chat_parses_text_response() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "content": [{
                "type": "text",
                "text": "Hello!"
            }],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
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
async fn chat_parses_tool_use() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "content": [
                {"type": "text", "text": "I'll search for that."},
                {
                    "type": "tool_use",
                    "id": "tu-1",
                    "name": "search",
                    "input": {"q": "rust"}
                }
            ],
            "usage": {"input_tokens": 10, "output_tokens": 20}
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("search")]);
    let response = model.chat(request).await.unwrap();

    assert_eq!(response.message.content(), "I'll search for that.");
    assert_eq!(response.message.tool_calls().len(), 1);
    assert_eq!(response.message.tool_calls()[0].name, "search");
    assert_eq!(response.message.tool_calls()[0].id, "tu-1");
}

#[tokio::test]
async fn chat_with_system_message() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "content": [{"type": "text", "text": "ok"}],
            "usage": null
        }),
    });

    let model = setup(backend);
    let messages = vec![Message::system("You are helpful"), Message::human("Hello")];
    let request = ChatRequest::new(messages);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "ok");
}

#[tokio::test]
async fn chat_with_tool_definitions() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "content": [{"type": "text", "text": "ok"}],
            "usage": null
        }),
    });

    let model = setup(backend);
    let tools = vec![ToolDefinition {
        name: "search".to_string(),
        description: "Search the web".to_string(),
        parameters: json!({"type": "object"}),
        extras: None,
    }];
    let request = ChatRequest::new(vec![Message::human("hi")]).with_tools(tools);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "ok");
}

#[tokio::test]
async fn chat_maps_tool_result_message() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "content": [{"type": "text", "text": "got it"}],
            "usage": null
        }),
    });

    let model = setup(backend);
    let messages = vec![
        Message::human("search"),
        Message::ai_with_tool_calls(
            "searching",
            vec![ToolCall {
                id: "tu-1".to_string(),
                name: "search".to_string(),
                arguments: json!({"q": "test"}),
            }],
        ),
        Message::tool("result data", "tu-1"),
    ];
    let request = ChatRequest::new(messages);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "got it");
}

#[tokio::test]
async fn chat_handles_rate_limit() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: json!({"error": {"message": "too many requests"}}),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("rate limit"));
}

#[tokio::test]
async fn stream_chat_parses_content_events() {
    let backend = Arc::new(FakeBackend::new());

    let sse = [
        "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
        "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n",
        "event: message_stop\ndata: {}\n\n",
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

#[tokio::test]
async fn stream_chat_parses_tool_use_start() {
    let backend = Arc::new(FakeBackend::new());

    let sse = [
        "event: content_block_start\ndata: {\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu-1\",\"name\":\"search\",\"input\":{}}}\n\n",
        "event: message_stop\ndata: {}\n\n",
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

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].tool_calls.len(), 1);
    assert_eq!(chunks[0].tool_calls[0].name, "search");
}
