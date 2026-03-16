use serde_json::Value;

/// Provider that proxies LLM requests through GitHub Copilot's API.
/// Uses the user's GitHub Copilot token for authentication.
#[allow(dead_code)]
pub struct CopilotProxyConfig {
    pub github_token: String,
    pub model: String, // e.g., "gpt-4o"
}

#[allow(dead_code)]
impl CopilotProxyConfig {
    pub fn new(github_token: String) -> Self {
        Self {
            github_token,
            model: "gpt-4o".into(),
        }
    }

    /// Exchange GitHub token for a Copilot session token.
    pub async fn get_copilot_token(
        &self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let resp = client
            .get("https://api.github.com/copilot_internal/v2/token")
            .header("Authorization", format!("token {}", self.github_token))
            .header("User-Agent", "Synapse/1.0")
            .send()
            .await?
            .json::<Value>()
            .await?;

        resp.get("token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| "failed to get copilot token".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_creates_with_defaults() {
        let config = CopilotProxyConfig::new("ghp_test123".into());
        assert_eq!(config.model, "gpt-4o");
    }
}
