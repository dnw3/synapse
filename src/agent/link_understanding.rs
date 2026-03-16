#![allow(dead_code)]

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::events::*;

/// Subscribes to MessageReceived events, extracts URLs, fetches og:meta metadata.
#[allow(dead_code)]
pub struct LinkUnderstandingSubscriber;

impl LinkUnderstandingSubscriber {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventSubscriber for LinkUnderstandingSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::MessageReceived)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        // Extract content from payload
        let content = event
            .payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Extract URLs using simple regex
        let urls = extract_urls(content);
        if urls.is_empty() {
            return Ok(EventAction::Continue);
        }

        // Fetch metadata for each URL (first 3 max)
        let mut link_previews = Vec::new();
        for url in urls.into_iter().take(3) {
            if let Ok(meta) = fetch_link_metadata(&url).await {
                link_previews.push(meta);
            }
        }

        if !link_previews.is_empty() {
            // Attach to event payload
            if let Some(obj) = event.payload.as_object_mut() {
                obj.insert(
                    "link_previews".to_string(),
                    serde_json::to_value(&link_previews).unwrap_or_default(),
                );
            }
            return Ok(EventAction::Modify);
        }

        Ok(EventAction::Continue)
    }
}

fn extract_urls(text: &str) -> Vec<String> {
    // Simple URL extraction — matches http:// and https:// URLs
    let mut urls = Vec::new();
    for word in text.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            c == '<' || c == '>' || c == '(' || c == ')' || c == '[' || c == ']'
        });
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            urls.push(trimmed.to_string());
        }
    }
    urls
}

#[derive(Debug, serde::Serialize)]
struct LinkMetadata {
    url: String,
    title: Option<String>,
    description: Option<String>,
    image: Option<String>,
}

async fn fetch_link_metadata(url: &str) -> Result<LinkMetadata, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let resp = client
        .get(url)
        .header("User-Agent", "Synapse-Bot/1.0")
        .send()
        .await?
        .text()
        .await?;

    // Parse og:meta tags
    let title = extract_meta(&resp, "og:title").or_else(|| extract_tag(&resp, "title"));
    let description =
        extract_meta(&resp, "og:description").or_else(|| extract_meta(&resp, "description"));
    let image = extract_meta(&resp, "og:image");

    Ok(LinkMetadata {
        url: url.to_string(),
        title,
        description,
        image,
    })
}

fn extract_meta(html: &str, property: &str) -> Option<String> {
    // Simple og:meta extraction without a full HTML parser
    let patterns = [
        format!("property=\"{}\" content=\"", property),
        format!("name=\"{}\" content=\"", property),
        format!("property='{}' content='", property),
    ];
    for pattern in &patterns {
        if let Some(start) = html.find(pattern.as_str()) {
            let after = &html[start + pattern.len()..];
            let quote = if pattern.ends_with('"') { '"' } else { '\'' };
            if let Some(end) = after.find(quote) {
                return Some(after[..end].to_string());
            }
        }
    }
    None
}

fn extract_tag(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    if let Some(start) = html.find(&open) {
        let after_open = &html[start..];
        if let Some(gt) = after_open.find('>') {
            let content_start = start + gt + 1;
            if let Some(end) = html[content_start..].find(&close) {
                let content = &html[content_start..content_start + end];
                return Some(content.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_urls_from_text() {
        let text = "Check out https://example.com and http://foo.bar/baz";
        let urls = extract_urls(text);
        assert_eq!(urls, vec!["https://example.com", "http://foo.bar/baz"]);
    }

    #[test]
    fn extract_urls_with_brackets() {
        let text = "Link: <https://example.com>";
        let urls = extract_urls(text);
        assert_eq!(urls, vec!["https://example.com"]);
    }

    #[test]
    fn no_urls() {
        assert!(extract_urls("no links here").is_empty());
    }

    #[test]
    fn extract_og_meta() {
        let html = r#"<meta property="og:title" content="Test Page">"#;
        assert_eq!(
            extract_meta(html, "og:title"),
            Some("Test Page".to_string())
        );
    }
}
