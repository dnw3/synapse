use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

// ======================== Request ID ========================

/// Machine identifier for LogID generation (computed once at startup).
/// Derived from HOSTNAME/POD_NAME env var, or a random fallback.
/// 6 hex chars (24 bits = 16M combinations) — collision-resistant for clusters.
fn machine_id() -> u32 {
    use std::sync::OnceLock;
    static MID: OnceLock<u32> = OnceLock::new();
    *MID.get_or_init(|| {
        let source = std::env::var("POD_NAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .or_else(|_| std::env::var("HOST"))
            .unwrap_or_else(|_| {
                // Fallback: random identifier for single-machine dev
                format!("{:06x}", uuid::Uuid::new_v4().as_u128() & 0xFF_FFFF)
            });
        // FNV-1a hash → 24 bits
        let mut h: u32 = 0x811c_9dc5;
        for b in source.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
        h & 0xFF_FFFF
    })
}

/// Generate a LogID: {13-digit decimal ms}{6-hex machine}{4-hex random}
///
/// Format (23 chars): `1773047905112a1b2c3d4e5f`
///   - [0..13]:  Unix millisecond timestamp (decimal, directly human-readable)
///   - [13..19]: machine identifier (hex, from HOSTNAME/POD_NAME FNV hash)
///   - [19..23]: random (hex, for uniqueness within same ms on same machine)
///
/// Decode timestamp (frontend): `parseInt(logid.slice(0, 13))` → milliseconds
/// Decode machine: `logid.slice(13, 19)` → hex machine hash
///
/// Inspired by ByteDance Argos LogID V2 format.
pub fn generate_request_id() -> String {
    let ts = Utc::now().timestamp_millis() as u64;
    let mid = machine_id();
    let rand = (uuid::Uuid::new_v4().as_u128() & 0xFFFF) as u16;
    format!("{:013}{:06x}{:04x}", ts, mid, rand)
}

// ======================== Log Config ========================

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    /// Console log level (trace/debug/info/warn/error/off). Default: "info"
    #[serde(default = "default_log_level")]
    pub level: String,

    /// File logging configuration.
    #[serde(default)]
    pub file: LogFileConfig,

    /// In-memory ring buffer for /api/logs queries.
    #[serde(default)]
    pub memory: LogMemoryConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFileConfig {
    /// Whether to write logs to files. Default: true
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Log directory path. Default: "~/.synapse/logs"
    #[serde(default = "default_log_path")]
    pub path: String,

    /// File log level (can be more verbose than console). Default: "debug"
    #[serde(default = "default_file_level")]
    pub level: String,

    /// Log format: "json" or "pretty". Default: "json"
    #[serde(default = "default_format")]
    pub format: String,

    /// Rotation strategy: "daily", "hourly", "never". Default: "daily"
    #[serde(default = "default_rotation")]
    pub rotation: String,

    /// Days to retain log files. 0 = keep forever. Default: 7
    #[serde(default = "default_max_days")]
    pub max_days: u32,

    /// Maximum number of log files to retain. Default: 30
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogMemoryConfig {
    /// Ring buffer capacity. Default: 10000
    #[serde(default = "default_memory_capacity")]
    pub capacity: usize,

    /// Minimum level for memory buffer. Default: "info"
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}
fn default_file_level() -> String {
    "debug".to_string()
}
fn default_log_path() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".synapse/logs")
        .to_string_lossy()
        .to_string()
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "json".to_string()
}
fn default_rotation() -> String {
    "daily".to_string()
}
fn default_max_days() -> u32 {
    7
}
fn default_max_files() -> u32 {
    30
}
fn default_memory_capacity() -> usize {
    10000
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: LogFileConfig::default(),
            memory: LogMemoryConfig::default(),
        }
    }
}

impl Default for LogFileConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            path: default_log_path(),
            level: default_file_level(),
            format: default_format(),
            rotation: default_rotation(),
            max_days: default_max_days(),
            max_files: default_max_files(),
        }
    }
}

impl Default for LogMemoryConfig {
    fn default() -> Self {
        Self {
            capacity: default_memory_capacity(),
            level: default_log_level(),
        }
    }
}

// ======================== Log Entry (for memory buffer + API) ========================

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub ts: String,
    pub level: String,
    pub request_id: Option<String>,
    pub target: String,
    pub message: String,
    pub fields: serde_json::Value,
}

// ======================== In-Memory Log Buffer ========================

#[derive(Clone)]
pub struct LogBuffer {
    entries: Arc<RwLock<VecDeque<LogEntry>>>,
    capacity: usize,
}

impl LogBuffer {
    /// Create a new buffer that retains at most `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(capacity.min(16384)))),
            capacity,
        }
    }

    /// Push a new log entry, evicting the oldest if at capacity.
    pub async fn push(&self, entry: LogEntry) {
        let mut entries = self.entries.write().await;
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Query entries with optional filters. Returns newest-first.
    pub async fn query(
        &self,
        limit: usize,
        level: Option<&str>,
        request_id: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        keyword: Option<&str>,
    ) -> Vec<LogEntry> {
        use chrono::DateTime;

        let from_dt = from.and_then(|s| DateTime::parse_from_rfc3339(s).ok());
        let to_dt = to.and_then(|s| DateTime::parse_from_rfc3339(s).ok());
        let keyword_lower = keyword.map(|k| k.to_lowercase());

        let entries = self.entries.read().await;
        entries
            .iter()
            .rev()
            .filter(|e| {
                if let Some(lvl) = level {
                    if !e.level.eq_ignore_ascii_case(lvl) {
                        return false;
                    }
                }
                if let Some(rid) = request_id {
                    if e.request_id.as_deref() != Some(rid) {
                        return false;
                    }
                }
                // Time range filtering
                if from_dt.is_some() || to_dt.is_some() {
                    if let Ok(entry_dt) = DateTime::parse_from_rfc3339(&e.ts) {
                        if let Some(ref f) = from_dt {
                            if entry_dt < *f {
                                return false;
                            }
                        }
                        if let Some(ref t) = to_dt {
                            if entry_dt > *t {
                                return false;
                            }
                        }
                    } else {
                        // If we can't parse the entry timestamp, skip it when time filters are active
                        return false;
                    }
                }
                // Keyword search: case-insensitive match across message, target, and fields
                if let Some(ref kw) = keyword_lower {
                    let msg_match = e.message.to_lowercase().contains(kw.as_str());
                    let target_match = e.target.to_lowercase().contains(kw.as_str());
                    let fields_match = e.fields.to_string().to_lowercase().contains(kw.as_str());
                    if !msg_match && !target_match && !fields_match {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .cloned()
            .collect()
    }
}

// ======================== Custom tracing Layer for LogBuffer ========================

/// A tracing subscriber layer that captures log events into an in-memory ring buffer.
pub struct MemoryLogLayer {
    buffer: LogBuffer,
    min_level: Level,
}

impl MemoryLogLayer {
    pub fn new(buffer: LogBuffer, level: &str) -> Self {
        let min_level = parse_level(level);
        Self { buffer, min_level }
    }
}

fn parse_level(level: &str) -> Level {
    match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    }
}

/// Visitor that extracts event fields into a JSON map and captures the message.
struct FieldVisitor {
    fields: serde_json::Map<String, serde_json::Value>,
    message: String,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            fields: serde_json::Map::new(),
            message: String::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(format!("{:?}", value)),
            );
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if let Some(n) = serde_json::Number::from_f64(value) {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::Number(n));
        }
    }
}

/// Extension data stored on spans to cache the request_id and context fields.
#[derive(Clone)]
struct RequestIdExtension(String);

/// Span context fields (method, path, etc.) stored on spans for enriching log entries.
#[derive(Clone, Default)]
struct SpanContextExtension {
    fields: serde_json::Map<String, serde_json::Value>,
}

/// Visitor to extract request_id from span attributes.
struct SpanFieldVisitor {
    request_id: Option<String>,
}

impl SpanFieldVisitor {
    fn new() -> Self {
        Self { request_id: None }
    }
}

impl Visit for SpanFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "request_id" {
            // format!("{:?}") adds quotes around strings; strip them
            let raw = format!("{:?}", value);
            self.request_id = Some(raw.trim_matches('"').to_string());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "request_id" {
            self.request_id = Some(value.to_string());
        }
    }
}

impl<S> Layer<S> for MemoryLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::new();
        attrs.record(&mut visitor);
        if let Some(span) = ctx.span(id) {
            if let Some(request_id) = visitor.request_id {
                span.extensions_mut().insert(RequestIdExtension(request_id));
            }
            // Store other span fields (method, path, etc.) for enriching log entries
            let mut field_visitor = FieldVisitor::new();
            attrs.record(&mut field_visitor);
            field_visitor.fields.remove("request_id"); // already stored separately
            if !field_visitor.fields.is_empty() {
                span.extensions_mut().insert(SpanContextExtension {
                    fields: field_visitor.fields,
                });
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Level filter
        if event.metadata().level() > &self.min_level {
            return;
        }

        // Extract fields from the event
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        // Walk up spans to find request_id
        let mut request_id = ctx.event_span(event).and_then(|span| {
            let mut current = Some(span);
            while let Some(s) = current {
                if let Some(ext) = s.extensions().get::<RequestIdExtension>() {
                    return Some(ext.0.clone());
                }
                current = s.parent();
            }
            None
        });

        // Fallback: check event fields for request_id (e.g. WS handler logs)
        if request_id.is_none() {
            if let Some(serde_json::Value::String(rid)) = visitor.fields.get("request_id") {
                request_id = Some(rid.clone());
                // Remove from fields to avoid duplication
                visitor.fields.remove("request_id");
            }
        }

        // Enrich with span context fields (method, path, etc.)
        if let Some(span) = ctx.event_span(event) {
            let mut current = Some(span);
            while let Some(s) = current {
                if let Some(ext) = s.extensions().get::<SpanContextExtension>() {
                    for (k, v) in &ext.fields {
                        visitor.fields.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                current = s.parent();
            }
        }

        let entry = LogEntry {
            ts: Utc::now().to_rfc3339(),
            level: event.metadata().level().to_string(),
            request_id,
            target: event.metadata().target().to_string(),
            message: visitor.message,
            fields: serde_json::Value::Object(visitor.fields),
        };

        let buffer = self.buffer.clone();
        tokio::spawn(async move {
            buffer.push(entry).await;
        });
    }
}

// ======================== Tracing Initialization ========================

/// Expand `~` prefix in paths to the user's home directory.
pub fn expand_log_path(path: &str) -> String {
    expand_tilde(path)
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// Initialize the tracing subscriber with console output, optional file logging,
/// and an in-memory log buffer.
///
/// Returns the `LogBuffer` so it can be shared with API handlers.
pub fn init_tracing(config: &LogConfig) -> LogBuffer {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    // Console layer: respect RUST_LOG env var, fall back to config level
    let console_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_filter(console_filter);

    // Memory buffer layer
    let log_buffer = LogBuffer::new(config.memory.capacity);
    let memory_layer = MemoryLogLayer::new(log_buffer.clone(), &config.memory.level);

    // File layer (optional)
    if config.file.enabled {
        let log_dir = expand_tilde(&config.file.path);
        let log_path = std::path::Path::new(&log_dir);

        // Ensure log directory exists
        if let Err(e) = std::fs::create_dir_all(log_path) {
            eprintln!("warning: failed to create log directory {}: {}", log_dir, e);
            tracing_subscriber::registry()
                .with(console_layer)
                .with(memory_layer)
                .init();
            return log_buffer;
        }

        let file_appender = match config.file.rotation.as_str() {
            "hourly" => tracing_appender::rolling::hourly(log_path, "synapse.log"),
            "never" => tracing_appender::rolling::never(log_path, "synapse.log"),
            _ => tracing_appender::rolling::daily(log_path, "synapse.log"),
        };

        let file_filter = EnvFilter::new(&config.file.level);

        let file_layer = if config.file.format == "json" {
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(file_appender)
                .with_target(true)
                .with_filter(file_filter)
                .boxed()
        } else {
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_target(true)
                .with_ansi(false)
                .with_filter(file_filter)
                .boxed()
        };

        tracing_subscriber::registry()
            .with(console_layer)
            .with(memory_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(console_layer)
            .with(memory_layer)
            .init();
    }

    log_buffer
}

// ======================== Log File Cleanup ========================

/// Remove log files older than `max_days` or exceeding `max_files` count.
pub fn cleanup_old_logs(log_dir: &Path, max_days: u32, max_files: u32) {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return;
    };

    let now = std::time::SystemTime::now();
    let max_age = std::time::Duration::from_secs(u64::from(max_days) * 86400);

    let mut log_files: Vec<(PathBuf, std::time::SystemTime)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "log")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?;
            Some((e.path(), modified))
        })
        .collect();

    // Sort oldest first
    log_files.sort_by_key(|(_, t)| *t);

    // Remove files older than max_days
    if max_days > 0 {
        log_files.retain(|(path, modified)| {
            if let Ok(age) = now.duration_since(*modified) {
                if age > max_age {
                    let _ = std::fs::remove_file(path);
                    return false;
                }
            }
            true
        });
    }

    // If still over max_files, remove oldest
    while log_files.len() > max_files as usize {
        if let Some((path, _)) = log_files.first() {
            let _ = std::fs::remove_file(path);
        }
        log_files.remove(0);
    }
}
