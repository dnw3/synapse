use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

use crate::{auth::TokenCache, LarkConfig};

/// Load Feishu/Lark documents and Wiki pages into Synaptic [`Document`]s.
///
/// Supports loading specific document tokens directly or traversing a Wiki space
/// to discover all nodes automatically.
///
/// # Example
///
/// ```rust,no_run
/// use synaptic_lark::{LarkConfig, LarkDocLoader};
/// use synaptic_core::Loader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = LarkConfig::new("cli_xxx", "secret_xxx");
/// let loader = LarkDocLoader::new(config)
///     .with_doc_tokens(vec!["doxcnAbcXxx".to_string()])
///     .with_wiki_space_id("space_xxx");
///
/// let docs = loader.load().await?;
/// for doc in &docs {
///     println!("Title: {}", doc.metadata["title"]);
///     println!("Content length: {}", doc.content.len());
/// }
/// # Ok(())
/// # }
/// ```
pub struct LarkDocLoader {
    token_cache: TokenCache,
    base_url: String,
    doc_tokens: Vec<String>,
    wiki_space_id: Option<String>,
    client: reqwest::Client,
}

impl LarkDocLoader {
    /// Create a new loader using the given config.
    pub fn new(config: LarkConfig) -> Self {
        let base_url = config.base_url.clone();
        Self {
            token_cache: config.token_cache(),
            base_url,
            doc_tokens: vec![],
            wiki_space_id: None,
            client: reqwest::Client::new(),
        }
    }

    /// Add specific document tokens to load (e.g. `"doxcnAbcXxx"`).
    pub fn with_doc_tokens(mut self, tokens: Vec<String>) -> Self {
        self.doc_tokens = tokens;
        self
    }

    /// Traverse a Wiki space to load all documents within it.
    pub fn with_wiki_space_id(mut self, space_id: impl Into<String>) -> Self {
        self.wiki_space_id = Some(space_id.into());
        self
    }

    async fn auth_header(&self) -> Result<String, SynapticError> {
        let token = self.token_cache.get_token().await?;
        Ok(format!("Bearer {token}"))
    }

    /// Fetch the raw text content of a document.
    async fn fetch_doc_content(&self, doc_token: &str) -> Result<Document, SynapticError> {
        let auth = self.auth_header().await?;
        let url = format!(
            "{}/docx/v1/documents/{}/raw_content",
            self.base_url, doc_token
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("Lark doc fetch: {e}")))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("Lark doc parse: {e}")))?;

        check_lark_code(&body, "fetch doc content")?;

        let content = body["data"]["content"].as_str().unwrap_or("").to_string();
        let title = body["data"]["title"].as_str().unwrap_or("").to_string();

        let mut metadata = HashMap::new();
        metadata.insert("doc_id".to_string(), Value::String(doc_token.to_string()));
        metadata.insert("title".to_string(), Value::String(title));
        metadata.insert(
            "source".to_string(),
            Value::String(format!("lark:doc:{doc_token}")),
        );
        metadata.insert(
            "url".to_string(),
            Value::String(format!("https://bytedance.feishu.cn/docx/{doc_token}")),
        );
        metadata.insert("doc_type".to_string(), Value::String("docx".to_string()));

        Ok(Document {
            id: doc_token.to_string(),
            content,
            metadata,
        })
    }

    /// Discover all doc tokens under a Wiki space node (paginates automatically).
    async fn list_wiki_nodes(&self, space_id: &str) -> Result<Vec<String>, SynapticError> {
        let auth = self.auth_header().await?;
        let mut tokens = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/wiki/v2/spaces/{}/nodes?page_size=50",
                self.base_url, space_id
            );
            if let Some(ref pt) = page_token {
                url.push_str(&format!("&page_token={pt}"));
            }

            let resp = self
                .client
                .get(&url)
                .header("Authorization", auth.clone())
                .send()
                .await
                .map_err(|e| SynapticError::Loader(format!("Lark wiki list: {e}")))?;

            let body: Value = resp
                .json()
                .await
                .map_err(|e| SynapticError::Loader(format!("Lark wiki parse: {e}")))?;

            check_lark_code(&body, "list wiki nodes")?;

            if let Some(items) = body["data"]["items"].as_array() {
                for item in items {
                    if let Some(obj_token) = item["obj_token"].as_str() {
                        let obj_type = item["obj_type"].as_str().unwrap_or("");
                        if obj_type == "docx" || obj_type == "doc" {
                            tokens.push(obj_token.to_string());
                        }
                    }
                }
            }

            let has_more = body["data"]["has_more"].as_bool().unwrap_or(false);
            if !has_more {
                break;
            }
            page_token = body["data"]["page_token"].as_str().map(|s| s.to_string());
        }
        Ok(tokens)
    }
}

fn check_lark_code(body: &Value, ctx: &str) -> Result<(), SynapticError> {
    let code = body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(SynapticError::Loader(format!(
            "Lark API error ({ctx}) code={code}: {}",
            body["msg"].as_str().unwrap_or("unknown")
        )));
    }
    Ok(())
}

#[async_trait]
impl Loader for LarkDocLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut all_tokens = self.doc_tokens.clone();

        if let Some(ref space_id) = self.wiki_space_id {
            let wiki_tokens = self.list_wiki_nodes(space_id).await?;
            all_tokens.extend(wiki_tokens);
        }

        let mut documents = Vec::new();
        for token in &all_tokens {
            match self.fetch_doc_content(token).await {
                Ok(doc) => documents.push(doc),
                Err(e) => {
                    tracing::warn!("Failed to load Lark doc {token}: {e}");
                }
            }
        }
        Ok(documents)
    }
}
