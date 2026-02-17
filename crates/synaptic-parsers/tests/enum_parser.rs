use synaptic_core::RunnableConfig;
use synaptic_parsers::EnumOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn accepts_valid_value() {
    let parser = EnumOutputParser::new(vec!["yes".to_string(), "no".to_string()]);
    let config = RunnableConfig::default();
    let result = parser.invoke("yes".to_string(), &config).await.unwrap();
    assert_eq!(result, "yes");
}

#[tokio::test]
async fn trims_whitespace() {
    let parser = EnumOutputParser::new(vec!["yes".to_string(), "no".to_string()]);
    let config = RunnableConfig::default();
    let result = parser.invoke("  yes  ".to_string(), &config).await.unwrap();
    assert_eq!(result, "yes");
}

#[tokio::test]
async fn rejects_invalid_value() {
    let parser = EnumOutputParser::new(vec!["yes".to_string(), "no".to_string()]);
    let config = RunnableConfig::default();
    let err = parser
        .invoke("maybe".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("expected one of"));
    assert!(err.to_string().contains("maybe"));
}
