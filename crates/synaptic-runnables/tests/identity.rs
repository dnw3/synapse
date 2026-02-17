use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::{Runnable, RunnablePassthrough};

#[tokio::test]
async fn passthrough_returns_same_value() -> Result<(), SynapseError> {
    let runnable = RunnablePassthrough;
    let config = RunnableConfig::default();
    let out = runnable.invoke("synapse".to_string(), &config).await?;
    assert_eq!(out, "synapse");
    Ok(())
}

#[tokio::test]
async fn passthrough_works_with_integers() -> Result<(), SynapseError> {
    let runnable = RunnablePassthrough;
    let config = RunnableConfig::default();
    let out = runnable.invoke(42i32, &config).await?;
    assert_eq!(out, 42);
    Ok(())
}

#[tokio::test]
async fn passthrough_batch() -> Result<(), SynapseError> {
    let runnable = RunnablePassthrough;
    let config = RunnableConfig::default();
    let results = runnable
        .batch(vec!["a".to_string(), "b".to_string()], &config)
        .await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].as_ref().unwrap(), "a");
    assert_eq!(results[1].as_ref().unwrap(), "b");
    Ok(())
}
