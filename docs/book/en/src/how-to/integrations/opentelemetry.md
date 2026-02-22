# OpenTelemetry

The Synaptic OpenTelemetry callback integrates with the OpenTelemetry ecosystem,
sending traces for every LLM call and tool invocation to your preferred observability
backend (Jaeger, Grafana Tempo, Honeycomb, Datadog, etc.).

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["callbacks", "otel"] }
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["http-proto"] }
```

## Configuration

Initialize your OTel tracer provider, then create the callback:

```rust,ignore
use synaptic::callbacks::OpenTelemetryCallback;

let callback = OpenTelemetryCallback::new("my-agent");
```

## Usage with an Agent

```rust,ignore
use synaptic::callbacks::OpenTelemetryCallback;
use std::sync::Arc;

let otel_cb = Arc::new(OpenTelemetryCallback::new("synaptic-agent"));
// Pass to any component that accepts a CallbackHandler
```

## Span Structure

Each LLM call creates a span named `synaptic.llm_called`
with attributes `synaptic.run_id` and `llm.message_count`.

Each tool invocation creates a span named `tool.{tool_name}`
with attributes `synaptic.run_id` and `tool.name`.

Run lifecycle: `synaptic.run_started`, `synaptic.run_finished`, `synaptic.run_failed`, `synaptic.run_step`.
