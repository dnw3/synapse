//! ClawHub Registry client — search, install, and update skills from the hub.

pub mod install;

use serde::{Deserialize, Serialize};

/// A skill entry returned from ClawHub search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSkillEntry {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub downloads: u64,
}

/// Detailed skill info from ClawHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSkillDetail {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub downloads: u64,
    pub readme: Option<String>,
    pub skill_md: Option<String>,
    pub files: Vec<String>,
}

/// HTTP client for the ClawHub REST API.
pub struct ClawHubClient {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl ClawHubClient {
    /// Create a new ClawHub client with the given base URL.
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            api_key,
        }
    }

    /// Create from config defaults.
    pub fn from_config(config: &crate::config::SynapseConfig) -> Self {
        let base_url = config
            .hub
            .as_ref()
            .and_then(|h| h.url.as_deref())
            .unwrap_or("https://hub.openclaw.ai/api");

        let api_key = config
            .hub
            .as_ref()
            .and_then(|h| h.api_key_env.as_deref())
            .and_then(|env_name| std::env::var(env_name).ok());

        Self::new(base_url, api_key)
    }

    /// Search for skills on ClawHub.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HubSkillEntry>, Box<dyn std::error::Error>> {
        let url = format!("{}/skills/search?q={}&limit={}", self.base_url, urlencoding::encode(query), limit);
        let mut req = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        tracing::info!(query = %query, limit = %limit, "hub search");

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("ClawHub search failed: HTTP {}", resp.status()).into());
        }

        let entries: Vec<HubSkillEntry> = resp.json().await?;
        Ok(entries)
    }

    /// Get detailed info about a specific skill.
    pub async fn get(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<HubSkillDetail, Box<dyn std::error::Error>> {
        let url = if let Some(ver) = version {
            format!("{}/skills/{}/{}", self.base_url, urlencoding::encode(name), ver)
        } else {
            format!("{}/skills/{}", self.base_url, urlencoding::encode(name))
        };

        let mut req = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        tracing::info!(name = %name, "hub get skill detail");

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("ClawHub get failed: HTTP {}", resp.status()).into());
        }

        let detail: HubSkillDetail = resp.json().await?;
        Ok(detail)
    }

    /// Download a skill's SKILL.md content.
    pub async fn download_skill_md(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = if let Some(ver) = version {
            format!("{}/skills/{}/{}/download", self.base_url, urlencoding::encode(name), ver)
        } else {
            format!("{}/skills/{}/download", self.base_url, urlencoding::encode(name))
        };

        let mut req = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        tracing::info!(name = %name, "hub download skill");

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("ClawHub download failed: HTTP {}", resp.status()).into());
        }

        Ok(resp.text().await?)
    }
}
