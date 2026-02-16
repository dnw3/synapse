use synapse_core::{RunnableConfig, SynapseError};
use synapse_runnables::{Runnable, RunnableLambda};

use synapse_chains::SequentialChain;

#[tokio::test]
async fn sequential_chain_runs_all_steps() -> Result<(), SynapseError> {
    let chain = SequentialChain::new(vec![
        RunnableLambda::new(|s: String| async move { Ok(format!("{s}-a")) }).boxed(),
        RunnableLambda::new(|s: String| async move { Ok(format!("{s}-b")) }).boxed(),
    ]);

    let config = RunnableConfig::default();
    let out = chain.invoke("start".to_string(), &config).await?;
    assert_eq!(out, "start-a-b");
    Ok(())
}
