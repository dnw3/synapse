use serde_json::json;
use synaptic_core::{ContentBlock, Message};

#[test]
fn content_block_text_serde_roundtrip() {
    let block = ContentBlock::Text {
        text: "hello world".into(),
    };
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "text");
    assert_eq!(json["text"], "hello world");
    let deserialized: ContentBlock = serde_json::from_value(json).unwrap();
    assert_eq!(block, deserialized);
}

#[test]
fn content_block_image_serde_roundtrip() {
    let block = ContentBlock::Image {
        url: "https://example.com/img.png".into(),
        detail: Some("high".into()),
    };
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "image");
    assert_eq!(json["url"], "https://example.com/img.png");
    assert_eq!(json["detail"], "high");
    let deserialized: ContentBlock = serde_json::from_value(json).unwrap();
    assert_eq!(block, deserialized);
}

#[test]
fn content_block_reasoning_serde_roundtrip() {
    let block = ContentBlock::Reasoning {
        content: "Let me think...".into(),
    };
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "reasoning");
    let deserialized: ContentBlock = serde_json::from_value(json).unwrap();
    assert_eq!(block, deserialized);
}

#[test]
fn content_block_data_serde_roundtrip() {
    let block = ContentBlock::Data {
        data: json!({"key": "value", "nested": [1, 2, 3]}),
    };
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "data");
    let deserialized: ContentBlock = serde_json::from_value(json).unwrap();
    assert_eq!(block, deserialized);
}

#[test]
fn message_with_content_blocks_roundtrip() {
    let msg = Message::human("describe this image").with_content_blocks(vec![
        ContentBlock::Text {
            text: "describe this image".into(),
        },
        ContentBlock::Image {
            url: "https://example.com/photo.jpg".into(),
            detail: None,
        },
    ]);

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg, deserialized);
    assert_eq!(deserialized.content_blocks().len(), 2);
}

#[test]
fn message_content_blocks_accessor() {
    let msg = Message::ai("answer").with_content_blocks(vec![
        ContentBlock::Text {
            text: "answer".into(),
        },
        ContentBlock::Reasoning {
            content: "I thought about it".into(),
        },
    ]);
    let blocks = msg.content_blocks();
    assert_eq!(blocks.len(), 2);
    match &blocks[1] {
        ContentBlock::Reasoning { content } => assert_eq!(content, "I thought about it"),
        other => panic!("expected Reasoning, got {:?}", other),
    }
}

#[test]
fn message_empty_content_blocks_omitted_in_json() {
    let msg = Message::human("hello");
    let json = serde_json::to_value(&msg).unwrap();
    // content_blocks should be omitted when empty
    assert!(json.get("content_blocks").is_none());
}
