# Redis 存储与缓存

本指南展示如何使用 Synaptic 的 Redis 集成来实现持久化的键值存储和 LLM 响应缓存。

## 概述

`synaptic_redis` crate 提供两个核心组件：

- **`RedisStore`** -- 实现 `Store` trait，提供持久化的键值存储，可替代 `InMemoryStore`
- **`RedisCache`** -- 实现 `LlmCache` trait，提供持久化的 LLM 响应缓存，可替代 `InMemoryCache`

两者都支持键前缀隔离和通过 URL 连接 Redis 实例。

## Cargo.toml 配置

```toml
[dependencies]
synaptic = { version = "0.3", features = ["redis"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## RedisStore

### 基础使用

通过 Redis URL 创建 Store：

```rust,ignore
use synaptic::redis::RedisStore;
use synaptic::store::Store;

let store = RedisStore::from_url("redis://127.0.0.1:6379").await?;
```

### 读写操作

`RedisStore` 实现了标准的 `Store` trait，用法与 `InMemoryStore` 完全一致：

```rust,ignore
use serde_json::json;
use synaptic::store::Store;

// 写入
store.put(&["users", "prefs"], "theme", json!("dark")).await?;
store.put(&["users", "prefs"], "lang", json!("zh-CN")).await?;

// 读取
let item = store.get(&["users", "prefs"], "theme").await?;
if let Some(item) = item {
    println!("主题: {}", item.value);
}

// 搜索命名空间下的所有条目
let items = store.search(&["users", "prefs"], None, 10).await?;
for item in &items {
    println!("{}: {}", item.key, item.value);
}

// 删除
store.delete(&["users", "prefs"], "theme").await?;

// 列出命名空间
let namespaces = store.list_namespaces(&["users"]).await?;
```

### 配置选项

使用 `RedisStoreConfig` 自定义 Store 行为：

```rust,ignore
use synaptic::redis::{RedisStore, RedisStoreConfig};

let config = RedisStoreConfig {
    prefix: "myapp".to_string(),   // Redis key 前缀
};

let store = RedisStore::from_url_with_config(
    "redis://127.0.0.1:6379",
    config,
).await?;
```

`prefix` 字段为所有 Redis key 添加前缀，避免与同一 Redis 实例上的其他应用冲突。例如，设置 `prefix: "myapp"` 后，命名空间 `["users", "prefs"]` 下的 key `"theme"` 实际存储为 `myapp:users:prefs:theme`。

## RedisCache

### 基础使用

通过 Redis URL 创建 LLM 缓存：

```rust,ignore
use std::sync::Arc;
use synaptic::redis::RedisCache;
use synaptic::cache::CachedChatModel;
use synaptic::core::ChatModel;

let cache = Arc::new(RedisCache::from_url("redis://127.0.0.1:6379").await?);
let base_model: Arc<dyn ChatModel> = Arc::new(model);

let cached_model = CachedChatModel::new(base_model, cache);

// 第一次调用命中 LLM
// let response1 = cached_model.chat(request.clone()).await?;

// 相同请求从 Redis 缓存返回
// let response2 = cached_model.chat(request.clone()).await?;
```

### 配置选项

使用 `RedisCacheConfig` 自定义缓存行为：

```rust,ignore
use std::time::Duration;
use synaptic::redis::{RedisCache, RedisCacheConfig};

let config = RedisCacheConfig {
    prefix: "llm_cache".to_string(),     // Redis key 前缀
    ttl: Some(Duration::from_secs(3600)), // 缓存过期时间：1 小时
};

let cache = RedisCache::from_url_with_config(
    "redis://127.0.0.1:6379",
    config,
).await?;
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `prefix` | `String` | Redis key 前缀，用于隔离不同应用的缓存 |
| `ttl` | `Option<Duration>` | 缓存条目的过期时间。`None` 表示永不过期 |

### TTL 策略选择

- **无 TTL**（`None`）-- 缓存永不过期，适合确定性查询（如 Embedding 生成）
- **短 TTL**（5 -- 30 分钟）-- 适合快速迭代的开发环境
- **中等 TTL**（1 -- 24 小时）-- 适合生产环境中变化不频繁的知识库问答
- **长 TTL**（数天）-- 适合静态内容的处理结果

## 常见模式

### 替换 InMemoryStore

将现有的 `InMemoryStore` 无缝替换为 `RedisStore`：

```rust,ignore
use std::sync::Arc;
use synaptic::redis::RedisStore;
use synaptic::store::Store;

// 之前：let store = Arc::new(InMemoryStore::new());
// 之后：
let store: Arc<dyn Store> = Arc::new(
    RedisStore::from_url("redis://127.0.0.1:6379").await?
);

// 后续代码无需修改 -- 接口完全一致
store.put(&["session", "abc"], "history", json!(messages)).await?;
```

### 替换 InMemoryCache

将 LLM 缓存从内存切换到 Redis：

```rust,ignore
use std::sync::Arc;
use std::time::Duration;
use synaptic::redis::{RedisCache, RedisCacheConfig};
use synaptic::cache::{CachedChatModel, LlmCache};

// 之前：let cache = Arc::new(InMemoryCache::with_ttl(Duration::from_secs(3600)));
// 之后：
let config = RedisCacheConfig {
    prefix: "llm".to_string(),
    ttl: Some(Duration::from_secs(3600)),
};
let cache: Arc<dyn LlmCache> = Arc::new(
    RedisCache::from_url_with_config("redis://127.0.0.1:6379", config).await?
);

let cached_model = CachedChatModel::new(base_model, cache);
```

### 与 Agent 配合使用

在 Agent 中使用 `RedisStore` 作为持久化存储后端：

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{create_agent, AgentOptions};
use synaptic::redis::RedisStore;

let store = Arc::new(RedisStore::from_url("redis://127.0.0.1:6379").await?);

let options = AgentOptions {
    store: Some(store),
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

当向 `create_agent` 提供 `RedisStore` 时，所有 `RuntimeAwareTool` 都会通过 `ToolRuntime` 接收到该 Store，实现跨会话的状态持久化。

### 同时使用 Store 和 Cache

在同一应用中同时使用 Redis 的存储和缓存功能：

```rust,ignore
use std::sync::Arc;
use synaptic::redis::{RedisStore, RedisStoreConfig, RedisCache, RedisCacheConfig};

// Store 用于 Agent 状态
let store_config = RedisStoreConfig {
    prefix: "app_store".to_string(),
};
let store = Arc::new(
    RedisStore::from_url_with_config("redis://127.0.0.1:6379", store_config).await?
);

// Cache 用于 LLM 响应缓存
let cache_config = RedisCacheConfig {
    prefix: "app_cache".to_string(),
    ttl: Some(Duration::from_secs(7200)),
};
let cache = Arc::new(
    RedisCache::from_url_with_config("redis://127.0.0.1:6379", cache_config).await?
);

// 不同的 prefix 确保 Store 和 Cache 的 key 互不冲突
```

## Redis Cluster

Synaptic 支持 Redis Cluster，适用于需要水平扩展和高可用的生产环境。

### Cargo.toml 配置

启用 `redis-cluster` feature：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["redis-cluster"] }
```

### 创建集群 Store

```rust,ignore
use synaptic::redis::RedisStore;

let store = RedisStore::from_cluster_nodes(&[
    "redis://127.0.0.1:7000/",
    "redis://127.0.0.1:7001/",
    "redis://127.0.0.1:7002/",
])?;
```

带自定义配置：

```rust,ignore
use synaptic::redis::{RedisStore, RedisStoreConfig};

let config = RedisStoreConfig {
    prefix: "myapp:store:".to_string(),
};
let store = RedisStore::from_cluster_nodes_with_config(
    &["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"],
    config,
)?;
```

### 创建集群 Cache

```rust,ignore
use synaptic::redis::{RedisCache, RedisCacheConfig};

let config = RedisCacheConfig {
    ttl: Some(3600),
    ..Default::default()
};
let cache = RedisCache::from_cluster_nodes_with_config(
    &["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"],
    config,
)?;
```

### 注意事项

- 所有 `Store`、`LlmCache` 和 `Checkpointer` 操作在单节点和集群后端上行为一致。API 完全相同——只有构造器不同。
- 键枚举操作（`search`、`clear`）在集群上使用 `KEYS`（redis-rs 自动分发到所有节点），而非单节点上的 `SCAN`。这些操作不在热路径上。
- `redis-cluster` feature 会引入 `redis` crate 的 `cluster-async` feature。

### 与 InMemoryStore / InMemoryCache 的区别

| 特性 | `InMemory*` | `Redis*` |
|------|------------|----------|
| 持久化 | 否（进程退出即丢失） | 是（Redis 持久化） |
| 跨进程共享 | 否 | 是 |
| TTL 支持 | 仅 `InMemoryCache` | Store 和 Cache 均支持 |
| 外部依赖 | 无 | Redis 服务 |
| 适用场景 | 开发测试、单进程应用 | 生产部署、分布式系统 |
