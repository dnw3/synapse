#![allow(dead_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic::core::SynapticError;
use synaptic::events::*;

/// Analysis result for a single media attachment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAnalysis {
    /// High-level category: "image", "audio", or "video".
    pub media_type: String,
    /// Original MIME type of the attachment (e.g. "image/png").
    pub mime_type: String,
    /// Human-readable description of the analysis to be performed / result.
    pub analysis: String,
    /// Source URL or data-URI for the attachment.
    pub url: String,
}

/// Subscribes to `MessageReceived` events.  When the payload contains an
/// `attachments` array each attachment is classified by MIME type and the
/// appropriate API request structure is described.
///
/// For **images** the subscriber builds an OpenAI Vision API request skeleton
/// (model, messages, max_tokens) and records a placeholder result, because no
/// live API key is required at analysis time.
///
/// For **audio** the subscriber builds a Whisper API request skeleton
/// (model, language hint) similarly with a placeholder transcript.
///
/// For **video** a generic media-analysis note is produced.
///
/// All results are collected into a `media_analysis` key injected into the
/// event payload so downstream subscribers (or the agent builder) can consume
/// them.
pub struct MediaUnderstandingSubscriber {
    /// Vision model name forwarded into the request skeleton (e.g.
    /// `"gpt-4o"` or `"gpt-4-vision-preview"`).
    vision_model: String,
    /// HTTP client reused across calls.
    client: reqwest::Client,
}

impl MediaUnderstandingSubscriber {
    /// Create a new subscriber with the given vision model name.
    pub fn new(vision_model: String) -> Self {
        Self {
            vision_model,
            client: reqwest::Client::new(),
        }
    }

    /// Classify a MIME type string into one of the three supported categories.
    ///
    /// Returns `None` when the MIME type is not a recognised media type.
    fn classify_mime(mime: &str) -> Option<&'static str> {
        let mime = mime.split(';').next().unwrap_or(mime).trim();
        if mime.starts_with("image/") {
            Some("image")
        } else if mime.starts_with("audio/") {
            Some("audio")
        } else if mime.starts_with("video/") {
            Some("video")
        } else {
            None
        }
    }

    /// Build an OpenAI Vision API request skeleton for `url` and return the
    /// description that would be sent.  The response is a placeholder string
    /// so that no live API key is required during analysis.
    fn prepare_vision_request(&self, url: &str, mime_type: &str) -> MediaAnalysis {
        // Build the canonical Vision API request structure.
        let request_body = serde_json::json!({
            "model": self.vision_model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image_url",
                            "image_url": { "url": url, "detail": "auto" }
                        },
                        {
                            "type": "text",
                            "text": "Describe this image in detail."
                        }
                    ]
                }
            ],
            "max_tokens": 1024
        });

        let analysis = format!(
            "[Vision API request prepared] model={} — {}",
            self.vision_model, request_body
        );

        MediaAnalysis {
            media_type: "image".to_string(),
            mime_type: mime_type.to_string(),
            analysis,
            url: url.to_string(),
        }
    }

    /// Build a Whisper API request skeleton for `url` and return the
    /// description that would be sent.  The response is a placeholder
    /// transcript so that no live API key is required during analysis.
    fn prepare_whisper_request(&self, url: &str, mime_type: &str) -> MediaAnalysis {
        // Derive the file format hint from the MIME sub-type.
        let format_hint = mime_type
            .split('/')
            .nth(1)
            .unwrap_or("audio")
            .split(';')
            .next()
            .unwrap_or("audio")
            .trim();

        let request_body = serde_json::json!({
            "model": "whisper-1",
            "file": url,
            "response_format": "json",
            "language": "auto",
            "format_hint": format_hint
        });

        let analysis = format!(
            "[Whisper API request prepared] format={} — {}",
            format_hint, request_body
        );

        MediaAnalysis {
            media_type: "audio".to_string(),
            mime_type: mime_type.to_string(),
            analysis,
            url: url.to_string(),
        }
    }

    /// Produce a generic analysis note for video attachments.
    fn prepare_video_note(&self, url: &str, mime_type: &str) -> MediaAnalysis {
        let analysis = format!(
            "[Video analysis] MIME={mime_type} — video understanding requires \
             frame extraction before Vision API ingestion."
        );
        MediaAnalysis {
            media_type: "video".to_string(),
            mime_type: mime_type.to_string(),
            analysis,
            url: url.to_string(),
        }
    }

    /// Extract the list of attachment objects from the event payload.
    ///
    /// Accepts both `attachments` (array of objects with `url`/`mime_type`
    /// keys) and a flat `attachment_url` + `attachment_mime` pair for
    /// simpler callers.
    fn extract_attachments(payload: &serde_json::Value) -> Vec<(String, String)> {
        let mut result = Vec::new();

        // Primary: structured `attachments` array.
        if let Some(arr) = payload.get("attachments").and_then(|v| v.as_array()) {
            for item in arr {
                let url = item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let mime = item
                    .get("mime_type")
                    .or_else(|| item.get("mime"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                if !url.is_empty() {
                    result.push((url, mime));
                }
            }
        }

        // Fallback: flat scalar fields.
        if result.is_empty() {
            let url = payload
                .get("attachment_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let mime = payload
                .get("attachment_mime")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string();
            if !url.is_empty() {
                result.push((url, mime));
            }
        }

        result
    }
}

#[async_trait]
impl EventSubscriber for MediaUnderstandingSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::MessageReceived)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let attachments = Self::extract_attachments(&event.payload);
        if attachments.is_empty() {
            return Ok(EventAction::Continue);
        }

        let mut analyses: Vec<MediaAnalysis> = Vec::new();

        for (url, mime_type) in &attachments {
            let media_type = Self::classify_mime(mime_type);
            tracing::debug!(
                url = %url,
                mime_type = %mime_type,
                media_type = ?media_type,
                "processing attachment"
            );

            match media_type {
                Some("image") => {
                    analyses.push(self.prepare_vision_request(url, mime_type));
                }
                Some("audio") => {
                    analyses.push(self.prepare_whisper_request(url, mime_type));
                }
                Some("video") => {
                    analyses.push(self.prepare_video_note(url, mime_type));
                }
                _ => {
                    tracing::debug!(
                        mime_type = %mime_type,
                        "skipping unsupported attachment MIME type"
                    );
                }
            }
        }

        if analyses.is_empty() {
            return Ok(EventAction::Continue);
        }

        tracing::info!(
            count = analyses.len(),
            "media analysis prepared for attachments"
        );

        if let Some(obj) = event.payload.as_object_mut() {
            obj.insert(
                "media_analysis".to_string(),
                serde_json::to_value(&analyses).unwrap_or_default(),
            );
        }

        Ok(EventAction::Modify)
    }

    fn name(&self) -> &str {
        "MediaUnderstandingSubscriber"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_subscriber() -> MediaUnderstandingSubscriber {
        MediaUnderstandingSubscriber::new("gpt-4o".to_string())
    }

    // -----------------------------------------------------------------------
    // MIME classification
    // -----------------------------------------------------------------------

    #[test]
    fn classify_image_mimes() {
        for mime in &["image/png", "image/jpeg", "image/gif", "image/webp"] {
            assert_eq!(
                MediaUnderstandingSubscriber::classify_mime(mime),
                Some("image"),
                "expected image for {mime}"
            );
        }
    }

    #[test]
    fn classify_audio_mimes() {
        for mime in &["audio/mpeg", "audio/wav", "audio/ogg", "audio/flac"] {
            assert_eq!(
                MediaUnderstandingSubscriber::classify_mime(mime),
                Some("audio"),
                "expected audio for {mime}"
            );
        }
    }

    #[test]
    fn classify_video_mimes() {
        for mime in &["video/mp4", "video/webm", "video/ogg"] {
            assert_eq!(
                MediaUnderstandingSubscriber::classify_mime(mime),
                Some("video"),
                "expected video for {mime}"
            );
        }
    }

    #[test]
    fn classify_unknown_mimes_return_none() {
        for mime in &[
            "application/pdf",
            "text/plain",
            "application/octet-stream",
            "",
        ] {
            assert_eq!(
                MediaUnderstandingSubscriber::classify_mime(mime),
                None,
                "expected None for {mime}"
            );
        }
    }

    #[test]
    fn classify_mime_ignores_parameters() {
        assert_eq!(
            MediaUnderstandingSubscriber::classify_mime("image/png; charset=utf-8"),
            Some("image")
        );
        assert_eq!(
            MediaUnderstandingSubscriber::classify_mime("audio/mpeg; bitrate=128"),
            Some("audio")
        );
    }

    // -----------------------------------------------------------------------
    // Attachment extraction
    // -----------------------------------------------------------------------

    #[test]
    fn extract_attachments_from_array() {
        let payload = json!({
            "attachments": [
                { "url": "https://example.com/a.png", "mime_type": "image/png" },
                { "url": "https://example.com/b.mp3", "mime": "audio/mpeg" }
            ]
        });
        let attachments = MediaUnderstandingSubscriber::extract_attachments(&payload);
        assert_eq!(attachments.len(), 2);
        assert_eq!(attachments[0].0, "https://example.com/a.png");
        assert_eq!(attachments[0].1, "image/png");
        assert_eq!(attachments[1].0, "https://example.com/b.mp3");
        assert_eq!(attachments[1].1, "audio/mpeg");
    }

    #[test]
    fn extract_attachments_fallback_flat_fields() {
        let payload = json!({
            "attachment_url": "https://example.com/photo.jpg",
            "attachment_mime": "image/jpeg"
        });
        let attachments = MediaUnderstandingSubscriber::extract_attachments(&payload);
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].0, "https://example.com/photo.jpg");
        assert_eq!(attachments[0].1, "image/jpeg");
    }

    #[test]
    fn extract_attachments_empty_payload() {
        let payload = json!({ "content": "hello" });
        assert!(MediaUnderstandingSubscriber::extract_attachments(&payload).is_empty());
    }

    #[test]
    fn extract_attachments_skips_items_without_url() {
        let payload = json!({
            "attachments": [
                { "mime_type": "image/png" },
                { "url": "https://example.com/ok.png", "mime_type": "image/png" }
            ]
        });
        let attachments = MediaUnderstandingSubscriber::extract_attachments(&payload);
        assert_eq!(attachments.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Vision request structure
    // -----------------------------------------------------------------------

    #[test]
    fn vision_request_contains_model_and_url() {
        let sub = make_subscriber();
        let result = sub.prepare_vision_request("https://example.com/img.png", "image/png");
        assert_eq!(result.media_type, "image");
        assert_eq!(result.mime_type, "image/png");
        assert_eq!(result.url, "https://example.com/img.png");
        assert!(result.analysis.contains("gpt-4o"));
        assert!(result.analysis.contains("Vision API request prepared"));
        // The JSON skeleton should encode the URL.
        assert!(result.analysis.contains("https://example.com/img.png"));
    }

    #[test]
    fn vision_request_structure_is_valid_json_embedded() {
        let sub = make_subscriber();
        let result = sub.prepare_vision_request("https://x.com/y.jpg", "image/jpeg");
        // After the prefix, the rest should contain parseable JSON fragments.
        assert!(result.analysis.contains("\"image_url\""));
        assert!(result.analysis.contains("\"max_tokens\""));
    }

    // -----------------------------------------------------------------------
    // Whisper request structure
    // -----------------------------------------------------------------------

    #[test]
    fn whisper_request_contains_format_and_url() {
        let sub = make_subscriber();
        let result = sub.prepare_whisper_request("https://example.com/clip.mp3", "audio/mpeg");
        assert_eq!(result.media_type, "audio");
        assert_eq!(result.mime_type, "audio/mpeg");
        assert_eq!(result.url, "https://example.com/clip.mp3");
        assert!(result.analysis.contains("Whisper API request prepared"));
        assert!(result.analysis.contains("mpeg"));
        assert!(result.analysis.contains("whisper-1"));
    }

    #[test]
    fn whisper_request_extracts_sub_type() {
        let sub = make_subscriber();
        let result = sub.prepare_whisper_request("https://example.com/clip.wav", "audio/wav");
        assert!(result.analysis.contains("wav"));
    }

    // -----------------------------------------------------------------------
    // Video note
    // -----------------------------------------------------------------------

    #[test]
    fn video_note_structure() {
        let sub = make_subscriber();
        let result = sub.prepare_video_note("https://example.com/vid.mp4", "video/mp4");
        assert_eq!(result.media_type, "video");
        assert_eq!(result.mime_type, "video/mp4");
        assert!(result.analysis.contains("video/mp4"));
        assert!(result.analysis.contains("frame extraction"));
    }

    // -----------------------------------------------------------------------
    // EventSubscriber integration
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn no_attachments_returns_continue() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({ "content": "hello world" }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Continue));
        assert!(event.payload.get("media_analysis").is_none());
    }

    #[tokio::test]
    async fn image_attachment_injects_media_analysis() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({
                "content": "look at this",
                "attachments": [
                    { "url": "https://example.com/photo.png", "mime_type": "image/png" }
                ]
            }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Modify));

        let analyses = event.payload["media_analysis"].as_array().unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0]["media_type"].as_str().unwrap(), "image");
        assert_eq!(analyses[0]["mime_type"].as_str().unwrap(), "image/png");
        assert_eq!(
            analyses[0]["url"].as_str().unwrap(),
            "https://example.com/photo.png"
        );
        assert!(analyses[0]["analysis"].as_str().unwrap().contains("gpt-4o"));
    }

    #[tokio::test]
    async fn audio_attachment_injects_whisper_analysis() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({
                "attachments": [
                    { "url": "https://example.com/voice.ogg", "mime_type": "audio/ogg" }
                ]
            }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Modify));

        let analyses = event.payload["media_analysis"].as_array().unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0]["media_type"].as_str().unwrap(), "audio");
        assert!(analyses[0]["analysis"]
            .as_str()
            .unwrap()
            .contains("Whisper"));
    }

    #[tokio::test]
    async fn video_attachment_injects_video_note() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({
                "attachments": [
                    { "url": "https://example.com/clip.mp4", "mime_type": "video/mp4" }
                ]
            }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Modify));

        let analyses = event.payload["media_analysis"].as_array().unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0]["media_type"].as_str().unwrap(), "video");
    }

    #[tokio::test]
    async fn mixed_attachments_all_classified() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({
                "attachments": [
                    { "url": "https://example.com/img.png", "mime_type": "image/png" },
                    { "url": "https://example.com/audio.mp3", "mime_type": "audio/mpeg" },
                    { "url": "https://example.com/vid.mp4", "mime_type": "video/mp4" },
                    { "url": "https://example.com/doc.pdf", "mime_type": "application/pdf" }
                ]
            }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Modify));

        // PDF should be skipped — only 3 media analyses.
        let analyses = event.payload["media_analysis"].as_array().unwrap();
        assert_eq!(analyses.len(), 3);

        let types: Vec<&str> = analyses
            .iter()
            .map(|a| a["media_type"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"image"));
        assert!(types.contains(&"audio"));
        assert!(types.contains(&"video"));
    }

    #[tokio::test]
    async fn unsupported_only_returns_continue() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({
                "attachments": [
                    { "url": "https://example.com/doc.pdf", "mime_type": "application/pdf" }
                ]
            }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Continue));
        assert!(event.payload.get("media_analysis").is_none());
    }

    #[tokio::test]
    async fn flat_attachment_fields_work() {
        let sub = make_subscriber();
        let mut event = Event::new(
            EventKind::MessageReceived,
            json!({
                "attachment_url": "https://example.com/img.jpeg",
                "attachment_mime": "image/jpeg"
            }),
        );
        let action = sub.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Modify));

        let analyses = event.payload["media_analysis"].as_array().unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0]["media_type"].as_str().unwrap(), "image");
    }

    #[test]
    fn subscriber_name() {
        let sub = make_subscriber();
        assert_eq!(sub.name(), "MediaUnderstandingSubscriber");
    }

    #[test]
    fn subscription_filter_is_message_received() {
        let sub = make_subscriber();
        let filters = sub.subscriptions();
        assert_eq!(filters.len(), 1);
        match &filters[0] {
            EventFilter::Exact(kind) => assert_eq!(*kind, EventKind::MessageReceived),
            other => panic!("expected Exact filter, got {other:?}"),
        }
    }
}
