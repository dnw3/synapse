use synapse_core::{MemoryStore, Message};
use synapse_memory::FileChatMessageHistory;

#[tokio::test]
async fn load_returns_empty_when_no_file() {
    let dir = tempdir();
    let store = FileChatMessageHistory::new(&dir);

    let messages = store.load("session1").await.unwrap();
    assert!(messages.is_empty());
}

#[tokio::test]
async fn append_and_load() {
    let dir = tempdir();
    let store = FileChatMessageHistory::new(&dir);

    store
        .append("session1", Message::human("Hello"))
        .await
        .unwrap();
    store
        .append("session1", Message::ai("Hi there"))
        .await
        .unwrap();

    let messages = store.load("session1").await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content(), "Hello");
    assert!(messages[0].is_human());
    assert_eq!(messages[1].content(), "Hi there");
    assert!(messages[1].is_ai());
}

#[tokio::test]
async fn sessions_are_isolated() {
    let dir = tempdir();
    let store = FileChatMessageHistory::new(&dir);

    store
        .append("session1", Message::human("msg1"))
        .await
        .unwrap();
    store
        .append("session2", Message::human("msg2"))
        .await
        .unwrap();

    let s1 = store.load("session1").await.unwrap();
    let s2 = store.load("session2").await.unwrap();

    assert_eq!(s1.len(), 1);
    assert_eq!(s1[0].content(), "msg1");
    assert_eq!(s2.len(), 1);
    assert_eq!(s2[0].content(), "msg2");
}

#[tokio::test]
async fn clear_empties_session() {
    let dir = tempdir();
    let store = FileChatMessageHistory::new(&dir);

    store
        .append("session1", Message::human("Hello"))
        .await
        .unwrap();
    assert_eq!(store.load("session1").await.unwrap().len(), 1);

    store.clear("session1").await.unwrap();
    let messages = store.load("session1").await.unwrap();
    assert!(messages.is_empty());
}

#[tokio::test]
async fn persists_across_instances() {
    let dir = tempdir();

    // First instance writes
    {
        let store = FileChatMessageHistory::new(&dir);
        store
            .append("session1", Message::human("Hello"))
            .await
            .unwrap();
    }

    // Second instance reads
    {
        let store = FileChatMessageHistory::new(&dir);
        let messages = store.load("session1").await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content(), "Hello");
    }
}

fn tempdir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "synapse_test_file_history_{}_{}",
        std::process::id(),
        id,
    ));
    // Clean up any leftover from previous runs
    let _ = std::fs::remove_dir_all(&dir);
    dir
}
