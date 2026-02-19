use serde_json::json;
use synaptic_core::{Message, ToolCall};
use synaptic_deep::middleware::patch_tool_calls::PatchToolCallsMiddleware;
use synaptic_middleware::{AgentMiddleware, ModelRequest, ModelResponse};

fn empty_request() -> ModelRequest {
    ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    }
}

#[tokio::test]
async fn fixes_string_arguments_to_json_object() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "tc1".to_string(),
                name: "write_file".to_string(),
                arguments: json!("{\"path\": \"test.txt\", \"content\": \"hello\"}"),
            }],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    // String args should be parsed into a JSON object
    assert!(calls[0].arguments.is_object());
    assert_eq!(calls[0].arguments["path"], "test.txt");
    assert_eq!(calls[0].arguments["content"], "hello");
}

#[tokio::test]
async fn noop_on_valid_object_args() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "tc1".to_string(),
                name: "read_file".to_string(),
                arguments: json!({"path": "test.txt"}),
            }],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments, json!({"path": "test.txt"}));
}

#[tokio::test]
async fn empty_tool_calls_noop() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai("no tools here"),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();
    assert!(response.message.tool_calls().is_empty());
    assert_eq!(response.message.content(), "no tools here");
}

#[tokio::test]
async fn removes_empty_name_tool_calls() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![
                ToolCall {
                    id: "tc1".to_string(),
                    name: "".to_string(),
                    arguments: json!({}),
                },
                ToolCall {
                    id: "tc2".to_string(),
                    name: "valid_tool".to_string(),
                    arguments: json!({"key": "value"}),
                },
            ],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "valid_tool");
}

#[tokio::test]
async fn strips_markdown_code_fences_and_parses() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "tc1".to_string(),
                name: "write_file".to_string(),
                arguments: json!("```json\n{\"path\": \"test.txt\"}\n```"),
            }],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    // After stripping fences the JSON should be parsed
    assert!(calls[0].arguments.is_object());
    assert_eq!(calls[0].arguments["path"], "test.txt");
}

#[tokio::test]
async fn multiple_tool_calls_all_processed() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![
                ToolCall {
                    id: "tc1".to_string(),
                    name: "tool_a".to_string(),
                    arguments: json!({"a": 1}),
                },
                ToolCall {
                    id: "tc2".to_string(),
                    name: "tool_b".to_string(),
                    arguments: json!("{\"b\": 2}"),
                },
            ],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 2);
    // First call already had valid object args
    assert_eq!(calls[0].arguments, json!({"a": 1}));
    // Second call had string args that should be parsed
    assert!(calls[1].arguments.is_object());
    assert_eq!(calls[1].arguments["b"], 2);
}

#[tokio::test]
async fn deduplicates_identical_tool_call_ids() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![
                ToolCall {
                    id: "same_id".to_string(),
                    name: "tool_a".to_string(),
                    arguments: json!({}),
                },
                ToolCall {
                    id: "same_id".to_string(),
                    name: "tool_b".to_string(),
                    arguments: json!({}),
                },
            ],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 2);
    // IDs should be unique after patching
    assert_ne!(calls[0].id, calls[1].id);
}

#[tokio::test]
async fn patches_empty_id() {
    let mw = PatchToolCallsMiddleware;
    let request = empty_request();
    let mut response = ModelResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "".to_string(),
                name: "read_file".to_string(),
                arguments: json!({"path": "f.txt"}),
            }],
        ),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();

    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    // Empty ID should be replaced with a generated one
    assert!(!calls[0].id.is_empty());
}
