use synapse_core::Message;
use synapse_graph::{MessageState, State};

#[test]
fn message_state_merge() {
    let mut a = MessageState::with_messages(vec![Message::human("hello")]);
    let b = MessageState::with_messages(vec![Message::ai("world")]);
    a.merge(b);
    assert_eq!(a.messages.len(), 2);
    assert_eq!(a.messages[0].content(), "hello");
    assert_eq!(a.messages[1].content(), "world");
}

#[test]
fn message_state_last_message() {
    let state = MessageState::with_messages(vec![Message::human("first"), Message::ai("second")]);
    let last = state.last_message().unwrap();
    assert_eq!(last.content(), "second");
    assert!(last.is_ai());
}

#[test]
fn message_state_default_empty() {
    let state = MessageState::default();
    assert!(state.messages.is_empty());
    assert!(state.last_message().is_none());
}
