use std::sync::Arc;

use futures::StreamExt;
use serde_json::json;
use synaptic_core::{ChatModel, ChatRequest, Message, ToolCall, ToolDefinition};
use synaptic_models::{FakeBackend, OpenAiChatModel, OpenAiConfig, ProviderResponse};

fn setup(backend: Arc<FakeBackend>) -> OpenAiChatModel {
    let config = OpenAiConfig::new("test-key", "gpt-4")
        .with_max_tokens(100)
        .with_temperature(0.7);
    OpenAiChatModel::new(config, backend)
}

#[tokio::test]
async fn chat_parses_text_response() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let response = model.chat(request).await.unwrap();

    assert_eq!(response.message.content(), "Hello!");
    assert!(response.message.tool_calls().is_empty());
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
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"q\":\"rust\"}"
                        }
                    }]
                }
            }],
            "usage": null
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("search for rust")]);
    let response = model.chat(request).await.unwrap();

    assert_eq!(response.message.tool_calls().len(), 1);
    assert_eq!(response.message.tool_calls()[0].name, "search");
    assert_eq!(
        response.message.tool_calls()[0].arguments,
        json!({"q": "rust"})
    );
}

#[tokio::test]
async fn chat_with_tool_definitions() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "using tool"
                }
            }],
            "usage": null
        }),
    });

    let model = setup(backend);
    let tools = vec![ToolDefinition {
        name: "search".to_string(),
        description: "Search the web".to_string(),
        parameters: json!({"type": "object", "properties": {"q": {"type": "string"}}}),
    }];
    let request = ChatRequest::new(vec![Message::human("search")]).with_tools(tools);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "using tool");
}

#[tokio::test]
async fn chat_handles_rate_limit() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: json!({
            "error": {
                "message": "too many requests"
            }
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("rate limit"));
}

#[tokio::test]
async fn chat_handles_api_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 500,
        body: json!({
            "error": {
                "message": "internal server error"
            }
        }),
    });

    let model = setup(backend);
    let request = ChatRequest::new(vec![Message::human("Hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("OpenAI API error"));
}

#[tokio::test]
async fn chat_maps_all_message_types() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "choices": [{"message": {"role": "assistant", "content": "ok"}}],
            "usage": null
        }),
    });

    let model = setup(backend);
    let messages = vec![
        Message::system("Be helpful"),
        Message::human("Hello"),
        Message::ai_with_tool_calls(
            "calling",
            vec![ToolCall {
                id: "c1".to_string(),
                name: "search".to_string(),
                arguments: json!({"q": "test"}),
            }],
        ),
        Message::tool("result", "c1"),
    ];
    let request = ChatRequest::new(messages);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "ok");
}

#[tokio::test]
async fn stream_chat_parses_sse() {
    let backend = Arc::new(FakeBackend::new());

    let sse_data = [
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: [DONE]\n\n",
    ];

    backend.push_stream_chunks(sse_data.iter().map(|s| bytes::Bytes::from(*s)).collect());

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
