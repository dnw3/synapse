# Tool Definition Extras

`ToolDefinition` 上的 `extras` 字段携带提供商特定的参数，这些参数不在标准的 name/description/parameters schema 范围内，例如 Anthropic 的 `cache_control` 或你的提供商适配器需要的任何自定义元数据。

## `extras` 字段

```rust,ignore
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    /// Provider-specific parameters (e.g., Anthropic's `cache_control`).
    pub extras: Option<HashMap<String, Value>>,
}
```

当 `extras` 为 `None`（默认值）时，不会序列化任何额外字段。提供商适配器在构建请求时会检查 `extras`，并将识别的键映射到提供商的传输格式中。

## 在 Tool Definition 上设置 Extras

通过直接填充字段来构建带有 extras 的 `ToolDefinition`：

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

## 常见用例

**Anthropic prompt caching** -- Anthropic 在工具定义上支持 `cache_control` 字段，用于对不常变化的工具 schema 启用 prompt 缓存：

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

**自定义元数据** -- 你可以为自己的适配器逻辑附加任意键值对：

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

## 在 Tool 实现中使用 Extras

在实现 `Tool` trait 时，从 `as_tool_definition()` 返回 extras：

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
