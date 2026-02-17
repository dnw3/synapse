use synapse_graph::Send;

#[test]
fn send_new_creates_instance() {
    let send = Send::new("process_chunk", serde_json::json!({"chunk": "part1"}));
    assert_eq!(send.node, "process_chunk");
    assert_eq!(send.state, serde_json::json!({"chunk": "part1"}));
}

#[test]
fn send_with_string_node() {
    let send = Send::new("my_node".to_string(), serde_json::json!(42));
    assert_eq!(send.node, "my_node");
    assert_eq!(send.state, serde_json::json!(42));
}

#[test]
fn send_is_clone() {
    let send1 = Send::new("node", serde_json::json!({"key": "value"}));
    let send2 = send1.clone();
    assert_eq!(send1.node, send2.node);
    assert_eq!(send1.state, send2.state);
}

#[test]
fn send_is_debug() {
    let send = Send::new("node", serde_json::json!(null));
    let debug = format!("{:?}", send);
    assert!(debug.contains("node"));
    assert!(debug.contains("Send"));
}

#[test]
fn send_with_complex_state() {
    let state = serde_json::json!({
        "messages": [{"role": "human", "content": "hello"}],
        "metadata": {"key": "value"},
        "counter": 42,
    });
    let send = Send::new("process", state.clone());
    assert_eq!(send.state["messages"][0]["content"], "hello");
    assert_eq!(send.state["counter"], 42);
}
