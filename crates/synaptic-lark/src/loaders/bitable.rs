use crate::{api::bitable::BitableApi, LarkConfig};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

/// Load Feishu/Lark Bitable records into Synaptic [`Document`]s for RAG pipelines.
///
/// Each Bitable record becomes one `Document`. The `content` field is populated from
/// the field named by [`with_content_field`], or the first string-typed field when
/// no explicit field is given. All other fields are stored in `metadata`.
///
/// # Example
///
/// ```rust,no_run
/// use synaptic_lark::{LarkConfig, LarkBitableLoader};
/// use synaptic_core::Loader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = LarkConfig::new("cli_xxx", "secret_xxx");
/// let loader = LarkBitableLoader::new(config)
///     .with_app("bascnAbcXxx")
///     .with_table("tblXxx")
///     .with_content_field("Description");
///
/// let docs = loader.load().await?;
/// for doc in &docs {
///     println!("Record: {}", doc.id);
///     println!("Content: {}", doc.content);
/// }
/// # Ok(())
/// # }
/// ```
pub struct LarkBitableLoader {
    api: BitableApi,
    app_token: Option<String>,
    table_id: Option<String>,
    view_id: Option<String>,
    /// Name of the field whose value becomes `Document.content`.
    /// When `None`, the first text-type field is used.
    content_field: Option<String>,
}

impl LarkBitableLoader {
    /// Create a new loader using the given config.
    pub fn new(config: LarkConfig) -> Self {
        Self {
            api: BitableApi::new(config),
            app_token: None,
            table_id: None,
            view_id: None,
            content_field: None,
        }
    }

    /// Set the Bitable app token (e.g. `"bascnAbcXxx"`).
    pub fn with_app(mut self, app_token: impl Into<String>) -> Self {
        self.app_token = Some(app_token.into());
        self
    }

    /// Set the table ID within the Bitable app (e.g. `"tblXxx"`).
    pub fn with_table(mut self, table_id: impl Into<String>) -> Self {
        self.table_id = Some(table_id.into());
        self
    }

    /// Optionally filter records by a specific view (e.g. `"vewXxx"`).
    pub fn with_view(mut self, view_id: impl Into<String>) -> Self {
        self.view_id = Some(view_id.into());
        self
    }

    /// Specify which field's value becomes the `Document.content`.
    ///
    /// When not set, the first string-typed field is used automatically.
    pub fn with_content_field(mut self, field: impl Into<String>) -> Self {
        self.content_field = Some(field.into());
        self
    }

    // ── Accessors (used in tests) ────────────────────────────────────────────

    /// Returns the configured app token, or `""` if not set.
    pub fn app_token(&self) -> &str {
        self.app_token.as_deref().unwrap_or("")
    }

    /// Returns the configured table ID, or `""` if not set.
    pub fn table_id(&self) -> &str {
        self.table_id.as_deref().unwrap_or("")
    }

    /// Returns the configured view ID if any.
    pub fn view_id(&self) -> Option<&str> {
        self.view_id.as_deref()
    }

    /// Returns the configured content field name if any.
    pub fn content_field(&self) -> Option<&str> {
        self.content_field.as_deref()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Convert a single Bitable record JSON object into a [`Document`].
    fn record_to_document(&self, record: &Value) -> Document {
        let record_id = record["record_id"].as_str().unwrap_or("").to_string();
        let fields = record["fields"].as_object();

        let mut metadata: HashMap<String, Value> = HashMap::new();
        metadata.insert("record_id".to_string(), Value::String(record_id.clone()));
        metadata.insert(
            "source".to_string(),
            Value::String("lark_bitable".to_string()),
        );

        let mut content = String::new();

        if let Some(fields_map) = fields {
            for (k, v) in fields_map {
                if let Some(ref cf) = self.content_field {
                    if k == cf {
                        content = value_to_text(v);
                    } else {
                        metadata.insert(k.clone(), v.clone());
                    }
                } else {
                    // Auto mode: use the first string-ish field as content.
                    if content.is_empty() {
                        if let Some(s) = v.as_str() {
                            content = s.to_string();
                        } else if v.is_array() || v.is_object() {
                            metadata.insert(k.clone(), v.clone());
                        } else {
                            content = v.to_string();
                        }
                    } else {
                        metadata.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        Document {
            id: record_id,
            content,
            metadata,
        }
    }
}

/// Convert a Bitable field value to a plain string.
///
/// Rich-text arrays (used for text/multi-line fields) are joined by extracting
/// each item's `"text"` property. Other scalar values fall back to `Value::to_string()`.
fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => v.to_string(),
    }
}

#[async_trait]
impl Loader for LarkBitableLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let app_token = self.app_token.as_deref().ok_or_else(|| {
            SynapticError::Config("LarkBitableLoader: app_token not set".to_string())
        })?;
        let table_id = self.table_id.as_deref().ok_or_else(|| {
            SynapticError::Config("LarkBitableLoader: table_id not set".to_string())
        })?;

        let mut docs = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let (items, next) = self
                .api
                .list_records_page(
                    app_token,
                    table_id,
                    self.view_id.as_deref(),
                    page_token.as_deref(),
                )
                .await?;
            for record in &items {
                docs.push(self.record_to_document(record));
            }
            match next {
                Some(pt) => page_token = Some(pt),
                None => break,
            }
        }

        Ok(docs)
    }
}
