use serde_json::json;
use synaptic_core::RunnableConfig;
use synaptic_runnables::{Runnable, RunnableLambda};

#[tokio::test]
async fn bind_transforms_config() {
    // A lambda that returns the run_name from config
    let r = RunnableLambda::new(|_s: String| async move { Ok("done".to_string()) });
    let bound = r.boxed().bind(|c| c.with_run_name("bound-name"));
    let config = RunnableConfig::default();
    let result = bound.invoke("input".to_string(), &config).await.unwrap();
    assert_eq!(result, "done");
}

#[tokio::test]
async fn bind_in_sequence() {
    let step1 = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let step2 = RunnableLambda::new(|s: String| async move { Ok(format!("({s})")) });
    let chain = step1.boxed().bind(|c| c.with_run_name("step1")) | step2.boxed();
    let config = RunnableConfig::default();
    let result = chain.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(result, "(HELLO)");
}

#[tokio::test]
async fn bind_merges_metadata() {
    let r = RunnableLambda::new(|s: String| async move { Ok(s) });
    let bound = r.boxed().bind(|c| c.with_metadata("key", json!("value")));
    let config = RunnableConfig::default();
    let result = bound.invoke("test".to_string(), &config).await.unwrap();
    assert_eq!(result, "test");
}
