use synapse_core::{RunnableConfig, SynapseError};
use synapse_runnables::{Runnable, RunnableBranch, RunnableLambda};

#[tokio::test]
async fn branch_selects_first_match() {
    let branch = RunnableBranch::new(
        vec![
            (
                Box::new(|s: &String| s.starts_with('A'))
                    as Box<dyn Fn(&String) -> bool + Send + Sync>,
                RunnableLambda::new(|s: String| async move { Ok(format!("A-branch: {s}")) })
                    .boxed(),
            ),
            (
                Box::new(|s: &String| s.starts_with('B')),
                RunnableLambda::new(|s: String| async move { Ok(format!("B-branch: {s}")) })
                    .boxed(),
            ),
        ],
        RunnableLambda::new(|s: String| async move { Ok(format!("default: {s}")) }).boxed(),
    );

    let config = RunnableConfig::default();
    let result = branch.invoke("Apple".to_string(), &config).await.unwrap();
    assert_eq!(result, "A-branch: Apple");
}

#[tokio::test]
async fn branch_selects_second_match() {
    let branch = RunnableBranch::new(
        vec![
            (
                Box::new(|s: &String| s.starts_with('A'))
                    as Box<dyn Fn(&String) -> bool + Send + Sync>,
                RunnableLambda::new(|s: String| async move { Ok(format!("A: {s}")) }).boxed(),
            ),
            (
                Box::new(|s: &String| s.starts_with('B')),
                RunnableLambda::new(|s: String| async move { Ok(format!("B: {s}")) }).boxed(),
            ),
        ],
        RunnableLambda::new(|s: String| async move { Ok(format!("default: {s}")) }).boxed(),
    );

    let config = RunnableConfig::default();
    let result = branch.invoke("Banana".to_string(), &config).await.unwrap();
    assert_eq!(result, "B: Banana");
}

#[tokio::test]
async fn branch_falls_through_to_default() {
    let branch = RunnableBranch::new(
        vec![(
            Box::new(|s: &String| s.starts_with('A')) as Box<dyn Fn(&String) -> bool + Send + Sync>,
            RunnableLambda::new(|s: String| async move { Ok(format!("A: {s}")) }).boxed(),
        )],
        RunnableLambda::new(|s: String| async move { Ok(format!("default: {s}")) }).boxed(),
    );

    let config = RunnableConfig::default();
    let result = branch.invoke("Cherry".to_string(), &config).await.unwrap();
    assert_eq!(result, "default: Cherry");
}

#[tokio::test]
async fn branch_propagates_error() {
    let branch = RunnableBranch::new(
        vec![(
            Box::new(|_s: &String| true) as Box<dyn Fn(&String) -> bool + Send + Sync>,
            RunnableLambda::new(|_s: String| async move {
                Err::<String, _>(SynapseError::Validation("bad".to_string()))
            })
            .boxed(),
        )],
        RunnableLambda::new(|s: String| async move { Ok(s) }).boxed(),
    );

    let config = RunnableConfig::default();
    let err = branch
        .invoke("test".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bad"));
}
