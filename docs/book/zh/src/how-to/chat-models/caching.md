# 缓存 LLM 响应

本指南展示如何缓存 LLM 响应，以避免重复的 API 调用并降低延迟。

## 概述

Synaptic 通过 `LlmCache` trait 提供两种缓存实现：

- **`InMemoryCache`** -- 精确匹配缓存，支持可选的 TTL 过期。
- **`SemanticCache`** -- 基于 Embedding 的相似度匹配，用于语义等价的查询。

两者均配合 `CachedChatModel` 使用，它包装任意 `ChatModel` 并在发起 API 调用前检查缓存。

## 使用 `InMemoryCache` 进行精确匹配缓存

最简单的缓存以请求的精确内容为键存储响应：

```rust
use std::sync::Arc;
use synaptic_core::ChatModel;
use synaptic_cache::{InMemoryCache, CachedChatModel};

let base_model: Arc<dyn ChatModel> = Arc::new(model);
let cache = Arc::new(InMemoryCache::new());
let cached_model = CachedChatModel::new(base_model, cache);

// First call hits the LLM
// let response1 = cached_model.chat(request.clone()).await?;

// Identical request returns cached response instantly
// let response2 = cached_model.chat(request.clone()).await?;
```

### 带 TTL 的缓存

设置生存时间，使缓存条目自动过期：

```rust
use std::time::Duration;
use std::sync::Arc;
use synaptic_cache::InMemoryCache;

// Entries expire after 1 hour
let cache = Arc::new(InMemoryCache::with_ttl(Duration::from_secs(3600)));

// Entries expire after 5 minutes
let cache = Arc::new(InMemoryCache::with_ttl(Duration::from_secs(300)));
```

TTL 过期后，该条目的缓存查找将返回 `None`，下一次请求将重新调用 LLM。

## 使用 `SemanticCache` 进行语义缓存

语义缓存使用 Embedding 查找相似查询，即使措辞不同也能匹配。例如，"What's the weather?" 和 "Tell me the current weather" 可以命中同一个缓存响应。

```rust
use std::sync::Arc;
use synaptic_cache::{SemanticCache, CachedChatModel};
use synaptic_embeddings::OpenAiEmbeddings;

let embeddings: Arc<dyn synaptic_embeddings::Embeddings> = Arc::new(embeddings_provider);

// Similarity threshold of 0.95 means only very similar queries match
let cache = Arc::new(SemanticCache::new(embeddings, 0.95));

let cached_model = CachedChatModel::new(base_model, cache);
```

查找缓存响应时：

1. 使用提供的 `Embeddings` 实现对查询进行向量化。
2. 将 Embedding 与所有存储条目通过余弦相似度进行比较。
3. 如果最佳匹配超过相似度阈值，则返回缓存的响应。

### 选择阈值

- **0.95 -- 0.99**：非常严格。只有几乎相同的查询才会匹配。适用于事实性问答，因为措辞的微小变化可能改变含义。
- **0.90 -- 0.95**：中等。能捕捉常见的改述。适用于通用聊天机器人。
- **0.80 -- 0.90**：宽松。匹配范围更广。当您需要积极缓存且近似回答可接受时适用。

## `LlmCache` trait

两种缓存类型都实现了 `LlmCache` trait：

```rust
#[async_trait]
pub trait LlmCache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapticError>;
    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapticError>;
    async fn clear(&self) -> Result<(), SynapticError>;
}
```

您可以为自定义缓存后端（Redis、SQLite 等）实现此 trait。

## 清除缓存

两种缓存实现都支持清除所有条目：

```rust
use synaptic_cache::LlmCache;

// cache implements LlmCache
// cache.clear().await?;
```

## 与其他包装器组合

由于 `CachedChatModel` 实现了 `ChatModel`，它可以与重试、速率限制和其他包装器组合使用：

```rust
use std::sync::Arc;
use synaptic_core::ChatModel;
use synaptic_cache::{InMemoryCache, CachedChatModel};
use synaptic_models::{RetryChatModel, RetryPolicy};

let base_model: Arc<dyn ChatModel> = Arc::new(model);

// Cache first, then retry on cache miss + API failure
let cache = Arc::new(InMemoryCache::new());
let cached = Arc::new(CachedChatModel::new(base_model, cache));
let reliable = RetryChatModel::new(cached, RetryPolicy::default());
```
