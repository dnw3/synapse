# 计算器工具

`CalculatorTool` 使用 `meval` crate 计算数学表达式。支持四则运算、幂次、三角函数和对数函数，非常适合需要精确计算而非靠 LLM 推测的智能体场景。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["tools"] }
```

无需 API 密钥。

## 使用示例

```rust,ignore
use synaptic::tools::CalculatorTool;
use synaptic::core::Tool;
use serde_json::json;

let tool = CalculatorTool;

let result = tool.call(json!({"expression": "2 + 3 * 4"})).await?;
// {"expression": "2 + 3 * 4", "result": 14.0}

let result = tool.call(json!({"expression": "sqrt(144) + log(10)"})).await?;
```

## 支持的运算

| 运算 | 示例 |
|---|---|
| 四则运算 | `2 + 3 * 4 - 1` |
| 幂次 | `2 ^ 10` |
| 平方根 | `sqrt(16)` |
| 绝对值 | `abs(-42)` |
| 三角函数 | `sin(3.14159)`, `cos(0)` |
| 对数 | `log(100)`（底 e），`log2(8)` |

## 注意事项

- 计算器使用 `meval` crate 解析表达式。
- 除以零及其他未定义运算将返回错误。
- 所有结果以 64 位浮点数返回。
