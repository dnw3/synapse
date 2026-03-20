//! WebSocket client for connecting to a remote Synapse Gateway.
//!
//! Used by `synapse connect <url>` to interact with a remote gateway
//! while preserving the local REPL experience.

use colored::Colorize;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message as WsMsg;

/// Events received from the gateway server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum WsEvent {
    #[serde(rename = "token")]
    Token { content: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult { name: String, content: String },
    #[serde(rename = "status")]
    Status { state: String },
    #[serde(rename = "canvas_update")]
    CanvasUpdate {
        block_type: String,
        content: String,
        language: Option<String>,
        attributes: Option<serde_json::Value>,
    },
    #[serde(rename = "done")]
    Done {},
    #[serde(rename = "error")]
    Error { message: String },
}

/// Commands sent to the gateway server.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum WsCommand {
    #[serde(rename = "message")]
    SendMessage { content: String },
    #[serde(rename = "cancel")]
    Cancel {},
}

/// Gateway client that connects over WebSocket.
pub struct GatewayClient {
    write: futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        WsMsg,
    >,
    read: futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
}

impl GatewayClient {
    /// Connect to a remote gateway WebSocket endpoint.
    pub async fn connect(
        url: &str,
        session: Option<&str>,
        _token: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Ensure URL ends with /ws (unified endpoint, session key sent per-request)
        let _ = session; // session key is now sent per-request in chat.send params
        let ws_url = if url.ends_with("/ws") {
            url.to_string()
        } else {
            let base = url.trim_end_matches('/');
            format!("{}/ws", base)
        };

        tracing::info!(url = %ws_url, "connecting to gateway");

        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await?;
        let (write, read) = ws_stream.split();

        tracing::info!("gateway connected");

        Ok(Self { write, read })
    }

    /// Send a message to the gateway.
    pub async fn send_message(&mut self, content: &str) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = WsCommand::SendMessage {
            content: content.to_string(),
        };
        let json = serde_json::to_string(&cmd)?;
        self.write.send(WsMsg::Text(json.into())).await?;
        Ok(())
    }

    /// Send a cancel signal.
    pub async fn send_cancel(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = WsCommand::Cancel {};
        let json = serde_json::to_string(&cmd)?;
        self.write.send(WsMsg::Text(json.into())).await?;
        Ok(())
    }

    /// Receive and display events until Done or Error.
    /// Returns the final response text.
    pub async fn recv_until_done(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let mut full_response = String::new();

        while let Some(msg) = self.read.next().await {
            let msg = msg?;
            let text = match msg {
                WsMsg::Text(t) => t.to_string(),
                WsMsg::Close(_) => break,
                _ => continue,
            };

            let event: WsEvent = match serde_json::from_str(&text) {
                Ok(e) => e,
                Err(_) => continue,
            };

            match event {
                WsEvent::Token { content } => {
                    full_response = content.clone();
                    // Print the token content
                    print!("{}", content);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                WsEvent::ToolCall { name, args } => {
                    let args_preview = serde_json::to_string(&args).unwrap_or_default();
                    let preview = if args_preview.len() > 100 {
                        format!("{}...", &args_preview[..97])
                    } else {
                        args_preview
                    };
                    eprintln!(
                        "\n{} {}({})",
                        "tool:".blue().bold(),
                        name.yellow(),
                        preview.dimmed()
                    );
                }
                WsEvent::ToolResult { name, content } => {
                    let preview = if content.len() > 200 {
                        format!("{}...", &content[..197])
                    } else {
                        content
                    };
                    eprintln!(
                        "{} {} → {}",
                        "result:".blue().bold(),
                        name.yellow(),
                        preview.dimmed()
                    );
                }
                WsEvent::Status { state } => {
                    eprintln!("{} {}", "status:".dimmed(), state.dimmed());
                }
                WsEvent::CanvasUpdate {
                    block_type,
                    content,
                    ..
                } => {
                    eprintln!(
                        "\n{} [{}]\n{}",
                        "canvas:".magenta().bold(),
                        block_type,
                        content
                    );
                }
                WsEvent::Done {} => {
                    println!(); // Final newline
                    break;
                }
                WsEvent::Error { message } => {
                    eprintln!("\n{} {}", "error:".red().bold(), message);
                    return Err(message.into());
                }
            }
        }

        Ok(full_response)
    }
}

/// Run the gateway client REPL — connect to a remote gateway and chat interactively.
pub async fn run_connect(
    url: &str,
    session: Option<&str>,
    token: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = GatewayClient::connect(url, session, token).await?;

    // Set up readline for interactive input
    let mut rl = rustyline::DefaultEditor::new()?;

    loop {
        let readline = rl.readline("you> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                if input == "/quit" || input == "/exit" {
                    break;
                }

                rl.add_history_entry(input).ok();

                // Send message and stream response
                client.send_message(input).await?;
                client.recv_until_done().await?;
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl-C → send cancel
                client.send_cancel().await.ok();
                tracing::info!("gateway request cancelled");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                tracing::error!(error = %e, "readline error");
                break;
            }
        }
    }

    Ok(())
}
