# Langfuse

[Langfuse](https://langfuse.com/) 是一个开源的 LLM 可观测性和分析平台。
此集成将 Synaptic 运行事件（LLM 调用、工具调用、链步骤）记录为 Langfuse 追踪，
用于调试、成本监控和质量评估。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["langfuse"] }
```

在 [cloud.langfuse.com](https://cloud.langfuse.com/) 注册或自托管。

## 配置

```rust,ignore
use synaptic::langfuse::{LangfuseCallback, LangfuseConfig};

let config = LangfuseConfig::new("pk-lf-...", "sk-lf-...");
let callback = LangfuseCallback::new(config).await.unwrap();
```

### 自托管实例

```rust,ignore
let config = LangfuseConfig::new("pk-lf-...", "sk-lf-...")
    .with_host("https://langfuse.your-company.com")
    .with_flush_batch_size(50);
```

## 使用方法

```rust,ignore
use synaptic::langfuse::{LangfuseCallback, LangfuseConfig};
use std::sync::Arc;

let config = LangfuseConfig::new("pk-lf-...", "sk-lf-...");
let callback = Arc::new(LangfuseCallback::new(config).await.unwrap());
// 事件会被缓冲，达到 batch_size 时自动刷新
// 应用关闭时，刷新剩余事件：
callback.flush().await.unwrap();
```

## 配置参考

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `public_key` | 必填 | Langfuse 公钥 |
| `secret_key` | 必填 | Langfuse 私钥 |
| `host` | https://cloud.langfuse.com | Langfuse 主机 URL |
| `flush_batch_size` | 20 | 自动刷新前缓冲的事件数 |
