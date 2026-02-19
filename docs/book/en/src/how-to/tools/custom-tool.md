# Custom Tools

Every tool in Synaptic implements the `Tool` trait from `synaptic-core`. This page shows how to define your own tools.

## The Tool Trait

The `Tool` trait requires three methods:

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

## Implementing a Tool

Here is a complete example of a weather tool:

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

Key points:

- The `#[async_trait]` attribute is required because `Tool` is an async trait.
- `name()` returns a `&'static str` -- this is the identifier the model uses when making tool calls.
- `description()` tells the model what the tool does. Write clear, concise descriptions so the model knows when to use this tool.
- `call()` receives arguments as a `serde_json::Value` (typically a JSON object) and returns a `Value` result.

## Error Handling

Return `SynapticError::Tool(...)` for tool-specific errors:

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

## Registering and Using

Once defined, wrap the tool in an `Arc` and register it:

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

See the [Tool Registry](registry.md) page for more on registration and execution.

## Full ReAct Agent Loop

Here is a complete offline example that defines tools, registers them, and wires them into a ReAct agent with `ScriptedChatModel`:

```rust,ignore
use std::sync::Arc;
use serde_json::{json, Value};
use synaptic_core::{ChatModel, ChatResponse, Message, Tool, ToolCall, SynapticError};
use synaptic_models::ScriptedChatModel;
use synaptic_graph::{create_react_agent, MessageState};

// 1. Define tools (using the trait)
struct AddTool;

#[async_trait::async_trait]
impl Tool for AddTool {
    fn name(&self) -> &'static str { "add" }
    fn description(&self) -> &'static str { "Add two numbers" }
    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(json!({"result": a + b}))
    }
}

// 2. Script the model to call the tool and then respond
let model: Arc<dyn ChatModel> = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "call_1".into(),
                name: "add".into(),
                arguments: r#"{"a": 3, "b": 4}"#.into(),
            }],
        ),
        usage: None,
    },
    ChatResponse {
        message: Message::ai("The sum is 7."),
        usage: None,
    },
]));

// 3. Build the agent
let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(AddTool)];
let agent = create_react_agent(model, tools)?;

// 4. Run it
let state = MessageState::with_messages(vec![
    Message::human("What is 3 + 4?"),
]);
let result = agent.invoke(state).await?.into_state();
assert_eq!(result.messages.last().unwrap().content(), "The sum is 7.");
```

## Tool Definitions for Models

To tell a chat model about available tools, create `ToolDefinition` values and attach them to a `ChatRequest`:

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

The `parameters` field follows the JSON Schema format that LLM providers expect.

## Using the `#[tool]` Macro

Instead of manually implementing the `Tool` trait, you can use the `#[tool]`
attribute macro from `synaptic-macros` to generate the boilerplate:

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

The macro generates the struct, `impl Tool`, JSON Schema from parameter types,
and a factory function — all from a single annotated function. Doc comments on
the function become the tool description; doc comments on parameters become
schema descriptions.

### Optional and Default Parameters

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

### Stateful Tools with `#[field]`

Tools that need to hold state (database connections, API clients, etc.) can use
`#[field]` to create struct fields that are hidden from the LLM schema:

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

For the full macro reference including `#[inject]`, `#[default]`, and middleware
macros, see the [Procedural Macros](../macros.md) page.
