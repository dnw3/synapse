# 自定义工具

Synaptic 中的每个工具都实现了 `synaptic-core` 中的 `Tool` trait。本页展示如何定义你自己的工具。

## Tool Trait

`Tool` trait 要求实现三个方法：

```rust
use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name used to identify this tool in registries and tool calls.
    fn name(&self) -> &'static str;

    /// Human-readable description sent to the model so it understands what this tool does.
    fn description(&self) -> &'static str;

    /// Execute the tool with the given JSON arguments and return a JSON result.
    async fn call(&self, args: Value) -> Result<Value, SynapticError>;
}
```

## 实现一个工具

下面是一个天气工具的完整示例：

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{Tool, SynapticError};

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &'static str {
        "get_weather"
    }

    fn description(&self) -> &'static str {
        "Get the current weather for a location"
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let location = args["location"]
            .as_str()
            .unwrap_or("unknown");

        // In production, call a real weather API here
        Ok(json!({
            "location": location,
            "temperature": 22,
            "condition": "sunny"
        }))
    }
}
```

要点：

- 由于 `Tool` 是异步 trait，需要 `#[async_trait]` 属性。
- `name()` 返回 `&'static str` -- 这是模型在进行工具调用时使用的标识符。
- `description()` 告诉模型这个工具的功能。编写清晰简洁的描述，让模型知道何时使用此工具。
- `call()` 接收 `serde_json::Value` 类型的参数（通常是 JSON 对象）并返回 `Value` 结果。

## 错误处理

对于工具特定的错误，返回 `SynapticError::Tool(...)`：

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{Tool, SynapticError};

struct DivisionTool;

#[async_trait]
impl Tool for DivisionTool {
    fn name(&self) -> &'static str {
        "divide"
    }

    fn description(&self) -> &'static str {
        "Divide two numbers"
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let a = args["a"].as_f64()
            .ok_or_else(|| SynapticError::Tool("missing argument 'a'".to_string()))?;
        let b = args["b"].as_f64()
            .ok_or_else(|| SynapticError::Tool("missing argument 'b'".to_string()))?;

        if b == 0.0 {
            return Err(SynapticError::Tool("division by zero".to_string()));
        }

        Ok(json!({"result": a / b}))
    }
}
```

## 注册和使用

定义完成后，将工具包装在 `Arc` 中并注册：

```rust
use std::sync::Arc;
use synaptic_tools::{ToolRegistry, SerialToolExecutor};
use serde_json::json;

let registry = ToolRegistry::new();
registry.register(Arc::new(WeatherTool))?;

let executor = SerialToolExecutor::new(registry);
let result = executor.execute("get_weather", json!({"location": "Tokyo"})).await?;
// result = {"location": "Tokyo", "temperature": 22, "condition": "sunny"}
```

有关注册和执行的更多信息，请参阅[工具注册表](registry.md)页面。

## 模型的工具定义

要告知聊天模型可用的工具，创建 `ToolDefinition` 值并附加到 `ChatRequest`：

```rust
use serde_json::json;
use synaptic_core::{ChatRequest, Message, ToolDefinition};

let tool_def = ToolDefinition {
    name: "get_weather".to_string(),
    description: "Get the current weather for a location".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "The city name"
            }
        },
        "required": ["location"]
    }),
};

let request = ChatRequest::new(vec![
    Message::human("What is the weather in Tokyo?"),
])
.with_tools(vec![tool_def]);
```

`parameters` 字段遵循 LLM 提供商期望的 JSON Schema 格式。

## 使用 `#[tool]` 宏

除了手动实现 `Tool` trait，你还可以使用 `synaptic-macros` 中的 `#[tool]` 属性宏来生成样板代码：

```rust,ignore
use synaptic_macros::tool;
use synaptic_core::SynapticError;
use serde_json::{json, Value};

/// Get the current weather for a location.
#[tool]
async fn get_weather(
    /// The city name
    location: String,
) -> Result<Value, SynapticError> {
    Ok(json!({
        "location": location,
        "temperature": 22,
        "condition": "sunny"
    }))
}

// `get_weather()` returns Arc<dyn Tool>
let tool = get_weather();
assert_eq!(tool.name(), "get_weather");
```

该宏从一个带注解的函数生成结构体、`impl Tool`、参数类型的 JSON Schema 以及工厂函数。函数上的文档注释成为工具描述；参数上的文档注释成为 schema 描述。

### 可选参数和默认值

```rust,ignore
#[tool]
async fn search(
    /// The search query
    query: String,
    /// Maximum results (default 10)
    #[default = 10]
    max_results: i64,
    /// Language filter
    language: Option<String>,
) -> Result<String, SynapticError> {
    let lang = language.unwrap_or_else(|| "en".into());
    Ok(format!("Searching '{}' (max {}, lang {})", query, max_results, lang))
}
```

### 使用 `#[field]` 的有状态工具

需要持有状态的工具（数据库连接、API 客户端等）可以使用 `#[field]` 创建对 LLM schema 隐藏的结构体字段：

```rust,ignore
use std::sync::Arc;

#[tool]
async fn db_query(
    #[field] pool: Arc<DbPool>,
    /// SQL query to execute
    query: String,
) -> Result<Value, SynapticError> {
    let result = pool.execute(&query).await?;
    Ok(serde_json::to_value(result).unwrap())
}

// Factory requires the field parameter
let tool = db_query(pool.clone());
```

有关 `#[inject]`、`#[default]` 和中间件宏的完整宏参考，请参阅[过程宏](../macros.md)页面。
