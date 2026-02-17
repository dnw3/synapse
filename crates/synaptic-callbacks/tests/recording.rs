use synaptic_callbacks::RecordingCallback;
use synaptic_core::{CallbackHandler, RunEvent};

#[tokio::test]
async fn records_events_in_order() {
    let callback = RecordingCallback::new();

    callback
        .on_event(RunEvent::RunStarted {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
        })
        .await
        .expect("start event");

    callback
        .on_event(RunEvent::RunFinished {
            run_id: "run-1".to_string(),
            output: "done".to_string(),
        })
        .await
        .expect("finish event");

    let events = callback.events().await;
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], RunEvent::RunStarted { .. }));
    assert!(matches!(events[1], RunEvent::RunFinished { .. }));
}

#[tokio::test]
async fn recording_starts_empty() {
    let callback = RecordingCallback::new();
    let events = callback.events().await;
    assert!(events.is_empty());
}

#[tokio::test]
async fn recording_captures_all_event_types() {
    let callback = RecordingCallback::new();

    callback
        .on_event(RunEvent::RunStarted {
            run_id: "r1".to_string(),
            session_id: "s1".to_string(),
        })
        .await
        .unwrap();
    callback
        .on_event(RunEvent::LlmCalled {
            run_id: "r1".to_string(),
            message_count: 2,
        })
        .await
        .unwrap();
    callback
        .on_event(RunEvent::ToolCalled {
            run_id: "r1".to_string(),
            tool_name: "echo".to_string(),
        })
        .await
        .unwrap();
    callback
        .on_event(RunEvent::RunStep {
            run_id: "r1".to_string(),
            step: 1,
        })
        .await
        .unwrap();
    callback
        .on_event(RunEvent::RunFinished {
            run_id: "r1".to_string(),
            output: "done".to_string(),
        })
        .await
        .unwrap();
    callback
        .on_event(RunEvent::RunFailed {
            run_id: "r1".to_string(),
            error: "oops".to_string(),
        })
        .await
        .unwrap();

    let events = callback.events().await;
    assert_eq!(events.len(), 6);
}

#[tokio::test]
async fn recording_clone_independence() {
    let callback = RecordingCallback::new();
    let clone = callback.clone();

    callback
        .on_event(RunEvent::RunStarted {
            run_id: "r1".to_string(),
            session_id: "s1".to_string(),
        })
        .await
        .unwrap();

    // Clone shares the same inner Arc, so both see the event
    let events_original = callback.events().await;
    let events_clone = clone.events().await;
    assert_eq!(events_original.len(), 1);
    assert_eq!(events_clone.len(), 1);
}
