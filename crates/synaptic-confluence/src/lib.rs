use async_trait::async_trait;
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

#[derive(Debug, Clone)]
pub struct ConfluenceConfig {
    pub domain: String,
    pub email: String,
    pub api_token: String,
    pub space_key: Option<String>,
    pub page_ids: Vec<String>,
}

impl ConfluenceConfig {
    pub fn new(
        domain: impl Into<String>,
        email: impl Into<String>,
        api_token: impl Into<String>,
    ) -> Self {
        Self {
            domain: domain.into(),
            email: email.into(),
            api_token: api_token.into(),
            space_key: None,
            page_ids: vec![],
        }
    }

    pub fn with_space_key(mut self, key: impl Into<String>) -> Self {
        self.space_key = Some(key.into());
        self
    }

    pub fn with_page_ids(mut self, ids: Vec<String>) -> Self {
        self.page_ids = ids;
        self
    }
}

pub struct ConfluenceLoader {
    config: ConfluenceConfig,
    client: reqwest::Client,
}

impl ConfluenceLoader {
    pub fn new(config: ConfluenceConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn auth_header(&self) -> String {
        let credentials = format!("{}:{}", self.config.email, self.config.api_token);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes())
        )
    }

    async fn fetch_page(&self, page_id: &str) -> Result<Document, SynapticError> {
        let url = format!(
            "https://{}/wiki/api/v2/pages/{}?body-format=storage",
            self.config.domain, page_id
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("Confluence fetch page: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("Confluence parse page: {e}")))?;

        let title = body["title"].as_str().unwrap_or("").to_string();
        let content_html = body["body"]["storage"]["value"].as_str().unwrap_or("");
        let content = strip_html_tags(content_html);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), Value::String(title));
        metadata.insert(
            "source".to_string(),
            Value::String(format!("confluence:{}", page_id)),
        );
        if let Some(space_id) = body["spaceId"].as_str() {
            metadata.insert("space_id".to_string(), Value::String(space_id.to_string()));
        }

        Ok(Document {
            id: page_id.to_string(),
            content,
            metadata,
        })
    }

    async fn fetch_space_pages(&self, space_key: &str) -> Result<Vec<String>, SynapticError> {
        let url = format!(
            "https://{}/wiki/api/v2/spaces/{}/pages?limit=50",
            self.config.domain, space_key
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("Confluence fetch space: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("Confluence parse space: {e}")))?;

        let ids = body["results"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|p| p["id"].as_str().map(|s| s.to_string()))
            .collect();
        Ok(ids)
    }
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[async_trait]
impl Loader for ConfluenceLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut page_ids = self.config.page_ids.clone();
        if let Some(ref space_key) = self.config.space_key {
            let space_ids = self.fetch_space_pages(space_key).await?;
            page_ids.extend(space_ids);
        }
        let mut documents = Vec::new();
        for page_id in &page_ids {
            match self.fetch_page(page_id).await {
                Ok(doc) => documents.push(doc),
                Err(e) => eprintln!("Warning: failed to load Confluence page {}: {}", page_id, e),
            }
        }
        Ok(documents)
    }
}
