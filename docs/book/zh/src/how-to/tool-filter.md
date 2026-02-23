# 工具过滤

工具过滤器在每个 Agent 轮次中动态控制哪些工具对模型可见。这使得渐进式工具发现、状态机工作流和访问控制成为可能。

## ToolFilter Trait

```rust,ignore
use synaptic::tools::{ToolFilter, FilterContext};

pub trait ToolFilter: Send + Sync {
    fn filter(&self, tools: Vec<ToolDefinition>, context: &FilterContext) -> Vec<ToolDefinition>;
}
```

### FilterContext

提供给每个过滤器的上下文包括当前轮次计数、上次调用的工具以及任意元数据。

```rust,ignore
pub struct FilterContext {
    pub turn_count: usize,
    pub last_tool: Option<String>,
    pub metadata: HashMap<String, Value>,
}
```

## 内置过滤器

### AllowListFilter

仅名称在允许列表中的工具可见。

```rust,ignore
use synaptic::tools::AllowListFilter;

let filter = AllowListFilter::new(["search", "read_file"]);
```

### DenyListFilter

按名称移除特定工具。

```rust,ignore
use synaptic::tools::DenyListFilter;

let filter = DenyListFilter::new(["delete_file", "execute_code"]);
```

### StateMachineFilter

根据状态转换和轮次计数控制工具可用性。

```rust,ignore
use synaptic::tools::StateMachineFilter;

let filter = StateMachineFilter::new()
    // 调用 "search" 后，仅 "read_file" 和 "summarize" 可用
    .after_tool("search", ["read_file", "summarize"])
    // "deploy" 在 3 轮后才可用
    .turn_threshold(3, ["deploy"]);
```

`after_tool` 规则根据上次调用的工具限制下一步可用的工具。`turn_threshold` 规则将工具限定在最低轮次计数之后才可用。

### CompositeFilter

按顺序应用多个过滤器。每个过滤器接收上一个的输出。

```rust,ignore
use synaptic::tools::CompositeFilter;

let filter = CompositeFilter::new(vec![
    Box::new(DenyListFilter::new(["dangerous_tool"])),
    Box::new(StateMachineFilter::new()
        .turn_threshold(5, ["advanced_tool"])),
]);
```

## 自定义过滤器

实现 `ToolFilter` trait 以支持自定义逻辑。

```rust,ignore
struct RoleBasedFilter { role: String }

impl ToolFilter for RoleBasedFilter {
    fn filter(&self, tools: Vec<ToolDefinition>, _ctx: &FilterContext) -> Vec<ToolDefinition> {
        tools.into_iter().filter(|t| {
            // 自定义访问控制逻辑
            !t.name.starts_with("admin_") || self.role == "admin"
        }).collect()
    }
}
```
