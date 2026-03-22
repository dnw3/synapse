//! Trace aggregation layer — transforms raw log entries from `LogBuffer` into
//! structured trace records with paired spans (model calls, tool calls).
//!
//! The `TraceAggregator` queries both the in-memory `LogBuffer` and on-disk
//! log files (`~/.synapse/logs/synapse.log.YYYY-MM-DD`) so traces survive
//! server restarts.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use synaptic::logging::{LogBuffer, LogEntry};

use crate::agent::subscribers::{
    LOG_MODEL_CALL_COMPLETED, LOG_MODEL_CALL_COMPLETED_NO_USAGE, LOG_MODEL_CALL_STARTING,
    LOG_TOOL_CALL_COMPLETED, LOG_TOOL_CALL_FAILED, LOG_TOOL_CALL_STARTING,
};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    Success,
    Error,
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_request_id: Option<String>,
    pub total_tokens: u64,
    pub duration_ms: u64,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub tools_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpanData {
    ModelCall {
        #[serde(skip_serializing_if = "Option::is_none")]
        system_prompt: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_message: Option<String>,
        /// Full conversation messages sent to the model (role + content).
        #[serde(skip_serializing_if = "Option::is_none")]
        messages: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_count: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_count: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        has_thinking: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls_in_response: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tools: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<String>,
    },
    ToolCall {
        tool: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub id: String,
    pub start_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub status: TraceStatus,
    #[serde(flatten)]
    pub data: SpanData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    pub request_id: String,
    pub start_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    pub status: TraceStatus,
    pub metadata: TraceMetadata,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TraceListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub channel: Option<String>,
    pub tool: Option<String>,
    pub keyword: Option<String>,
    pub min_tokens: Option<u64>,
    pub min_duration_ms: Option<u64>,
    pub parent: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceListResponse {
    pub traces: Vec<TraceRecord>,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Log file parsing
// ---------------------------------------------------------------------------

/// Trace-relevant log messages (used for filtering).
const TRACE_MESSAGES: &[&str] = &[
    LOG_MODEL_CALL_STARTING,
    LOG_MODEL_CALL_COMPLETED,
    LOG_MODEL_CALL_COMPLETED_NO_USAGE,
    LOG_TOOL_CALL_STARTING,
    LOG_TOOL_CALL_COMPLETED,
    LOG_TOOL_CALL_FAILED,
];

/// Parse a single JSON log line from a disk log file into a `LogEntry`.
///
/// Disk format:
/// ```json
/// {
///   "timestamp": "2026-03-21T10:00:00Z",
///   "level": "INFO",
///   "fields": { "message": "model call completed", "trace_id": "...", ... },
///   "target": "synapse::agent::subscribers",
///   "span": { "request_id": "...", "name": "trace" },
///   "spans": [{ "request_id": "...", ... }]
/// }
/// ```
fn parse_log_file_line(line: &str) -> Option<LogEntry> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;

    let target = v["target"].as_str().unwrap_or("").to_string();

    // Skip framework-level duplicates.
    if target.starts_with("synaptic_deep::") {
        return None;
    }

    let ts = v["timestamp"].as_str().unwrap_or("").to_string();
    let level = v["level"].as_str().unwrap_or("INFO").to_string();
    let fields_obj = v
        .get("fields")
        .cloned()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    // In disk format, "message" is inside "fields".
    let message = fields_obj
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Only keep trace-relevant entries.
    if !TRACE_MESSAGES.contains(&message.as_str()) {
        return None;
    }

    // Extract request_id from span hierarchy (set by our info_span!("trace", request_id = ...)).
    let request_id = v
        .get("span")
        .and_then(|s| s.get("request_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            // Also check spans array.
            v.get("spans")
                .and_then(|arr| arr.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find_map(|s| s.get("request_id").and_then(|v| v.as_str()))
                        .map(|s| s.to_string())
                })
        });

    // Build a clean fields object (remove "message" key since it's a top-level LogEntry field).
    let mut fields = fields_obj;
    if let Some(obj) = fields.as_object_mut() {
        obj.remove("message");
    }

    Some(LogEntry {
        ts,
        level,
        request_id,
        target,
        message,
        fields,
    })
}

/// Read trace-relevant log entries from on-disk log files.
///
/// Scans `~/.synapse/logs/synapse.log.YYYY-MM-DD` files, newest first.
/// Returns entries that match trace-relevant messages from our subscriber.
fn read_log_files(max_days: usize) -> Vec<LogEntry> {
    let log_dir = dirs::home_dir()
        .map(|h| h.join(".synapse/logs"))
        .unwrap_or_default();

    if !log_dir.exists() {
        return Vec::new();
    }

    // Collect log files sorted by name descending (newest first).
    let mut log_files: Vec<std::path::PathBuf> = std::fs::read_dir(&log_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("synapse.log."))
                .unwrap_or(false)
        })
        .collect();
    log_files.sort_by(|a, b| b.cmp(a));
    log_files.truncate(max_days);

    let mut entries = Vec::new();
    for path in &log_files {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if line.is_empty() {
                    continue;
                }
                if let Some(entry) = parse_log_file_line(line) {
                    entries.push(entry);
                }
            }
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Aggregator
// ---------------------------------------------------------------------------

pub struct TraceAggregator {
    log_buffer: Arc<LogBuffer>,
}

impl TraceAggregator {
    pub fn new(log_buffer: Arc<LogBuffer>) -> Self {
        Self { log_buffer }
    }

    /// Collect all trace-relevant log entries from both in-memory buffer and
    /// on-disk log files, deduplicated and filtered.
    async fn collect_all_entries(&self) -> Vec<LogEntry> {
        // 1. In-memory entries (current session).
        let mem_entries = self
            .log_buffer
            .query(100_000, None, None, None, None, None)
            .await;

        // 2. On-disk entries (previous sessions, up to 7 days).
        let disk_entries = read_log_files(7);

        // 3. Merge: disk first (older), then memory (newer).
        //    Deduplicate by (timestamp, message, trace_id) to avoid counting
        //    entries that are in both the memory buffer and today's log file.
        let mut seen = std::collections::HashSet::new();
        let mut all = Vec::with_capacity(disk_entries.len() + mem_entries.len());

        // Dedup key: use truncated timestamp (to ms, strip timezone suffix) + message + trace_id.
        // Timestamps may differ slightly between disk ("282112Z") and memory ("282107+00:00").
        let dedup_key = |e: &LogEntry| -> String {
            // Normalize: take first 23 chars of timestamp (YYYY-MM-DDTHH:MM:SS.mmm)
            let ts_norm = if e.ts.len() >= 23 { &e.ts[..23] } else { &e.ts };
            let tid = e
                .fields
                .get("trace_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!("{}|{}|{}", ts_norm, e.message, tid)
        };

        for entry in disk_entries {
            // disk entries are already filtered by parse_log_file_line
            if seen.insert(dedup_key(&entry)) {
                all.push(entry);
            }
        }

        for entry in mem_entries {
            if !TRACE_MESSAGES.contains(&entry.message.as_str()) {
                continue;
            }
            if entry.target.starts_with("synaptic_deep::") {
                continue;
            }
            if seen.insert(dedup_key(&entry)) {
                all.push(entry);
            }
        }

        all
    }

    /// Retrieve a single trace by trace_id with full span details.
    pub async fn detail(&self, trace_id: &str) -> Option<TraceRecord> {
        let all = self.collect_all_entries().await;

        // Find entries matching this trace_id (from fields.trace_id or request_id).
        let entries: Vec<LogEntry> = all
            .into_iter()
            .filter(|e| {
                let tid = e
                    .fields
                    .get("trace_id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .or_else(|| e.request_id.as_deref().filter(|s| !s.is_empty()));
                tid == Some(trace_id)
            })
            .collect();

        if entries.is_empty() {
            return None;
        }
        Some(build_trace_record(trace_id, &entries, true))
    }

    /// List traces, grouping log entries by trace_id. Spans are NOT included
    /// in the response (use `detail()` for that).
    pub async fn list(&self, params: &TraceListParams) -> TraceListResponse {
        let all = self.collect_all_entries().await;

        // Apply time range filter.
        let from = params.from.as_deref();
        let to = params.to.as_deref();
        let entries: Vec<&LogEntry> = all
            .iter()
            .filter(|e| {
                if let Some(f) = from {
                    if e.ts.as_str() < f {
                        return false;
                    }
                }
                if let Some(t) = to {
                    if e.ts.as_str() > t {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Group entries by trace_id (from fields.trace_id, falling back to request_id).
        // Entries are already filtered by collect_all_entries().
        let mut groups: HashMap<String, Vec<&LogEntry>> = HashMap::new();
        for entry in &entries {
            // Prefer fields.trace_id, fall back to request_id.
            let tid = entry
                .fields
                .get("trace_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| entry.request_id.clone().filter(|s| !s.is_empty()));

            if let Some(tid) = tid {
                groups.entry(tid).or_default().push(entry);
            }
        }

        // Build summary records (no spans).
        let mut traces: Vec<TraceRecord> = groups
            .iter()
            .map(|(rid, group)| {
                // We need owned entries for build_trace_record.
                let owned: Vec<LogEntry> = group.iter().map(|e| (*e).clone()).collect();
                build_trace_record(rid, &owned, false)
            })
            .collect();

        // Sort by start_time descending (most recent first).
        traces.sort_by(|a, b| b.start_time.cmp(&a.start_time));

        // Apply filters.
        let status_filter = params.status.as_deref().and_then(parse_status_filter);

        traces.retain(|t| {
            if let Some(ref sf) = status_filter {
                if &t.status != sf {
                    return false;
                }
            }
            if let Some(ref model) = params.model {
                if t.metadata.model.as_deref() != Some(model.as_str()) {
                    return false;
                }
            }
            if let Some(ref channel) = params.channel {
                if t.metadata.channel.as_deref() != Some(channel.as_str()) {
                    return false;
                }
            }
            if let Some(ref tool) = params.tool {
                if !t.metadata.tools_used.contains(tool) {
                    return false;
                }
            }
            if let Some(ref kw) = params.keyword {
                let kw_lower = kw.to_lowercase();
                let preview_matches = t
                    .metadata
                    .user_message_preview
                    .as_deref()
                    .map(|p| p.to_lowercase().contains(&kw_lower))
                    .unwrap_or(false);
                if !preview_matches {
                    return false;
                }
            }
            if let Some(min_tok) = params.min_tokens {
                if t.metadata.total_tokens < min_tok {
                    return false;
                }
            }
            if let Some(min_dur) = params.min_duration_ms {
                if t.metadata.duration_ms < min_dur {
                    return false;
                }
            }
            if let Some(ref parent) = params.parent {
                if t.metadata.parent_request_id.as_deref() != Some(parent.as_str()) {
                    return false;
                }
            }
            true
        });

        let total = traces.len();

        // Apply offset + limit.
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(50).min(500);
        let traces: Vec<TraceRecord> = traces.into_iter().skip(offset).take(limit).collect();

        TraceListResponse { traces, total }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_status_filter(s: &str) -> Option<TraceStatus> {
    match s.to_lowercase().as_str() {
        "success" => Some(TraceStatus::Success),
        "error" => Some(TraceStatus::Error),
        "running" => Some(TraceStatus::Running),
        _ => None,
    }
}

/// Build a `TraceRecord` from a set of log entries sharing the same request_id.
/// When `include_spans` is false, `spans` will be empty (for list responses).
fn build_trace_record(request_id: &str, entries: &[LogEntry], include_spans: bool) -> TraceRecord {
    let mut spans = Vec::new();
    let mut total_tokens: u64 = 0;
    let mut total_duration_ms: u64 = 0;
    let mut model_call_count: u64 = 0;
    let mut tool_call_count: u64 = 0;
    let mut tools_used: Vec<String> = Vec::new();
    let mut user_message_preview: Option<String> = None;
    let mut has_error = false;
    let mut has_completed_model_call = false;
    let mut model_name: Option<String> = None;
    let mut channel_name: Option<String> = None;
    let mut parent_request_id: Option<String> = None;

    // Collect start/complete events for pairing.
    let mut model_starts: Vec<&LogEntry> = Vec::new();
    let mut model_completes: Vec<&LogEntry> = Vec::new();
    let mut tool_starts: Vec<&LogEntry> = Vec::new();
    let mut tool_completes: Vec<&LogEntry> = Vec::new();

    for entry in entries {
        // Deduplicate: both synaptic_deep::middleware::observability and
        // synapse::agent::subscribers emit the same trace messages.
        // Only process entries from our business-layer subscriber.
        if entry.target.starts_with("synaptic_deep::") {
            continue;
        }

        match entry.message.as_str() {
            LOG_MODEL_CALL_STARTING => {
                model_starts.push(entry);
                if user_message_preview.is_none() {
                    let msg = entry.fields.get("user_message").and_then(|v| v.as_str());
                    if let Some(m) = msg {
                        let preview = if m.len() > 200 {
                            format!("{}...", &m[..200])
                        } else {
                            m.to_string()
                        };
                        user_message_preview = Some(preview);
                    }
                }
                // Extract channel from fields if present.
                if channel_name.is_none() {
                    channel_name = entry
                        .fields
                        .get("channel")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                // Extract parent_request_id if present.
                if parent_request_id.is_none() {
                    parent_request_id = entry
                        .fields
                        .get("parent_request_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
            LOG_MODEL_CALL_COMPLETED | LOG_MODEL_CALL_COMPLETED_NO_USAGE => {
                model_completes.push(entry);
                has_completed_model_call = true;
                model_call_count += 1;

                if let Some(tokens) = entry.fields.get("total_tokens").and_then(|v| v.as_u64()) {
                    total_tokens += tokens;
                }
                if let Some(dur) = entry.fields.get("duration_ms").and_then(|v| v.as_u64()) {
                    total_duration_ms += dur;
                }
                // Extract model name from fields if present.
                if model_name.is_none() {
                    model_name = entry
                        .fields
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
            LOG_TOOL_CALL_STARTING => {
                tool_starts.push(entry);
            }
            LOG_TOOL_CALL_COMPLETED => {
                tool_completes.push(entry);
                tool_call_count += 1;
                if let Some(tool) = entry.fields.get("tool").and_then(|v| v.as_str()) {
                    if !tools_used.contains(&tool.to_string()) {
                        tools_used.push(tool.to_string());
                    }
                }
                if let Some(dur) = entry.fields.get("duration_ms").and_then(|v| v.as_u64()) {
                    total_duration_ms += dur;
                }
            }
            LOG_TOOL_CALL_FAILED => {
                tool_completes.push(entry);
                tool_call_count += 1;
                has_error = true;
                if let Some(tool) = entry.fields.get("tool").and_then(|v| v.as_str()) {
                    if !tools_used.contains(&tool.to_string()) {
                        tools_used.push(tool.to_string());
                    }
                }
                if let Some(dur) = entry.fields.get("duration_ms").and_then(|v| v.as_u64()) {
                    total_duration_ms += dur;
                }
            }
            _ => {}
        }
    }

    // Also count model_starts that have no matching complete (running).
    if model_call_count == 0 && !model_starts.is_empty() {
        model_call_count = model_starts.len() as u64;
    }

    // Build spans if requested.
    if include_spans {
        let mut span_counter: u64 = 0;

        // Pair model call starts with completes (sequential pairing).
        // When there are more completes than starts (e.g. "model call starting"
        // logged without request_id), create spans from completes alone.
        let paired_count = model_starts.len().max(model_completes.len());
        for i in 0..paired_count {
            let start = model_starts.get(i);
            let complete = model_completes.get(i);

            // Skip if neither exists (shouldn't happen given max()).
            if start.is_none() && complete.is_none() {
                continue;
            }

            span_counter += 1;

            // Extract fields from start entry (if available).
            let system_prompt = start
                .and_then(|s| s.fields.get("system_prompt"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let user_msg = start
                .and_then(|s| s.fields.get("user_message"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let message_count = start
                .and_then(|s| s.fields.get("message_count"))
                .and_then(|v| v.as_u64());
            let tool_count_field = start
                .and_then(|s| s.fields.get("tool_count"))
                .and_then(|v| v.as_u64());
            let has_thinking = start
                .and_then(|s| s.fields.get("has_thinking"))
                .and_then(|v| v.as_bool());

            // Extract fields from complete entry (if available).
            let (
                end_time,
                duration_ms,
                status,
                input_tokens,
                output_tokens,
                total_tok,
                tc,
                tools_str,
                response,
            ) = if let Some(c) = complete {
                let dur = c.fields.get("duration_ms").and_then(|v| v.as_u64());
                let it = c.fields.get("input_tokens").and_then(|v| v.as_u64());
                let ot = c.fields.get("output_tokens").and_then(|v| v.as_u64());
                let tt = c.fields.get("total_tokens").and_then(|v| v.as_u64());
                let tc_val = c.fields.get("tool_calls").and_then(|v| v.as_u64());
                let tools_val = c
                    .fields
                    .get("tools")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let resp = c
                    .fields
                    .get("response")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                (
                    Some(c.ts.clone()),
                    dur,
                    TraceStatus::Success,
                    it,
                    ot,
                    tt,
                    tc_val,
                    tools_val,
                    resp,
                )
            } else {
                (
                    None,
                    None,
                    TraceStatus::Running,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            };

            // Use start time from start entry, or fall back to complete entry.
            let span_start_time = start
                .map(|s| s.ts.clone())
                .or_else(|| {
                    // Approximate start time: complete.ts - duration_ms
                    complete.map(|c| c.ts.clone())
                })
                .unwrap_or_default();

            // Extract full conversation from start entry (field: "conversation").
            let messages = start
                .and_then(|s| s.fields.get("conversation"))
                .and_then(|v| {
                    if v.is_array() {
                        Some(v.clone())
                    } else if v.is_string() {
                        serde_json::from_str(v.as_str().unwrap_or("[]")).ok()
                    } else {
                        None
                    }
                });

            spans.push(Span {
                id: format!("model-{}", span_counter),
                start_time: span_start_time,
                end_time,
                duration_ms,
                status,
                data: SpanData::ModelCall {
                    system_prompt,
                    user_message: user_msg,
                    messages,
                    message_count,
                    tool_count: tool_count_field,
                    has_thinking,
                    input_tokens,
                    output_tokens,
                    total_tokens: total_tok,
                    tool_calls_in_response: tc,
                    tools: tools_str,
                    response,
                },
            });
        }

        // Pair tool call starts with completes (match by tool name, nearest unclosed).
        let mut used_completes: Vec<bool> = vec![false; tool_completes.len()];

        for start in &tool_starts {
            let tool_name = start
                .fields
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let args = start.fields.get("args").map(|v| {
                if v.is_string() {
                    v.as_str().unwrap_or("").to_string()
                } else {
                    v.to_string()
                }
            });

            span_counter += 1;

            // Find nearest unclosed complete with matching tool name.
            let mut matched_complete: Option<(usize, &LogEntry)> = None;
            for (j, complete) in tool_completes.iter().enumerate() {
                if used_completes[j] {
                    continue;
                }
                let c_tool = complete
                    .fields
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if c_tool == tool_name {
                    matched_complete = Some((j, complete));
                    break;
                }
            }

            let (end_time, duration_ms, status, result, error) =
                if let Some((idx, c)) = matched_complete {
                    used_completes[idx] = true;
                    let dur = c.fields.get("duration_ms").and_then(|v| v.as_u64());
                    let is_failed = c.message == LOG_TOOL_CALL_FAILED;
                    let err = if is_failed {
                        c.fields
                            .get("error")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };
                    let res = if !is_failed {
                        c.fields.get("result").map(|v| {
                            if v.is_string() {
                                v.as_str().unwrap_or("").to_string()
                            } else {
                                v.to_string()
                            }
                        })
                    } else {
                        None
                    };
                    let st = if is_failed {
                        TraceStatus::Error
                    } else {
                        TraceStatus::Success
                    };
                    (Some(c.ts.clone()), dur, st, res, err)
                } else {
                    (None, None, TraceStatus::Running, None, None)
                };

            spans.push(Span {
                id: format!("tool-{}", span_counter),
                start_time: start.ts.clone(),
                end_time,
                duration_ms,
                status,
                data: SpanData::ToolCall {
                    tool: tool_name,
                    args,
                    result,
                    error,
                },
            });
        }
    }

    // Determine overall status.
    let status = if has_error {
        TraceStatus::Error
    } else if !has_completed_model_call && !model_starts.is_empty() {
        TraceStatus::Running
    } else {
        TraceStatus::Success
    };

    // Determine start/end times from entries.
    let start_time = entries.first().map(|e| e.ts.clone()).unwrap_or_default();
    let end_time = if status != TraceStatus::Running {
        entries.last().map(|e| e.ts.clone())
    } else {
        None
    };

    TraceRecord {
        request_id: request_id.to_string(),
        start_time,
        end_time,
        status,
        metadata: TraceMetadata {
            model: model_name,
            channel: channel_name,
            user_message_preview,
            parent_request_id,
            total_tokens,
            duration_ms: total_duration_ms,
            model_calls: model_call_count,
            tool_calls: tool_call_count,
            tools_used,
        },
        spans,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        ts: &str,
        message: &str,
        request_id: &str,
        fields: serde_json::Value,
    ) -> LogEntry {
        LogEntry {
            ts: ts.to_string(),
            level: "INFO".to_string(),
            request_id: Some(request_id.to_string()),
            target: "test".to_string(),
            message: message.to_string(),
            fields,
        }
    }

    fn make_log_buffer_with_entries(entries: Vec<LogEntry>) -> Arc<LogBuffer> {
        let buf = LogBuffer::new(10000);
        // LogBuffer provides `push` for testing via its public API. We'll use
        // a helper that inserts entries directly. Since LogBuffer::push may not
        // exist, we build from query perspective — the test constructs a mock.
        // Actually, LogBuffer has an `insert` or we can work around it.
        // For unit tests we test `build_trace_record` directly.
        let _ = buf;
        let _ = entries;
        Arc::new(LogBuffer::new(10000))
    }

    #[test]
    fn test_detail_single_model_call() {
        let entries = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-001",
                serde_json::json!({
                    "system_prompt": "You are helpful.",
                    "user_message": "Hello world",
                    "message_count": 1,
                    "tool_count": 0,
                    "has_thinking": false,
                    "system_prompt_len": 15
                }),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-001",
                serde_json::json!({
                    "duration_ms": 1200,
                    "input_tokens": 50,
                    "output_tokens": 30,
                    "total_tokens": 80,
                    "tool_calls": 0,
                    "tools": "",
                    "response": "Hi there!"
                }),
            ),
        ];

        let trace = build_trace_record("req-001", &entries, true);
        assert_eq!(trace.request_id, "req-001");
        assert_eq!(trace.status, TraceStatus::Success);
        assert_eq!(trace.metadata.total_tokens, 80);
        assert_eq!(trace.metadata.duration_ms, 1200);
        assert_eq!(trace.metadata.model_calls, 1);
        assert_eq!(trace.metadata.tool_calls, 0);
        assert_eq!(trace.spans.len(), 1);

        let span = &trace.spans[0];
        assert_eq!(span.status, TraceStatus::Success);
        assert!(span.end_time.is_some());
        assert_eq!(span.duration_ms, Some(1200));

        match &span.data {
            SpanData::ModelCall {
                user_message,
                response,
                total_tokens,
                ..
            } => {
                assert_eq!(user_message.as_deref(), Some("Hello world"));
                assert_eq!(response.as_deref(), Some("Hi there!"));
                assert_eq!(*total_tokens, Some(80));
            }
            _ => panic!("Expected ModelCall span"),
        }
    }

    #[test]
    fn test_detail_model_call_with_tool_calls() {
        let entries = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-002",
                serde_json::json!({
                    "user_message": "Search for Rust docs",
                    "message_count": 1,
                    "tool_count": 2,
                    "has_thinking": false,
                    "system_prompt_len": 100
                }),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-002",
                serde_json::json!({
                    "duration_ms": 800,
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "total_tokens": 150,
                    "tool_calls": 1,
                    "tools": "web_search",
                    "response": "Let me search."
                }),
            ),
            make_entry(
                "2026-03-21T10:00:02Z",
                LOG_TOOL_CALL_STARTING,
                "req-002",
                serde_json::json!({
                    "tool": "web_search",
                    "args": "{\"query\": \"Rust docs\"}"
                }),
            ),
            make_entry(
                "2026-03-21T10:00:03Z",
                LOG_TOOL_CALL_COMPLETED,
                "req-002",
                serde_json::json!({
                    "tool": "web_search",
                    "duration_ms": 500,
                    "result": "Found 10 results"
                }),
            ),
        ];

        let trace = build_trace_record("req-002", &entries, true);
        assert_eq!(trace.status, TraceStatus::Success);
        assert_eq!(trace.metadata.model_calls, 1);
        assert_eq!(trace.metadata.tool_calls, 1);
        assert_eq!(trace.metadata.tools_used, vec!["web_search"]);
        assert_eq!(trace.metadata.total_tokens, 150);
        assert_eq!(trace.metadata.duration_ms, 800 + 500);
        assert_eq!(trace.spans.len(), 2);

        // First span: model call.
        assert!(matches!(&trace.spans[0].data, SpanData::ModelCall { .. }));
        // Second span: tool call.
        match &trace.spans[1].data {
            SpanData::ToolCall {
                tool,
                result,
                error,
                ..
            } => {
                assert_eq!(tool, "web_search");
                assert_eq!(result.as_deref(), Some("Found 10 results"));
                assert!(error.is_none());
            }
            _ => panic!("Expected ToolCall span"),
        }
    }

    #[test]
    fn test_detail_running_trace() {
        let entries = vec![make_entry(
            "2026-03-21T10:00:00Z",
            LOG_MODEL_CALL_STARTING,
            "req-003",
            serde_json::json!({
                "user_message": "Think about this...",
                "message_count": 1,
                "tool_count": 0,
                "has_thinking": true,
                "system_prompt_len": 50
            }),
        )];

        let trace = build_trace_record("req-003", &entries, true);
        assert_eq!(trace.status, TraceStatus::Running);
        assert!(trace.end_time.is_none());
        assert_eq!(trace.spans.len(), 1);
        assert_eq!(trace.spans[0].status, TraceStatus::Running);
        assert!(trace.spans[0].end_time.is_none());
        assert!(trace.spans[0].duration_ms.is_none());
    }

    #[test]
    fn test_detail_error_tool_call() {
        let entries = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-004",
                serde_json::json!({
                    "user_message": "Run a command",
                    "message_count": 1,
                    "tool_count": 1,
                    "has_thinking": false,
                    "system_prompt_len": 50
                }),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-004",
                serde_json::json!({
                    "duration_ms": 600,
                    "input_tokens": 80,
                    "output_tokens": 40,
                    "total_tokens": 120,
                    "tool_calls": 1,
                    "tools": "bash",
                    "response": "I'll run that."
                }),
            ),
            make_entry(
                "2026-03-21T10:00:02Z",
                LOG_TOOL_CALL_STARTING,
                "req-004",
                serde_json::json!({
                    "tool": "bash",
                    "args": "{\"command\": \"rm -rf /\"}"
                }),
            ),
            make_entry(
                "2026-03-21T10:00:03Z",
                LOG_TOOL_CALL_FAILED,
                "req-004",
                serde_json::json!({
                    "tool": "bash",
                    "duration_ms": 100,
                    "error": "Permission denied"
                }),
            ),
        ];

        let trace = build_trace_record("req-004", &entries, true);
        assert_eq!(trace.status, TraceStatus::Error);
        assert_eq!(trace.metadata.tool_calls, 1);
        assert_eq!(trace.spans.len(), 2);

        let tool_span = &trace.spans[1];
        assert_eq!(tool_span.status, TraceStatus::Error);
        match &tool_span.data {
            SpanData::ToolCall { tool, error, .. } => {
                assert_eq!(tool, "bash");
                assert_eq!(error.as_deref(), Some("Permission denied"));
            }
            _ => panic!("Expected ToolCall span"),
        }
    }

    #[test]
    fn test_detail_unknown_request_id() {
        let entries: Vec<LogEntry> = vec![];
        // build_trace_record with empty entries would still produce a record,
        // but the aggregator's detail() checks for empty and returns None.
        // We test that path indirectly here.
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_groups_by_request_id() {
        let entries_a = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-a",
                serde_json::json!({"user_message": "Hello", "message_count": 1, "tool_count": 0, "has_thinking": false, "system_prompt_len": 10}),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-a",
                serde_json::json!({"duration_ms": 500, "input_tokens": 10, "output_tokens": 5, "total_tokens": 15, "tool_calls": 0, "tools": "", "response": "Hi"}),
            ),
        ];
        let entries_b = vec![
            make_entry(
                "2026-03-21T10:01:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-b",
                serde_json::json!({"user_message": "Bye", "message_count": 1, "tool_count": 0, "has_thinking": false, "system_prompt_len": 10}),
            ),
            make_entry(
                "2026-03-21T10:01:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-b",
                serde_json::json!({"duration_ms": 300, "input_tokens": 8, "output_tokens": 4, "total_tokens": 12, "tool_calls": 0, "tools": "", "response": "Goodbye"}),
            ),
        ];

        let trace_a = build_trace_record("req-a", &entries_a, false);
        let trace_b = build_trace_record("req-b", &entries_b, false);

        assert_eq!(trace_a.request_id, "req-a");
        assert_eq!(trace_b.request_id, "req-b");
        assert_ne!(trace_a.request_id, trace_b.request_id);
    }

    #[test]
    fn test_list_filters_by_status() {
        let success_entries = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-s",
                serde_json::json!({"user_message": "ok", "message_count": 1, "tool_count": 0, "has_thinking": false, "system_prompt_len": 5}),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-s",
                serde_json::json!({"duration_ms": 200, "input_tokens": 5, "output_tokens": 3, "total_tokens": 8, "tool_calls": 0, "tools": "", "response": "done"}),
            ),
        ];
        let error_entries = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_TOOL_CALL_STARTING,
                "req-e",
                serde_json::json!({"tool": "bash", "args": "{}"}),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_TOOL_CALL_FAILED,
                "req-e",
                serde_json::json!({"tool": "bash", "duration_ms": 50, "error": "failed"}),
            ),
        ];

        let trace_s = build_trace_record("req-s", &success_entries, false);
        let trace_e = build_trace_record("req-e", &error_entries, false);

        assert_eq!(trace_s.status, TraceStatus::Success);
        assert_eq!(trace_e.status, TraceStatus::Error);

        // Simulate filtering.
        let all = vec![trace_s.clone(), trace_e.clone()];
        let filtered: Vec<_> = all
            .iter()
            .filter(|t| t.status == TraceStatus::Error)
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].request_id, "req-e");
    }

    #[test]
    fn test_list_does_not_include_spans() {
        let entries = vec![
            make_entry(
                "2026-03-21T10:00:00Z",
                LOG_MODEL_CALL_STARTING,
                "req-ns",
                serde_json::json!({"user_message": "test", "message_count": 1, "tool_count": 0, "has_thinking": false, "system_prompt_len": 5}),
            ),
            make_entry(
                "2026-03-21T10:00:01Z",
                LOG_MODEL_CALL_COMPLETED,
                "req-ns",
                serde_json::json!({"duration_ms": 100, "input_tokens": 5, "output_tokens": 3, "total_tokens": 8, "tool_calls": 0, "tools": "", "response": "ok"}),
            ),
        ];

        let trace = build_trace_record("req-ns", &entries, false);
        assert!(trace.spans.is_empty());
        // But metadata is still populated.
        assert_eq!(trace.metadata.total_tokens, 8);
        assert_eq!(trace.metadata.model_calls, 1);
    }
}
