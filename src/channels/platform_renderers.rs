//! Platform-specific IR renderers that live in synapse until dedicated adapter crates exist.
//!
//! - [`SlackRenderer`] — Slack mrkdwn format.
//! - [`TelegramRenderer`] — Telegram HTML parse mode.

use synaptic::core::message_ir::{
    apply_spans, escape_html, render_table_plain, Block, IRRenderer, InlineFormatter, MessageIR,
    RenderOptions,
};

// ===========================================================================
// Slack mrkdwn renderer
// ===========================================================================

/// Renders IR to Slack mrkdwn format.
pub struct SlackRenderer;

impl IRRenderer for SlackRenderer {
    fn render(&self, ir: &MessageIR, options: &RenderOptions) -> String {
        let mut out = String::new();
        for (i, block) in ir.blocks.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match block {
                Block::Paragraph(rt) => {
                    out.push_str(&apply_spans(rt, &SlackFormatter));
                    out.push('\n');
                }
                Block::CodeBlock { code, .. } => {
                    out.push_str("```\n");
                    out.push_str(code);
                    out.push_str("\n```\n");
                }
                Block::Heading { text, .. } => {
                    // Slack has no heading syntax; use bold
                    out.push('*');
                    out.push_str(&apply_spans(text, &SlackFormatter));
                    out.push_str("*\n");
                }
                Block::List { ordered, items } => {
                    for (idx, item) in items.iter().enumerate() {
                        if *ordered {
                            out.push_str(&format!("{}. ", idx + 1));
                        } else {
                            out.push_str("\u{2022} ");
                        }
                        out.push_str(&apply_spans(item, &SlackFormatter));
                        out.push('\n');
                    }
                }
                Block::Blockquote(rt) => {
                    for line in apply_spans(rt, &SlackFormatter).lines() {
                        out.push_str("&gt; ");
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                Block::Table { headers, rows } => {
                    render_table_plain(&mut out, headers, rows, options.table_mode);
                }
                Block::ThematicBreak => {
                    out.push_str("---\n");
                }
                Block::Image { alt, url } => {
                    if options.preserve_images {
                        out.push_str(&format!("<{}|{}>\n", url, alt));
                    }
                }
            }
        }
        out
    }
}

struct SlackFormatter;

impl InlineFormatter for SlackFormatter {
    fn wrap_bold(&self, text: &str) -> String {
        format!("*{}*", text)
    }
    fn wrap_italic(&self, text: &str) -> String {
        format!("_{}_", text)
    }
    fn wrap_strikethrough(&self, text: &str) -> String {
        format!("~{}~", text)
    }
    fn wrap_code(&self, text: &str) -> String {
        format!("`{}`", text)
    }
    fn wrap_spoiler(&self, text: &str) -> String {
        // Slack doesn't have spoiler syntax
        text.to_string()
    }
    fn wrap_link(&self, label: &str, href: &str) -> String {
        format!("<{}|{}>", href, label)
    }
}

// ===========================================================================
// Telegram HTML renderer
// ===========================================================================

/// Renders IR to Telegram HTML parse mode.
pub struct TelegramRenderer;

impl IRRenderer for TelegramRenderer {
    fn render(&self, ir: &MessageIR, options: &RenderOptions) -> String {
        let mut out = String::new();
        for (i, block) in ir.blocks.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match block {
                Block::Paragraph(rt) => {
                    out.push_str(&apply_spans(rt, &TelegramFormatter));
                    out.push('\n');
                }
                Block::CodeBlock { language, code } => {
                    if let Some(lang) = language {
                        out.push_str(&format!(
                            "<pre><code class=\"language-{}\">{}</code></pre>\n",
                            escape_html(lang),
                            escape_html(code)
                        ));
                    } else {
                        out.push_str(&format!("<pre><code>{}</code></pre>\n", escape_html(code)));
                    }
                }
                Block::Heading { text, .. } => {
                    out.push_str("<b>");
                    out.push_str(&apply_spans(text, &TelegramFormatter));
                    out.push_str("</b>\n");
                }
                Block::List { ordered, items } => {
                    for (idx, item) in items.iter().enumerate() {
                        if *ordered {
                            out.push_str(&format!("{}. ", idx + 1));
                        } else {
                            out.push_str("\u{2022} ");
                        }
                        out.push_str(&apply_spans(item, &TelegramFormatter));
                        out.push('\n');
                    }
                }
                Block::Blockquote(rt) => {
                    out.push_str("<blockquote>");
                    out.push_str(&apply_spans(rt, &TelegramFormatter));
                    out.push_str("</blockquote>\n");
                }
                Block::Table { headers, rows } => {
                    render_table_plain(&mut out, headers, rows, options.table_mode);
                }
                Block::ThematicBreak => {
                    out.push_str("---\n");
                }
                Block::Image { alt, url } => {
                    if options.preserve_images {
                        out.push_str(&format!(
                            "<a href=\"{}\">{}</a>\n",
                            escape_html(url),
                            escape_html(alt)
                        ));
                    }
                }
            }
        }
        out
    }
}

struct TelegramFormatter;

impl InlineFormatter for TelegramFormatter {
    fn wrap_bold(&self, text: &str) -> String {
        format!("<b>{}</b>", text)
    }
    fn wrap_italic(&self, text: &str) -> String {
        format!("<i>{}</i>", text)
    }
    fn wrap_strikethrough(&self, text: &str) -> String {
        format!("<s>{}</s>", text)
    }
    fn wrap_code(&self, text: &str) -> String {
        format!("<code>{}</code>", text)
    }
    fn wrap_spoiler(&self, text: &str) -> String {
        format!("<tg-spoiler>{}</tg-spoiler>", text)
    }
    fn wrap_link(&self, label: &str, href: &str) -> String {
        format!("<a href=\"{}\">{}</a>", escape_html(href), label)
    }
    fn escape_text(&self, text: &str) -> String {
        escape_html(text)
    }
}
