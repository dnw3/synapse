use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::{Runnable, RunnableLambda};

#[tokio::test]
async fn pipe_two_steps() {
    let chain = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) }).boxed()
        | RunnableLambda::new(|s: String| async move { Ok(format!("[{s}]")) }).boxed();

    let config = RunnableConfig::default();
    let result = chain.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, "[HELLO]");
}

#[tokio::test]
async fn pipe_three_steps() {
    let chain = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) }).boxed()
        | RunnableLambda::new(|s: String| async move { Ok(format!("{s}!")) }).boxed()
        | RunnableLambda::new(|s: String| async move { Ok(format!("[{s}]")) }).boxed();

    let config = RunnableConfig::default();
    let result = chain.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, "[HELLO!]");
}

#[tokio::test]
async fn pipe_propagates_error() {
    let chain = RunnableLambda::new(|_s: String| async move {
        Err::<String, _>(SynapseError::Validation("fail".to_string()))
    })
    .boxed()
        | RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) }).boxed();

    let config = RunnableConfig::default();
    let err = chain
        .invoke("hello".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("fail"));
}

#[tokio::test]
async fn pipe_different_types() {
    let chain = RunnableLambda::new(|s: String| async move { Ok(s.len()) }).boxed()
        | RunnableLambda::new(|n: usize| async move { Ok(format!("length={n}")) }).boxed();

    let config = RunnableConfig::default();
    let result = chain.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, "length=5");
}
