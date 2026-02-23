# 文件持久化

Synaptic 通过 `FileStore`（`Store` trait）和 `StoreCheckpointer`（基于任意 `Store` 的 `Checkpointer` trait）提供文件系统持久化支持。

## FileStore

`FileStore` 以目录为基础的布局实现 `Store` trait。每个条目存储为一个 JSON 文件。

**Feature flag**：`store-filesystem`

### 目录布局

```
{root}/{namespace...}/{key}.json
```

例如，`store.put(&["users", "prefs"], "theme", json!("dark"))` 会写入 `{root}/users/prefs/theme.json`。

### 基本用法

```rust,ignore
use synaptic::store::FileStore;

let store = FileStore::new("/tmp/my-store");

// 写入
store.put(&["app", "settings"], "theme", json!("dark")).await?;

// 读取
let item = store.get(&["app", "settings"], "theme").await?;

// 搜索（对 key 和 value 进行子串匹配）
let results = store.search(&["app"], Some("theme"), 10).await?;

// 删除
store.delete(&["app", "settings"], "theme").await?;

// 列出命名空间
let namespaces = store.list_namespaces(&["app"]).await?;
```

### 配合嵌入模型

`FileStore` 支持可选的嵌入模型用于语义搜索，与 `InMemoryStore` 一致。

```rust,ignore
use synaptic::store::FileStore;
use synaptic::openai::OpenAiEmbeddings;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = FileStore::new("/tmp/my-store").with_embeddings(embeddings);
```

## StoreCheckpointer

`StoreCheckpointer` 基于任意 `Store` 实现 `Checkpointer` trait。它取代了旧的 `FileSaver`，采用统一的、后端无关的方式——同一个 checkpointer 可以配合 `InMemoryStore`、`FileStore`、`RedisStore` 或任何其他 `Store` 实现使用。

**Feature flag**：`store-filesystem`（当使用 `FileStore` 作为后端存储时）

### 工作原理

检查点存储在命名空间 `["checkpoints", "{thread_id}"]` 下，以检查点 ID 作为 key。检查点 ID 基于时间戳的十六进制格式，因此字母序即为时间序。

当以 `FileStore` 为后端时，磁盘布局为：

```
{root}/checkpoints/{thread_id}/{checkpoint_id}.json
```

### 用法

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{StoreCheckpointer, CheckpointConfig};
use synaptic::store::FileStore;

let store = Arc::new(FileStore::new("/tmp/my-data"));
let checkpointer = StoreCheckpointer::new(store);

// 与编译后的 Graph 一起使用
let graph = builder.compile_with_checkpointer(Arc::new(checkpointer))?;

let config = CheckpointConfig::new("thread-1");
let result = graph.invoke_with_config(state, config).await?;
```

### 手动检查点操作

```rust,ignore
use synaptic::graph::{Checkpointer, CheckpointConfig};

let config = CheckpointConfig::new("thread-1");

// 获取最新的检查点
let latest = checkpointer.get(&config).await?;

// 列出某个线程的所有检查点
let all = checkpointer.list(&config).await?;
```

### 统一命名空间

由于 `StoreCheckpointer` 以普通 `Store` 为后端，同一个 `FileStore` 实例可以同时处理记忆、检查点和会话。每个子系统使用不同的命名空间前缀：

```rust,ignore
use std::sync::Arc;
use synaptic::store::FileStore;
use synaptic::graph::StoreCheckpointer;

let store = Arc::new(FileStore::new("/tmp/my-data"));

// 检查点存储在 {root}/checkpoints/{thread_id}/{id}.json
let checkpointer = StoreCheckpointer::new(store.clone());

// 应用数据存储在 {root}/app/settings/{key}.json
store.put(&["app", "settings"], "theme", json!("dark")).await?;
```

这种单 Store 方式无需为不同用途配置多个目录，所有持久化状态集中在同一位置。

## Cargo.toml

```toml
[dependencies]
synaptic = { version = "0.3", features = ["store-filesystem", "graph"] }
```

## 从 FileSaver 迁移

如果你之前使用了 `graph-filesystem` feature 下的 `FileSaver`，请切换到基于 `FileStore` 的 `StoreCheckpointer`：

| 之前 | 之后 |
|------|------|
| `use synaptic::graph::FileSaver` | `use synaptic::graph::StoreCheckpointer` |
| `FileSaver::new("/tmp/checkpoints")` | `StoreCheckpointer::new(Arc::new(FileStore::new("/tmp/data")))` |
| Feature：`graph-filesystem` | Feature：`store-filesystem` + `graph` |

检查点数据格式相同，已有的检查点文件保持兼容。

`FileStore` 和 `StoreCheckpointer` 适用于单进程部署。对于分布式系统，建议使用基于数据库的 `Store` 实现，如 `RedisStore`。
