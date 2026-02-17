use synapse_core::RunnableConfig;
use synapse_parsers::BooleanOutputParser;
use synapse_runnables::Runnable;

#[tokio::test]
async fn parses_true() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(parser.invoke("true".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_false() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(!parser.invoke("false".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_yes() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(parser.invoke("yes".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_no() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(!parser.invoke("no".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_y() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(parser.invoke("y".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_n() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(!parser.invoke("n".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_1() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(parser.invoke("1".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn parses_0() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(!parser.invoke("0".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn case_insensitive() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(parser.invoke("TRUE".to_string(), &config).await.unwrap());
    assert!(parser.invoke("Yes".to_string(), &config).await.unwrap());
    assert!(!parser.invoke("FALSE".to_string(), &config).await.unwrap());
    assert!(!parser.invoke("No".to_string(), &config).await.unwrap());
    assert!(parser.invoke("Y".to_string(), &config).await.unwrap());
    assert!(!parser.invoke("N".to_string(), &config).await.unwrap());
}

#[tokio::test]
async fn trims_whitespace() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    assert!(parser
        .invoke("  true  ".to_string(), &config)
        .await
        .unwrap());
    assert!(!parser
        .invoke("\nfalse\n".to_string(), &config)
        .await
        .unwrap());
}

#[tokio::test]
async fn error_on_invalid_input() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    let err = parser
        .invoke("maybe".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("cannot parse"));
    assert!(err.to_string().contains("maybe"));
}

#[tokio::test]
async fn error_on_empty_input() {
    let parser = BooleanOutputParser;
    let config = RunnableConfig::default();
    let err = parser.invoke("".to_string(), &config).await.unwrap_err();
    assert!(err.to_string().contains("cannot parse"));
}
