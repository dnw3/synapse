use synaptic_callbacks::TracingCallback;
use synaptic_core::{CallbackHandler, RunEvent};

#[tokio::test]
async fn tracing_callback_handles_all_events() {
    let cb = TracingCallback::new();

    cb.on_event(RunEvent::RunStarted {
        run_id: "r1".to_string(),
        session_id: "s1".to_string(),
    })
    .await
    .expect("RunStarted");

    cb.on_event(RunEvent::RunStep {
        run_id: "r1".to_string(),
        step: 1,
    })
    .await
    .expect("RunStep");

    cb.on_event(RunEvent::LlmCalled {
        run_id: "r1".to_string(),
        message_count: 3,
    })
    .await
    .expect("LlmCalled");

    cb.on_event(RunEvent::ToolCalled {
        run_id: "r1".to_string(),
        tool_name: "search".to_string(),
    })
    .await
    .expect("ToolCalled");

    cb.on_event(RunEvent::RunFinished {
        run_id: "r1".to_string(),
        output: "done".to_string(),
    })
    .await
    .expect("RunFinished");

    cb.on_event(RunEvent::RunFailed {
        run_id: "r1".to_string(),
        error: "something went wrong".to_string(),
    })
    .await
    .expect("RunFailed");
}

#[tokio::test]
async fn tracing_callback_is_default() {
    let _cb = TracingCallback::default();
}
