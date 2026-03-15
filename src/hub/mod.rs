//! ClawHub Registry client — search, install, and update skills from the hub.

pub mod install;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types matching the real ClawHub API v1 responses
// ---------------------------------------------------------------------------

/// A search result item from `GET /api/v1/search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubSearchResult {
    pub score: Option<f64>,
    pub slug: String,
    pub display_name: Option<String>,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub updated_at: Option<u64>,
}

/// Wrapper for the search response.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<HubSearchResult>,
}

/// A skill list item from `GET /api/v1/skills`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubSkillListItem {
    pub slug: String,
    pub display_name: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub stats: Option<HubSkillStats>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
    pub latest_version: Option<HubLatestVersion>,
    pub metadata: Option<HubSkillMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubSkillStats {
    pub downloads: Option<u64>,
    pub stars: Option<u64>,
    pub versions: Option<u64>,
    pub installs_all_time: Option<u64>,
    pub installs_current: Option<u64>,
    pub comments: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubLatestVersion {
    pub version: Option<String>,
    pub created_at: Option<u64>,
    pub changelog: Option<String>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSkillMetadata {
    pub os: Option<Vec<String>>,
    pub systems: Option<Vec<String>>,
}

/// Wrapper for the skills list response.
#[derive(Debug, Deserialize)]
struct SkillsListResponse {
    items: Vec<HubSkillListItem>,
}

/// A version entry for a skill on ClawHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubVersionEntry {
    pub version: String,
    pub created_at: Option<String>,
    pub downloads: Option<u64>,
}

/// Result of publishing a skill to ClawHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubPublishResult {
    pub ok: bool,
    #[serde(rename = "skillId")]
    pub skill_id: Option<String>,
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
}

/// Extracted file listing from a skill zip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFileEntry {
    pub name: String,
    pub size: u64,
}

/// Response for skill files extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFilesResponse {
    pub files: Vec<SkillFileEntry>,
    pub skill_md: Option<String>,
}

/// Current authenticated user info from ClawHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubUser {
    pub handle: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// HTTP client for the ClawHub REST API.
pub struct ClawHubClient {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl ClawHubClient {
    /// Create a new ClawHub client with the given base URL.
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            api_key,
        }
    }

    /// Create from config defaults.
    pub fn from_config(config: &crate::config::SynapseConfig) -> Self {
        let base_url = config
            .hub
            .as_ref()
            .and_then(|h| h.url.as_deref())
            .unwrap_or("https://clawhub.ai");

        let api_key = config
            .hub
            .as_ref()
            .and_then(|h| h.api_key_env.as_deref())
            .and_then(|env_name| std::env::var(env_name).ok());

        Self::new(base_url, api_key)
    }

    /// Whether hub is configured (has API key).
    pub fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            req.bearer_auth(key)
        } else {
            req
        }
    }

    // -----------------------------------------------------------------------
    // Search (vector search via /api/v1/search)
    // -----------------------------------------------------------------------

    /// Search for skills on ClawHub using vector search.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HubSearchResult>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/search?q={}&limit={}",
            self.base_url,
            urlencoding::encode(query),
            limit
        );
        let req = self.auth(self.client.get(&url));

        tracing::info!(query = %query, limit = %limit, "hub search");

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("ClawHub search failed: HTTP {}", resp.status()).into());
        }

        let body: SearchResponse = resp.json().await?;
        Ok(body.results)
    }

    // -----------------------------------------------------------------------
    // List (paginated browse via /api/v1/skills)
    // -----------------------------------------------------------------------

    /// List skills from ClawHub with sorting and pagination.
    pub async fn list(
        &self,
        limit: usize,
        sort: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<Vec<HubSkillListItem>, Box<dyn std::error::Error>> {
        let mut url = format!("{}/api/v1/skills?limit={}", self.base_url, limit);
        if let Some(s) = sort {
            url.push_str(&format!("&sort={}", s));
        }
        if let Some(c) = cursor {
            url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
        }
        let req = self.auth(self.client.get(&url));

        tracing::info!(limit = %limit, sort = ?sort, "hub list skills");

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("ClawHub list failed: HTTP {}", resp.status()).into());
        }

        let body: SkillsListResponse = resp.json().await?;
        Ok(body.items)
    }

    // -----------------------------------------------------------------------
    // Detail
    // -----------------------------------------------------------------------

    /// Get detailed info about a skill (includes owner, metadata).
    pub async fn detail(
        &self,
        slug: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/skills/{}",
            self.base_url,
            urlencoding::encode(slug)
        );
        let req = self.auth(self.client.get(&url));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("detail failed: HTTP {}", resp.status()).into());
        }
        Ok(resp.json().await?)
    }

    // -----------------------------------------------------------------------
    // Download
    // -----------------------------------------------------------------------

    /// Download a skill as a zip archive (returns bytes).
    pub async fn download_zip(
        &self,
        slug: &str,
        version: Option<&str>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let url = match version {
            Some(v) => format!(
                "{}/api/v1/download?slug={}&version={}",
                self.base_url,
                urlencoding::encode(slug),
                v
            ),
            None => format!(
                "{}/api/v1/download?slug={}",
                self.base_url,
                urlencoding::encode(slug)
            ),
        };
        let req = self.auth(self.client.get(&url));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("download failed: HTTP {}", resp.status()).into());
        }
        // Enforce size limit to prevent OOM from malicious/huge responses
        const MAX_SKILL_ZIP: u64 = 50 * 1024 * 1024; // 50 MB
        if let Some(len) = resp.content_length() {
            if len > MAX_SKILL_ZIP {
                return Err(
                    format!("skill zip too large: {} bytes (max {})", len, MAX_SKILL_ZIP).into(),
                );
            }
        }
        let bytes = resp.bytes().await?;
        if bytes.len() as u64 > MAX_SKILL_ZIP {
            return Err(format!(
                "skill zip too large: {} bytes (max {})",
                bytes.len(),
                MAX_SKILL_ZIP
            )
            .into());
        }
        Ok(bytes.to_vec())
    }

    // -----------------------------------------------------------------------
    // Skill files (extract from zip)
    // -----------------------------------------------------------------------

    /// Download a skill zip and extract file listing + SKILL.md content.
    pub async fn skill_files(
        &self,
        slug: &str,
    ) -> Result<SkillFilesResponse, Box<dyn std::error::Error>> {
        let zip_bytes = self.download_zip(slug, None).await?;
        let cursor = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor)?;

        let mut files = Vec::new();
        let mut skill_md = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            let size = file.size();
            files.push(SkillFileEntry {
                name: name.clone(),
                size,
            });

            // Extract SKILL.md content
            if name == "SKILL.md" || name.ends_with("/SKILL.md") {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut file, &mut content)?;
                skill_md = Some(content);
            }
        }

        Ok(SkillFilesResponse { files, skill_md })
    }

    /// Download a skill zip and extract content of a specific file by name.
    pub async fn skill_file_content(
        &self,
        slug: &str,
        file_path: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let zip_bytes = self.download_zip(slug, None).await?;
        let cursor = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.name() == file_path {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut file, &mut content)?;
                return Ok(Some(content));
            }
        }

        Ok(None)
    }

    // -----------------------------------------------------------------------
    // Versions / Resolve
    // -----------------------------------------------------------------------

    /// List versions of a skill.
    pub async fn list_versions(
        &self,
        slug: &str,
    ) -> Result<Vec<HubVersionEntry>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/skills/{}/versions",
            self.base_url,
            urlencoding::encode(slug)
        );
        let req = self.auth(self.client.get(&url));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("list versions failed: HTTP {}", resp.status()).into());
        }
        Ok(resp.json().await?)
    }

    /// Resolve a skill fingerprint to a known version.
    #[allow(dead_code)]
    pub async fn resolve_fingerprint(
        &self,
        slug: &str,
        hash: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/resolve?slug={}&hash={}",
            self.base_url,
            urlencoding::encode(slug),
            hash
        );
        let resp = self.auth(self.client.get(&url)).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("resolve failed: HTTP {}", resp.status()).into());
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(body
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    // -----------------------------------------------------------------------
    // Star / Unstar
    // -----------------------------------------------------------------------

    /// Star a skill.
    pub async fn star(&self, slug: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/stars/{}",
            self.base_url,
            urlencoding::encode(slug)
        );
        let req = self.auth(self.client.post(&url));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("star failed: HTTP {}", resp.status()).into());
        }
        Ok(())
    }

    /// Unstar a skill.
    pub async fn unstar(&self, slug: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/stars/{}",
            self.base_url,
            urlencoding::encode(slug)
        );
        let req = self.auth(self.client.delete(&url));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("unstar failed: HTTP {}", resp.status()).into());
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Publish
    // -----------------------------------------------------------------------

    /// Publish a skill to ClawHub.
    pub async fn publish(
        &self,
        slug: &str,
        version: &str,
        files: Vec<(String, Vec<u8>)>,
    ) -> Result<HubPublishResult, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/skills", self.base_url);
        let mut form = reqwest::multipart::Form::new()
            .text("slug", slug.to_string())
            .text("version", version.to_string());
        for (name, content) in files {
            let part = reqwest::multipart::Part::bytes(content).file_name(name.clone());
            form = form.part(name, part);
        }
        let req = self.auth(self.client.post(&url).multipart(form));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let body = resp.text().await?;
            return Err(format!("publish failed: {}", body).into());
        }
        Ok(resp.json().await?)
    }

    // -----------------------------------------------------------------------
    // Whoami
    // -----------------------------------------------------------------------

    /// Get current user info (whoami).
    pub async fn whoami(&self) -> Result<HubUser, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/whoami", self.base_url);
        let req = self.auth(self.client.get(&url));
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(format!("whoami failed: HTTP {}", resp.status()).into());
        }
        Ok(resp.json().await?)
    }
}
