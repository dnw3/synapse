//! Card assembler for Lark Card JSON 2.0.
//!
//! Takes IR-rendered [`LarkCardElement`]s and wraps them in a complete
//! Card JSON 2.0 skeleton with header, body, and optional footer.

use serde_json::{json, Value};
use synaptic::lark::card_elements::LarkCardElement;

use crate::config::bot::LarkCardConfig;

/// Maximum elements we target after coalescing (leave room for footer).
const DEFAULT_MAX_ELEMENTS: usize = 180;

/// Assemble a complete Lark Card JSON 2.0 from IR-rendered elements and config.
pub fn assemble_final_card(
    elements: Vec<LarkCardElement>,
    config: &LarkCardConfig,
    bot_name: &str,
) -> Value {
    let title = if config.header_title.is_empty() {
        bot_name
    } else {
        &config.header_title
    };

    // Coalesce if over the limit
    let mut body_elements = coalesce_elements(elements, DEFAULT_MAX_ELEMENTS);

    // Convert elements to JSON objects
    let mut json_elements: Vec<Value> = body_elements.drain(..).map(element_to_json).collect();

    // Footer: timestamp
    if config.show_timestamp {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        json_elements.push(json!({
            "tag": "markdown",
            "element_id": "ts_footer",
            "content": format!("_{}_", now),
        }));
    }

    // Footer: feedback buttons
    if config.show_feedback {
        json_elements.push(json!({ "tag": "hr", "element_id": "fb_hr" }));
        json_elements.push(json!({
            "tag": "action",
            "element_id": "fb_actions",
            "actions": [
                {
                    "tag": "button",
                    "text": { "tag": "plain_text", "content": "\u{1f44d}" },
                    "type": "primary",
                    "value": { "action": "feedback_positive" },
                },
                {
                    "tag": "button",
                    "text": { "tag": "plain_text", "content": "\u{1f44e}" },
                    "type": "default",
                    "value": { "action": "feedback_negative" },
                },
            ],
        }));
    }

    // Build header
    let mut header = json!({
        "template": &config.template,
        "title": {
            "tag": "plain_text",
            "content": title,
        },
    });

    if !config.header_icon.is_empty() {
        header["icon"] = json!({
            "tag": "standard_icon",
            "token": &config.header_icon,
        });
    }

    json!({
        "schema": "2.0",
        "config": {
            "update_multi": true,
        },
        "header": header,
        "body": {
            "elements": json_elements,
        },
    })
}

/// Merge consecutive markdown elements when the count exceeds `max_elements`.
///
/// Non-markdown elements (hr, img, action, etc.) are preserved in place.
/// Consecutive markdown elements are joined with `"\n\n"` and use the
/// first element's ID.
pub fn coalesce_elements(
    elements: Vec<LarkCardElement>,
    max_elements: usize,
) -> Vec<LarkCardElement> {
    if elements.len() <= max_elements {
        return elements;
    }

    let mut result: Vec<LarkCardElement> = Vec::new();

    for elem in elements {
        if elem.tag == "markdown" {
            // Try to merge with the previous element if it is also markdown
            if let Some(last) = result.last_mut() {
                if last.tag == "markdown" {
                    // Extract content from both and join
                    let prev_content = last
                        .properties
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let curr_content = elem
                        .properties
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let merged = format!("{}\n\n{}", prev_content, curr_content);
                    last.properties["content"] = Value::String(merged);
                    continue;
                }
            }
            // No previous markdown to merge with — push as new
            result.push(elem);
        } else {
            result.push(elem);
        }
    }

    result
}

/// Convert a [`LarkCardElement`] to a flat JSON object with tag, element_id,
/// and all flattened properties at the top level.
fn element_to_json(elem: LarkCardElement) -> Value {
    let mut obj = json!({
        "tag": elem.tag,
        "element_id": elem.element_id,
    });

    if let Value::Object(props) = elem.properties {
        if let Value::Object(ref mut map) = obj {
            for (k, v) in props {
                map.insert(k, v);
            }
        }
    }

    obj
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> LarkCardConfig {
        LarkCardConfig {
            template: "blue".to_string(),
            header_title: String::new(),
            header_icon: String::new(),
            show_feedback: true,
            show_timestamp: false,
        }
    }

    fn make_md_element(id: &str, content: &str) -> LarkCardElement {
        LarkCardElement {
            tag: "markdown".to_string(),
            element_id: id.to_string(),
            properties: json!({ "content": content }),
        }
    }

    fn make_hr_element(id: &str) -> LarkCardElement {
        LarkCardElement {
            tag: "hr".to_string(),
            element_id: id.to_string(),
            properties: json!({}),
        }
    }

    #[test]
    fn test_assemble_has_schema_2() {
        let config = default_config();
        let card = assemble_final_card(vec![], &config, "TestBot");
        assert_eq!(card["schema"], "2.0");
    }

    #[test]
    fn test_assemble_has_header_with_config() {
        let config = default_config();
        let card = assemble_final_card(vec![], &config, "MyBot");
        assert_eq!(card["header"]["template"], "blue");
        assert_eq!(card["header"]["title"]["content"], "MyBot");
    }

    #[test]
    fn test_assemble_uses_custom_title() {
        let mut config = default_config();
        config.header_title = "Custom Title".to_string();
        let card = assemble_final_card(vec![], &config, "MyBot");
        assert_eq!(card["header"]["title"]["content"], "Custom Title");
    }

    #[test]
    fn test_assemble_includes_body_elements() {
        let config = default_config();
        let elements = vec![
            make_md_element("e0md", "Hello world"),
            make_md_element("e1md", "Second paragraph"),
        ];
        let card = assemble_final_card(elements, &config, "Bot");
        let body = &card["body"]["elements"];
        // Should contain the markdown elements (plus feedback footer)
        assert!(body.as_array().unwrap().len() >= 2);
        assert_eq!(body[0]["content"], "Hello world");
        assert_eq!(body[1]["content"], "Second paragraph");
    }

    #[test]
    fn test_assemble_has_feedback_buttons_when_enabled() {
        let mut config = default_config();
        config.show_feedback = true;
        let card = assemble_final_card(vec![], &config, "Bot");
        let elems = card["body"]["elements"].as_array().unwrap();
        // Should have hr + action block
        let has_action = elems.iter().any(|e| e["tag"] == "action");
        assert!(has_action, "Expected feedback action block");
    }

    #[test]
    fn test_assemble_no_feedback_buttons_when_disabled() {
        let mut config = default_config();
        config.show_feedback = false;
        let card = assemble_final_card(vec![], &config, "Bot");
        let elems = card["body"]["elements"].as_array().unwrap();
        let has_action = elems.iter().any(|e| e["tag"] == "action");
        assert!(!has_action, "Expected no feedback action block");
    }

    #[test]
    fn test_assemble_update_multi_true() {
        let config = default_config();
        let card = assemble_final_card(vec![], &config, "Bot");
        assert_eq!(card["config"]["update_multi"], true);
    }

    #[test]
    fn test_coalesce_merges_when_over_limit() {
        // Create 200 markdown elements
        let elements: Vec<LarkCardElement> = (0..200)
            .map(|i| make_md_element(&format!("e{}md", i), &format!("Para {}", i)))
            .collect();
        assert_eq!(elements.len(), 200);

        let coalesced = coalesce_elements(elements, 180);
        // All are markdown and consecutive, so they should all merge into 1
        assert!(
            coalesced.len() <= 180,
            "Expected <= 180, got {}",
            coalesced.len()
        );
        // The merged element should use the first element's ID
        assert_eq!(coalesced[0].element_id, "e0md");
    }

    #[test]
    fn test_coalesce_preserves_non_markdown() {
        // md, md, hr, md, md — should become: md, hr, md when over limit
        let elements = vec![
            make_md_element("e0", "A"),
            make_md_element("e1", "B"),
            make_hr_element("e2"),
            make_md_element("e3", "C"),
            make_md_element("e4", "D"),
        ];
        // Force coalescing by setting max to 3
        let coalesced = coalesce_elements(elements, 3);
        assert_eq!(coalesced.len(), 3);
        assert_eq!(coalesced[0].tag, "markdown");
        assert_eq!(coalesced[1].tag, "hr");
        assert_eq!(coalesced[2].tag, "markdown");
        // Check merged content
        let content = coalesced[0]
            .properties
            .get("content")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(content.contains("A"));
        assert!(content.contains("B"));
    }

    #[test]
    fn test_coalesce_no_op_under_limit() {
        let elements = vec![make_md_element("e0", "A"), make_md_element("e1", "B")];
        let coalesced = coalesce_elements(elements, 180);
        assert_eq!(coalesced.len(), 2);
    }
}
