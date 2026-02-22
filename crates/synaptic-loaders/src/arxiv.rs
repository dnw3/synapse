use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

/// Loader for arXiv papers via the arXiv API (returns abstracts as documents).
pub struct ArxivLoader {
    client: reqwest::Client,
    query: String,
    max_results: usize,
}

impl ArxivLoader {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            query: query.into(),
            max_results: 10,
        }
    }

    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }
}

#[async_trait]
impl Loader for ArxivLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let encoded_query = urlencoding::encode(&self.query);
        let url = format!(
            "http://export.arxiv.org/api/query?search_query={}&max_results={}&sortBy=submittedDate",
            encoded_query, self.max_results
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("arXiv fetch: {e}")))?;
        let text = resp
            .text()
            .await
            .map_err(|e| SynapticError::Loader(format!("arXiv read: {e}")))?;

        parse_arxiv_xml(&text)
    }
}

fn parse_arxiv_xml(xml: &str) -> Result<Vec<Document>, SynapticError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut documents = Vec::new();
    let mut current_entry: Option<HashMap<String, String>> = None;
    let mut current_field: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "entry" => {
                        current_entry = Some(HashMap::new());
                    }
                    "id" | "title" | "summary" | "published" => {
                        if current_entry.is_some() {
                            current_field = Some(name);
                        }
                    }
                    "author" if current_entry.is_some() => {
                        current_field = Some("author_container".to_string());
                    }
                    "name" if current_field.as_deref() == Some("author_container") => {
                        current_field = Some("author_name".to_string());
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if let (Some(entry), Some(field)) = (current_entry.as_mut(), &current_field) {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        match field.as_str() {
                            "id" => {
                                entry.insert(
                                    "id".into(),
                                    text.replace("http://arxiv.org/abs/", "")
                                        .replace("https://arxiv.org/abs/", ""),
                                );
                            }
                            "title" => {
                                entry.entry("title".into()).or_insert(text);
                            }
                            "summary" => {
                                entry.insert("summary".into(), text);
                            }
                            "published" => {
                                entry.insert("published".into(), text);
                            }
                            "author_name" => {
                                let authors =
                                    entry.entry("authors".into()).or_insert_with(String::new);
                                if !authors.is_empty() {
                                    authors.push_str(", ");
                                }
                                authors.push_str(&text);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if name == "entry" {
                    if let Some(entry) = current_entry.take() {
                        let arxiv_id = entry
                            .get("id")
                            .cloned()
                            .unwrap_or_else(|| format!("arxiv-{}", documents.len()));
                        let content = entry.get("summary").cloned().unwrap_or_default();
                        let mut metadata = HashMap::new();
                        if let Some(title) = entry.get("title") {
                            metadata.insert("title".to_string(), Value::String(title.clone()));
                        }
                        if let Some(authors) = entry.get("authors") {
                            metadata.insert("authors".to_string(), Value::String(authors.clone()));
                        }
                        if let Some(published) = entry.get("published") {
                            metadata
                                .insert("published".to_string(), Value::String(published.clone()));
                        }
                        metadata.insert(
                            "source".to_string(),
                            Value::String(format!("arxiv:{}", arxiv_id)),
                        );
                        metadata.insert(
                            "url".to_string(),
                            Value::String(format!("https://arxiv.org/abs/{}", arxiv_id)),
                        );
                        documents.push(Document {
                            id: arxiv_id,
                            content,
                            metadata,
                        });
                    }
                }
                if matches!(
                    name.as_str(),
                    "id" | "title" | "summary" | "published" | "name" | "author"
                ) {
                    current_field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(SynapticError::Loader(format!("XML parse error: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    Ok(documents)
}
