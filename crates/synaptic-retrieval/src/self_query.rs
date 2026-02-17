use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapseError};

use crate::{Document, Retriever};

/// Describes a metadata field for the LLM to understand available filters.
#[derive(Debug, Clone)]
pub struct MetadataFieldInfo {
    pub name: String,
    pub description: String,
    pub field_type: String,
}

/// Uses a ChatModel to parse a user query into a structured query + metadata filters,
/// then applies those filters to results from a base retriever.
pub struct SelfQueryRetriever {
    base: Arc<dyn Retriever>,
    model: Arc<dyn ChatModel>,
    field_info: Vec<MetadataFieldInfo>,
}

impl SelfQueryRetriever {
    pub fn new(
        base: Arc<dyn Retriever>,
        model: Arc<dyn ChatModel>,
        field_info: Vec<MetadataFieldInfo>,
    ) -> Self {
        Self {
            base,
            model,
            field_info,
        }
    }

    fn build_prompt(&self, query: &str) -> String {
        let fields_desc = self
            .field_info
            .iter()
            .map(|f| format!("- {} ({}): {}", f.name, f.field_type, f.description))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Given the following user query, extract a search query and any metadata filters.

Available metadata fields:
{fields_desc}

Respond with a JSON object with two keys:
- "query": the text query to search for (string)
- "filters": an array of filter objects, each with "field", "op" (one of "eq", "gt", "lt", "gte", "lte", "contains"), and "value"

If no filters apply, use an empty array.

User query: {query}

Respond with ONLY the JSON object, no explanation."#
        )
    }

    async fn parse_query(&self, query: &str) -> Result<(String, Vec<Filter>), SynapseError> {
        let prompt = self.build_prompt(query);
        let request = ChatRequest::new(vec![Message::human(prompt)]);
        let response = self.model.chat(request).await?;
        let content = response.message.content().to_string();

        // Try to parse as JSON
        let parsed: Value = serde_json::from_str(content.trim()).map_err(|_| {
            SynapseError::Retriever(format!("Failed to parse self-query response: {content}"))
        })?;

        let search_query = parsed["query"].as_str().unwrap_or(query).to_string();

        let filters = parsed["filters"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        let field = f["field"].as_str()?.to_string();
                        let op = f["op"].as_str().unwrap_or("eq").to_string();
                        let value = f["value"].clone();
                        // Only include filters for known fields
                        if self.field_info.iter().any(|fi| fi.name == field) {
                            Some(Filter { field, op, value })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok((search_query, filters))
    }
}

#[derive(Debug, Clone)]
struct Filter {
    field: String,
    op: String,
    value: Value,
}

fn apply_filter(doc: &Document, filter: &Filter) -> bool {
    let meta_value = match doc.metadata.get(&filter.field) {
        Some(v) => v,
        None => return false,
    };

    match filter.op.as_str() {
        "eq" => meta_value == &filter.value,
        "contains" => {
            if let (Some(mv), Some(fv)) = (meta_value.as_str(), filter.value.as_str()) {
                mv.contains(fv)
            } else {
                false
            }
        }
        "gt" => compare_values(meta_value, &filter.value).is_some_and(|c| c > 0),
        "gte" => compare_values(meta_value, &filter.value).is_some_and(|c| c >= 0),
        "lt" => compare_values(meta_value, &filter.value).is_some_and(|c| c < 0),
        "lte" => compare_values(meta_value, &filter.value).is_some_and(|c| c <= 0),
        _ => true, // unknown op passes through
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<i32> {
    match (a.as_f64(), b.as_f64()) {
        (Some(av), Some(bv)) => {
            if av > bv {
                Some(1)
            } else if av < bv {
                Some(-1)
            } else {
                Some(0)
            }
        }
        _ => match (a.as_str(), b.as_str()) {
            (Some(av), Some(bv)) => Some(av.cmp(bv) as i32),
            _ => None,
        },
    }
}

#[async_trait]
impl Retriever for SelfQueryRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        let (search_query, filters) = self.parse_query(query).await?;

        let docs = self.base.retrieve(&search_query, top_k * 2).await?;

        let filtered: Vec<Document> = if filters.is_empty() {
            docs
        } else {
            docs.into_iter()
                .filter(|doc| filters.iter().all(|f| apply_filter(doc, f)))
                .collect()
        };

        Ok(filtered.into_iter().take(top_k).collect())
    }
}
