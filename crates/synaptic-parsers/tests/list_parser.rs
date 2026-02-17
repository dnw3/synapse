use synaptic_core::RunnableConfig;
use synaptic_parsers::ListOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn splits_by_newline() {
    let parser = ListOutputParser::newline();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("apple\nbanana\ncherry".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, vec!["apple", "banana", "cherry"]);
}

#[tokio::test]
async fn splits_by_comma() {
    let parser = ListOutputParser::comma();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("red, green, blue".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, vec!["red", "green", "blue"]);
}

#[tokio::test]
async fn skips_empty_lines() {
    let parser = ListOutputParser::newline();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("first\n\nsecond\n\n".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, vec!["first", "second"]);
}
