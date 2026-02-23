//! Lark streaming bot example — demonstrates streaming card output.
//!
//! This bot replies to every incoming message with a streaming card that
//! simulates progressive AI generation (typewriter effect).
//!
//! ## Setup
//!
//! 1. Create a Feishu bot at <https://open.feishu.cn/app>
//! 2. Enable long-connection (WebSocket) mode in the bot configuration
//! 3. Set environment variables:
//!    ```sh
//!    export LARK_APP_ID=cli_xxx
//!    export LARK_APP_SECRET=your_secret
//!    ```
//! 4. Run:
//!    ```sh
//!    cargo run -p lark_streaming_bot
//!    ```

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::lark::bot::{
    LarkBotClient, LarkLongConnListener, LarkMessageEvent, MessageHandler, StreamingCardOptions,
};
use synaptic::lark::LarkConfig;

struct StreamingHandler;

#[async_trait]
impl MessageHandler for StreamingHandler {
    async fn handle(
        &self,
        event: LarkMessageEvent,
        client: &LarkBotClient,
    ) -> Result<(), SynapticError> {
        let user_text = event.text();
        println!("[user] {}", user_text);

        // Start a streaming card reply
        let opts = StreamingCardOptions::new().with_title("AI Response");
        let writer = client.streaming_reply(event.message_id(), opts).await?;

        // Simulate progressive generation
        let response = format!(
            "You said: **{}**\n\nLet me think about that...\n\n\
             Here's my response:\n\n\
             1. First point about your question\n\
             2. Second point with more detail\n\
             3. Third point wrapping up\n\n\
             Hope this helps!",
            user_text
        );

        // Stream the response character by character (in chunks for efficiency)
        let chunks: Vec<&str> = response
            .as_bytes()
            .chunks(10)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect();

        for chunk in chunks {
            writer.write(chunk).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        writer.finish().await?;
        println!(
            "[bot] streaming reply completed for message {}",
            event.message_id()
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_id = std::env::var("LARK_APP_ID").expect("LARK_APP_ID must be set");
    let app_secret = std::env::var("LARK_APP_SECRET").expect("LARK_APP_SECRET must be set");

    println!("Starting Lark streaming bot (app_id={app_id})...");

    let config = LarkConfig::new(&app_id, &app_secret);
    LarkLongConnListener::new(config)
        .with_message_handler(StreamingHandler)
        .run()
        .await?;

    Ok(())
}
