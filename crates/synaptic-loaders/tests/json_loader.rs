use synaptic_loaders::{JsonLoader, Loader};

#[tokio::test]
async fn loads_json_array() {
    let json = r#"[
        {"id": "1", "content": "First doc"},
        {"id": "2", "content": "Second doc"}
    ]"#;

    let loader = JsonLoader::new(json);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].id, "1");
    assert_eq!(docs[0].content, "First doc");
    assert_eq!(docs[1].id, "2");
    assert_eq!(docs[1].content, "Second doc");
}

#[tokio::test]
async fn loads_json_array_with_custom_keys() {
    let json = r#"[
        {"name": "Alice", "text": "Hello"},
        {"name": "Bob", "text": "World"}
    ]"#;

    let loader = JsonLoader::new(json)
        .with_id_key("name")
        .with_content_key("text");
    let docs = loader.load().await.unwrap();

    assert_eq!(docs[0].id, "Alice");
    assert_eq!(docs[0].content, "Hello");
}

#[tokio::test]
async fn loads_single_json_object() {
    let json = r#"{"id": "single", "content": "Only one"}"#;

    let loader = JsonLoader::new(json);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "single");
    assert_eq!(docs[0].content, "Only one");
}

#[tokio::test]
async fn returns_error_on_invalid_json() {
    let loader = JsonLoader::new("not json");
    let err = loader.load().await.unwrap_err();
    assert!(err.to_string().contains("invalid JSON"));
}
