//! Calculator tool for evaluating mathematical expressions.

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

/// Calculator tool for evaluating mathematical expressions.
///
/// Uses the `meval` crate to evaluate expressions. Supports arithmetic,
/// power, trigonometric, and logarithmic functions.
///
/// # Example
///
/// ```rust,ignore
/// use synaptic_tools::CalculatorTool;
/// use synaptic_core::Tool;
///
/// let tool = CalculatorTool;
/// let result = tool.call(serde_json::json!({"expression": "2 + 3 * 4"})).await?;
/// assert_eq!(result["result"], 14.0);
/// ```
pub struct CalculatorTool;

impl Default for CalculatorTool {
    fn default() -> Self {
        CalculatorTool
    }
}

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str {
        "calculator"
    }

    fn description(&self) -> &'static str {
        "Evaluate mathematical expressions. Supports +, -, *, /, ^ (power), sqrt(), abs(), \
         sin(), cos(), tan(), log(). Example: '2 + 3 * 4' returns 14."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate, e.g. '2 + 3 * 4' or 'sqrt(16)'"
                }
            },
            "required": ["expression"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let expr = args["expression"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'expression' parameter".to_string()))?;

        let result = meval::eval_str(expr)
            .map_err(|e| SynapticError::Tool(format!("math evaluation error: {e}")))?;

        Ok(json!({ "expression": expr, "result": result }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata() {
        let tool = CalculatorTool;
        assert_eq!(tool.name(), "calculator");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn tool_schema() {
        let tool = CalculatorTool;
        let schema = tool.parameters().unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["expression"].is_object());
    }

    #[tokio::test]
    async fn basic_arithmetic() {
        let tool = CalculatorTool;
        let result = tool.call(json!({"expression": "2 + 3 * 4"})).await.unwrap();
        assert_eq!(result["result"].as_f64().unwrap(), 14.0);
        assert_eq!(result["expression"], "2 + 3 * 4");
    }

    #[tokio::test]
    async fn sqrt_expression() {
        let tool = CalculatorTool;
        let result = tool.call(json!({"expression": "sqrt(16)"})).await.unwrap();
        assert_eq!(result["result"].as_f64().unwrap(), 4.0);
    }

    #[tokio::test]
    async fn power_expression() {
        let tool = CalculatorTool;
        let result = tool.call(json!({"expression": "2 ^ 10"})).await.unwrap();
        assert_eq!(result["result"].as_f64().unwrap(), 1024.0);
    }

    #[tokio::test]
    async fn missing_expression_returns_error() {
        let tool = CalculatorTool;
        let result = tool.call(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expression"));
    }

    #[tokio::test]
    async fn invalid_expression_returns_error() {
        let tool = CalculatorTool;
        let result = tool.call(json!({"expression": "not_a_number + ???"})).await;
        assert!(result.is_err());
    }
}
