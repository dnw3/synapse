# HumanInTheLoopMiddleware

在工具执行前暂停以请求人工审批。当某些工具调用（如数据库写入、支付、部署）需要人工监督时，可以使用此 Middleware。

## 构造函数

根据审批范围有两种构造方式：

```rust,ignore
use synaptic_middleware::HumanInTheLoopMiddleware;

// Require approval for ALL tool calls
let mw = HumanInTheLoopMiddleware::new(callback);

// Require approval only for specific tools
let mw = HumanInTheLoopMiddleware::for_tools(
    callback,
    vec!["delete_record".to_string(), "send_email".to_string()],
);
```

### ApprovalCallback Trait

你需要实现 `ApprovalCallback` trait 来定义如何获取审批：

```rust,ignore
use synaptic_middleware::ApprovalCallback;

struct CliApproval;

#[async_trait]
impl ApprovalCallback for CliApproval {
    async fn approve(&self, tool_name: &str, arguments: &Value) -> Result<bool, SynapticError> {
        println!("Tool '{}' wants to run with args: {}", tool_name, arguments);
        println!("Approve? (y/n)");
        // Read user input and return true/false
        Ok(true)
    }
}
```

返回 `Ok(true)` 表示批准，`Ok(false)` 表示拒绝（模型会收到一条拒绝消息），或返回 `Err(...)` 来中止整个 Agent 运行。

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::HumanInTheLoopMiddleware;

let approval = Arc::new(CliApproval);
let hitl = HumanInTheLoopMiddleware::for_tools(
    approval,
    vec!["delete_record".to_string()],
);

let options = AgentOptions {
    middleware: vec![Arc::new(hitl)],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `wrap_tool_call`
- 当工具调用到达时，Middleware 检查是否需要审批：
  - 如果使用 `new()` 构造，所有工具都需要审批。
  - 如果使用 `for_tools()` 构造，只有指定的工具需要审批。
- 对于需要审批的工具，调用 `callback.approve(tool_name, arguments)`。
- 如果批准（`true`），工具调用通过 `next.call(request)` 正常执行。
- 如果拒绝（`false`），Middleware 返回一条 `Value::String` 消息说明调用被拒绝。该消息作为工具结果反馈给模型，使其可以调整计划。

## 示例：带日志的选择性审批

```rust,ignore
struct AuditApproval {
    auto_approve: HashSet<String>,
}

#[async_trait]
impl ApprovalCallback for AuditApproval {
    async fn approve(&self, tool_name: &str, arguments: &Value) -> Result<bool, SynapticError> {
        if self.auto_approve.contains(tool_name) {
            tracing::info!("Auto-approved: {}", tool_name);
            return Ok(true);
        }
        tracing::warn!("Requires manual approval: {} with {:?}", tool_name, arguments);
        // In production, this could send a Slack message, webhook, etc.
        Ok(false) // reject by default until approved
    }
}
```

这种模式允许你自动批准安全的操作，同时对危险操作进行把关。
