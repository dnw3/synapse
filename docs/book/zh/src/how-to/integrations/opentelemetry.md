# OpenTelemetry

Synaptic 的 OpenTelemetry 回调与 OpenTelemetry 生态系统集成，
将每次 LLM 调用和工具调用的追踪信息发送到您首选的可观测性后端
（Jaeger、Grafana Tempo、Honeycomb、Datadog 等）。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["callbacks", "otel"] }
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["http-proto"] }
```

## 配置

初始化 OTel tracer provider，然后创建回调：

```rust,ignore
use synaptic::callbacks::OpenTelemetryCallback;

let callback = OpenTelemetryCallback::new("my-agent");
```

## 与 Agent 配合使用

```rust,ignore
use synaptic::callbacks::OpenTelemetryCallback;
use std::sync::Arc;

let otel_cb = Arc::new(OpenTelemetryCallback::new("synaptic-agent"));
// 传递给任何接受 CallbackHandler 的组件
```

## Span 结构

每次 LLM 调用创建名为 `synaptic.llm_called` 的 span，
属性：`synaptic.run_id`、`llm.message_count`。

每次工具调用创建名为 `tool.{tool_name}` 的 span，
属性：`synaptic.run_id`、`tool.name`。

运行生命周期：`synaptic.run_started`、`synaptic.run_finished`、`synaptic.run_failed`、`synaptic.run_step`。
