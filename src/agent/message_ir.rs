//! Message IR (Intermediate Representation) for cross-format message handling.
//!
//! This module provides a simplified, synapse-level IR that wraps the richer
//! `synaptic::core::message_ir` types with a flat enum representation suited for
//! quick inspection, transformation, and bot-channel formatting.
//!
//! For full parse fidelity (inline styles, link spans, Signal ranges, Lark cards)
//! use `synaptic::core::message_ir` directly. This module delegates to it
//! internally so behaviour is consistent with the `channels::formatter` pipeline.

#![allow(dead_code)]
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core IR type
// ---------------------------------------------------------------------------

/// A single content node in a message, represented as a flat enum.
///
/// This is the synapse-level IR. It is intentionally simpler than
/// `synaptic::core::message_ir::Block` to make pattern matching and quick
/// manipulation straightforward. The full synaptic IR (with inline style spans
/// and link maps) is used by the rendering pipeline under the hood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageIR {
    /// Plain paragraph text (may contain inline markup, passed through as-is).
    Text(String),
    /// Fenced or indented code block.
    Code { language: String, content: String },
    /// An image reference.
    Image { url: String, alt: Option<String> },
    /// A hyperlink.
    Link { url: String, title: Option<String> },
    /// An ordered or unordered list.
    List {
        ordered: bool,
        items: Vec<MessageIR>,
    },
    /// A section heading (level 1–6).
    Heading { level: u8, content: String },
    /// A block quote.
    Quote(String),
    /// A table with named columns.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// A horizontal rule / thematic break.
    HorizontalRule,
    /// Inline bold text (for convenience when constructing IR programmatically).
    Bold(String),
    /// Inline italic text.
    Italic(String),
}

// ---------------------------------------------------------------------------
// markdown_to_ir
// ---------------------------------------------------------------------------

/// Parse markdown text into a list of [`MessageIR`] nodes.
///
/// Delegates to `synaptic::core::message_ir::parse_markdown` for accurate
/// CommonMark parsing, then maps each `Block` to the simpler synapse IR.
pub fn markdown_to_ir(md: &str) -> Vec<MessageIR> {
    use synaptic::core::message_ir::parse_markdown;

    let doc = parse_markdown(md);
    doc.blocks.into_iter().map(block_to_ir).collect()
}

/// Convert a synaptic `Block` into a synapse `MessageIR` node.
fn block_to_ir(block: synaptic::core::message_ir::Block) -> MessageIR {
    use synaptic::core::message_ir::Block;

    match block {
        Block::Paragraph(rt) => MessageIR::Text(rt.text),
        Block::Heading { level, text } => MessageIR::Heading {
            level,
            content: text.text,
        },
        Block::CodeBlock { language, code } => MessageIR::Code {
            language: language.unwrap_or_default(),
            content: code,
        },
        Block::Blockquote(rt) => MessageIR::Quote(rt.text),
        Block::List { ordered, items } => MessageIR::List {
            ordered,
            items: items
                .into_iter()
                .map(|rt| MessageIR::Text(rt.text))
                .collect(),
        },
        Block::Table { headers, rows } => MessageIR::Table { headers, rows },
        Block::ThematicBreak => MessageIR::HorizontalRule,
        Block::Image { alt, url } => MessageIR::Image {
            url,
            alt: if alt.is_empty() { None } else { Some(alt) },
        },
    }
}

// ---------------------------------------------------------------------------
// ir_to_markdown
// ---------------------------------------------------------------------------

/// Render a slice of [`MessageIR`] nodes back to Markdown text.
///
/// This provides a best-effort round-trip. Inline `Bold` / `Italic` / `Link`
/// nodes are rendered with standard Markdown syntax. `Text` nodes are passed
/// through verbatim (they may already contain Markdown inline markup).
pub fn ir_to_markdown(ir: &[MessageIR]) -> String {
    let mut out = String::new();
    for (i, node) in ir.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        render_node_markdown(node, &mut out);
    }
    out
}

fn render_node_markdown(node: &MessageIR, out: &mut String) {
    match node {
        MessageIR::Text(s) => {
            out.push_str(s);
            out.push('\n');
        }
        MessageIR::Heading { level, content } => {
            for _ in 0..*level {
                out.push('#');
            }
            out.push(' ');
            out.push_str(content);
            out.push('\n');
        }
        MessageIR::Code { language, content } => {
            out.push_str("```");
            out.push_str(language);
            out.push('\n');
            out.push_str(content);
            out.push_str("\n```\n");
        }
        MessageIR::Quote(s) => {
            for line in s.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        }
        MessageIR::List { ordered, items } => {
            for (idx, item) in items.iter().enumerate() {
                if *ordered {
                    out.push_str(&format!("{}. ", idx + 1));
                } else {
                    out.push_str("- ");
                }
                // Items may themselves be Text or nested nodes; render inline.
                match item {
                    MessageIR::Text(s) => out.push_str(s),
                    MessageIR::Bold(s) => {
                        out.push_str("**");
                        out.push_str(s);
                        out.push_str("**");
                    }
                    MessageIR::Italic(s) => {
                        out.push('*');
                        out.push_str(s);
                        out.push('*');
                    }
                    other => {
                        let mut sub = String::new();
                        render_node_markdown(other, &mut sub);
                        out.push_str(sub.trim_end_matches('\n'));
                    }
                }
                out.push('\n');
            }
        }
        MessageIR::Table { headers, rows } => {
            out.push('|');
            for h in headers {
                out.push_str(&format!(" {} |", h));
            }
            out.push('\n');
            out.push('|');
            for _ in headers {
                out.push_str(" --- |");
            }
            out.push('\n');
            for row in rows {
                out.push('|');
                for cell in row {
                    out.push_str(&format!(" {} |", cell));
                }
                out.push('\n');
            }
        }
        MessageIR::HorizontalRule => {
            out.push_str("---\n");
        }
        MessageIR::Image { url, alt } => {
            let alt_text = alt.as_deref().unwrap_or("");
            out.push_str(&format!("![{}]({})\n", alt_text, url));
        }
        MessageIR::Link { url, title } => {
            let label = title.as_deref().unwrap_or(url.as_str());
            out.push_str(&format!("[{}]({})\n", label, url));
        }
        MessageIR::Bold(s) => {
            out.push_str(&format!("**{}**\n", s));
        }
        MessageIR::Italic(s) => {
            out.push_str(&format!("*{}*\n", s));
        }
    }
}

// ---------------------------------------------------------------------------
// ir_to_channel_format
// ---------------------------------------------------------------------------

/// Convert a slice of [`MessageIR`] nodes to channel-specific formatted strings,
/// respecting `limit` characters per chunk.
///
/// The nodes are first rendered back to Markdown, then passed through the
/// `channels::formatter::format_for_channel` pipeline which handles per-platform
/// rendering (Slack mrkdwn, Telegram HTML, plain text, etc.) and chunking.
///
/// `channel` should be one of the well-known channel identifiers:
/// `"lark"`, `"slack"`, `"telegram"`, `"discord"`, `"whatsapp"`, etc.
/// Unknown channels fall back to plain-text chunking.
pub fn ir_to_channel_format(ir: &[MessageIR], channel: &str, limit: usize) -> Vec<String> {
    let md = ir_to_markdown(ir);
    crate::channels::formatter::format_for_channel(&md, channel, limit)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- markdown_to_ir ----------------------------------------------------

    #[test]
    fn parse_heading_levels() {
        let ir = markdown_to_ir("# H1\n## H2\n### H3");
        assert_eq!(ir.len(), 3);
        assert!(matches!(&ir[0], MessageIR::Heading { level: 1, content } if content == "H1"));
        assert!(matches!(&ir[1], MessageIR::Heading { level: 2, content } if content == "H2"));
        assert!(matches!(&ir[2], MessageIR::Heading { level: 3, content } if content == "H3"));
    }

    #[test]
    fn parse_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let ir = markdown_to_ir(md);
        assert_eq!(ir.len(), 1);
        match &ir[0] {
            MessageIR::Code { language, content } => {
                assert_eq!(language, "rust");
                assert_eq!(content, "fn main() {}");
            }
            other => panic!("expected Code, got {:?}", other),
        }
    }

    #[test]
    fn parse_blockquote() {
        let ir = markdown_to_ir("> some quoted text");
        assert_eq!(ir.len(), 1);
        assert!(matches!(&ir[0], MessageIR::Quote(s) if s == "some quoted text"));
    }

    #[test]
    fn parse_plain_paragraph() {
        let ir = markdown_to_ir("Hello, world!");
        assert_eq!(ir.len(), 1);
        assert!(matches!(&ir[0], MessageIR::Text(s) if s == "Hello, world!"));
    }

    #[test]
    fn parse_unordered_list() {
        let md = "- alpha\n- beta\n- gamma";
        let ir = markdown_to_ir(md);
        assert_eq!(ir.len(), 1);
        match &ir[0] {
            MessageIR::List { ordered, items } => {
                assert!(!ordered);
                assert_eq!(items.len(), 3);
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[test]
    fn parse_horizontal_rule() {
        let md = "before\n\n---\n\nafter";
        let ir = markdown_to_ir(md);
        assert!(
            ir.iter().any(|n| matches!(n, MessageIR::HorizontalRule)),
            "expected HorizontalRule in {:?}",
            ir
        );
    }

    #[test]
    fn parse_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let ir = markdown_to_ir(md);
        assert_eq!(ir.len(), 1);
        match &ir[0] {
            MessageIR::Table { headers, rows } => {
                assert_eq!(headers, &["A", "B"]);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0], vec!["1", "2"]);
            }
            other => panic!("expected Table, got {:?}", other),
        }
    }

    // ---- ir_to_markdown (round-trip) ---------------------------------------

    #[test]
    fn roundtrip_heading() {
        let original = "## Section Title";
        let ir = markdown_to_ir(original);
        let md = ir_to_markdown(&ir);
        assert!(md.contains("## Section Title"), "got: {}", md);
    }

    #[test]
    fn roundtrip_code_block() {
        let original = "```python\nprint('hello')\n```";
        let ir = markdown_to_ir(original);
        let md = ir_to_markdown(&ir);
        assert!(md.contains("```python"), "got: {}", md);
        assert!(md.contains("print('hello')"), "got: {}", md);
    }

    #[test]
    fn roundtrip_quote() {
        let original = "> wise words";
        let ir = markdown_to_ir(original);
        let md = ir_to_markdown(&ir);
        assert!(md.contains("> wise words"), "got: {}", md);
    }

    #[test]
    fn roundtrip_table() {
        let original = "| Name | Age |\n|------|-----|\n| Alice | 30 |";
        let ir = markdown_to_ir(original);
        let md = ir_to_markdown(&ir);
        assert!(md.contains("| Name |"), "got: {}", md);
        assert!(md.contains("| Alice |"), "got: {}", md);
    }

    #[test]
    fn roundtrip_horizontal_rule() {
        let original = "before\n\n---\n\nafter";
        let ir = markdown_to_ir(original);
        let md = ir_to_markdown(&ir);
        assert!(md.contains("---"), "got: {}", md);
    }

    #[test]
    fn ir_to_markdown_bold_italic_nodes() {
        // Directly-constructed Bold/Italic nodes (not from parsed MD)
        let ir = vec![
            MessageIR::Bold("strong text".to_string()),
            MessageIR::Italic("emphasis".to_string()),
        ];
        let md = ir_to_markdown(&ir);
        assert!(md.contains("**strong text**"), "got: {}", md);
        assert!(md.contains("*emphasis*"), "got: {}", md);
    }

    // ---- ir_to_channel_format ----------------------------------------------

    #[test]
    fn channel_format_respects_limit() {
        // Build a message from multiple blocks that will be chunked by the IR chunker.
        // The IR chunker splits at block boundaries, so we need multiple blocks
        // each large enough to exceed the chunk limit.
        let ir = vec![
            MessageIR::Text("A".repeat(120)),
            MessageIR::Text("B".repeat(120)),
            MessageIR::Text("C".repeat(120)),
        ];
        // Limit of 130 forces block-level splitting (each block is ~124 bytes with overhead).
        let chunks = ir_to_channel_format(&ir, "discord", 130);
        // More than one chunk should be produced.
        assert!(
            chunks.len() > 1,
            "expected multiple chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn channel_format_short_message_single_chunk() {
        let ir = vec![MessageIR::Text("Hello, world!".to_string())];
        let chunks = ir_to_channel_format(&ir, "slack", 4000);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].contains("Hello, world!"));
    }

    #[test]
    fn channel_format_preserves_content() {
        let ir = vec![
            MessageIR::Heading {
                level: 2,
                content: "Title".to_string(),
            },
            MessageIR::Text("Body text.".to_string()),
        ];
        let chunks = ir_to_channel_format(&ir, "telegram", 4096);
        let combined = chunks.join(" ");
        assert!(combined.contains("Title"), "got: {}", combined);
        assert!(combined.contains("Body text"), "got: {}", combined);
    }
}
