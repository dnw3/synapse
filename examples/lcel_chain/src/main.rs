use serde_json::{json, Value};
use synaptic::core::{RunnableConfig, SynapticError};
use synaptic::runnables::{
    BoxRunnable, Runnable, RunnableLambda, RunnableParallel, RunnablePassthrough,
};
use synaptic_macros::chain;

// ---------------------------------------------------------------------------
// #[chain] functions used in RunnableParallel (Value output)
// ---------------------------------------------------------------------------

#[chain]
async fn to_upper_value(s: String) -> Result<Value, SynapticError> {
    Ok(Value::String(s.to_uppercase()))
}

#[chain]
async fn to_lower_value(s: String) -> Result<Value, SynapticError> {
    Ok(Value::String(s.to_lowercase()))
}

#[chain]
async fn get_length(s: String) -> Result<Value, SynapticError> {
    Ok(json!(s.len()))
}

// ---------------------------------------------------------------------------
// Typed #[chain] functions (String output — no serialization overhead)
// ---------------------------------------------------------------------------

#[chain]
async fn to_upper(s: String) -> Result<String, SynapticError> {
    Ok(s.to_uppercase())
}

#[chain]
async fn exclaim(s: String) -> Result<String, SynapticError> {
    Ok(format!("{}!", s))
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let config = RunnableConfig::default();

    // --- RunnableLambda ---
    println!("=== RunnableLambda ===");
    let upper = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let result = upper.invoke("hello synaptic".to_string(), &config).await?;
    println!("upper: {result}");

    // --- Pipe operator ---
    println!("\n=== Pipe Operator ===");
    let exclaim_lambda = RunnableLambda::new(|s: String| async move { Ok(format!("{s}!")) });
    let chain = upper.boxed() | exclaim_lambda.boxed();
    let result = chain.invoke("hello".to_string(), &config).await?;
    println!("upper | exclaim: {result}");

    // --- Three-step pipeline ---
    println!("\n=== Three-Step Pipeline ===");
    let step1 = RunnableLambda::new(|s: String| async move { Ok(s.trim().to_string()) });
    let step2 = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let step3 = RunnableLambda::new(|s: String| async move { Ok(format!("[{s}]")) });
    let pipeline = step1.boxed() | step2.boxed() | step3.boxed();
    let result = pipeline
        .invoke("  hello world  ".to_string(), &config)
        .await?;
    println!("trim | upper | bracket: {result}");

    // --- Typed #[chain] pipe composition (String -> String) ---
    println!("\n=== Typed #[chain] Pipe ===");
    let typed_pipeline = to_upper() | exclaim();
    let result = typed_pipeline
        .invoke("hello".to_string(), &config)
        .await?;
    println!("to_upper | exclaim: {result}");

    // --- RunnableParallel (using #[chain] macro functions with Value output) ---
    println!("\n=== RunnableParallel ===");
    let branches = RunnableParallel::new(vec![
        ("upper".to_string(), to_upper_value()),
        ("lower".to_string(), to_lower_value()),
        ("length".to_string(), get_length()),
    ]);
    let result = branches.invoke("Hello World".to_string(), &config).await?;
    println!("parallel: {result}");

    // --- RunnablePassthrough ---
    println!("\n=== RunnablePassthrough ===");
    let passthrough: BoxRunnable<String, String> = RunnablePassthrough.boxed();
    let result = passthrough.invoke("unchanged".to_string(), &config).await?;
    println!("passthrough: {result}");

    println!("\nAll LCEL demos completed successfully!");
    Ok(())
}
