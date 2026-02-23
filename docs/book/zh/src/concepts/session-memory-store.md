# Session、Memory 与 Store

Synaptic 有三个与"记忆"相关的 crate：`synaptic-store`、`synaptic-memory` 和 `synaptic-session`。它们工作在不同的抽象层级，服务于不同目的。核心设计原则是**三层共享同一个 `Store` 后端**——一个存储引擎，多种视图。本页解释每一层的定位、三者的关系以及何时使用哪个。

## 三层架构

```text
┌─────────────────────────────────────────────────────┐
│  synaptic-session            (会话生命周期)           │
│  SessionManager · .memory() · .checkpointer()       │
│  "哪次对话？能恢复吗？"                               │
├─────────────────────────────────────────────────────┤
│  synaptic-memory             (对话记忆策略)           │
│  ChatMessageHistory · Buffer · Window · Summary      │
│  "记住多少轮？怎么裁剪？"                             │
├─────────────────────────────────────────────────────┤
│  synaptic-store              (通用键值存储)           │
│  InMemoryStore · FileStore                           │
│  "数据存哪？怎么读写？"                               │
└─────────────────────────────────────────────────────┘
       ▲ 三层共享同一个 Store 实例
```

每一层建立在下一层之上。越底层，抽象越通用。与早期版本各层各自维护持久化后端不同，新架构通过命名空间约定将所有数据汇集到 `Store` trait。

## 第一层：Store（数据持久化）

**Crate：** `synaptic-store`

Store 是通用的键值存储层。它不了解对话或 AI——只是按命名空间和键存取 JSON 值。

```rust
use synaptic::store::{InMemoryStore, FileStore};

// 内存存储（开发/测试用）
let store = InMemoryStore::new();

// 文件存储（跨重启持久化）
let store = FileStore::new("/data/myapp");

// 存储任何东西：用户配置、缓存结果、Agent 知识库
store.put(&["users", "alice"], "preferences", item).await?;
let prefs = store.get(&["users", "alice"], "preferences").await?;
```

**核心特征：**
- 命名空间 + 键的寻址方式（类似文件系统路径）
- 任意 JSON 值
- CRUD + search + list_namespaces
- 可选的语义搜索（需配置 embeddings）

**适用场景：** 需要持久化任意数据——用户配置、缓存计算结果、Agent 知识库、跨会话状态。

### 命名空间设计：`&[&str]`

`Store` trait 使用 `namespace: &[&str]`——多级路径，类似 Python LangChain 的 `tuple[str, ...]`。它的工作方式类似文件系统目录路径，但使用借用的字符串切片的切片，这意味着调用端零分配。

```rust
// 两级命名空间：类别 + 会话 ID
store.put(&["memory", "session_abc"], "messages", value).await?;

// 一级命名空间：扁平集合
store.put(&["sessions"], "session_abc", metadata).await?;

// 三级命名空间：更深的层级
store.put(&["agents", "weather-bot", "cache"], "forecast", data).await?;

// 列出某前缀下的所有命名空间
let ns = store.list_namespaces(&["memory"]).await?;
// 返回：[["memory", "session_abc"], ["memory", "session_xyz"], ...]
```

这种设计让每个子系统拥有自己的命名空间前缀，同时共享同一个 store 实例。Synaptic 内置类型使用的命名空间约定如下：

| 命名空间 | 键 | 数据 |
|----------|-----|------|
| `["memory", "{session_id}"]` | `"messages"` | 对话消息（JSON 数组） |
| `["memory", "{session_id}"]` | `"summary"` | 摘要文本（用于摘要策略） |
| `["checkpoints", "{thread_id}"]` | `"{checkpoint_id}"` | Graph 检查点快照 |
| `["sessions"]` | `"{session_id}"` | 会话元数据（id、created_at） |

因为所有数据都通过 `Store` trait，将 `InMemoryStore` 换成 `FileStore`（或未来的 `RedisStore`）只需修改一行代码，即可同时切换 memory、checkpoint 和 session 的持久化后端。

## 第二层：Memory（对话上下文管理）

**Crate：** `synaptic-memory`

Memory 专门管理对话历史。`ChatMessageHistory` 实现了 `MemoryStore` trait，以任意 `Store` 为后端。消息以完整的 serde JSON 序列化，保留 `tool_calls`、`tool_call_id` 及所有消息元数据。

```rust
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let history = ChatMessageHistory::new(store);

// 对话过程中追加消息
history.append("session_1", Message::human("Hello")).await?;

// 加载完整历史
let messages = history.load("session_1").await?;
```

Memory **策略**包装 `ChatMessageHistory`，控制发送给 LLM 的历史消息量：

```rust
use synaptic::memory::{ChatMessageHistory, ConversationWindowMemory};

let history = ChatMessageHistory::new(store.clone());
let memory = ConversationWindowMemory::new(history, 10); // 保留最近 10 轮
let context = memory.load("session_1").await?;
```

**核心特征：**
- 按会话隔离的消息存储，完整保真的 JSON 序列化
- 策略控制发送给 LLM 的内容（Buffer、Window、Summary、TokenBuffer、SummaryBuffer）
- 通过 `ChatMessageHistory` 可使用任何 `Store` 后端

**适用场景：** 需要为 LLM 管理对话上下文——决定保留多少消息、是否摘要旧消息、如何在 token 预算内。

### Memory 与 Store 对比

| 方面 | Store | Memory |
|------|-------|--------|
| **用途** | 通用键值存储 | 对话上下文管理 |
| **键** | 命名空间 + key | Session ID |
| **值类型** | 任意 JSON (`Value`) | `Message` |
| **操作** | CRUD + search + list | Append + load + clear |
| **策略** | 无（原始存储） | Buffer、Window、Summary、TokenBuffer、SummaryBuffer |
| **使用场景** | Agent 知识库、用户配置 | 对话历史、LLM 上下文 |

## 第三层：Session（会话生命周期）

**Crate：** `synaptic-session`

Session 管理整个对话的生命周期——创建、列举、恢复和删除会话。它作为协调者，分发 `ChatMessageHistory` 和 `StoreCheckpointer` 实例，且都由同一个 store 支撑。

```rust
use synaptic::session::SessionManager;
use synaptic::store::FileStore;
use std::sync::Arc;

let store = Arc::new(FileStore::new("/data/myapp"));
let manager = SessionManager::new(store);

// 创建新会话（返回会话 ID）
let session_id = manager.create_session().await?;

// 获取 memory 和 checkpointer —— 都使用同一个 store
let memory = manager.memory();
let checkpointer = manager.checkpointer();

// 通过 memory 追加消息
memory.append(&session_id, Message::human("Hello")).await?;
memory.append(&session_id, Message::ai("Hi there!")).await?;

// 稍后：列出并恢复
let sessions = manager.list_sessions().await?;
let history = memory.load(&session_id).await?;

// 删除会话及所有关联数据
manager.delete_session(&session_id).await?;
```

**核心特征：**
- 会话 CRUD（创建/列举/获取/删除）
- `.memory()` 返回共享同一 store 的 `ChatMessageHistory`
- `.checkpointer()` 返回共享同一 store 的 `StoreCheckpointer`
- `delete_session()` 一次性清理消息、摘要和检查点
- 生成唯一会话 ID（UUID v4）

**适用场景：** 构建 CLI、聊天机器人或多会话应用，用户需要恢复之前的对话。

## 三者如何协作

在统一架构中，三层共享同一个 `Arc<dyn Store>`：

```text
                  Arc<dyn Store>
                  （单一实例）
                  ┌─────────┐
                  │ FileStore│
                  └────┬────┘
           ┌──────────┼──────────┐
           ▼          ▼          ▼
    ChatMessageHistory  StoreCheckpointer  SessionManager
    ["memory", sid]     ["checkpoints", t] ["sessions"]
```

```text
用户发起对话
    │
    ▼
SessionManager.create_session()              ← Session 层：生命周期管理
    │
    ▼
manager.memory().load(session_id)            ← Memory 层：上下文策略
    │
    ▼
Store.get(&["memory", sid], "messages")      ← Store 层：数据持久化
    │
    ▼
LLM.chat(context_messages)                  ← AI 模型
    │
    ▼
manager.memory().append(session_id, msg)     ← Memory 层：保存新消息
    (store.put 自动调用)                     ← Store 层：持久化数据
```

### 示例：构建持久化聊天 Agent

```rust,ignore
use synaptic::session::SessionManager;
use synaptic::memory::ConversationWindowMemory;
use synaptic::store::FileStore;
use std::sync::Arc;

// 一个 store 服务所有层
let store = Arc::new(FileStore::new("/data/myapp"));

// Session 管理器协调生命周期
let sessions = SessionManager::new(store.clone());
let session_id = sessions.create_session().await?;

// Memory 策略包装 store 支撑的 history
let memory = ConversationWindowMemory::new(sessions.memory(), 20);

// 对话循环
loop {
    let user_input = read_input();

    // 通过 Memory 策略加载上下文
    let mut context = memory.load(&session_id).await?;
    context.push(Message::human(&user_input));

    // 调用 LLM
    let response = model.chat(ChatRequest::new(context)).await?;

    // 保存到 memory —— 策略决定保留什么，
    // store 以完整保真 JSON 持久化
    memory.append(&session_id, Message::human(&user_input)).await?;
    memory.append(&session_id, response.message.clone()).await?;
}
```

因为 memory 和 session 共享同一个 store，所以不存在数据重复。Memory 策略控制 LLM 看到什么，而 store 以完整保真的 JSON 保留完整的消息历史（包括 `tool_calls`、`tool_call_id` 及所有元数据）。

## 何时用什么

| 场景 | 使用 |
|------|------|
| 跨会话存储用户偏好 | **Store** (`FileStore`) |
| 保留最近 10 条消息给 LLM | **Memory** (`ConversationWindowMemory`) |
| 重启后恢复对话 | **Session** (`SessionManager`) |
| 缓存工具执行结果 | **Store** (`InMemoryStore`) |
| 摘要旧消息以节省 token | **Memory** (`ConversationSummaryMemory`) |
| 列出所有历史对话 | **Session** (`SessionManager::list_sessions`) |
| 存储嵌入用于语义搜索 | **Store**（配合 embeddings） |
| 按会话持久化 Graph 检查点 | **Graph** (`StoreCheckpointer`) |

## 相关概念

- **Condenser**（`synaptic-condenser`）— 工作在 Memory 层，提供额外的上下文压缩策略（滚动、token 预算、LLM 摘要、流水线）。可以理解为"增强版的 Memory 策略"，通过中间件组合使用。

- **Graph Checkpointing**（`synaptic-graph::StoreCheckpointer`）— 持久化 Graph 执行状态（最后执行到哪个节点、完整的状态快照）到共享 store 的命名空间 `["checkpoints", "{thread_id}"]` 下。

## 参见

- [记忆](memory.md) — 记忆策略详解
- [键值存储](store.md) — Store trait 和操作
- [会话管理](../how-to/session.md) — 使用指南
- [文件持久化](../how-to/persistence.md) — FileStore 使用
- [上下文压缩](../how-to/condenser.md) — Condenser 策略
