#![allow(dead_code)]
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasOutput {
    pub html: String,
    pub interactive: bool,
    pub actions: Vec<CanvasAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAction {
    pub id: String,
    pub label: String,
    pub callback: String,
}

#[async_trait]
pub trait CanvasRenderer: Send + Sync {
    fn canvas_type(&self) -> &str;
    fn render(
        &self,
        data: &Value,
    ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct CanvasEngine {
    renderers: HashMap<String, Arc<dyn CanvasRenderer>>,
}

impl CanvasEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            renderers: HashMap::new(),
        };
        // Register built-in renderers
        engine.register(Arc::new(DiagramRenderer));
        engine.register(Arc::new(FormRenderer));
        engine.register(Arc::new(TableRenderer));
        engine.register(Arc::new(PlotRenderer));
        engine.register(Arc::new(CardRenderer));
        engine
    }

    pub fn register(&mut self, renderer: Arc<dyn CanvasRenderer>) {
        self.renderers
            .insert(renderer.canvas_type().to_string(), renderer);
    }

    pub fn render(&self, canvas_type: &str, data: &Value) -> Option<CanvasOutput> {
        self.renderers.get(canvas_type)?.render(data).ok()
    }
}

impl Default for CanvasEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DiagramRenderer — wraps content in a Mermaid.js div
// ---------------------------------------------------------------------------

pub struct DiagramRenderer;

#[async_trait]
impl CanvasRenderer for DiagramRenderer {
    fn canvas_type(&self) -> &str {
        "diagram"
    }

    fn render(
        &self,
        data: &Value,
    ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>> {
        let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");

        let html = format!(r#"<div class="mermaid">{content}</div>"#);

        Ok(CanvasOutput {
            html,
            interactive: false,
            actions: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// FormRenderer — generates HTML form from JSON schema fields
// ---------------------------------------------------------------------------

pub struct FormRenderer;

#[async_trait]
impl CanvasRenderer for FormRenderer {
    fn canvas_type(&self) -> &str {
        "form"
    }

    fn render(
        &self,
        data: &Value,
    ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>> {
        let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("Form");
        let action = data.get("action").and_then(|v| v.as_str()).unwrap_or("#");
        let method = data
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("post");

        let empty_fields: Vec<Value> = vec![];
        let fields: &[Value] = data
            .get("fields")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&empty_fields);

        let mut fields_html = String::new();
        for field in fields {
            let name = field
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("field");
            let label = field.get("label").and_then(|v| v.as_str()).unwrap_or(name);
            let field_type = field.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            let placeholder = field
                .get("placeholder")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let required = field
                .get("required")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let required_attr = if required { " required" } else { "" };

            match field_type {
                "textarea" => {
                    fields_html.push_str(&format!(
                        r#"<div class="form-group"><label for="{name}">{label}</label><textarea id="{name}" name="{name}" placeholder="{placeholder}"{required_attr}></textarea></div>"#
                    ));
                }
                "select" => {
                    let empty_options: Vec<Value> = vec![];
                    let options: &[Value] = field
                        .get("options")
                        .and_then(|v| v.as_array())
                        .map(|v| v.as_slice())
                        .unwrap_or(&empty_options);
                    let mut options_html = String::new();
                    for opt in options {
                        let opt_value = opt.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let opt_label = opt
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(opt_value);
                        options_html.push_str(&format!(
                            r#"<option value="{opt_value}">{opt_label}</option>"#
                        ));
                    }
                    fields_html.push_str(&format!(
                        r#"<div class="form-group"><label for="{name}">{label}</label><select id="{name}" name="{name}"{required_attr}>{options_html}</select></div>"#
                    ));
                }
                _ => {
                    fields_html.push_str(&format!(
                        r#"<div class="form-group"><label for="{name}">{label}</label><input type="{field_type}" id="{name}" name="{name}" placeholder="{placeholder}"{required_attr} /></div>"#
                    ));
                }
            }
        }

        let submit_label = data
            .get("submit_label")
            .and_then(|v| v.as_str())
            .unwrap_or("Submit");

        let html = format!(
            r#"<div class="canvas-form"><h3 class="form-title">{title}</h3><form action="{action}" method="{method}">{fields_html}<div class="form-actions"><button type="submit" class="btn-primary">{submit_label}</button></div></form></div>"#
        );

        let actions = vec![CanvasAction {
            id: "submit".to_string(),
            label: submit_label.to_string(),
            callback: "form.submit".to_string(),
        }];

        Ok(CanvasOutput {
            html,
            interactive: true,
            actions,
        })
    }
}

// ---------------------------------------------------------------------------
// TableRenderer — generates HTML table from rows/columns data
// ---------------------------------------------------------------------------

pub struct TableRenderer;

#[async_trait]
impl CanvasRenderer for TableRenderer {
    fn canvas_type(&self) -> &str {
        "table"
    }

    fn render(
        &self,
        data: &Value,
    ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>> {
        let empty_columns: Vec<Value> = vec![];
        let empty_rows: Vec<Value> = vec![];

        let columns: &[Value] = data
            .get("columns")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&empty_columns);
        let rows: &[Value] = data
            .get("rows")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&empty_rows);
        let title = data.get("title").and_then(|v| v.as_str());

        // Build header row
        let mut header_html = String::from("<thead><tr>");
        for col in columns {
            let col_label = col
                .get("label")
                .and_then(|v| v.as_str())
                .or_else(|| col.as_str())
                .unwrap_or("");
            header_html.push_str(&format!("<th>{col_label}</th>"));
        }
        header_html.push_str("</tr></thead>");

        // Build column keys for cell lookup
        let col_keys: Vec<&str> = columns
            .iter()
            .map(|col| {
                col.get("key")
                    .and_then(|v| v.as_str())
                    .or_else(|| col.as_str())
                    .unwrap_or("")
            })
            .collect();

        // Build body rows
        let mut body_html = String::from("<tbody>");
        for row in rows {
            body_html.push_str("<tr>");
            if col_keys.is_empty() {
                // No column definitions — render row as array of values
                if let Some(arr) = row.as_array() {
                    for cell in arr {
                        let cell_text = cell.as_str().unwrap_or(&cell.to_string()).to_string();
                        body_html.push_str(&format!("<td>{cell_text}</td>"));
                    }
                }
            } else {
                for key in &col_keys {
                    let cell_val = row.get(key);
                    let cell_text = cell_val
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| cell_val.map(|v| v.to_string()).unwrap_or_default());
                    body_html.push_str(&format!("<td>{cell_text}</td>"));
                }
            }
            body_html.push_str("</tr>");
        }
        body_html.push_str("</tbody>");

        let title_html = title
            .map(|t| format!(r#"<caption class="table-title">{t}</caption>"#))
            .unwrap_or_default();

        let html = format!(
            r#"<div class="canvas-table"><table class="data-table">{title_html}{header_html}{body_html}</table></div>"#
        );

        Ok(CanvasOutput {
            html,
            interactive: false,
            actions: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// PlotRenderer — generates Recharts-compatible JSON wrapper
// (frontend renders the actual chart using this payload)
// ---------------------------------------------------------------------------

pub struct PlotRenderer;

#[async_trait]
impl CanvasRenderer for PlotRenderer {
    fn canvas_type(&self) -> &str {
        "plot"
    }

    fn render(
        &self,
        data: &Value,
    ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>> {
        let chart_type = data
            .get("chart_type")
            .and_then(|v| v.as_str())
            .unwrap_or("line");
        let title = data
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Chart");

        // Serialize the full data payload for Recharts consumption
        let recharts_payload = serde_json::json!({
            "type": chart_type,
            "title": title,
            "data": data.get("data").unwrap_or(&Value::Array(vec![])),
            "x_key": data.get("x_key").and_then(|v| v.as_str()).unwrap_or("x"),
            "y_keys": data.get("y_keys").unwrap_or(&Value::Array(vec![])),
            "colors": data.get("colors").unwrap_or(&Value::Array(vec![])),
            "width": data.get("width").and_then(|v| v.as_u64()).unwrap_or(600),
            "height": data.get("height").and_then(|v| v.as_u64()).unwrap_or(400),
        });

        let payload_str =
            serde_json::to_string(&recharts_payload).unwrap_or_else(|_| "{}".to_string());

        let html = format!(
            r#"<div class="canvas-plot" data-recharts="{}" data-chart-type="{chart_type}"><div class="plot-title">{title}</div><div class="plot-placeholder" style="width:{}px;height:{}px;display:flex;align-items:center;justify-content:center;background:#f5f5f5;border-radius:8px;"><span>Chart: {title}</span></div></div>"#,
            html_escape(&payload_str),
            recharts_payload
                .get("width")
                .and_then(|v| v.as_u64())
                .unwrap_or(600),
            recharts_payload
                .get("height")
                .and_then(|v| v.as_u64())
                .unwrap_or(400),
        );

        Ok(CanvasOutput {
            html,
            interactive: false,
            actions: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// CardRenderer — generates a styled info card with title, description, image
// ---------------------------------------------------------------------------

pub struct CardRenderer;

#[async_trait]
impl CanvasRenderer for CardRenderer {
    fn canvas_type(&self) -> &str {
        "card"
    }

    fn render(
        &self,
        data: &Value,
    ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>> {
        let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("Card");
        let description = data
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let image_url = data.get("image").and_then(|v| v.as_str());
        let link_url = data.get("link").and_then(|v| v.as_str());
        let badge = data.get("badge").and_then(|v| v.as_str());

        let image_html = image_url
            .map(|url| {
                format!(
                    r#"<div class="card-image"><img src="{url}" alt="{title}" loading="lazy" /></div>"#
                )
            })
            .unwrap_or_default();

        let badge_html = badge
            .map(|b| format!(r#"<span class="card-badge">{b}</span>"#))
            .unwrap_or_default();

        let description_html = if description.is_empty() {
            String::new()
        } else {
            format!(r#"<p class="card-description">{description}</p>"#)
        };

        let card_inner = format!(
            r#"{image_html}<div class="card-body">{badge_html}<h3 class="card-title">{title}</h3>{description_html}</div>"#
        );

        let html = if let Some(url) = link_url {
            format!(
                r#"<div class="canvas-card"><a href="{url}" class="card-link" target="_blank" rel="noopener noreferrer">{card_inner}</a></div>"#
            )
        } else {
            format!(r#"<div class="canvas-card">{card_inner}</div>"#)
        };

        let mut actions = vec![];
        if let Some(url) = link_url {
            actions.push(CanvasAction {
                id: "open_link".to_string(),
                label: "Open".to_string(),
                callback: format!("window.open('{url}', '_blank')"),
            });
        }

        Ok(CanvasOutput {
            html,
            interactive: link_url.is_some(),
            actions,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diagram_renders_mermaid() {
        let engine = CanvasEngine::new();
        let data = json!({ "content": "graph TD; A-->B; B-->C;" });
        let output = engine.render("diagram", &data).expect("should render");
        assert!(output.html.contains(r#"class="mermaid""#));
        assert!(output.html.contains("graph TD; A-->B; B-->C;"));
        assert!(!output.interactive);
        assert!(output.actions.is_empty());
    }

    #[test]
    fn diagram_renders_empty_content() {
        let renderer = DiagramRenderer;
        let output = renderer.render(&json!({})).unwrap();
        assert!(output.html.contains(r#"class="mermaid""#));
    }

    #[test]
    fn table_renders_html() {
        let engine = CanvasEngine::new();
        let data = json!({
            "title": "Users",
            "columns": [
                {"key": "name", "label": "Name"},
                {"key": "age",  "label": "Age"}
            ],
            "rows": [
                {"name": "Alice", "age": "30"},
                {"name": "Bob",   "age": "25"}
            ]
        });
        let output = engine.render("table", &data).expect("should render");
        assert!(output.html.contains("<table"));
        assert!(output.html.contains("<th>Name</th>"));
        assert!(output.html.contains("<th>Age</th>"));
        assert!(output.html.contains("<td>Alice</td>"));
        assert!(output.html.contains("<td>Bob</td>"));
        assert!(output.html.contains("Users"));
    }

    #[test]
    fn table_renders_without_columns() {
        let renderer = TableRenderer;
        let data = json!({ "rows": [["cell1", "cell2"], ["cell3", "cell4"]] });
        let output = renderer.render(&data).unwrap();
        assert!(output.html.contains("<tbody>"));
    }

    #[test]
    fn card_renders_with_title() {
        let engine = CanvasEngine::new();
        let data = json!({
            "title": "Hello World",
            "description": "A test card",
            "badge": "New"
        });
        let output = engine.render("card", &data).expect("should render");
        assert!(output.html.contains("Hello World"));
        assert!(output.html.contains("A test card"));
        assert!(output.html.contains("New"));
        assert!(output.html.contains("canvas-card"));
        assert!(!output.interactive);
    }

    #[test]
    fn card_renders_with_link() {
        let renderer = CardRenderer;
        let data = json!({
            "title": "Link Card",
            "link": "https://example.com"
        });
        let output = renderer.render(&data).unwrap();
        assert!(output.html.contains("https://example.com"));
        assert!(output.interactive);
        assert_eq!(output.actions.len(), 1);
        assert_eq!(output.actions[0].id, "open_link");
    }

    #[test]
    fn card_renders_with_image() {
        let renderer = CardRenderer;
        let data = json!({
            "title": "Image Card",
            "image": "https://example.com/image.png"
        });
        let output = renderer.render(&data).unwrap();
        assert!(output.html.contains("card-image"));
        assert!(output.html.contains("https://example.com/image.png"));
    }

    #[test]
    fn form_renders_fields() {
        let engine = CanvasEngine::new();
        let data = json!({
            "title": "Contact",
            "action": "/submit",
            "method": "post",
            "fields": [
                {"name": "email", "label": "Email", "type": "email", "required": true},
                {"name": "message", "label": "Message", "type": "textarea"}
            ],
            "submit_label": "Send"
        });
        let output = engine.render("form", &data).expect("should render");
        assert!(output.html.contains("Contact"));
        assert!(output.html.contains(r#"type="email""#));
        assert!(output.html.contains("<textarea"));
        assert!(output.html.contains("required"));
        assert!(output.html.contains("Send"));
        assert!(output.interactive);
    }

    #[test]
    fn form_renders_select_field() {
        let renderer = FormRenderer;
        let data = json!({
            "fields": [{
                "name": "color",
                "label": "Color",
                "type": "select",
                "options": [
                    {"value": "red", "label": "Red"},
                    {"value": "blue", "label": "Blue"}
                ]
            }]
        });
        let output = renderer.render(&data).unwrap();
        assert!(output.html.contains("<select"));
        assert!(output.html.contains(r#"value="red""#));
        assert!(output.html.contains("Blue"));
    }

    #[test]
    fn plot_renders_recharts_wrapper() {
        let engine = CanvasEngine::new();
        let data = json!({
            "chart_type": "bar",
            "title": "Sales",
            "data": [{"x": "Jan", "y": 100}, {"x": "Feb", "y": 200}],
            "x_key": "x",
            "y_keys": ["y"]
        });
        let output = engine.render("plot", &data).expect("should render");
        assert!(output.html.contains("canvas-plot"));
        assert!(output.html.contains("data-recharts"));
        assert!(output.html.contains("bar"));
        assert!(output.html.contains("Sales"));
    }

    #[test]
    fn engine_render_unknown_type_returns_none() {
        let engine = CanvasEngine::new();
        assert!(engine.render("unknown_type", &json!({})).is_none());
    }

    #[test]
    fn engine_register_custom_renderer() {
        struct EchoRenderer;
        #[async_trait::async_trait]
        impl CanvasRenderer for EchoRenderer {
            fn canvas_type(&self) -> &str {
                "echo"
            }
            fn render(
                &self,
                data: &Value,
            ) -> Result<CanvasOutput, Box<dyn std::error::Error + Send + Sync>> {
                Ok(CanvasOutput {
                    html: data.to_string(),
                    interactive: false,
                    actions: vec![],
                })
            }
        }
        let mut engine = CanvasEngine::new();
        engine.register(Arc::new(EchoRenderer));
        let out = engine.render("echo", &json!({"hello": "world"}));
        assert!(out.is_some());
        assert!(out.unwrap().html.contains("hello"));
    }
}
