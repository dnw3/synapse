use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use synaptic_core::SynapticError;
use tokio::sync::Mutex;

use crate::api::cardkit::CardKitApi;
use crate::api::message::MessageApi;
use crate::LarkConfig;

/// Default throttle interval between card updates (500ms).
const DEFAULT_THROTTLE_MS: u64 = 500;

/// Options for creating a streaming card.
#[derive(Debug, Clone)]
pub struct StreamingCardOptions {
    /// Card header title.  Defaults to `""` (no header).
    pub title: String,
    /// Throttle interval between updates.  Defaults to 500ms.
    pub throttle: Duration,
}

impl Default for StreamingCardOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            throttle: Duration::from_millis(DEFAULT_THROTTLE_MS),
        }
    }
}

impl StreamingCardOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_throttle(mut self, dur: Duration) -> Self {
        self.throttle = dur;
        self
    }
}

/// Inner mutable state of the streaming card writer.
struct WriterState {
    card_id: String,
    message_id: String,
    sequence: i64,
    content: String,
    last_update: Instant,
    finished: bool,
}

/// The element_id used for the streaming markdown component.
const ELEMENT_ID: &str = "streaming_content";

/// A managed streaming card writer that handles the create → stream → finalize lifecycle.
///
/// The writer creates a Feishu Card entity with `streaming_mode: true`, sends it
/// as a message, and then progressively updates the markdown element's content
/// using the CardKit element content API (typewriter effect).
///
/// On [`finish`], a full card update disables `streaming_mode` so the client
/// stops showing the "Generating..." indicator.
///
/// # Example
///
/// ```rust,ignore
/// use synaptic_lark::{LarkConfig, bot::StreamingCardWriter, bot::StreamingCardOptions};
///
/// let config = LarkConfig::new("cli_xxx", "secret");
/// let opts = StreamingCardOptions::new().with_title("AI Response");
/// let writer = StreamingCardWriter::reply(config, "om_original_msg_id", opts).await?;
///
/// writer.write("Hello ").await?;
/// writer.write("World!").await?;
/// writer.finish().await?;
/// ```
pub struct StreamingCardWriter {
    cardkit: CardKitApi,
    state: Arc<Mutex<WriterState>>,
    options: StreamingCardOptions,
}

impl StreamingCardWriter {
    /// Create a streaming card and send it to a chat.  Returns the writer.
    ///
    /// Uses `receive_id_type` + `receive_id` to address the recipient
    /// (e.g. `"chat_id"` / `"oc_xxx"`).
    pub async fn send(
        config: LarkConfig,
        receive_id_type: &str,
        receive_id: &str,
        options: StreamingCardOptions,
    ) -> Result<Self, SynapticError> {
        let cardkit = CardKitApi::new(config.clone());
        let msg_api = MessageApi::new(config);

        let card_json = build_card_json_streaming(&options.title, "", true);
        let card_id = cardkit.create(&card_json).await?;

        let content_json = json!({
            "type": "card",
            "data": { "card_id": &card_id }
        })
        .to_string();
        let message_id = msg_api
            .send(receive_id_type, receive_id, "interactive", &content_json)
            .await?;

        Ok(Self {
            cardkit,
            state: Arc::new(Mutex::new(WriterState {
                card_id,
                message_id,
                sequence: 0,
                content: String::new(),
                last_update: Instant::now(),
                finished: false,
            })),
            options,
        })
    }

    /// Create a streaming card and reply to an existing message.  Returns the writer.
    pub async fn reply(
        config: LarkConfig,
        reply_to_message_id: &str,
        options: StreamingCardOptions,
    ) -> Result<Self, SynapticError> {
        let cardkit = CardKitApi::new(config.clone());
        let msg_api = MessageApi::new(config);

        let card_json = build_card_json_streaming(&options.title, "", true);
        let card_id = cardkit.create(&card_json).await?;

        let content_json = json!({
            "type": "card",
            "data": { "card_id": &card_id }
        })
        .to_string();
        let message_id = msg_api
            .reply(reply_to_message_id, "interactive", &content_json)
            .await?;

        Ok(Self {
            cardkit,
            state: Arc::new(Mutex::new(WriterState {
                card_id,
                message_id,
                sequence: 0,
                content: String::new(),
                last_update: Instant::now(),
                finished: false,
            })),
            options,
        })
    }

    /// Append text to the streaming card.
    ///
    /// If the throttle interval has elapsed since the last update, the card
    /// is updated immediately.  Otherwise the content is buffered and will
    /// be flushed on the next write or on [`finish`].
    pub async fn write(&self, text: &str) -> Result<(), SynapticError> {
        let mut state = self.state.lock().await;
        if state.finished {
            return Err(SynapticError::Tool(
                "StreamingCardWriter: already finished".to_string(),
            ));
        }
        state.content.push_str(text);

        let elapsed = state.last_update.elapsed();
        if elapsed >= self.options.throttle {
            self.flush_inner(&mut state).await?;
        }
        Ok(())
    }

    /// Flush any buffered content and send a final card update.
    ///
    /// The final update disables `streaming_mode` so the Feishu client stops
    /// showing the "Generating..." indicator.
    pub async fn finish(&self) -> Result<(), SynapticError> {
        let mut state = self.state.lock().await;
        if state.finished {
            return Ok(());
        }
        state.finished = true;
        // Final update with streaming_mode off to clear "Generating..." state.
        state.sequence += 1;
        let card_json = build_card_json_streaming(&self.options.title, &state.content, false);
        self.cardkit
            .update(&state.card_id, state.sequence, &card_json)
            .await?;
        state.last_update = Instant::now();
        Ok(())
    }

    /// Force an immediate flush of buffered content.
    pub async fn flush(&self) -> Result<(), SynapticError> {
        let mut state = self.state.lock().await;
        if state.finished {
            return Ok(());
        }
        self.flush_inner(&mut state).await
    }

    /// Returns the `card_id` of this streaming card.
    pub async fn card_id(&self) -> String {
        self.state.lock().await.card_id.clone()
    }

    /// Returns the `message_id` of the sent message.
    pub async fn message_id(&self) -> String {
        self.state.lock().await.message_id.clone()
    }

    /// Flush buffered content using the element-level streaming API (typewriter effect).
    async fn flush_inner(&self, state: &mut WriterState) -> Result<(), SynapticError> {
        state.sequence += 1;
        self.cardkit
            .stream_content(&state.card_id, ELEMENT_ID, &state.content, state.sequence)
            .await?;
        state.last_update = Instant::now();
        Ok(())
    }
}

/// Build a Card JSON 2.0 structure with a markdown body.
///
/// When `streaming` is `true`, enables client-side typewriter animation:
/// - `streaming_mode: true` in config
/// - `element_id` on the markdown element for targeted updates
/// - Default `streaming_config` with `print_frequency_ms: 30` and `print_step: 2`
///
/// Set `streaming` to `false` on the final update to clear the "Generating..." state.
pub fn build_card_json(title: &str, markdown_content: &str) -> Value {
    build_card_json_streaming(title, markdown_content, false)
}

/// Build a Card JSON 2.0 structure with streaming mode control.
///
/// - `streaming = true` — enables typewriter animation on the Feishu client
/// - `streaming = false` — static card (or final update to end the animation)
pub fn build_card_json_streaming(title: &str, markdown_content: &str, streaming: bool) -> Value {
    let mut config = json!({ "update_multi": true });
    if streaming {
        config["streaming_mode"] = json!(true);
        config["streaming_config"] = json!({
            "print_frequency_ms": { "default": 30 },
            "print_step": { "default": 2 },
            "print_strategy": "fast"
        });
    }

    let mut card = json!({
        "schema": "2.0",
        "config": config,
        "body": {
            "elements": [{
                "tag": "markdown",
                "content": markdown_content,
                "element_id": "streaming_content"
            }]
        }
    });
    if !title.is_empty() {
        card["header"] = json!({
            "title": {
                "tag": "plain_text",
                "content": title
            }
        });
    }
    card
}
