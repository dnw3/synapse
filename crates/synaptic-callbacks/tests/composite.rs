use std::sync::Arc;

use synaptic_callbacks::{CompositeCallback, RecordingCallback};
use synaptic_core::{CallbackHandler, RunEvent};

#[tokio::test]
async fn composite_dispatches_to_all_handlers() {
    let r1 = Arc::new(RecordingCallback::new());
    let r2 = Arc::new(RecordingCallback::new());

    let composite = CompositeCallback::new(vec![
        Arc::clone(&r1) as Arc<dyn CallbackHandler>,
        Arc::clone(&r2) as Arc<dyn CallbackHandler>,
    ]);

    composite
        .on_event(RunEvent::RunStarted {
            run_id: "r1".to_string(),
            session_id: "s1".to_string(),
        })
        .await
        .expect("dispatch event");

    let events1 = r1.events().await;
    let events2 = r2.events().await;

    assert_eq!(events1.len(), 1);
    assert_eq!(events2.len(), 1);
    assert!(matches!(events1[0], RunEvent::RunStarted { .. }));
    assert!(matches!(events2[0], RunEvent::RunStarted { .. }));
}

/// A callback that always returns an error, used to test error propagation.
struct FailingCallback;

#[async_trait::async_trait]
impl CallbackHandler for FailingCallback {
    async fn on_event(&self, _event: RunEvent) -> Result<(), synaptic_core::SynapseError> {
        Err(synaptic_core::SynapseError::Callback(
            "forced failure".to_string(),
        ))
    }
}

#[tokio::test]
async fn composite_propagates_error() {
    let ok_handler = Arc::new(RecordingCallback::new());
    let fail_handler = Arc::new(FailingCallback);

    let composite = CompositeCallback::new(vec![
        ok_handler as Arc<dyn CallbackHandler>,
        fail_handler as Arc<dyn CallbackHandler>,
    ]);

    let result = composite
        .on_event(RunEvent::RunStarted {
            run_id: "r1".to_string(),
            session_id: "s1".to_string(),
        })
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("forced failure"));
}

#[tokio::test]
async fn composite_dispatches_multiple_events_in_order() {
    let recorder = Arc::new(RecordingCallback::new());
    let composite = CompositeCallback::new(vec![Arc::clone(&recorder) as Arc<dyn CallbackHandler>]);

    composite
        .on_event(RunEvent::RunStarted {
            run_id: "r1".to_string(),
            session_id: "s1".to_string(),
        })
        .await
        .unwrap();
    composite
        .on_event(RunEvent::LlmCalled {
            run_id: "r1".to_string(),
            message_count: 3,
        })
        .await
        .unwrap();
    composite
        .on_event(RunEvent::ToolCalled {
            run_id: "r1".to_string(),
            tool_name: "echo".to_string(),
        })
        .await
        .unwrap();
    composite
        .on_event(RunEvent::RunFinished {
            run_id: "r1".to_string(),
            output: "done".to_string(),
        })
        .await
        .unwrap();

    let events = recorder.events().await;
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], RunEvent::RunStarted { .. }));
    assert!(matches!(events[3], RunEvent::RunFinished { .. }));
}

#[tokio::test]
async fn composite_with_empty_handlers() {
    let composite = CompositeCallback::new(vec![]);
    // Should not error even with no handlers
    let result = composite
        .on_event(RunEvent::RunStarted {
            run_id: "r1".to_string(),
            session_id: "s1".to_string(),
        })
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn composite_with_single_handler() {
    let recorder = Arc::new(RecordingCallback::new());
    let composite = CompositeCallback::new(vec![Arc::clone(&recorder) as Arc<dyn CallbackHandler>]);

    composite
        .on_event(RunEvent::ToolCalled {
            run_id: "r1".to_string(),
            tool_name: "search".to_string(),
        })
        .await
        .unwrap();
    let events = recorder.events().await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], RunEvent::ToolCalled { .. }));
}
