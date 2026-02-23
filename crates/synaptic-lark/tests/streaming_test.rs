#![cfg(feature = "bot")]

use synaptic_lark::bot::{build_card_json, build_card_json_streaming, StreamingCardOptions};
use synaptic_lark::LarkConfig;

// ── Card JSON builder ────────────────────────────────────────────

#[test]
fn build_card_json_with_title() {
    let card = build_card_json("AI Response", "Hello world");
    assert_eq!(card["schema"], "2.0");
    assert_eq!(card["config"]["update_multi"], true);
    assert_eq!(card["header"]["title"]["content"], "AI Response");
    assert_eq!(card["body"]["elements"][0]["tag"], "markdown");
    assert_eq!(card["body"]["elements"][0]["content"], "Hello world");
    assert_eq!(
        card["body"]["elements"][0]["element_id"],
        "streaming_content"
    );
}

#[test]
fn build_card_json_without_title() {
    let card = build_card_json("", "Some content");
    assert!(card.get("header").is_none());
    assert_eq!(card["body"]["elements"][0]["content"], "Some content");
}

#[test]
fn build_card_json_empty_content() {
    let card = build_card_json("Title", "");
    assert_eq!(card["body"]["elements"][0]["content"], "");
}

#[test]
fn build_card_json_markdown_content() {
    let md = "## Heading\n\n- item 1\n- item 2\n\n```rust\nfn main() {}\n```";
    let card = build_card_json("", md);
    assert_eq!(card["body"]["elements"][0]["content"], md);
}

// ── Card JSON builder (streaming mode) ──────────────────────────

#[test]
fn build_card_json_streaming_enabled() {
    let card = build_card_json_streaming("Title", "content", true);
    assert_eq!(card["config"]["streaming_mode"], true);
    assert_eq!(card["config"]["streaming_config"]["print_strategy"], "fast");
    assert_eq!(
        card["config"]["streaming_config"]["print_frequency_ms"]["default"],
        30
    );
    assert_eq!(
        card["config"]["streaming_config"]["print_step"]["default"],
        2
    );
    assert_eq!(
        card["body"]["elements"][0]["element_id"],
        "streaming_content"
    );
}

#[test]
fn build_card_json_streaming_disabled() {
    let card = build_card_json_streaming("Title", "final content", false);
    assert!(card["config"].get("streaming_mode").is_none());
    assert!(card["config"].get("streaming_config").is_none());
    assert_eq!(card["config"]["update_multi"], true);
    assert_eq!(card["body"]["elements"][0]["content"], "final content");
}

// ── StreamingCardOptions ─────────────────────────────────────────

#[test]
fn streaming_options_defaults() {
    let opts = StreamingCardOptions::default();
    assert_eq!(opts.title, "");
    assert_eq!(opts.throttle, std::time::Duration::from_millis(500));
}

#[test]
fn streaming_options_builder() {
    let opts = StreamingCardOptions::new()
        .with_title("My Title")
        .with_throttle(std::time::Duration::from_millis(200));
    assert_eq!(opts.title, "My Title");
    assert_eq!(opts.throttle, std::time::Duration::from_millis(200));
}

// ── LarkBotClient card method existence ──────────────────────────

#[test]
fn bot_client_has_config() {
    let client = synaptic_lark::LarkBotClient::new(LarkConfig::new("cli_test", "secret_test"));
    assert_eq!(client.app_id(), "cli_test");
}

// ── Integration tests (require credentials) ──────────────────────

/// Full streaming card lifecycle: create → write → write → finish.
///
/// Requires env vars: LARK_APP_ID, LARK_APP_SECRET, LARK_TEST_CHAT_ID
#[tokio::test]
#[ignore = "requires LARK_APP_ID, LARK_APP_SECRET, and LARK_TEST_CHAT_ID"]
async fn integration_streaming_send() {
    let app_id = std::env::var("LARK_APP_ID").unwrap();
    let app_secret = std::env::var("LARK_APP_SECRET").unwrap();
    let chat_id = std::env::var("LARK_TEST_CHAT_ID").unwrap();

    let config = LarkConfig::new(&app_id, &app_secret);
    let client = synaptic_lark::LarkBotClient::new(config);

    let opts = StreamingCardOptions::new().with_title("Streaming Test");
    let writer = client
        .streaming_send("chat_id", &chat_id, opts)
        .await
        .expect("streaming_send failed");

    let card_id = writer.card_id().await;
    assert!(!card_id.is_empty(), "card_id should not be empty");

    let msg_id = writer.message_id().await;
    assert!(!msg_id.is_empty(), "message_id should not be empty");

    // Simulate streaming content
    writer.write("Hello ").await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
    writer.write("World! ").await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
    writer.write("This is streaming.").await.unwrap();

    writer.finish().await.unwrap();
}

/// Full streaming card lifecycle via reply.
#[tokio::test]
#[ignore = "requires LARK_APP_ID, LARK_APP_SECRET, and LARK_TEST_MESSAGE_ID"]
async fn integration_streaming_reply() {
    let app_id = std::env::var("LARK_APP_ID").unwrap();
    let app_secret = std::env::var("LARK_APP_SECRET").unwrap();
    let message_id = std::env::var("LARK_TEST_MESSAGE_ID").unwrap();

    let config = LarkConfig::new(&app_id, &app_secret);
    let client = synaptic_lark::LarkBotClient::new(config);

    let opts = StreamingCardOptions::new().with_title("Reply Stream");
    let writer = client
        .streaming_reply(&message_id, opts)
        .await
        .expect("streaming_reply failed");

    writer.write("Thinking...").await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    writer.write("\n\nDone!").await.unwrap();

    writer.finish().await.unwrap();
}

/// Low-level create_card and update_card test.
#[tokio::test]
#[ignore = "requires LARK_APP_ID and LARK_APP_SECRET"]
async fn integration_create_and_update_card() {
    let app_id = std::env::var("LARK_APP_ID").unwrap();
    let app_secret = std::env::var("LARK_APP_SECRET").unwrap();

    let config = LarkConfig::new(&app_id, &app_secret);
    let client = synaptic_lark::LarkBotClient::new(config);

    let card_json = build_card_json("Test Card", "Initial content");
    let card_id = client.create_card(&card_json).await.unwrap();
    assert!(!card_id.is_empty());

    // Update the card
    let updated_card = build_card_json("Test Card", "Updated content");
    client
        .update_card(&card_id, 1, &updated_card)
        .await
        .unwrap();

    // Second update with higher sequence
    let final_card = build_card_json("Test Card", "Final content");
    client.update_card(&card_id, 2, &final_card).await.unwrap();
}
