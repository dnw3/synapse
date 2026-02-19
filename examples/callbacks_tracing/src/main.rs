use std::sync::Arc;

use synaptic::callbacks::{CompositeCallback, RecordingCallback, TracingCallback};
use synaptic::core::{CallbackHandler, RunEvent, SynapticError};

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // Initialize tracing subscriber for TracingCallback output
    tracing_subscriber::fmt::init();

    // --- RecordingCallback ---
    println!("=== RecordingCallback ===");
    let recorder = RecordingCallback::new();
    recorder
        .on_event(RunEvent::RunStarted {
            run_id: "run-1".into(),
            session_id: "sess-1".into(),
        })
        .await?;
    recorder
        .on_event(RunEvent::LlmCalled {
            run_id: "run-1".into(),
            message_count: 3,
        })
        .await?;
    recorder
        .on_event(RunEvent::ToolCalled {
            run_id: "run-1".into(),
            tool_name: "calculator".into(),
        })
        .await?;
    recorder
        .on_event(RunEvent::RunFinished {
            run_id: "run-1".into(),
            output: "42".into(),
        })
        .await?;

    let events = recorder.events().await;
    println!("Recorded {} events:", events.len());
    for event in &events {
        println!("  {:?}", event);
    }

    // --- TracingCallback ---
    println!("\n=== TracingCallback ===");
    let tracer = TracingCallback::new();
    println!("(Events below are emitted via tracing framework)");
    tracer
        .on_event(RunEvent::RunStarted {
            run_id: "run-2".into(),
            session_id: "sess-2".into(),
        })
        .await?;
    tracer
        .on_event(RunEvent::RunStep {
            run_id: "run-2".into(),
            step: 1,
        })
        .await?;
    tracer
        .on_event(RunEvent::ToolCalled {
            run_id: "run-2".into(),
            tool_name: "search".into(),
        })
        .await?;
    tracer
        .on_event(RunEvent::RunFinished {
            run_id: "run-2".into(),
            output: "done".into(),
        })
        .await?;

    // --- CompositeCallback ---
    println!("\n=== CompositeCallback ===");
    let recorder2 = RecordingCallback::new();
    let recorder2_clone = recorder2.clone();
    let composite = CompositeCallback::new(vec![
        Arc::new(recorder2_clone),
        Arc::new(TracingCallback::new()),
    ]);

    composite
        .on_event(RunEvent::RunStarted {
            run_id: "run-3".into(),
            session_id: "sess-3".into(),
        })
        .await?;
    composite
        .on_event(RunEvent::LlmCalled {
            run_id: "run-3".into(),
            message_count: 5,
        })
        .await?;
    composite
        .on_event(RunEvent::RunFinished {
            run_id: "run-3".into(),
            output: "composite result".into(),
        })
        .await?;

    let events = recorder2.events().await;
    println!("Composite recorder captured {} events", events.len());

    println!("\nCallbacks & tracing demo completed successfully!");
    Ok(())
}
