# 会话管理

`synaptic::session` 模块提供基于 Store 的会话生命周期管理。所有会话数据——元数据、消息和 Graph 检查点——都存储在同一个 `Store` 中，便于切换后端（内存、文件系统或任何自定义实现）。

## 配置

添加 `session` feature（自动引入 `graph`、`memory` 和 `store`）：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["session"] }
```

如需文件系统持久化，还需启用 `store-filesystem`：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["session", "store-filesystem"] }
```

## SessionManager

`SessionManager` 是核心入口。使用任意 `Arc<dyn Store>` 构造：

```rust,ignore
use std::sync::Arc;
use synaptic::session::SessionManager;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let manager = SessionManager::new(store);
```

### 创建会话

每个会话会分配一个唯一的 UUID。元数据持久化在 Store 的 `["sessions"]` 命名空间下。

```rust,ignore
let session_id = manager.create_session().await?;
println!("会话 ID: {session_id}");
```

### 列出会话

返回按创建时间排序的所有会话。

```rust,ignore
let sessions = manager.list_sessions().await?;
for info in &sessions {
    println!("{} (创建时间: {})", info.id, info.created_at);
}
```

### 获取会话

通过 ID 获取单个会话的元数据：

```rust,ignore
if let Some(info) = manager.get_session(&session_id).await? {
    println!("找到会话: {}", info.id);
}
```

### 删除会话

`delete_session` 会删除与该会话关联的**所有**数据：元数据、消息、摘要和检查点。

```rust,ignore
manager.delete_session(&session_id).await?;
```

## SessionInfo

`SessionInfo` 是为每个会话存储的元数据结构体：

| 字段         | 类型     | 描述                        |
|------------- |--------- |---------------------------- |
| `id`         | `String` | 唯一会话标识符（UUID）       |
| `created_at` | `String` | ISO 格式的创建时间戳         |

## 共享 Store 访问

核心设计原则是 `SessionManager`、`ChatMessageHistory` 和 `StoreCheckpointer` 共享**同一个**底层 Store。这意味着单个 Store 即可处理会话的所有数据。

### 消息接口

调用 `.memory()` 获取一个与同一 Store 关联的 `ChatMessageHistory`。用它来追加和加载任意会话的消息：

```rust,ignore
use synaptic::core::Message;

let memory = manager.memory();

// 追加消息
memory.append(&session_id, Message::human("你好")).await?;
memory.append(&session_id, Message::ai("你好！有什么可以帮助你的？")).await?;

// 加载对话历史
let messages = memory.load(&session_id).await?;
assert_eq!(messages.len(), 2);
```

### 检查点接口

调用 `.checkpointer()` 获取一个 `StoreCheckpointer`，用于 `CompiledGraph`：

```rust,ignore
use std::sync::Arc;

let checkpointer = manager.checkpointer();

// 传递给 Graph 编译
let graph = builder.compile_with_checkpointer(Arc::new(checkpointer))?;
```

### 底层 Store

需要时可直接访问原始 Store 引用：

```rust,ignore
let store = manager.store();
```

## 完整示例

```rust,ignore
use std::sync::Arc;
use synaptic::core::{Message, Store};
use synaptic::session::SessionManager;
use synaptic::store::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建基于 Store 的会话管理器
    let store = Arc::new(InMemoryStore::new());
    let manager = SessionManager::new(store);

    // 创建会话
    let session_id = manager.create_session().await?;

    // 使用消息接口存储消息
    let memory = manager.memory();
    memory.append(&session_id, Message::human("什么是 Rust？")).await?;
    memory.append(&session_id, Message::ai("Rust 是一门系统编程语言。")).await?;

    // 加载消息
    let messages = memory.load(&session_id).await?;
    println!("消息数: {}", messages.len());

    // 使用检查点器实现 Graph 状态持久化
    let _checkpointer = manager.checkpointer();

    // 列出所有会话
    let sessions = manager.list_sessions().await?;
    println!("会话总数: {}", sessions.len());

    // 清理——删除元数据、消息和检查点
    manager.delete_session(&session_id).await?;

    Ok(())
}
```

## 使用 FileStore 进行持久化

如需跨进程重启的持久会话，使用 `FileStore` 代替 `InMemoryStore`：

```rust,ignore
use std::sync::Arc;
use synaptic::session::SessionManager;
use synaptic::store::FileStore;

let store = Arc::new(FileStore::new(".sessions").await?);
let manager = SessionManager::new(store);

// 使用方式完全相同——数据持久化到磁盘
let session_id = manager.create_session().await?;
let memory = manager.memory();
memory.append(&session_id, Message::human("你好")).await?;
```

## 数据布局

所有会话数据按 Store 命名空间组织：

| 命名空间                     | 键           | 内容                |
|---------------------------- |------------- |-------------------- |
| `["sessions"]`              | `session_id` | `SessionInfo` JSON  |
| `["memory", session_id]`    | `"messages"` | 消息历史             |
| `["memory", session_id]`    | `"summary"`  | 对话摘要             |
| `["checkpoints", session_id]` | 检查点键    | Graph 状态           |

调用 `delete_session` 时，会删除给定会话 ID 在这些命名空间下的所有条目。
