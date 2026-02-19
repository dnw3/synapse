# 节点缓存

`CachePolicy` 与 `add_node_with_cache()` 配合使用，可以在单个图节点上启用基于哈希的结果缓存。当在 TTL 窗口内看到相同的序列化输入状态时，将返回缓存的输出而不重新执行节点。适用于相同输入产生相同输出的高开销节点（LLM 调用、API 请求）。

## 设置

```rust,ignore
use std::time::Duration;
use synaptic_graph::{CachePolicy, FnNode, StateGraph, MessageState, END};
use synaptic_core::Message;

let expensive = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Expensive result"));
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node_with_cache(
        "llm_call",
        expensive,
        CachePolicy::new(Duration::from_secs(60)),
    )
    .add_edge("llm_call", END)
    .set_entry_point("llm_call")
    .compile()?;
```

## 工作原理

1. 在执行缓存节点之前，图将当前状态序列化为 JSON 并计算哈希值。
2. 如果缓存中包含该 `(node_name, state_hash)` 的有效（未过期）条目，将立即返回缓存的 `NodeOutput`——不会调用 `process()`。
3. 缓存未命中时，节点正常执行并存储结果。

缓存保存在 `CompiledGraph` 内部的 `Arc<RwLock<HashMap>>` 中，在同一实例的多次 `invoke()` 调用之间持久存在。

## 示例：验证缓存命中

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use synaptic_core::SynapticError;
use synaptic_graph::{CachePolicy, Node, NodeOutput, State, StateGraph, END};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MyState { counter: usize }

impl State for MyState {
    fn merge(&mut self, other: Self) { self.counter += other.counter; }
}

struct TrackedNode { call_count: Arc<AtomicUsize> }

#[async_trait]
impl Node<MyState> for TrackedNode {
    async fn process(&self, mut state: MyState) -> Result<NodeOutput<MyState>, SynapticError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        state.counter += 1;
        Ok(state.into())
    }
}

let calls = Arc::new(AtomicUsize::new(0));
let graph = StateGraph::new()
    .add_node_with_cache("n", TrackedNode { call_count: calls.clone() },
        CachePolicy::new(Duration::from_secs(60)))
    .add_edge("n", END)
    .set_entry_point("n")
    .compile()?;

// First call: cache miss
graph.invoke(MyState::default()).await?;
assert_eq!(calls.load(Ordering::SeqCst), 1);

// Same input: cache hit -- node not called
graph.invoke(MyState::default()).await?;
assert_eq!(calls.load(Ordering::SeqCst), 1);

// Different input: cache miss
graph.invoke(MyState { counter: 5 }).await?;
assert_eq!(calls.load(Ordering::SeqCst), 2);
```

## TTL 过期

缓存条目在配置的 TTL 后过期。下一次使用相同输入的调用将重新执行节点：

```rust,ignore
let graph = StateGraph::new()
    .add_node_with_cache("n", my_node,
        CachePolicy::new(Duration::from_millis(100)))
    .add_edge("n", END)
    .set_entry_point("n")
    .compile()?;

graph.invoke(state.clone()).await?;                       // executes
tokio::time::sleep(Duration::from_millis(150)).await;
graph.invoke(state.clone()).await?;                       // executes again
```

## 混合缓存和非缓存节点

只有使用 `add_node_with_cache()` 添加的节点会被缓存。使用 `add_node()` 添加的节点始终执行：

```rust,ignore
let graph = StateGraph::new()
    .add_node_with_cache("llm", llm_node, CachePolicy::new(Duration::from_secs(300)))
    .add_node("format", format_node) // always runs
    .set_entry_point("llm")
    .add_edge("llm", "format")
    .add_edge("format", END)
    .compile()?;
```

## 注意事项

- **State 必须实现 `Serialize`。** 缓存键是 JSON 序列化状态的哈希值。
- **缓存作用域。** 缓存存在于 `CompiledGraph` 实例上。新的 `compile()` 从空缓存开始。
- **支持 Command。** 缓存条目存储完整的 `NodeOutput`，包括 `Command` 变体。
