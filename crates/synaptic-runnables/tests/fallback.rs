use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::{Runnable, RunnableLambda, RunnableWithFallbacks};

#[tokio::test]
async fn primary_succeeds() {
    let runnable = RunnableWithFallbacks::new(
        RunnableLambda::new(|s: String| async move { Ok(format!("primary: {s}")) }).boxed(),
        vec![RunnableLambda::new(|s: String| async move { Ok(format!("fallback: {s}")) }).boxed()],
    );

    let config = RunnableConfig::default();
    let result = runnable.invoke("input".to_string(), &config).await.unwrap();
    assert_eq!(result, "primary: input");
}

#[tokio::test]
async fn fallback_used_on_primary_failure() {
    let runnable = RunnableWithFallbacks::new(
        RunnableLambda::new(|_s: String| async move {
            Err::<String, _>(SynapseError::Model("primary failed".to_string()))
        })
        .boxed(),
        vec![RunnableLambda::new(|s: String| async move { Ok(format!("fallback: {s}")) }).boxed()],
    );

    let config = RunnableConfig::default();
    let result = runnable.invoke("input".to_string(), &config).await.unwrap();
    assert_eq!(result, "fallback: input");
}

#[tokio::test]
async fn second_fallback_used() {
    let runnable = RunnableWithFallbacks::new(
        RunnableLambda::new(|_s: String| async move {
            Err::<String, _>(SynapseError::Model("primary failed".to_string()))
        })
        .boxed(),
        vec![
            RunnableLambda::new(|_s: String| async move {
                Err::<String, _>(SynapseError::Model("fallback1 failed".to_string()))
            })
            .boxed(),
            RunnableLambda::new(|s: String| async move { Ok(format!("fallback2: {s}")) }).boxed(),
        ],
    );

    let config = RunnableConfig::default();
    let result = runnable.invoke("input".to_string(), &config).await.unwrap();
    assert_eq!(result, "fallback2: input");
}

#[tokio::test]
async fn all_fail_returns_last_error() {
    let runnable = RunnableWithFallbacks::new(
        RunnableLambda::new(|_s: String| async move {
            Err::<String, _>(SynapseError::Model("primary".to_string()))
        })
        .boxed(),
        vec![RunnableLambda::new(|_s: String| async move {
            Err::<String, _>(SynapseError::Model("fallback".to_string()))
        })
        .boxed()],
    );

    let config = RunnableConfig::default();
    let err = runnable
        .invoke("input".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("fallback"));
}
