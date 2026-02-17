use serde_json::{json, Value};
use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::{Runnable, RunnableLambda, RunnableParallel};

#[tokio::test]
async fn parallel_runs_branches_and_merges() {
    let parallel = RunnableParallel::new(vec![
        (
            "upper".to_string(),
            RunnableLambda::new(|s: String| async move { Ok(Value::String(s.to_uppercase())) })
                .boxed(),
        ),
        (
            "lower".to_string(),
            RunnableLambda::new(|s: String| async move { Ok(Value::String(s.to_lowercase())) })
                .boxed(),
        ),
    ]);

    let config = RunnableConfig::default();
    let result = parallel.invoke("Hello".to_string(), &config).await.unwrap();

    assert_eq!(result["upper"], json!("HELLO"));
    assert_eq!(result["lower"], json!("hello"));
}

#[tokio::test]
async fn parallel_propagates_error() {
    let parallel = RunnableParallel::new(vec![
        (
            "ok".to_string(),
            RunnableLambda::new(|s: String| async move { Ok(Value::String(s)) }).boxed(),
        ),
        (
            "fail".to_string(),
            RunnableLambda::new(|_s: String| async move {
                Err::<Value, _>(SynapseError::Validation("branch failed".to_string()))
            })
            .boxed(),
        ),
    ]);

    let config = RunnableConfig::default();
    let err = parallel
        .invoke("input".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("branch failed"));
}

#[tokio::test]
async fn parallel_with_single_branch() {
    let parallel = RunnableParallel::new(vec![(
        "only".to_string(),
        RunnableLambda::new(|s: String| async move { Ok(Value::String(s)) }).boxed(),
    )]);

    let config = RunnableConfig::default();
    let result = parallel.invoke("test".to_string(), &config).await.unwrap();
    assert_eq!(result["only"], json!("test"));
}
