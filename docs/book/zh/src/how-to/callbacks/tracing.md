# Tracing Callback

`TracingCallback` 将 Synaptic 的 Callback 系统与 Rust [`tracing`](https://docs.rs/tracing) 生态系统集成。它不是将事件存储在内存中，而是发出结构化的 tracing span 和事件，这些事件会流入你配置的任何 subscriber——终端输出、JSON 日志、OpenTelemetry 等。

## 设置

首先，初始化一个 tracing subscriber。最简单的方式是使用 `tracing-subscriber` 的 `fmt` subscriber：

```rust
use tracing_subscriber;

// Initialize the default subscriber (prints to stderr)
tracing_subscriber::fmt::init();
```

然后创建 Callback：

```rust
use synaptic_callbacks::TracingCallback;

let callback = TracingCallback::new();
```

将此 Callback 传递给你的 Agent 或与 `CompositeCallback` 一起使用。

## 日志内容

`TracingCallback` 将每个 `RunEvent` 变体映射到一个 `tracing` 调用：

| RunEvent | Tracing 级别 | 关键字段 |
|----------|-------------|----------|
| `RunStarted` | `info!` | `run_id`, `session_id` |
| `RunStep` | `info!` | `run_id`, `step` |
| `LlmCalled` | `info!` | `run_id`, `message_count` |
| `ToolCalled` | `info!` | `run_id`, `tool_name` |
| `RunFinished` | `info!` | `run_id`, `output_len` |
| `RunFailed` | `error!` | `run_id`, `error` |

除 `RunFailed` 外，所有事件都以 `INFO` 级别记录。失败以 `ERROR` 级别记录。

## 输出示例

使用默认的 `fmt` subscriber，你可能会看到：

```
2026-02-17T10:30:00.123Z  INFO synaptic: run started run_id="abc-123" session_id="user-1"
2026-02-17T10:30:00.456Z  INFO synaptic: LLM called run_id="abc-123" message_count=3
2026-02-17T10:30:01.234Z  INFO synaptic: tool called run_id="abc-123" tool_name="calculator"
2026-02-17T10:30:01.567Z  INFO synaptic: run finished run_id="abc-123" output_len=42
```

## 与 Tracing 生态系统集成

因为 `TracingCallback` 使用标准的 `tracing` 宏，它可以与任何兼容的 subscriber 配合使用：

- **`tracing-subscriber`** -- 终端格式化、过滤、分层。
- **`tracing-opentelemetry`** -- 将 span 导出到 Jaeger、Zipkin 或任何 OTLP 收集器。
- **`tracing-appender`** -- 将日志写入滚动文件。
- **JSON 输出** -- 使用 `tracing_subscriber::fmt().json()` 进行结构化日志采集。

```rust
// Example: JSON-formatted logs
tracing_subscriber::fmt()
    .json()
    .init();

let callback = TracingCallback::new();
```

## 何时使用

在以下情况下使用 `TracingCallback`：

- 你希望以最少的设置获得生产级结构化日志。
- 你的应用程序已经在使用 `tracing` 生态系统。
- 你需要将 Agent 遥测数据导出到可观测平台（Datadog、Grafana 等）。

对于测试时的事件检查，考虑使用 [RecordingCallback](recording.md)，它存储事件以便程序化访问。
