use std::sync::Arc;
use synaptic_callbacks::{CompositeCallback, RecordingCallback};
use synaptic_core::{CallbackHandler, RunEvent};

#[tokio::test]
async fn recording_concurrent_events() {
    let recorder = Arc::new(RecordingCallback::new());
    let mut handles = Vec::new();

    for i in 0..20 {
        let r = recorder.clone();
        handles.push(tokio::spawn(async move {
            r.on_event(RunEvent::RunStep {
                run_id: format!("r{i}"),
                step: i,
            })
            .await
            .unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let events = recorder.events().await;
    assert_eq!(events.len(), 20);
}

#[tokio::test]
async fn composite_three_handlers() {
    let r1 = Arc::new(RecordingCallback::new());
    let r2 = Arc::new(RecordingCallback::new());
    let r3 = Arc::new(RecordingCallback::new());

    let composite = CompositeCallback::new(vec![
        r1.clone() as Arc<dyn CallbackHandler>,
        r2.clone() as Arc<dyn CallbackHandler>,
        r3.clone() as Arc<dyn CallbackHandler>,
    ]);

    composite
        .on_event(RunEvent::RunStarted {
            run_id: "r1".into(),
            session_id: "s1".into(),
        })
        .await
        .unwrap();

    assert_eq!(r1.events().await.len(), 1);
    assert_eq!(r2.events().await.len(), 1);
    assert_eq!(r3.events().await.len(), 1);
}

#[tokio::test]
async fn recording_session_isolation_via_event_data() {
    let recorder = RecordingCallback::new();

    recorder
        .on_event(RunEvent::RunStarted {
            run_id: "run-a".into(),
            session_id: "session-1".into(),
        })
        .await
        .unwrap();
    recorder
        .on_event(RunEvent::RunStarted {
            run_id: "run-b".into(),
            session_id: "session-2".into(),
        })
        .await
        .unwrap();

    let events = recorder.events().await;
    assert_eq!(events.len(), 2);

    // Verify different session IDs are captured
    match (&events[0], &events[1]) {
        (
            RunEvent::RunStarted {
                session_id: s1, ..
            },
            RunEvent::RunStarted {
                session_id: s2, ..
            },
        ) => {
            assert_eq!(s1, "session-1");
            assert_eq!(s2, "session-2");
        }
        _ => panic!("expected two RunStarted events"),
    }
}

#[tokio::test]
async fn composite_full_lifecycle() {
    let recorder = Arc::new(RecordingCallback::new());
    let composite =
        CompositeCallback::new(vec![recorder.clone() as Arc<dyn CallbackHandler>]);

    // Simulate a full agent lifecycle
    let events = vec![
        RunEvent::RunStarted {
            run_id: "r1".into(),
            session_id: "s1".into(),
        },
        RunEvent::LlmCalled {
            run_id: "r1".into(),
            message_count: 1,
        },
        RunEvent::ToolCalled {
            run_id: "r1".into(),
            tool_name: "search".into(),
        },
        RunEvent::LlmCalled {
            run_id: "r1".into(),
            message_count: 3,
        },
        RunEvent::RunFinished {
            run_id: "r1".into(),
            output: "answer".into(),
        },
    ];

    for event in &events {
        composite.on_event(event.clone()).await.unwrap();
    }

    let recorded = recorder.events().await;
    assert_eq!(recorded.len(), 5);
}
