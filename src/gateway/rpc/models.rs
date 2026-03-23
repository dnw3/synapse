//! RPC handlers for model provider listing.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// models.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let config = &ctx.state.core.config;

    let mut provider_models: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(models) = &config.model_catalog {
        for m in models {
            let provider = m.provider.clone().unwrap_or_else(|| "default".to_string());
            provider_models
                .entry(provider)
                .or_default()
                .push(m.name.clone());
        }
    }

    let mut providers = Vec::new();
    if let Some(catalog) = &config.provider_catalog {
        for p in catalog {
            let models = provider_models.remove(&p.name).unwrap_or_default();
            providers.push(json!({
                "name": p.name,
                "base_url": p.base_url,
                "models": models,
            }));
        }
    }

    if !providers.iter().any(|p| {
        p.get("name")
            .and_then(|v| v.as_str())
            .map(|n| n == "default")
            .unwrap_or(false)
    }) {
        let base_model = config.model_config().model.clone();
        let base_provider = config.model_config().provider.clone();
        providers.insert(
            0,
            json!({
                "name": base_provider,
                "base_url": config.model_config().base_url.clone().unwrap_or_default(),
                "models": [base_model],
            }),
        );
    }

    Ok(json!(providers))
}
