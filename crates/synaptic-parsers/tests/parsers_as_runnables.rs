use synaptic_core::{Message, RunnableConfig};
use synaptic_parsers::{
    BooleanOutputParser, EnumOutputParser, JsonOutputParser, ListOutputParser, StrOutputParser,
};
use synaptic_runnables::Runnable;

#[tokio::test]
async fn str_parser_as_runnable_invoke() {
    let parser = StrOutputParser;
    let config = RunnableConfig::default();
    let msg = Message::ai("Hello, world!");
    let result = parser.invoke(msg, &config).await.unwrap();
    assert_eq!(result, "Hello, world!");
}

#[tokio::test]
async fn str_parser_with_system_message() {
    let parser = StrOutputParser;
    let config = RunnableConfig::default();
    let msg = Message::system("System content");
    let result = parser.invoke(msg, &config).await.unwrap();
    assert_eq!(result, "System content");
}

#[tokio::test]
async fn json_parser_as_runnable_invoke() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let result = parser
        .invoke(r#"{"name": "test", "value": 42}"#.to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result["name"], "test");
    assert_eq!(result["value"], 42);
}

#[tokio::test]
async fn json_parser_nested_objects() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let input = r#"{"outer": {"inner": {"deep": true}}}"#;
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result["outer"]["inner"]["deep"], true);
}

#[tokio::test]
async fn json_parser_array_input() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let result = parser
        .invoke("[1, 2, 3]".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn json_parser_with_whitespace() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let input = "  \n  {\"key\": \"value\"}  \n  ";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result["key"], "value");
}

#[tokio::test]
async fn json_parser_invalid_returns_error() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let err = parser
        .invoke("not json at all".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid JSON"));
}

#[tokio::test]
async fn json_parser_empty_string_error() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let err = parser
        .invoke("".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid JSON"));
}

#[tokio::test]
async fn list_parser_newline_separator() {
    let parser = ListOutputParser::default(); // default is newline
    let config = RunnableConfig::default();
    let result = parser
        .invoke("apple\nbanana\ncherry".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, vec!["apple", "banana", "cherry"]);
}

#[tokio::test]
async fn list_parser_comma_separator() {
    let parser = ListOutputParser::comma();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("  a ,  b , c  ".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn list_parser_empty_lines_filtered() {
    let parser = ListOutputParser::default();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("a\n\nb\n\nc".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn boolean_parser_true_variants() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();

    for input in &["true", "TRUE", "True", "yes", "YES", "y", "Y", "1"] {
        let result = parser.invoke(input.to_string(), &config).await.unwrap();
        assert!(result, "expected true for input '{input}'");
    }
}

#[tokio::test]
async fn boolean_parser_false_variants() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();

    for input in &["false", "FALSE", "False", "no", "NO", "n", "N", "0"] {
        let result = parser.invoke(input.to_string(), &config).await.unwrap();
        assert!(!result, "expected false for input '{input}'");
    }
}

#[tokio::test]
async fn boolean_parser_invalid_returns_error() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    let err = parser
        .invoke("perhaps".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("cannot parse"));
}

#[tokio::test]
async fn enum_parser_valid_value() {
    let parser = EnumOutputParser::new(vec![
        "red".to_string(),
        "green".to_string(),
        "blue".to_string(),
    ]);
    let config = RunnableConfig::default();
    let result = parser.invoke("green".to_string(), &config).await.unwrap();
    assert_eq!(result, "green");
}

#[tokio::test]
async fn enum_parser_invalid_value_error() {
    let parser = EnumOutputParser::new(vec!["a".to_string(), "b".to_string()]);
    let config = RunnableConfig::default();
    let err = parser
        .invoke("c".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("expected one of"));
}

#[tokio::test]
async fn enum_parser_case_sensitive() {
    let parser = EnumOutputParser::new(vec!["Yes".to_string(), "No".to_string()]);
    let config = RunnableConfig::default();
    // Lowercase "yes" should fail because EnumOutputParser is case-sensitive
    let err = parser
        .invoke("yes".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("expected one of"));
}
