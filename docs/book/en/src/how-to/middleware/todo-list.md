# TodoListMiddleware

Injects task-planning state into the agent's context by maintaining a shared todo list. Use this when your agent performs multi-step operations and you want it to track progress across model calls.

## Constructor

```rust,ignore
use synaptic_middleware::TodoListMiddleware;

let mw = TodoListMiddleware::new();
```

### Managing Tasks

The middleware provides async methods to add and complete tasks programmatically:

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

Each task gets a unique auto-incrementing ID. Tasks have an `id`, `task` (description), and `done` (completion status).

## Usage with `create_agent`

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

## How It Works

- **Lifecycle hook:** `before_model`
- Before each model call, the middleware checks the current todo list.
- If the list is non-empty, it inserts a system message at the beginning of the request's message list containing the formatted task list.
- The model sees the current state of all tasks, including which ones are done.

The injected message looks like:

```text
Current TODO list:
  [ ] #1: Gather user requirements
  [x] #2: Generate implementation plan
  [ ] #3: Write code
```

This gives the model awareness of the overall plan and progress, enabling it to work through tasks methodically. You can call `complete()` from tool implementations or external code to update progress between agent steps.

## Example: Tool-driven Task Completion

Combine with a custom tool that marks tasks as done:

```rust,ignore
let todo = Arc::new(TodoListMiddleware::new());
todo.add("Fetch data from API").await;
todo.add("Transform data").await;
todo.add("Save results").await;

// The agent sees the todo list in its context and can
// reason about which tasks remain. Your tools can call
// todo.complete(id) when they finish their work.
```
