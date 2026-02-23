# Tool Filtering

Tool filters dynamically control which tools are visible to the model at each agent turn. This enables progressive tool disclosure, state-machine workflows, and access control.

## ToolFilter Trait

```rust,ignore
use synaptic::tools::{ToolFilter, FilterContext};

pub trait ToolFilter: Send + Sync {
    fn filter(&self, tools: Vec<ToolDefinition>, context: &FilterContext) -> Vec<ToolDefinition>;
}
```

### FilterContext

The context provided to each filter includes the current turn count, the last tool called, and arbitrary metadata.

```rust,ignore
pub struct FilterContext {
    pub turn_count: usize,
    pub last_tool: Option<String>,
    pub metadata: HashMap<String, Value>,
}
```

## Built-in Filters

### AllowListFilter

Only tools whose names appear in the allow list are visible.

```rust,ignore
use synaptic::tools::AllowListFilter;

let filter = AllowListFilter::new(["search", "read_file"]);
```

### DenyListFilter

Removes specific tools by name.

```rust,ignore
use synaptic::tools::DenyListFilter;

let filter = DenyListFilter::new(["delete_file", "execute_code"]);
```

### StateMachineFilter

Controls tool availability based on state transitions and turn count.

```rust,ignore
use synaptic::tools::StateMachineFilter;

let filter = StateMachineFilter::new()
    // After "search" is called, only "read_file" and "summarize" are available
    .after_tool("search", ["read_file", "summarize"])
    // "deploy" becomes available only after 3 turns
    .turn_threshold(3, ["deploy"]);
```

The `after_tool` rule restricts the next available tools based on the last tool called. The `turn_threshold` rule gates tools behind a minimum turn count.

### CompositeFilter

Applies multiple filters in sequence. Each filter receives the output of the previous one.

```rust,ignore
use synaptic::tools::CompositeFilter;

let filter = CompositeFilter::new(vec![
    Box::new(DenyListFilter::new(["dangerous_tool"])),
    Box::new(StateMachineFilter::new()
        .turn_threshold(5, ["advanced_tool"])),
]);
```

## Custom Filters

Implement the `ToolFilter` trait for custom logic.

```rust,ignore
struct RoleBasedFilter { role: String }

impl ToolFilter for RoleBasedFilter {
    fn filter(&self, tools: Vec<ToolDefinition>, _ctx: &FilterContext) -> Vec<ToolDefinition> {
        tools.into_iter().filter(|t| {
            // Custom access control logic
            !t.name.starts_with("admin_") || self.role == "admin"
        }).collect()
    }
}
```
