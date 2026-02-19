use serde_json::{json, Value};
use synaptic::core::SynapticError;
use synaptic::macros::tool;
use synaptic::tools::{SerialToolExecutor, ToolRegistry};

/// Echo the given JSON payload back to the caller
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(json!({ "echo": args }))
}

/// Add two numbers together
#[tool(name = "add")]
async fn add(
    /// The first number
    a: f64,
    /// The second number
    b: f64,
) -> Result<Value, SynapticError> {
    Ok(json!({ "result": a + b }))
}

/// Reverse a string
#[tool(name = "reverse")]
async fn reverse(
    /// The text to reverse
    text: String,
) -> Result<Value, SynapticError> {
    let reversed: String = text.chars().rev().collect();
    Ok(json!({ "reversed": reversed }))
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // --- 1. Register tools ---
    let registry = ToolRegistry::new();
    registry.register(echo())?;
    registry.register(add())?;
    registry.register(reverse())?;

    // --- 2. Execute tools via executor ---
    let executor = SerialToolExecutor::new(registry);

    let echo_result = executor
        .execute("echo", json!({ "message": "hello from synaptic" }))
        .await?;
    println!("echo result: {echo_result}");

    let add_result = executor.execute("add", json!({ "a": 3.0, "b": 4.5 })).await?;
    println!("add result:  {add_result}");

    let rev_result = executor
        .execute("reverse", json!({ "text": "Synaptic" }))
        .await?;
    println!("reverse result: {rev_result}");

    // --- 3. Error handling: unknown tool ---
    println!("\n=== Error handling ===");
    match executor.execute("nonexistent", json!({})).await {
        Ok(_) => println!("  unexpected success"),
        Err(e) => println!("  calling unknown tool: {e}"),
    }

    Ok(())
}
