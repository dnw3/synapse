# Tool 节点

`ToolNode` 是一个预构建的图节点，能够自动分发状态中最后一条 AI 消息里的工具调用。它将 `synaptic_tools` crate 的执行基础设施与图系统桥接起来，使构建工具调用代理循环变得简单直接。

## 工作原理

当 `ToolNode` 处理状态时，它会：

1. 读取状态中的**最后一条消息**。
2. 从该消息中提取所有 `tool_calls`（AI 消息携带工具调用请求）。
3. 通过提供的 `SerialToolExecutor` 执行每个工具调用。
4. 为每个工具调用结果追加一条 `Message::tool(result, call_id)` 消息。
5. 返回更新后的状态。

如果最后一条消息没有工具调用，该节点会原样传递状态。

## 设置

通过提供一个已注册工具的 `SerialToolExecutor` 来创建 `ToolNode`：

```rust
use synaptic_graph::ToolNode;
use synaptic_tools::{ToolRegistry, SerialToolExecutor};
use synaptic_core::{Tool, ToolDefinition, SynapticError};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

// 定义一个工具
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calculator".to_string(),
            description: "Evaluates math expressions".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": { "type": "string" }
                },
                "required": ["expression"]
            }),
        }
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let expr = args["expression"].as_str().unwrap_or("0");
        Ok(Value::String(format!("Result: {expr}")))
    }
}

// 注册并创建执行器
let registry = ToolRegistry::new();
registry.register(Arc::new(CalculatorTool)).await?;

let executor = SerialToolExecutor::new(registry);
let tool_node = ToolNode::new(executor);
```

## 在图中使用 ToolNode

`ToolNode` 实现了 `Node<MessageState>`，因此可以直接添加到 `StateGraph` 中：

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, END};
use synaptic_core::{Message, ToolCall};

// 一个产生工具调用的代理节点
let agent = FnNode::new(|mut state: MessageState| async move {
    let tool_call = ToolCall {
        id: "call-1".to_string(),
        name: "calculator".to_string(),
        arguments: serde_json::json!({"expression": "2+2"}),
    };
    state.messages.push(Message::ai_with_tool_calls("", vec![tool_call]));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("agent", agent)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_edge("agent", "tools")
    .add_edge("tools", END)
    .compile()?;

let result = graph.invoke(MessageState::new()).await?.into_state();
// 状态现在包含：
//   [0] 带有 tool_calls 的 AI 消息
//   [1] 包含 "Result: 2+2" 的 Tool 消息
```

## `tools_condition` -- 标准路由函数

Synaptic 提供了一个 `tools_condition` 函数，实现了标准的路由逻辑：如果最后一条消息包含工具调用则返回 `"tools"`，否则返回 `END`。这样就不需要编写自定义路由闭包了：

```rust
use synaptic_graph::{StateGraph, MessageState, tools_condition, END};

let graph = StateGraph::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_conditional_edges("agent", tools_condition)
    .add_edge("tools", "agent")  // 工具结果返回给代理
    .compile()?;
```

## 代理循环模式

在典型的 ReAct 代理中，工具节点将结果反馈给代理节点，由代理决定是继续调用工具还是生成最终答案。使用 `tools_condition` 或条件边来实现这个循环：

```rust
use std::collections::HashMap;
use synaptic_graph::{StateGraph, MessageState, END};

let graph = StateGraph::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_conditional_edges_with_path_map(
        "agent",
        |state: &MessageState| {
            // 如果最后一条消息包含工具调用，则跳转到 tools
            if let Some(msg) = state.last_message() {
                if !msg.tool_calls().is_empty() {
                    return "tools".to_string();
                }
            }
            END.to_string()
        },
        HashMap::from([
            ("tools".to_string(), "tools".to_string()),
            (END.to_string(), END.to_string()),
        ]),
    )
    .add_edge("tools", "agent")  // 工具结果返回给代理
    .compile()?;
```

这正是 `create_react_agent()` 内部自动实现的模式（内部使用 `tools_condition`）。

## `create_react_agent`

为了方便使用，Synaptic 提供了一个工厂函数来组装标准的 ReAct 代理图：

```rust
use synaptic_graph::create_react_agent;

let graph = create_react_agent(model, tools);
```

它会创建一个包含 `"agent"` 和 `"tools"` 节点的已编译图，并通过条件循环将它们连接起来，等同于上面展示的手动设置。

## `RuntimeAwareTool` 注入

`ToolNode` 支持 `RuntimeAwareTool` 实例，这些实例通过 `ToolRuntime` 接收当前的图状态、存储引用和工具调用 ID。使用 `with_runtime_tool()` 注册运行时感知工具：

```rust
use synaptic_graph::ToolNode;
use synaptic_core::{RuntimeAwareTool, ToolRuntime};

let tool_node = ToolNode::new(executor)
    .with_store(store)            // 将 store 注入到 ToolRuntime 中
    .with_runtime_tool(my_tool);  // 注册一个 RuntimeAwareTool
```

当使用 `AgentOptions { store: Some(store), .. }` 调用 `create_agent` 时，store 会自动连接到 `ToolNode` 中。
