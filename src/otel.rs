//! OpenTelemetry initialization for Synapse.
//!
//! When the `otel` feature is enabled, this module sets up an OTLP span
//! exporter that sends traces to the endpoint configured via the
//! `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable.

use opentelemetry::global;
use opentelemetry_sdk::trace::SdkTracerProvider;

/// Initialize the OpenTelemetry OTLP exporter.
///
/// Returns `Some(provider)` if the endpoint is configured, `None` otherwise.
/// The caller should hold the returned provider and call `shutdown()` on exit.
pub fn init_otel() -> Option<SdkTracerProvider> {
    let endpoint = match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(ep) if !ep.is_empty() => ep,
        _ => {
            tracing::debug!("OpenTelemetry compiled in but OTEL_EXPORTER_OTLP_ENDPOINT not set");
            return None;
        }
    };

    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
    {
        Ok(exp) => exp,
        Err(e) => {
            tracing::error!(error = %e, "failed to build OTLP exporter");
            return None;
        }
    };

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());

    tracing::info!(endpoint = %endpoint, "OpenTelemetry tracing enabled");

    Some(provider)
}
