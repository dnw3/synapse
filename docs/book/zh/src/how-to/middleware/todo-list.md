# TodoListMiddleware

通过维护共享的待办事项列表，将任务规划状态注入 Agent 的上下文中。当你的 Agent 执行多步骤操作并希望它在多次模型调用之间跟踪进度时，可以使用此 Middleware。

## 构造函数

```rust,ignore
use synaptic_middleware::TodoListMiddleware;

let mw = TodoListMiddleware::new();
```

### 管理任务

该 Middleware 提供异步方法来以编程方式添加和完成任务：

```rust,ignore
let mw = TodoListMiddleware::new();

// Add tasks before or during agent execution
let id1 = mw.add("Research competitor pricing").await;
let id2 = mw.add("Draft summary report").await;

// Mark tasks as done
mw.complete(id1).await;

// Inspect current state
let items = mw.items().await;
```

每个任务都有一个自动递增的唯一 ID。任务包含 `id`、`task`（描述）和 `done`（完成状态）。

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::TodoListMiddleware;

let todo = Arc::new(TodoListMiddleware::new());
todo.add("Gather user requirements").await;
todo.add("Generate implementation plan").await;
todo.add("Write code").await;

let options = AgentOptions {
    middleware: vec![todo.clone()],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `before_model`
- 在每次模型调用前，Middleware 检查当前的待办事项列表。
- 如果列表非空，则在请求的消息列表开头插入一条包含格式化任务列表的系统消息。
- 模型可以看到所有任务的当前状态，包括哪些已完成。

注入的消息格式如下：

```text
Current TODO list:
  [ ] #1: Gather user requirements
  [x] #2: Generate implementation plan
  [ ] #3: Write code
```

这使模型能够了解整体计划和进度，从而有条不紊地完成各项任务。你可以在工具实现或外部代码中调用 `complete()` 来在 Agent 步骤之间更新进度。

## 示例：通过工具驱动的任务完成

与标记任务完成的自定义工具组合使用：

```rust,ignore
let todo = Arc::new(TodoListMiddleware::new());
todo.add("Fetch data from API").await;
todo.add("Transform data").await;
todo.add("Save results").await;

// The agent sees the todo list in its context and can
// reason about which tasks remain. Your tools can call
// todo.complete(id) when they finish their work.
```
