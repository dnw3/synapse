use synaptic_core::{ChatRequest, Message, ToolChoice, ToolDefinition};

#[test]
fn tool_choice_serde_roundtrip_auto() {
    let choice = ToolChoice::Auto;
    let json = serde_json::to_string(&choice).unwrap();
    assert_eq!(json, r#""auto""#);
    let back: ToolChoice = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ToolChoice::Auto);
}

#[test]
fn tool_choice_serde_roundtrip_required() {
    let choice = ToolChoice::Required;
    let json = serde_json::to_string(&choice).unwrap();
    assert_eq!(json, r#""required""#);
    let back: ToolChoice = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ToolChoice::Required);
}

#[test]
fn tool_choice_serde_roundtrip_none() {
    let choice = ToolChoice::None;
    let json = serde_json::to_string(&choice).unwrap();
    assert_eq!(json, r#""none""#);
    let back: ToolChoice = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ToolChoice::None);
}

#[test]
fn tool_choice_serde_roundtrip_specific() {
    let choice = ToolChoice::Specific("search".to_string());
    let json = serde_json::to_string(&choice).unwrap();
    let back: ToolChoice = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ToolChoice::Specific("search".to_string()));
}

#[test]
fn chat_request_with_tool_choice() {
    let req = ChatRequest::new(vec![Message::human("hi")])
        .with_tools(vec![ToolDefinition {
            name: "search".to_string(),
            description: "Search".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }])
        .with_tool_choice(ToolChoice::Required);

    assert_eq!(req.tool_choice, Some(ToolChoice::Required));
    assert_eq!(req.tools.len(), 1);
}

#[test]
fn chat_request_tool_choice_none_by_default() {
    let req = ChatRequest::new(vec![Message::human("hi")]);
    assert_eq!(req.tool_choice, None);
}

#[test]
fn chat_request_serde_omits_tool_choice_when_none() {
    let req = ChatRequest::new(vec![Message::human("hi")]);
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("tool_choice").is_none());
}

#[test]
fn chat_request_serde_includes_tool_choice_when_set() {
    let req = ChatRequest::new(vec![Message::human("hi")]).with_tool_choice(ToolChoice::Auto);
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["tool_choice"], "auto");
}
