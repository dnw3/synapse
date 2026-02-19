use std::collections::HashMap;

use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_runnables::Runnable;

use crate::FormatInstructions;

/// Represents a parsed XML element.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlElement {
    pub tag: String,
    pub text: Option<String>,
    pub attributes: HashMap<String, String>,
    pub children: Vec<XmlElement>,
}

/// Parses XML-formatted LLM output into an `XmlElement` tree.
///
/// Performs simple XML parsing without requiring a full XML library.
/// Handles basic tags with text content and nested elements.
pub struct XmlOutputParser {
    /// Optional root tag to extract from within a larger output.
    root_tag: Option<String>,
}

impl XmlOutputParser {
    /// Creates a new parser with no root tag filter.
    pub fn new() -> Self {
        Self { root_tag: None }
    }

    /// Creates a new parser that only parses content within the specified root tag.
    pub fn with_root_tag(tag: impl Into<String>) -> Self {
        Self {
            root_tag: Some(tag.into()),
        }
    }
}

impl Default for XmlOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatInstructions for XmlOutputParser {
    fn get_format_instructions(&self) -> String {
        match &self.root_tag {
            Some(tag) => {
                format!("Your response should be valid XML wrapped in <{tag}>...</{tag}> tags.")
            }
            None => "Your response should be valid XML.".to_string(),
        }
    }
}

#[async_trait]
impl Runnable<String, XmlElement> for XmlOutputParser {
    async fn invoke(
        &self,
        input: String,
        _config: &RunnableConfig,
    ) -> Result<XmlElement, SynapticError> {
        let xml = if let Some(root_tag) = &self.root_tag {
            let open = format!("<{}", root_tag);
            let close = format!("</{}>", root_tag);
            let start = input.find(&open).ok_or_else(|| {
                SynapticError::Parsing(format!("root tag <{}> not found in input", root_tag))
            })?;
            let end = input.find(&close).ok_or_else(|| {
                SynapticError::Parsing(format!("closing tag </{}> not found in input", root_tag))
            })?;
            &input[start..end + close.len()]
        } else {
            input.trim()
        };

        let mut pos = 0;
        parse_element(xml, &mut pos)
    }
}

/// Skip whitespace characters, advancing `pos`.
fn skip_whitespace(input: &str, pos: &mut usize) {
    let bytes = input.as_bytes();
    while *pos < bytes.len() && (bytes[*pos] as char).is_ascii_whitespace() {
        *pos += 1;
    }
}

/// Parse a single XML element starting at `pos`.
fn parse_element(input: &str, pos: &mut usize) -> Result<XmlElement, SynapticError> {
    skip_whitespace(input, pos);

    if *pos >= input.len() || input.as_bytes()[*pos] != b'<' {
        return Err(SynapticError::Parsing(format!(
            "expected '<' at position {pos}",
            pos = *pos
        )));
    }
    *pos += 1; // skip '<'

    // Parse tag name
    let tag_start = *pos;
    while *pos < input.len() {
        let ch = input.as_bytes()[*pos] as char;
        if ch.is_ascii_whitespace() || ch == '>' || ch == '/' {
            break;
        }
        *pos += 1;
    }
    let tag = input[tag_start..*pos].to_string();
    if tag.is_empty() {
        return Err(SynapticError::Parsing("empty tag name".to_string()));
    }

    // Parse attributes
    let attributes = parse_attributes(input, pos)?;

    skip_whitespace(input, pos);

    // Check for self-closing tag
    if *pos < input.len() && input.as_bytes()[*pos] == b'/' {
        *pos += 1; // skip '/'
        if *pos >= input.len() || input.as_bytes()[*pos] != b'>' {
            return Err(SynapticError::Parsing(
                "expected '>' after '/' in self-closing tag".to_string(),
            ));
        }
        *pos += 1; // skip '>'
        return Ok(XmlElement {
            tag,
            text: None,
            attributes,
            children: Vec::new(),
        });
    }

    // Expect '>'
    if *pos >= input.len() || input.as_bytes()[*pos] != b'>' {
        return Err(SynapticError::Parsing(format!(
            "expected '>' for tag <{tag}>"
        )));
    }
    *pos += 1; // skip '>'

    // Parse content: text and/or child elements
    let mut children = Vec::new();
    let mut text_parts: Vec<String> = Vec::new();

    loop {
        if *pos >= input.len() {
            return Err(SynapticError::Parsing(format!(
                "unexpected end of input, missing closing tag </{tag}>"
            )));
        }

        // Check for closing tag
        let closing = format!("</{tag}>");
        if input[*pos..].starts_with(&closing) {
            *pos += closing.len();
            break;
        }

        // Check for child element
        if input.as_bytes()[*pos] == b'<' {
            // Make sure it's not a closing tag for something else
            if *pos + 1 < input.len() && input.as_bytes()[*pos + 1] == b'/' {
                return Err(SynapticError::Parsing(format!(
                    "unexpected closing tag at position {pos}, expected </{tag}>",
                    pos = *pos
                )));
            }
            let child = parse_element(input, pos)?;
            children.push(child);
        } else {
            // Collect text content until we hit a '<'
            let text_start = *pos;
            while *pos < input.len() && input.as_bytes()[*pos] != b'<' {
                *pos += 1;
            }
            let part = input[text_start..*pos].to_string();
            let trimmed = part.trim().to_string();
            if !trimmed.is_empty() {
                text_parts.push(trimmed);
            }
        }
    }

    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(" "))
    };

    Ok(XmlElement {
        tag,
        text,
        attributes,
        children,
    })
}

/// Parse attributes inside an opening tag. `pos` should be right after the tag name.
fn parse_attributes(
    input: &str,
    pos: &mut usize,
) -> Result<HashMap<String, String>, SynapticError> {
    let mut attributes = HashMap::new();

    loop {
        skip_whitespace(input, pos);

        if *pos >= input.len() {
            break;
        }

        let ch = input.as_bytes()[*pos] as char;
        if ch == '>' || ch == '/' {
            break;
        }

        // Parse attribute name
        let name_start = *pos;
        while *pos < input.len() {
            let c = input.as_bytes()[*pos] as char;
            if c == '=' || c.is_ascii_whitespace() || c == '>' || c == '/' {
                break;
            }
            *pos += 1;
        }
        let name = input[name_start..*pos].to_string();
        if name.is_empty() {
            return Err(SynapticError::Parsing("empty attribute name".to_string()));
        }

        skip_whitespace(input, pos);

        // Expect '='
        if *pos >= input.len() || input.as_bytes()[*pos] != b'=' {
            return Err(SynapticError::Parsing(format!(
                "expected '=' after attribute name '{name}'"
            )));
        }
        *pos += 1; // skip '='

        skip_whitespace(input, pos);

        // Expect quoted value
        if *pos >= input.len() {
            return Err(SynapticError::Parsing(
                "unexpected end of input in attribute value".to_string(),
            ));
        }

        let quote = input.as_bytes()[*pos] as char;
        if quote != '"' && quote != '\'' {
            return Err(SynapticError::Parsing(format!(
                "expected quote for attribute '{name}' value, got '{quote}'"
            )));
        }
        *pos += 1; // skip opening quote

        let value_start = *pos;
        while *pos < input.len() && input.as_bytes()[*pos] as char != quote {
            *pos += 1;
        }
        if *pos >= input.len() {
            return Err(SynapticError::Parsing(format!(
                "unterminated attribute value for '{name}'"
            )));
        }
        let value = input[value_start..*pos].to_string();
        *pos += 1; // skip closing quote

        attributes.insert(name, value);
    }

    Ok(attributes)
}
