use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use synaptic_core::{ChatResponse, Message};
use synaptic_models::ScriptedChatModel;
use synaptic_retrieval::{
    Document, InMemoryRetriever, MetadataFieldInfo, Retriever, SelfQueryRetriever,
};

fn make_docs() -> Vec<Document> {
    vec![
        Document::with_metadata(
            "1",
            "rust programming guide",
            HashMap::from([
                ("category".to_string(), json!("programming")),
                ("year".to_string(), json!(2024)),
            ]),
        ),
        Document::with_metadata(
            "2",
            "python data science tutorial",
            HashMap::from([
                ("category".to_string(), json!("programming")),
                ("year".to_string(), json!(2023)),
            ]),
        ),
        Document::with_metadata(
            "3",
            "cooking recipes for beginners",
            HashMap::from([
                ("category".to_string(), json!("cooking")),
                ("year".to_string(), json!(2024)),
            ]),
        ),
    ]
}

fn make_field_info() -> Vec<MetadataFieldInfo> {
    vec![
        MetadataFieldInfo {
            name: "category".to_string(),
            description: "The category of the document".to_string(),
            field_type: "string".to_string(),
        },
        MetadataFieldInfo {
            name: "year".to_string(),
            description: "The year the document was published".to_string(),
            field_type: "integer".to_string(),
        },
    ]
}

#[tokio::test]
async fn self_query_with_filter() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(
            r#"{"query": "programming", "filters": [{"field": "category", "op": "eq", "value": "programming"}]}"#,
        ),
        usage: None,
    }]));

    let docs = make_docs();
    let base = Arc::new(InMemoryRetriever::new(docs));
    let retriever = SelfQueryRetriever::new(base, model, make_field_info());

    let results = retriever
        .retrieve("programming tutorials", 10)
        .await
        .unwrap();

    // Should only return programming docs
    for doc in &results {
        assert_eq!(doc.metadata["category"], "programming");
    }
}

#[tokio::test]
async fn self_query_no_filters() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(r#"{"query": "rust", "filters": []}"#),
        usage: None,
    }]));

    let docs = make_docs();
    let base = Arc::new(InMemoryRetriever::new(docs));
    let retriever = SelfQueryRetriever::new(base, model, make_field_info());

    let results = retriever.retrieve("rust programming", 10).await.unwrap();
    // Should pass through without filtering
    assert!(!results.is_empty());
}

#[tokio::test]
async fn self_query_unknown_fields_ignored() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(
            r#"{"query": "cooking", "filters": [{"field": "unknown_field", "op": "eq", "value": "x"}]}"#,
        ),
        usage: None,
    }]));

    let docs = make_docs();
    let base = Arc::new(InMemoryRetriever::new(docs));
    let retriever = SelfQueryRetriever::new(base, model, make_field_info());

    let results = retriever.retrieve("cooking", 10).await.unwrap();
    // Unknown fields are ignored, so no filtering happens
    assert!(!results.is_empty());
}

#[tokio::test]
async fn self_query_numeric_filter() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(
            r#"{"query": "guide", "filters": [{"field": "year", "op": "gte", "value": 2024}]}"#,
        ),
        usage: None,
    }]));

    let docs = make_docs();
    let base = Arc::new(InMemoryRetriever::new(docs));
    let retriever = SelfQueryRetriever::new(base, model, make_field_info());

    let results = retriever.retrieve("2024 guides", 10).await.unwrap();
    for doc in &results {
        assert_eq!(doc.metadata["year"], 2024);
    }
}
