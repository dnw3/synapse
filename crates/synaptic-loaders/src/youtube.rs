use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

/// Loader for YouTube video transcripts.
pub struct YoutubeLoader {
    client: reqwest::Client,
    video_ids: Vec<String>,
    language: String,
}

impl YoutubeLoader {
    pub fn new(video_ids: Vec<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            video_ids,
            language: "en".to_string(),
        }
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = lang.into();
        self
    }

    async fn fetch_transcript(&self, video_id: &str) -> Result<String, SynapticError> {
        let url = format!(
            "https://www.youtube.com/api/timedtext?v={}&lang={}&fmt=json3",
            video_id, self.language
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("YouTube fetch: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("YouTube parse: {e}")))?;

        let text = body["events"]
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .filter_map(|event| {
                        event["segs"].as_array().map(|segs| {
                            segs.iter()
                                .filter_map(|seg| seg["utf8"].as_str())
                                .collect::<Vec<_>>()
                                .join("")
                        })
                    })
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        Ok(text)
    }

    async fn fetch_title(&self, video_id: &str) -> Option<String> {
        let url = format!(
            "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v={}&format=json",
            video_id
        );
        self.client
            .get(&url)
            .send()
            .await
            .ok()?
            .json::<Value>()
            .await
            .ok()?["title"]
            .as_str()
            .map(|s| s.to_string())
    }
}

#[async_trait]
impl Loader for YoutubeLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut documents = Vec::new();
        for video_id in &self.video_ids {
            let content = self.fetch_transcript(video_id).await?;
            if content.is_empty() {
                continue;
            }
            let title = self.fetch_title(video_id).await;
            let mut metadata = HashMap::new();
            metadata.insert(
                "source".to_string(),
                Value::String(format!("youtube:{}", video_id)),
            );
            metadata.insert(
                "url".to_string(),
                Value::String(format!("https://www.youtube.com/watch?v={}", video_id)),
            );
            if let Some(t) = title {
                metadata.insert("title".to_string(), Value::String(t));
            }
            documents.push(Document {
                id: video_id.clone(),
                content,
                metadata,
            });
        }
        Ok(documents)
    }
}
