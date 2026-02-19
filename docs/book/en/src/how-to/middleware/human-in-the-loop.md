# HumanInTheLoopMiddleware

Pauses tool execution to request human approval before proceeding. Use this when certain tool calls (e.g., database writes, payments, deployments) require human oversight.

## Constructor

There are two constructors depending on the scope of approval:

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

You must implement the `ApprovalCallback` trait to define how approval is obtained:

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

Return `Ok(true)` to approve, `Ok(false)` to reject (the model receives a rejection message), or `Err(...)` to abort the entire agent run.

## Usage with `create_agent`

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

## How It Works

- **Lifecycle hook:** `wrap_tool_call`
- When a tool call arrives, the middleware checks whether it requires approval:
  - If constructed with `new()`, all tools require approval.
  - If constructed with `for_tools()`, only the named tools require approval.
- For tools that require approval, it calls `callback.approve(tool_name, arguments)`.
- If approved (`true`), the tool call proceeds normally via `next.call(request)`.
- If rejected (`false`), the middleware returns a `Value::String` message saying the call was rejected. This message is fed back to the model as the tool result, allowing it to adjust its plan.

## Example: Selective Approval with Logging

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

This pattern lets you auto-approve safe operations while gating dangerous ones.
