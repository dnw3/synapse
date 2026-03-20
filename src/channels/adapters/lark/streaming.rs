use async_trait::async_trait;
use synaptic::lark::StreamingCardWriter;

use crate::channels::handler::{CompletionMeta, StreamingOutput, ToolCallInfo};
use crate::config::bots::LarkCardConfig;

// ---------------------------------------------------------------------------
// Streaming output adapter
// ---------------------------------------------------------------------------

pub(super) struct LarkStreamingOutput {
    pub writer: StreamingCardWriter,
    pub card_config: LarkCardConfig,
    pub bot_name: String,
}

#[async_trait]
impl StreamingOutput for LarkStreamingOutput {
    async fn on_token(&self, token: &str) {
        tracing::debug!(len = token.len(), "lark streaming: on_token");
        self.writer.write(token).await.ok();
    }

    async fn on_tool_call(&self, info: &ToolCallInfo) {
        // Skip internal polling tools
        if info.name == "TaskOutput" || info.name == "TaskStatus" {
            return;
        }

        let display = if info.name == "task" {
            // Extract description from task args for user-friendly display
            let desc = serde_json::from_str::<serde_json::Value>(&info.args)
                .ok()
                .and_then(|v| v["description"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "sub-task".to_string());
            // Truncate long descriptions
            let desc = if desc.len() > 60 {
                format!("{}...", &desc[..57])
            } else {
                desc
            };
            format!("\n\u{1f916} {}\n", desc)
        } else {
            format!("\n\u{1f527} {}\n", info.name)
        };

        self.writer.write(&display).await.ok();
        self.writer.flush().await.ok();
    }

    async fn on_complete(&self, full_response: &str, meta: Option<&CompletionMeta>) {
        use synaptic::core::message_ir::{parse_markdown, RenderOptions, RenderTarget};
        use synaptic::lark::card_elements::render_lark_card_elements;

        // Skip styled card for very short responses
        let ir = parse_markdown(full_response);
        if ir.blocks.len() <= 1 && full_response.len() < 20 {
            self.writer.finish().await.ok();
            return;
        }

        let options = RenderOptions::new(RenderTarget::LarkCard);
        let mut elements = render_lark_card_elements(&ir, &options);

        // Build footer info line from metadata + config
        if let Some(meta) = meta {
            let footer = build_card_footer(&self.card_config, meta);
            if !footer.is_empty() {
                elements.push(synaptic::lark::card_elements::LarkCardElement {
                    tag: "hr".into(),
                    element_id: "meta_hr".into(),
                    properties: serde_json::json!({}),
                });
                elements.push(synaptic::lark::card_elements::LarkCardElement {
                    tag: "markdown".into(),
                    element_id: "meta_info".into(),
                    properties: serde_json::json!({"content": footer}),
                });
            }
        }

        let card_json = assemble_final_card(elements, &self.card_config, &self.bot_name);
        // Debug: log last few elements to check footer presence
        if let Some(elems) = card_json["body"]["elements"].as_array() {
            let last_3: Vec<_> = elems.iter().rev().take(3).collect();
            tracing::debug!(last_elements = ?last_3, "card footer debug");
        }
        if let Err(e) = self.writer.finish_with_card(card_json).await {
            tracing::error!("lark streaming: finish_with_card failed: {e}");
        }
    }

    async fn on_error(&self, error: &str) {
        use synaptic::lark::card_elements::LarkCardElement;

        let error_config = LarkCardConfig {
            template: "red".into(),
            header_title: "Error".into(),
            show_feedback: false,
            ..self.card_config.clone()
        };
        let elements = vec![LarkCardElement {
            tag: "markdown".into(),
            element_id: "e0md".into(),
            properties: serde_json::json!({"content": error}),
        }];
        let card_json = assemble_final_card(elements, &error_config, &self.bot_name);
        self.writer.finish_with_card(card_json).await.ok();
    }
}

// ---------------------------------------------------------------------------
// Business-specific card elements (feedback, timestamp, metadata)
// ---------------------------------------------------------------------------

/// Append business-specific footer elements (timestamp, feedback buttons)
/// to a card element list based on [`LarkCardConfig`].
fn append_business_footer(
    elements: &mut Vec<synaptic::lark::card_elements::LarkCardElement>,
    config: &LarkCardConfig,
) {
    // Timestamp
    if config.show_timestamp {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        elements.push(synaptic::lark::card_elements::LarkCardElement {
            tag: "markdown".into(),
            element_id: "ts_footer".into(),
            properties: serde_json::json!({ "content": format!("_{}_", now) }),
        });
    }

    // Feedback buttons
    if config.show_feedback {
        elements.push(synaptic::lark::card_elements::LarkCardElement {
            tag: "hr".into(),
            element_id: "fb_hr".into(),
            properties: serde_json::json!({}),
        });
        elements.push(synaptic::lark::card_elements::LarkCardElement {
            tag: "column_set".into(),
            element_id: "fb_actions".into(),
            properties: serde_json::json!({
                "flex_mode": "none",
                "horizontal_spacing": "8px",
                "columns": [
                    {
                        "tag": "column",
                        "width": "auto",
                        "elements": [{
                            "tag": "button",
                            "text": { "tag": "plain_text", "content": "\u{1f44d}" },
                            "type": "text",
                            "size": "small",
                            "value": { "action": "feedback_positive" },
                        }]
                    },
                    {
                        "tag": "column",
                        "width": "auto",
                        "elements": [{
                            "tag": "button",
                            "text": { "tag": "plain_text", "content": "\u{1f44e}" },
                            "type": "text",
                            "size": "small",
                            "value": { "action": "feedback_negative" },
                        }]
                    }
                ]
            }),
        });
    }
}

/// Build a framework [`CardConfig`] from synapse's [`LarkCardConfig`] and bot name.
fn make_card_config(config: &LarkCardConfig, bot_name: &str) -> synaptic::lark::CardConfig {
    let title = if config.header_title.is_empty() {
        bot_name
    } else {
        &config.header_title
    };
    synaptic::lark::CardConfig {
        header_title: title.to_string(),
        template: config.template.clone(),
        header_icon: config.header_icon.clone(),
    }
}

/// Assemble a complete card from elements using the framework's card builder,
/// with business-specific footer elements appended.
pub(super) fn assemble_final_card(
    mut elements: Vec<synaptic::lark::card_elements::LarkCardElement>,
    config: &LarkCardConfig,
    bot_name: &str,
) -> serde_json::Value {
    append_business_footer(&mut elements, config);
    let card_config = make_card_config(config, bot_name);
    synaptic::lark::assemble_card(elements, &card_config)
}

// ---------------------------------------------------------------------------
// Card footer
// ---------------------------------------------------------------------------

/// Build a metadata footer line for the card based on config flags.
pub(super) fn build_card_footer(config: &LarkCardConfig, meta: &CompletionMeta) -> String {
    let mut parts = Vec::new();

    if config.show_usage && (meta.input_tokens > 0 || meta.output_tokens > 0) {
        let input = format_token_count(meta.input_tokens);
        let output = format_token_count(meta.output_tokens);
        let total = format_token_count(meta.input_tokens + meta.output_tokens);
        parts.push(format!(
            "\u{1f4ca} {} \u{2191}  {} \u{2193}  {} total",
            input, output, total
        ));
    }

    if config.show_latency && meta.duration_ms > 0 {
        parts.push(format!("\u{23f1} {}", format_duration(meta.duration_ms)));
    }

    let main_line = parts.join("  ");

    if config.show_logid {
        if let Some(ref rid) = meta.request_id {
            if !rid.is_empty() {
                if main_line.is_empty() {
                    return format!("\u{1f517} `{}`", rid);
                }
                return format!("{}\n\u{1f517} `{}`", main_line, rid);
            }
        }
    }

    main_line
}

fn format_token_count(tokens: u32) -> String {
    if tokens >= 100_000 {
        format!("{:.0}K", tokens as f64 / 1000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1000.0)
    } else {
        format!("{}", tokens)
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m {}s", mins, secs)
    }
}
