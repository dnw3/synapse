use async_trait::async_trait;
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

/// Loader for GitHub repository files via the GitHub API.
pub struct GitHubLoader {
    client: reqwest::Client,
    owner: String,
    repo: String,
    paths: Vec<String>,
    token: Option<String>,
    branch: Option<String>,
    extensions: Vec<String>,
}

impl GitHubLoader {
    pub fn new(owner: impl Into<String>, repo: impl Into<String>, paths: Vec<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            owner: owner.into(),
            repo: repo.into(),
            paths,
            token: None,
            branch: None,
            extensions: vec![],
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    pub fn with_extensions(mut self, exts: Vec<String>) -> Self {
        self.extensions = exts;
        self
    }

    fn matches_extension(&self, path: &str) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        self.extensions
            .iter()
            .any(|ext| path.ends_with(ext.as_str()))
    }

    async fn fetch_path(&self, path: &str, docs: &mut Vec<Document>) -> Result<(), SynapticError> {
        let mut url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner, self.repo, path
        );
        if let Some(ref branch) = self.branch {
            url.push_str(&format!("?ref={}", branch));
        }
        let mut req = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "synaptic-github-loader");
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("token {}", token));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("GitHub fetch: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("GitHub parse: {e}")))?;

        if body.is_array() {
            // Directory — recurse
            for item in body.as_array().unwrap() {
                let item_type = item["type"].as_str().unwrap_or("");
                let item_path = item["path"].as_str().unwrap_or("").to_string();
                match item_type {
                    "file" if self.matches_extension(&item_path) => {
                        Box::pin(self.fetch_path(&item_path, docs)).await?;
                    }
                    "dir" => {
                        Box::pin(self.fetch_path(&item_path, docs)).await?;
                    }
                    _ => {}
                }
            }
        } else if let Some("file") = body["type"].as_str() {
            let encoded = body["content"].as_str().unwrap_or("").replace('\n', "");
            let content = base64::engine::general_purpose::STANDARD
                .decode(&encoded)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_default();
            let file_path = body["path"].as_str().unwrap_or(path).to_string();
            let mut metadata = HashMap::new();
            metadata.insert(
                "source".to_string(),
                Value::String(format!("github:{}/{}/{}", self.owner, self.repo, file_path)),
            );
            metadata.insert(
                "sha".to_string(),
                Value::String(body["sha"].as_str().unwrap_or("").to_string()),
            );
            if let Some(ref branch) = self.branch {
                metadata.insert("branch".to_string(), Value::String(branch.clone()));
            }
            docs.push(Document {
                id: file_path.clone(),
                content,
                metadata,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl Loader for GitHubLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut documents = Vec::new();
        for path in &self.paths {
            self.fetch_path(path, &mut documents).await?;
        }
        Ok(documents)
    }
}
