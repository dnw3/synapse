# 图持久化检查点

默认情况下，可以使用 [`StoreCheckpointer`] 搭配 [`InMemoryStore`] 后端，将图状态存储在进程内存中。这意味着进程重启后状态会丢失——不适合生产环境。

Synaptic 提供四种持久化检查点后端：

| 后端 | Crate | 适用场景 |
|------|-------|----------|
| Redis | `synaptic-redis` | 低延迟、支持 TTL 自动过期 |
| PostgreSQL | `synaptic-pgvector` | 关系型工作负载、ACID 保证 |
| SQLite | `synaptic-sqlite` | 单机部署、无需外部服务 |
| MongoDB | `synaptic-mongodb` | 分布式部署、文档型存储 |

## 设置

在 `Cargo.toml` 中添加对应的依赖：

```toml
# Redis 检查点
[dependencies]
synaptic = { version = "0.2", features = ["agent", "redis"] }
synaptic-redis = { version = "0.2" }

# PostgreSQL 检查点
synaptic = { version = "0.2", features = ["agent", "pgvector"] }
synaptic-pgvector = { version = "0.2" }

# SQLite 检查点（无需外部服务）
synaptic = { version = "0.2", features = ["agent", "sqlite"] }
synaptic-sqlite = { version = "0.2" }

# MongoDB 检查点
synaptic = { version = "0.2", features = ["agent", "mongodb"] }
synaptic-mongodb = { version = "0.2" }
```

## Redis 检查点

### 快速开始

```rust,ignore
use synaptic_redis::{RedisCheckpointer, RedisCheckpointerConfig};
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// 连接到 Redis
let checkpointer = RedisCheckpointer::from_url("redis://127.0.0.1/").await?;

// 使用持久化检查点构建图
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));

// 使用 thread_id 运行以实现持久化
let state = MessageState { messages: vec![Message::human("你好")] };
let config = RunnableConfig::default().with_metadata("thread_id", "user-123");
let result = graph.invoke_with_config(state, config).await?;
```

### 配置

```rust,ignore
use synaptic_redis::RedisCheckpointerConfig;

let config = RedisCheckpointerConfig::new("redis://127.0.0.1/")
    .with_ttl(86400)          // 检查点 24 小时后过期
    .with_prefix("myapp");    // 自定义键前缀（默认："synaptic"）

let checkpointer = RedisCheckpointer::new(config).await?;
```

### 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `url` | `String` | 必填 | Redis 连接 URL |
| `ttl` | `Option<u64>` | `None` | 检查点键的 TTL（秒） |
| `prefix` | `String` | `"synaptic"` | 所有检查点键的键前缀 |

### 键方案

Redis 使用以下键方案存储检查点：

- **检查点数据**：`{prefix}:checkpoint:{thread_id}:{checkpoint_id}` — JSON 序列化的 `Checkpoint`
- **线程索引**：`{prefix}:idx:{thread_id}` — 按时间顺序排列的检查点 ID Redis LIST

## PostgreSQL 检查点

### 快速开始

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic_pgvector::PgCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// 创建连接池
let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

// 创建并初始化检查点（若表不存在则自动创建）
let checkpointer = PgCheckpointer::new(pool);
checkpointer.initialize().await?;

// 构建图
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));
```

### 数据库 Schema

`initialize()` 会在表不存在时创建以下表：

```sql
CREATE TABLE IF NOT EXISTS synaptic_checkpoints (
    thread_id     TEXT        NOT NULL,
    checkpoint_id TEXT        NOT NULL,
    state         JSONB       NOT NULL,
    next_node     TEXT,
    parent_id     TEXT,
    metadata      JSONB       NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (thread_id, checkpoint_id)
);
```

### 自定义表名

```rust,ignore
let checkpointer = PgCheckpointer::new(pool)
    .with_table("my_custom_checkpoints");
checkpointer.initialize().await?;
```

## SQLite 检查点

`SqliteCheckpointer` 将检查点存储在本地 SQLite 数据库中。无需外部服务，适合单机部署、命令行工具和开发环境。

### 快速开始

```rust,ignore
use synaptic_sqlite::SqliteCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// 基于文件的持久化存储（重启后仍保留）
let checkpointer = SqliteCheckpointer::new("/var/lib/myapp/checkpoints.db")?;

// 构建图
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));

let state = MessageState { messages: vec![Message::human("你好")] };
let config = RunnableConfig::default().with_metadata("thread_id", "user-123");
let result = graph.invoke_with_config(state, config).await?;
```

### 内存模式（适合测试）

```rust,ignore
use synaptic_sqlite::SqliteCheckpointer;

let checkpointer = SqliteCheckpointer::in_memory()?;
```

### 数据库 Schema

`SqliteCheckpointer::new()` 自动创建两张表：

```sql
-- 检查点状态存储
CREATE TABLE IF NOT EXISTS synaptic_checkpoints (
    thread_id     TEXT    NOT NULL,
    checkpoint_id TEXT    NOT NULL,
    state         TEXT    NOT NULL,  -- JSON 序列化的 Checkpoint
    created_at    INTEGER NOT NULL,  -- Unix 时间戳
    PRIMARY KEY (thread_id, checkpoint_id)
);

-- 用于最新/列表查询的有序索引
CREATE TABLE IF NOT EXISTS synaptic_checkpoint_idx (
    thread_id     TEXT    NOT NULL,
    checkpoint_id TEXT    NOT NULL,
    seq           INTEGER NOT NULL,  -- 每个线程单调递增
    PRIMARY KEY (thread_id, checkpoint_id)
);
```

### 注意事项

- 使用内置 `rusqlite`（`bundled` feature）——无需系统安装 `libsqlite3`。
- 异步操作使用 `tokio::task::spawn_blocking`，不阻塞异步运行时。
- `PUT` 具有幂等性：相同的 `checkpoint_id` 重复插入会替换数据，但不会在索引中新增重复条目。

## MongoDB 检查点

`MongoCheckpointer` 将检查点存储在 MongoDB 中，适合多进程共享状态的分布式部署。

### 快速开始

```rust,ignore
use synaptic_mongodb::MongoCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

let client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
let db = client.database("myapp");
let checkpointer = MongoCheckpointer::new(&db, "graph_checkpoints").await?;

let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));

let state = MessageState { messages: vec![Message::human("你好")] };
let config = RunnableConfig::default().with_metadata("thread_id", "user-123");
let result = graph.invoke_with_config(state, config).await?;
```

### 文档结构

每个检查点存储为一个 MongoDB 文档：

```json
{
  "thread_id":     "user-123",
  "checkpoint_id": "18f4a2b1-0001",
  "seq":           0,
  "state":         "{...序列化的 Checkpoint JSON...}",
  "created_at":    { "$date": "2026-02-22T00:00:00Z" }
}
```

自动创建两个索引：

- **唯一索引**（`thread_id`, `checkpoint_id`）——确保写入幂等性。
- **复合索引**（`thread_id`, `seq`）——用于 `list()` 有序查询和 `get()` 获取最新检查点。

### 注意事项

- 兼容 MongoDB Atlas 和自托管 MongoDB 5.0+。
- `put()` 使用 upsert 语义：相同检查点 ID 的重复写入是安全的。
- 无 `checkpoint_id` 的 `get()` 返回 `seq` 最大的文档（即最新检查点）。

## 结合人工干预的持久化工作流

持久化检查点支持有状态的人工干预工作流：

```rust,ignore
use synaptic::graph::{StateGraph, MessageState, StreamMode};
use synaptic_sqlite::SqliteCheckpointer;
use std::sync::Arc;

let checkpointer = Arc::new(SqliteCheckpointer::new("/var/lib/myapp/checkpoints.db")?);

// 编译图，在 "human_review" 节点前中断
let graph = builder
    .interrupt_before(vec!["human_review"])
    .compile_with_checkpointer(checkpointer)?;

// 第一次调用——图在 "human_review" 前暂停
let config = RunnableConfig::default().with_metadata("thread_id", "session-42");
let result = graph.invoke_with_config(initial_state, config.clone()).await?;

// 注入人工反馈并恢复执行
let updated = graph.update_state(config.clone(), feedback_state).await?;
let final_result = graph.invoke_with_config(updated, config).await?;
```

## 时间旅行调试

通过检查点 ID 检索任意历史检查点，用于调试或重放：

```rust,ignore
use synaptic_graph::{CheckpointConfig, Checkpointer};

let config = CheckpointConfig::with_checkpoint_id("thread-123", "specific-checkpoint-id");
if let Some(checkpoint) = checkpointer.get(&config).await? {
    println!("检查点状态：{:?}", checkpoint.state);
}

// 列出线程的所有检查点
let all = checkpointer.list(&CheckpointConfig::new("thread-123")).await?;
println!("总检查点数：{}", all.len());
```

## 选型对比

| 检查点 | 持久化 | 外部依赖 | TTL | 分布式 |
|--------|--------|----------|-----|--------|
| `StoreCheckpointer` | 否（进程内存） | 无 | 否 | 否 |
| `SqliteCheckpointer` | 是（文件） | 无 | 否 | 否 |
| `RedisCheckpointer` | 是 | Redis | 是 | 是 |
| `PgCheckpointer` | 是 | PostgreSQL | 否 | 是 |
| `MongoCheckpointer` | 是 | MongoDB | 否 | 是 |

## 错误处理

```rust,ignore
use synaptic::core::SynapticError;

match checkpointer.get(&config).await {
    Ok(Some(cp)) => println!("已加载检查点：{}", cp.id),
    Ok(None) => println!("未找到检查点"),
    Err(SynapticError::Store(msg)) => eprintln!("存储错误：{msg}"),
    Err(e) => return Err(e.into()),
}
```
