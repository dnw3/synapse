# Handoff Tools

Handoff Tools 用于表示将对话从一个 Agent 转移到另一个 Agent 的意图。

## create_handoff_tool

`create_handoff_tool` 函数创建一个 `Tool`，当被调用时返回一条转移消息。该工具被命名为 `transfer_to_<agent_name>`，路由逻辑使用这个命名约定来检测 Handoff。

```rust,ignore
use synaptic_graph::create_handoff_tool;

let handoff = create_handoff_tool("billing", "Transfer to the billing specialist");
// handoff.name()        => "transfer_to_billing"
// handoff.description() => "Transfer to the billing specialist"
```

当被调用时，该工具返回：

```json
"Transferring to agent 'billing'."
```

## 在自定义 Agent 中使用 Handoff Tools

你可以在构建 Agent 时将 Handoff Tools 与常规工具一起注册：

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, create_handoff_tool, AgentOptions};

let escalate = create_handoff_tool("human_review", "Escalate to a human reviewer");

let mut all_tools: Vec<Arc<dyn Tool>> = my_tools;
all_tools.push(escalate);

let agent = create_agent(model, all_tools, AgentOptions::default())?;
```

模型会将 `transfer_to_human_review` 视为可用工具。当它决定调用该工具时，你的图的条件边可以检测到 Handoff 并相应地进行路由。

## 构建自定义拓扑

对于不适合 Supervisor 或 Swarm 模式的工作流，可以将 Handoff Tools 与手动构建的 `StateGraph` 结合使用：

```rust,ignore
use std::collections::HashMap;
use synaptic_graph::{
    create_handoff_tool, StateGraph, FnNode, MessageState, END,
};

// Create handoff tools
let to_reviewer = create_handoff_tool("reviewer", "Send to reviewer");
let to_publisher = create_handoff_tool("publisher", "Send to publisher");

// Build nodes (agent_node, reviewer_node, publisher_node defined elsewhere)

let graph = StateGraph::new()
    .add_node("drafter", drafter_node)
    .add_node("reviewer", reviewer_node)
    .add_node("publisher", publisher_node)
    .set_entry_point("drafter")
    .add_conditional_edges("drafter", |state: &MessageState| {
        if let Some(last) = state.last_message() {
            for tc in last.tool_calls() {
                if tc.name == "transfer_to_reviewer" {
                    return "reviewer".to_string();
                }
                if tc.name == "transfer_to_publisher" {
                    return "publisher".to_string();
                }
            }
        }
        END.to_string()
    })
    .add_edge("reviewer", "drafter")
    .add_edge("publisher", END)
    .compile()?;
```

## 命名约定

Handoff 工具始终命名为 `transfer_to_<agent_name>`。`create_supervisor` 和 `create_swarm` 在内部路由时都依赖此约定。如果你构建自定义拓扑，请在条件边中匹配相同的模式。

## 注意事项

- Handoff Tools 不接受参数。模型使用空对象 `{}` 调用它们。
- 工具本身只返回一条字符串消息 -- 实际的路由由图的条件边处理，而不是由工具执行来处理。
- 你可以为每个 Agent 创建多个 Handoff Tools，以构建复杂的路由图（例如，一个 Agent 可以移交给三个不同的专家）。
