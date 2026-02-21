# SQLite 缓存

本指南展示如何使用 Synaptic 的 SQLite 集成实现持久化的 LLM 响应缓存。SQLite 缓存不需要外部服务器，适合本地开发和单机部署场景。

## 设置

在 `Cargo.toml` 中添加 `sqlite` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "sqlite"] }
```

无需额外安装 SQLite -- 该 crate 内嵌了 SQLite 引擎。

## 配置

### 基于文件的缓存

使用 `SqliteCacheConfig` 创建基于文件的缓存配置：

```rust,ignore
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};

let config = SqliteCacheConfig::new("./cache.db");
let cache = SqliteCache::new(config).await?;
```

数据库文件会自动创建（如果不存在），缓存表也会自动初始化。

### 内存缓存

适用于测试或临时缓存场景：

```rust,ignore
let config = SqliteCacheConfig::in_memory();
let cache = SqliteCache::new(config).await?;
```

内存缓存在程序结束后会丢失所有数据。

### 设置 TTL

缓存条目可以设置过期时间（TTL）：

```rust,ignore
use std::time::Duration;

let config = SqliteCacheConfig::new("./cache.db")
    .with_ttl(Duration::from_secs(3600));  // 1 小时后过期

let cache = SqliteCache::new(config).await?;
```

不设置 TTL 时，缓存条目永不过期。

## 用法

### 配合 CachedChatModel 使用

`SqliteCache` 实现了 `LlmCache` trait，可以与 `CachedChatModel` 配合使用缓存 LLM 响应：

```rust,ignore
use std::sync::Arc;
use synaptic::core::ChatModel;
use synaptic::cache::CachedChatModel;
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};
use synaptic::openai::OpenAiChatModel;

// 创建模型
let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));

// 创建 SQLite 缓存
let cache = Arc::new(SqliteCache::new(SqliteCacheConfig::new("./llm_cache.db")).await?);

// 包装为缓存模型
let cached_model = CachedChatModel::new(model, cache);

// 第一次调用 -- 请求 LLM 并缓存响应
let response = cached_model.chat(&request).await?;

// 相同请求的第二次调用 -- 直接返回缓存结果
let response = cached_model.chat(&request).await?;
```

### 直接使用缓存

如果需要直接操作缓存：

```rust,ignore
use synaptic::core::LlmCache;
use synaptic::sqlite::SqliteCache;

let cache = SqliteCache::new(SqliteCacheConfig::new("./cache.db")).await?;

// 查找缓存
let cached = cache.lookup("cache-key").await?;

// 写入缓存
cache.update("cache-key", &response).await?;

// 清空缓存
cache.clear().await?;
```

### 适用场景

SQLite 缓存在以下场景特别有用：

- **本地开发** -- 避免重复调用 API，节省成本和时间
- **单机部署** -- 无需额外的 Redis 等外部服务
- **测试** -- 使用 `in_memory()` 进行快速的隔离测试
- **CI/CD** -- 在持续集成中缓存 LLM 响应以加速测试

如果需要跨多个进程或机器共享缓存，建议使用 [Redis 缓存](redis.md)。

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `path` | `String` | 必填（文件模式） | 数据库文件路径 |
| `ttl` | `Option<Duration>` | `None` | 缓存条目过期时间，`None` 表示永不过期 |

| 构造器 | 说明 |
|--------|------|
| `SqliteCacheConfig::new(path)` | 基于文件的持久化缓存 |
| `SqliteCacheConfig::in_memory()` | 内存缓存（程序结束即丢失） |
