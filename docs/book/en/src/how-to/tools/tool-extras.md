# Tool Definition Extras

The `extras` field on `ToolDefinition` carries provider-specific parameters that fall outside the standard name/description/parameters schema, such as Anthropic's `cache_control` or any custom metadata your provider adapter needs.

## The `extras` Field

```rust,ignore
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    /// Provider-specific parameters (e.g., Anthropic's `cache_control`).
    pub extras: Option<HashMap<String, Value>>,
}
```

When `extras` is `None` (the default), no additional fields are serialized. Provider adapters inspect `extras` during request building and map recognized keys into the provider's wire format.

## Setting Extras on a Tool Definition

Build a `ToolDefinition` with extras by populating the field directly:

```rust,ignore
use std::collections::HashMap;
use serde_json::{json, Value};
use synaptic_core::ToolDefinition;

let mut extras = HashMap::new();
extras.insert("cache_control".to_string(), json!({"type": "ephemeral"}));

let tool_def = ToolDefinition {
    name: "search".to_string(),
    description: "Search the web".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" }
        },
        "required": ["query"]
    }),
    extras: Some(extras),
};
```

## Common Use Cases

**Anthropic prompt caching** -- Anthropic supports a `cache_control` field on tool definitions to enable prompt caching for tool schemas that rarely change:

```rust,ignore
let mut extras = HashMap::new();
extras.insert("cache_control".to_string(), json!({"type": "ephemeral"}));

let def = ToolDefinition {
    name: "lookup".to_string(),
    description: "Look up a record".to_string(),
    parameters: json!({"type": "object", "properties": {}}),
    extras: Some(extras),
};
```

**Custom metadata** -- You can attach arbitrary key-value pairs for your own adapter logic:

```rust,ignore
let mut extras = HashMap::new();
extras.insert("priority".to_string(), json!("high"));
extras.insert("timeout_ms".to_string(), json!(5000));

let def = ToolDefinition {
    name: "deploy".to_string(),
    description: "Deploy the service".to_string(),
    parameters: json!({"type": "object", "properties": {}}),
    extras: Some(extras),
};
```

## Extras in Tool Implementations

When implementing the `Tool` trait, return extras from `as_tool_definition()`:

```rust,ignore
use synaptic_core::Tool;

impl Tool for MyTool {
    fn name(&self) -> &'static str { "my_tool" }
    fn description(&self) -> &'static str { "Does something" }

    fn as_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters().unwrap_or(json!({"type": "object", "properties": {}})),
            extras: Some(HashMap::from([
                ("cache_control".to_string(), json!({"type": "ephemeral"})),
            ])),
        }
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        Ok(json!("done"))
    }
}
```
