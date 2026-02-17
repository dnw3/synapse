use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::{Runnable, RunnableLambda};

#[tokio::test]
async fn lambda_transforms_input() {
    let upper = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let config = RunnableConfig::default();
    let result = upper.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, "HELLO");
}

#[tokio::test]
async fn lambda_can_return_error() {
    let failing = RunnableLambda::new(|_s: String| async move {
        Err::<String, _>(SynapseError::Validation("bad input".to_string()))
    });
    let config = RunnableConfig::default();
    let err = failing
        .invoke("anything".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bad input"));
}

#[tokio::test]
async fn lambda_with_different_types() {
    let length = RunnableLambda::new(|s: String| async move { Ok(s.len()) });
    let config = RunnableConfig::default();
    let result = length.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, 5);
}

#[tokio::test]
async fn lambda_boxed_for_composition() {
    let upper = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) }).boxed();
    let config = RunnableConfig::default();
    let result = upper.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, "HELLO");
}
