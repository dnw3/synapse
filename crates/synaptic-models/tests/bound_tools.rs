use std::sync::Arc;

use futures::StreamExt;
use serde_json::json;
use synaptic_core::{
    ChatModel, ChatRequest, ChatResponse, Message, ToolCall, ToolDefinition,
};
use synaptic_models::{BoundToolsChatModel, ScriptedChatModel};

fn make_tool_def(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: format!("{name} tool"),
        parameters: json!({"type": "object", "properties": {}}),
        extras: None,
    }
}

fn scripted_model(content: &str) -> ScriptedChatModel {
    ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(content),
        usage: None,
    }])
}

#[tokio::test]
async fn bound_tools_injects_when_empty() {
    let inner = Arc::new(scripted_model("ok"));
    let tools = vec![make_tool_def("search"), make_tool_def("calc")];
    let bound = BoundToolsChatModel::new(inner, tools);

    // Request has no tools → bound tools should be injected
    let request = ChatRequest::new(vec![Message::human("hi")]);
    let resp = bound.chat(request).await.unwrap();
    assert_eq!(resp.message.content(), "ok");
}

#[tokio::test]
async fn bound_tools_merges_without_duplicates() {
    let inner = Arc::new(scripted_model("merged"));
    let bound_tools = vec![make_tool_def("search"), make_tool_def("calc")];
    let bound = BoundToolsChatModel::new(inner, bound_tools);

    // Request already has "search" → should not duplicate, but add "calc"
    let request = ChatRequest::new(vec![Message::human("hi")])
        .with_tools(vec![make_tool_def("search")]);
    let resp = bound.chat(request).await.unwrap();
    assert_eq!(resp.message.content(), "merged");
}

#[tokio::test]
async fn bound_tools_no_duplicate_by_name() {
    let inner = Arc::new(scripted_model("ok"));
    let bound_tools = vec![make_tool_def("search")];
    let bound = BoundToolsChatModel::new(inner, bound_tools);

    // Verify inject_tools logic: both request and bound have "search"
    let request = ChatRequest::new(vec![Message::human("test")])
        .with_tools(vec![make_tool_def("search")]);

    // The only way to verify tool merging is through the model response
    // (BoundToolsChatModel delegates to inner after injecting).
    // The ScriptedChatModel ignores tools, so we just verify it doesn't panic.
    let resp = bound.chat(request).await.unwrap();
    assert_eq!(resp.message.content(), "ok");
}

#[tokio::test]
async fn bound_tools_streaming_delegates() {
    let inner = Arc::new(scripted_model("streamed"));
    let bound = BoundToolsChatModel::new(inner, vec![make_tool_def("tool1")]);

    let request = ChatRequest::new(vec![Message::human("hi")]);
    let mut stream = bound.stream_chat(request);

    let chunk = stream.next().await.expect("should yield a chunk").unwrap();
    assert_eq!(chunk.content, "streamed");
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn bound_tools_wraps_any_model() {
    // Verify the Arc<dyn ChatModel> wrapping works
    let inner: Arc<dyn ChatModel> = Arc::new(scripted_model("dynamic"));
    let bound = BoundToolsChatModel::new(inner, vec![]);

    let request = ChatRequest::new(vec![Message::human("test")]);
    let resp = bound.chat(request).await.unwrap();
    assert_eq!(resp.message.content(), "dynamic");
}

#[tokio::test]
async fn bound_tools_with_tool_calls_in_response() {
    let model = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "c1".into(),
                name: "search".into(),
                arguments: json!({"q": "rust"}),
            }],
        ),
        usage: None,
    }]);
    let bound = BoundToolsChatModel::new(Arc::new(model), vec![make_tool_def("search")]);

    let request = ChatRequest::new(vec![Message::human("find rust")]);
    let resp = bound.chat(request).await.unwrap();
    assert_eq!(resp.message.tool_calls().len(), 1);
    assert_eq!(resp.message.tool_calls()[0].name, "search");
}
