# Calculator Tool

The `CalculatorTool` evaluates mathematical expressions using the `meval` crate. It supports arithmetic, power, trigonometric, and logarithmic functions — perfect for agents that need to perform calculations without hallucinating.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["tools"] }
```

No API key required.

## Usage

```rust,ignore
use synaptic::tools::CalculatorTool;
use synaptic::core::Tool;
use serde_json::json;

let tool = CalculatorTool;

let result = tool.call(json!({"expression": "2 + 3 * 4"})).await?;
// {"expression": "2 + 3 * 4", "result": 14.0}

let result = tool.call(json!({"expression": "sqrt(144) + log(10)"})).await?;
```

## Supported Operations

| Operation | Example |
|---|---|
| Arithmetic | `2 + 3 * 4 - 1` |
| Power | `2 ^ 10` |
| Square root | `sqrt(16)` |
| Absolute value | `abs(-42)` |
| Trigonometry | `sin(3.14159)`, `cos(0)` |
| Logarithm | `log(100)` (base e), `log2(8)` |

## Notes

- The calculator uses the `meval` crate for expression parsing.
- Division by zero and other undefined operations return an error.
- All results are returned as 64-bit floating-point values.
