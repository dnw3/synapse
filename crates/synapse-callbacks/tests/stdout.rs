use synapse_callbacks::StdOutCallbackHandler;
use synapse_core::{CallbackHandler, RunEvent};

#[tokio::test]
async fn stdout_handler_processes_all_event_types() {
    let handler = StdOutCallbackHandler::new();

    handler
        .on_event(RunEvent::RunStarted {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
        })
        .await
        .expect("RunStarted");

    handler
        .on_event(RunEvent::RunStep {
            run_id: "run-1".to_string(),
            step: 1,
        })
        .await
        .expect("RunStep");

    handler
        .on_event(RunEvent::LlmCalled {
            run_id: "run-1".to_string(),
            message_count: 3,
        })
        .await
        .expect("LlmCalled");

    handler
        .on_event(RunEvent::ToolCalled {
            run_id: "run-1".to_string(),
            tool_name: "search".to_string(),
        })
        .await
        .expect("ToolCalled");

    handler
        .on_event(RunEvent::RunFinished {
            run_id: "run-1".to_string(),
            output: "done".to_string(),
        })
        .await
        .expect("RunFinished");

    handler
        .on_event(RunEvent::RunFailed {
            run_id: "run-1".to_string(),
            error: "oops".to_string(),
        })
        .await
        .expect("RunFailed");
}

#[tokio::test]
async fn verbose_handler_works() {
    let handler = StdOutCallbackHandler::verbose();

    handler
        .on_event(RunEvent::RunStarted {
            run_id: "run-v".to_string(),
            session_id: "session-v".to_string(),
        })
        .await
        .expect("verbose RunStarted");

    handler
        .on_event(RunEvent::RunFinished {
            run_id: "run-v".to_string(),
            output: "verbose output".to_string(),
        })
        .await
        .expect("verbose RunFinished");
}
