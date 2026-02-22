#[cfg(feature = "otel")]
mod tests {
    use synaptic_callbacks::OpenTelemetryCallback;

    #[test]
    fn otel_callback_new() {
        let _cb = OpenTelemetryCallback::new("test-service");
    }

    #[test]
    fn otel_callback_service_name() {
        let _cb = OpenTelemetryCallback::new("my-agent");
    }
}
