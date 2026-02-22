use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

/// Loader for Notion pages via the Notion API.
pub struct NotionLoader {
    client: reqwest::Client,
    token: String,
    page_ids: Vec<String>,
}

impl NotionLoader {
    pub fn new(token: impl Into<String>, page_ids: Vec<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            token: token.into(),
            page_ids,
        }
    }

    async fn fetch_page_title(&self, page_id: &str) -> Result<String, SynapticError> {
        let url = format!("https://api.notion.com/v1/pages/{}", page_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", "2022-06-28")
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("Notion fetch page: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("Notion parse page: {e}")))?;

        let title = body["properties"]["title"]["title"][0]["plain_text"]
            .as_str()
            .or_else(|| body["properties"]["Name"]["title"][0]["plain_text"].as_str())
            .unwrap_or("Untitled")
            .to_string();
        Ok(title)
    }

    async fn fetch_blocks(&self, block_id: &str) -> Result<Vec<Value>, SynapticError> {
        let url = format!(
            "https://api.notion.com/v1/blocks/{}/children?page_size=100",
            block_id
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", "2022-06-28")
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("Notion fetch blocks: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("Notion parse blocks: {e}")))?;

        Ok(body["results"].as_array().cloned().unwrap_or_default())
    }

    fn extract_rich_text(rich_text: &Value) -> String {
        rich_text
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t["plain_text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    }

    fn block_to_text(block: &Value) -> Option<String> {
        let block_type = block["type"].as_str()?;
        match block_type {
            "paragraph" => Some(Self::extract_rich_text(&block["paragraph"]["rich_text"])),
            "heading_1" => Some(format!(
                "# {}",
                Self::extract_rich_text(&block["heading_1"]["rich_text"])
            )),
            "heading_2" => Some(format!(
                "## {}",
                Self::extract_rich_text(&block["heading_2"]["rich_text"])
            )),
            "heading_3" => Some(format!(
                "### {}",
                Self::extract_rich_text(&block["heading_3"]["rich_text"])
            )),
            "bulleted_list_item" => Some(format!(
                "- {}",
                Self::extract_rich_text(&block["bulleted_list_item"]["rich_text"])
            )),
            "numbered_list_item" => Some(format!(
                "1. {}",
                Self::extract_rich_text(&block["numbered_list_item"]["rich_text"])
            )),
            "quote" => Some(format!(
                "> {}",
                Self::extract_rich_text(&block["quote"]["rich_text"])
            )),
            "callout" => Some(Self::extract_rich_text(&block["callout"]["rich_text"])),
            "code" => {
                let lang = block["code"]["language"].as_str().unwrap_or("");
                let code = Self::extract_rich_text(&block["code"]["rich_text"]);
                Some(format!("```{}\n{}\n```", lang, code))
            }
            _ => None,
        }
    }
}

#[async_trait]
impl Loader for NotionLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut documents = Vec::new();
        for page_id in &self.page_ids {
            let title = self
                .fetch_page_title(page_id)
                .await
                .unwrap_or_else(|_| "Untitled".to_string());
            let blocks = self.fetch_blocks(page_id).await?;
            let content = blocks
                .iter()
                .filter_map(Self::block_to_text)
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut metadata = HashMap::new();
            metadata.insert(
                "source".to_string(),
                Value::String(format!("notion:{}", page_id)),
            );
            metadata.insert("title".to_string(), Value::String(title));
            documents.push(Document {
                id: page_id.clone(),
                content,
                metadata,
            });
        }
        Ok(documents)
    }
}
