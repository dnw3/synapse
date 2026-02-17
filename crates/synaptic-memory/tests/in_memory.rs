use synaptic_core::{MemoryStore, Message};
use synaptic_memory::InMemoryStore;

#[tokio::test]
async fn stores_and_reads_messages_by_session() {
    let store = InMemoryStore::new();
    let msg = Message::human("hello");

    store
        .append("session-a", msg.clone())
        .await
        .expect("append should work");

    let loaded = store.load("session-a").await.expect("load should work");

    assert_eq!(loaded, vec![msg]);
}

#[tokio::test]
async fn isolates_sessions() {
    let store = InMemoryStore::new();
    store
        .append("session-a", Message::human("A"))
        .await
        .expect("append A");
    store
        .append("session-b", Message::human("B"))
        .await
        .expect("append B");

    let a = store.load("session-a").await.expect("load a");
    let b = store.load("session-b").await.expect("load b");

    assert_eq!(a[0].content(), "A");
    assert_eq!(b[0].content(), "B");
}
