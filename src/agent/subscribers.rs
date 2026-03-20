//! EventSubscriber implementations for business-layer lifecycle hooks.
//!
//! These subscribers are NOT yet wired into the agent builder — they exist alongside
//! the old middleware so we can switch over gradually.  Each subscriber is marked
//! `#[allow(dead_code)]` until the integration step.

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::events::{Event, EventAction, EventFilter, EventKind, EventSubscriber};

// ---------------------------------------------------------------------------
// 1. TracingSubscriber — replaces AgentTracingMiddleware
// ---------------------------------------------------------------------------

/// Observes model calls and tool calls, emitting structured tracing log lines
/// with latency measurements.
///
/// Maps to:
///   `BeforeModelCall`  → record start time (Intercept, so we can time the full call)
///   `LlmOutput`        → log completion + latency (Parallel)
///   `AfterToolCall`    → log tool result + latency (Parallel)
///
/// Because `BeforeModelCall` is Intercept mode (not `wrap_model_call`), the
/// subscriber stores a per-request start time in a `DashMap` keyed by
/// `request_id` from the event metadata, and always returns
/// `EventAction::Continue` so processing is not blocked.
#[allow(dead_code)]
pub struct TracingSubscriber {
    /// request_id → call start Instant
    timers: dashmap::DashMap<String, std::time::Instant>,
}

impl TracingSubscriber {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            timers: dashmap::DashMap::new(),
        }
    }

    /// Derive a timer key from the event metadata.  Falls back to a constant
    /// so we don't lose the timing even if no request_id is set.
    fn timer_key(event: &Event) -> String {
        event
            .metadata
            .request_id
            .clone()
            .unwrap_or_else(|| "default".to_string())
    }
}

#[async_trait]
impl EventSubscriber for TracingSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::AnyOf(vec![
            EventKind::BeforeModelCall,
            EventKind::LlmOutput,
            EventKind::BeforeToolCall,
            EventKind::AfterToolCall,
        ])]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        match event.kind {
            EventKind::BeforeModelCall => {
                // Extract request metadata from the payload for logging.
                let message_count = event.payload["message_count"].as_u64().unwrap_or(0);
                let tool_count = event.payload["tool_count"].as_u64().unwrap_or(0);
                let has_thinking = event.payload["has_thinking"].as_bool().unwrap_or(false);
                let system_prompt_len = event.payload["system_prompt_len"].as_u64().unwrap_or(0);
                let system_prompt = event.payload["system_prompt"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let user_message = event.payload["user_message"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                tracing::info!(
                    message_count,
                    tool_count,
                    has_thinking,
                    system_prompt_len,
                    system_prompt = %system_prompt,
                    user_message = %user_message,
                    "model call starting"
                );

                // Record timer so LlmOutput can compute duration.
                self.timers
                    .insert(Self::timer_key(event), std::time::Instant::now());
            }

            EventKind::LlmOutput => {
                let key = Self::timer_key(event);
                let duration_ms = self
                    .timers
                    .remove(&key)
                    .map(|(_, start)| start.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                let tool_calls_count = event.payload["tool_calls_count"].as_u64().unwrap_or(0);
                let tools_summary = event.payload["tools_summary"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let content = event.payload["content"].as_str().unwrap_or("").to_string();

                if event.payload["input_tokens"].is_number() {
                    let input_tokens = event.payload["input_tokens"].as_u64().unwrap_or(0);
                    let output_tokens = event.payload["output_tokens"].as_u64().unwrap_or(0);
                    let total_tokens = event.payload["total_tokens"].as_u64().unwrap_or(0);
                    tracing::info!(
                        duration_ms,
                        input_tokens,
                        output_tokens,
                        total_tokens,
                        tool_calls = tool_calls_count,
                        tools = %tools_summary,
                        response = %content,
                        "model call completed"
                    );
                } else {
                    tracing::info!(
                        duration_ms,
                        tool_calls = tool_calls_count,
                        tools = %tools_summary,
                        response = %content,
                        "model call completed (no usage)"
                    );
                }
            }

            EventKind::BeforeToolCall => {
                let tool_name = event.payload["tool"].as_str().unwrap_or("?").to_string();
                let args = event.payload["args"].to_string();
                tracing::info!(tool = %tool_name, args = %args, "tool call starting");
                // Record timer keyed by "tool:<request_id>:<tool_name>"
                let key = format!("tool:{}:{}", Self::timer_key(event), tool_name);
                self.timers.insert(key, std::time::Instant::now());
            }

            EventKind::AfterToolCall => {
                let tool_name = event.payload["tool"].as_str().unwrap_or("?").to_string();
                let key = format!("tool:{}:{}", Self::timer_key(event), tool_name);
                let duration_ms = self
                    .timers
                    .remove(&key)
                    .map(|(_, start)| start.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                if event.payload["error"].is_string() {
                    let error = event.payload["error"].as_str().unwrap_or("").to_string();
                    tracing::error!(
                        tool = %tool_name,
                        duration_ms,
                        error = %error,
                        "tool call failed"
                    );
                } else {
                    let result_str = event.payload["result"].to_string();
                    tracing::info!(
                        tool = %tool_name,
                        duration_ms,
                        result = %result_str,
                        "tool call completed"
                    );
                }
            }

            _ => {}
        }
        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "TracingSubscriber"
    }
}

// ---------------------------------------------------------------------------
// 2. ThinkingSubscriber — replaces ThinkingMiddleware
// ---------------------------------------------------------------------------

/// Injects `thinking` configuration into the model request payload before
/// the call is dispatched.
///
/// Maps to: `BeforePromptBuild` (Sequential — can mutate the payload).
#[allow(dead_code)]
pub struct ThinkingSubscriber {
    /// Static thinking config to inject.  `None` means adaptive mode.
    config: Option<synaptic::core::ThinkingConfig>,
    adaptive: bool,
}

impl ThinkingSubscriber {
    /// Create with a fixed thinking config.
    #[allow(dead_code)]
    pub fn new(config: Option<synaptic::core::ThinkingConfig>) -> Self {
        Self {
            config,
            adaptive: false,
        }
    }

    /// Create in adaptive mode — adjusts thinking level based on payload complexity hints.
    #[allow(dead_code)]
    pub fn adaptive() -> Self {
        Self {
            config: None,
            adaptive: true,
        }
    }

    /// Estimate conversation complexity from payload fields.
    fn estimate_complexity(payload: &serde_json::Value) -> u32 {
        let mut score: u32 = 0;

        let msg_count = payload["message_count"].as_u64().unwrap_or(0);
        if msg_count > 20 {
            score += 3;
        } else if msg_count > 10 {
            score += 2;
        } else if msg_count > 5 {
            score += 1;
        }

        let total_chars = payload["total_chars"].as_u64().unwrap_or(0);
        if total_chars > 10000 {
            score += 3;
        } else if total_chars > 3000 {
            score += 2;
        } else if total_chars > 1000 {
            score += 1;
        }

        let tool_count = payload["tool_count"].as_u64().unwrap_or(0);
        if tool_count > 10 {
            score += 2;
        } else if tool_count > 3 {
            score += 1;
        }

        if let Some(content) = payload["last_user_message"].as_str() {
            let content = content.to_lowercase();
            let complex_keywords = [
                "analyze",
                "debug",
                "refactor",
                "architect",
                "design",
                "implement",
                "optimize",
                "explain",
                "compare",
                "evaluate",
                "plan",
                "review",
                "分析",
                "调试",
                "重构",
                "设计",
                "实现",
                "优化",
            ];
            let matches = complex_keywords
                .iter()
                .filter(|k| content.contains(*k))
                .count();
            score += matches as u32;
        }

        score
    }

    fn complexity_to_config(score: u32) -> Option<synaptic::core::ThinkingConfig> {
        match score {
            0..=2 => None,
            3..=5 => Some(synaptic::core::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(2000),
            }),
            6..=8 => Some(synaptic::core::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(10000),
            }),
            _ => Some(synaptic::core::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(50000),
            }),
        }
    }
}

#[async_trait]
impl EventSubscriber for ThinkingSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::BeforePromptBuild)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let thinking = if self.adaptive {
            let score = Self::estimate_complexity(&event.payload);
            Self::complexity_to_config(score)
        } else {
            self.config.clone()
        };

        if let Some(tc) = thinking {
            event.payload["thinking"] = serde_json::json!({
                "enabled": tc.enabled,
                "budget_tokens": tc.budget_tokens,
            });
            return Ok(EventAction::Modify);
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "ThinkingSubscriber"
    }
}

// ---------------------------------------------------------------------------
// 3. VerboseSubscriber — replaces VerboseMiddleware
// ---------------------------------------------------------------------------

/// Appends verbosity instructions to the system prompt before a model call.
///
/// Maps to: `BeforePromptBuild` (Sequential — can mutate the payload).
#[allow(dead_code)]
pub struct VerboseSubscriber {
    level: String,
}

impl VerboseSubscriber {
    #[allow(dead_code)]
    pub fn new(level: &str) -> Self {
        Self {
            level: level.to_string(),
        }
    }
}

#[async_trait]
impl EventSubscriber for VerboseSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::BeforePromptBuild)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let suffix = match self.level.as_str() {
            "off" => Some(
                "\n\n[INSTRUCTION: Be extremely concise. Answer directly without explanation. \
                 Omit filler words, preamble, and unnecessary transitions.]",
            ),
            "full" => Some(
                "\n\n[INSTRUCTION: Explain your reasoning step by step. Be thorough and detailed. \
                 Show your work and consider edge cases.]",
            ),
            _ => None,
        };

        if let Some(suffix) = suffix {
            let current = event.payload["system_prompt"]
                .as_str()
                .unwrap_or("")
                .to_string();
            event.payload["system_prompt"] =
                serde_json::Value::String(format!("{}{}", current, suffix));
            return Ok(EventAction::Modify);
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "VerboseSubscriber"
    }
}

// ---------------------------------------------------------------------------
// 4. LoopDetectionSubscriber — replaces LoopDetectionMiddleware
// ---------------------------------------------------------------------------

/// Detects when the agent repeatedly calls the same tool(s) with the same
/// arguments.  After `max_repeats` consecutive identical hashes it mutates
/// the response payload to inject a correction message.
///
/// Maps to: `LlmOutput` (Parallel — observe model output to track tool call
/// patterns and flag loops for the next turn).
#[allow(dead_code)]
pub struct LoopDetectionSubscriber {
    max_repeats: usize,
    history: tokio::sync::Mutex<Vec<u64>>,
}

impl LoopDetectionSubscriber {
    #[allow(dead_code)]
    pub fn new(max_repeats: usize) -> Self {
        Self {
            max_repeats,
            history: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    fn hash_tool_calls_payload(payload: &serde_json::Value) -> Option<u64> {
        let tool_calls = payload["tool_calls"].as_array()?;
        if tool_calls.is_empty() {
            return None;
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for tc in tool_calls {
            tc["name"].as_str().unwrap_or("").hash(&mut hasher);
            tc["arguments"].to_string().hash(&mut hasher);
        }
        Some(hasher.finish())
    }
}

#[async_trait]
impl EventSubscriber for LoopDetectionSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::LlmOutput)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let is_loop = if let Some(hash) = Self::hash_tool_calls_payload(&event.payload) {
            let mut history = self.history.lock().await;

            let mut repeat_count = 0;
            for h in history.iter().rev() {
                if *h == hash {
                    repeat_count += 1;
                } else {
                    break;
                }
            }

            history.push(hash);
            let len = history.len();
            if len > 50 {
                history.drain(..len - 50);
            }

            repeat_count >= self.max_repeats
        } else {
            self.history.lock().await.clear();
            false
        };

        if is_loop {
            tracing::warn!("Loop detected — injecting correction");
            event.payload["loop_detected"] = serde_json::Value::Bool(true);
            event.payload["correction"] = serde_json::Value::String(
                "I notice I've been repeating the same action. Let me try a different approach."
                    .to_string(),
            );
            return Ok(EventAction::Modify);
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "LoopDetectionSubscriber"
    }
}

// ---------------------------------------------------------------------------
// 5. ToolPolicySubscriber — replaces ToolPolicyMiddleware
// ---------------------------------------------------------------------------

/// Enforces tool allow/deny lists and owner-only restrictions.
///
/// Maps to:
///   `BeforePromptBuild` (Sequential) — remove disallowed tools from the list
///                                      before the model sees them.
///   `LlmOutput`        (Parallel)   — detect owner-only tool violations in
///                                      the model response and inject a block.
#[allow(dead_code)]
pub struct ToolPolicySubscriber {
    config: Arc<crate::config::ToolPolicyConfig>,
}

impl ToolPolicySubscriber {
    #[allow(dead_code)]
    pub fn new(config: crate::config::ToolPolicyConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    fn is_owner_only(&self, tool_name: &str) -> bool {
        let expanded = crate::agent::tool_policy::expand_tool_groups(
            &self.config.owner_only_tools,
            &self.config.tool_groups,
        );
        expanded
            .iter()
            .any(|pat| Self::tool_matches(pat, tool_name))
    }

    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if !self.config.tool_allow.is_empty() {
            let allowed = crate::agent::tool_policy::expand_tool_groups(
                &self.config.tool_allow,
                &self.config.tool_groups,
            );
            if !allowed.iter().any(|pat| Self::tool_matches(pat, tool_name)) {
                return false;
            }
        }

        if !self.config.tool_deny.is_empty() {
            let denied = crate::agent::tool_policy::expand_tool_groups(
                &self.config.tool_deny,
                &self.config.tool_groups,
            );
            if denied.iter().any(|pat| Self::tool_matches(pat, tool_name)) {
                return false;
            }
        }

        true
    }

    fn tool_matches(pattern: &str, name: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return name.starts_with(prefix);
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return name.ends_with(suffix);
        }
        pattern == name
    }
}

#[async_trait]
impl EventSubscriber for ToolPolicySubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::AnyOf(vec![
            EventKind::BeforePromptBuild,
            EventKind::LlmOutput,
        ])]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        match event.kind {
            EventKind::BeforePromptBuild => {
                let has_filters =
                    !self.config.tool_allow.is_empty() || !self.config.tool_deny.is_empty();
                if !has_filters {
                    return Ok(EventAction::Continue);
                }

                if let Some(tools) = event.payload["tools"].as_array_mut() {
                    tools.retain(|td| {
                        let name = td["name"].as_str().unwrap_or("");
                        self.is_tool_allowed(name)
                    });
                    return Ok(EventAction::Modify);
                }
            }

            EventKind::LlmOutput => {
                if self.config.owner_only_tools.is_empty() {
                    return Ok(EventAction::Continue);
                }

                if let Some(tool_calls) = event.payload["tool_calls"].as_array() {
                    let violations: Vec<String> = tool_calls
                        .iter()
                        .filter_map(|tc| tc["name"].as_str())
                        .filter(|name| self.is_owner_only(name))
                        .map(|s| s.to_string())
                        .collect();

                    if !violations.is_empty() {
                        event.payload["owner_only_violation"] = serde_json::Value::Bool(true);
                        event.payload["violation_message"] = serde_json::Value::String(format!(
                            "I cannot execute the following owner-only tool(s): {}. \
                             This operation requires owner privileges.",
                            violations.join(", ")
                        ));
                        return Ok(EventAction::Modify);
                    }
                }
            }

            _ => {}
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "ToolPolicySubscriber"
    }
}

// ---------------------------------------------------------------------------
// 6. MemoryRecallSubscriber — auto-recalls memories before each LLM call
// ---------------------------------------------------------------------------

/// Auto-recalls relevant memories before each LLM call.
/// Subscribes to BeforePromptBuild (Sequential/Mutable).
#[allow(dead_code)]
pub struct MemoryRecallSubscriber {
    memory: Arc<dyn synaptic::memory::MemoryProvider>,
    recall_limit: usize,
}

impl MemoryRecallSubscriber {
    #[allow(dead_code)]
    pub fn new(memory: Arc<dyn synaptic::memory::MemoryProvider>, recall_limit: usize) -> Self {
        Self {
            memory,
            recall_limit,
        }
    }
}

#[async_trait]
impl EventSubscriber for MemoryRecallSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::BeforePromptBuild)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        // Extract last user messages as query
        let query = event
            .payload
            .get("user_message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if query.is_empty() {
            return Ok(EventAction::Continue);
        }

        let session_key = event.payload.get("session_key").and_then(|v| v.as_str());

        match self
            .memory
            .search(query, session_key, self.recall_limit)
            .await
        {
            Ok(results) if !results.is_empty() => {
                let memory_text = results
                    .iter()
                    .map(|r| format!("- {}", r.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                if let Some(obj) = event.payload.as_object_mut() {
                    obj.insert(
                        "memory_context".to_string(),
                        serde_json::Value::String(memory_text),
                    );
                }
                tracing::debug!(count = results.len(), "recalled memories for prompt");
                Ok(EventAction::Modify)
            }
            Ok(_) => Ok(EventAction::Continue),
            Err(e) => {
                tracing::warn!(error = %e, "memory recall failed");
                Ok(EventAction::Continue) // Don't block on memory failures
            }
        }
    }

    fn name(&self) -> &str {
        "MemoryRecallSubscriber"
    }
}

// ---------------------------------------------------------------------------
// 7. MemoryCaptureSubscriber — auto-captures messages to STM
// ---------------------------------------------------------------------------

/// Auto-captures messages to STM for Viking provider.
/// Subscribes to MessageReceived + AgentEnd (Parallel).
#[allow(dead_code)]
pub struct MemoryCaptureSubscriber {
    memory: Arc<dyn synaptic::memory::MemoryProvider>,
}

impl MemoryCaptureSubscriber {
    #[allow(dead_code)]
    pub fn new(memory: Arc<dyn synaptic::memory::MemoryProvider>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl EventSubscriber for MemoryCaptureSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::AnyOf(vec![
            EventKind::MessageReceived,
            EventKind::AgentEnd,
        ])]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let session_key = event
            .payload
            .get("session_key")
            .or_else(|| event.payload.get("sessionKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match event.kind {
            EventKind::MessageReceived => {
                let content = event
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !content.is_empty() {
                    let _ = self.memory.add_message(session_key, "user", content).await;
                }
            }
            EventKind::AgentEnd => {
                let content = event
                    .payload
                    .get("response")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !content.is_empty() {
                    let _ = self
                        .memory
                        .add_message(session_key, "assistant", content)
                        .await;
                }
            }
            _ => {}
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "MemoryCaptureSubscriber"
    }
}

// ---------------------------------------------------------------------------
// 8. CostTrackingSubscriber — replaces CostTrackingMw in builder.rs
// ---------------------------------------------------------------------------

/// Records token usage from every LLM response for cost accounting.
///
/// Maps to: `LlmOutput` (Parallel — read-only observation of usage data).
///
/// Dual recording:
/// - Framework `CostTrackingCallback` for aggregate snapshots used by the UI.
/// - Business `UsageTracker` for multi-dimensional per-record persistence.
pub struct CostTrackingSubscriber {
    tracker: Arc<synaptic::callbacks::CostTrackingCallback>,
    /// Multi-dimensional usage tracker with JSONL persistence.
    usage_tracker: Arc<crate::gateway::usage::UsageTracker>,
}

impl CostTrackingSubscriber {
    pub fn new(
        tracker: Arc<synaptic::callbacks::CostTrackingCallback>,
        usage_tracker: Arc<crate::gateway::usage::UsageTracker>,
    ) -> Self {
        Self {
            tracker,
            usage_tracker,
        }
    }
}

#[async_trait]
impl EventSubscriber for CostTrackingSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::Exact(EventKind::LlmOutput)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        if event.payload["input_tokens"].is_number() {
            let input_tokens = event.payload["input_tokens"].as_u64().unwrap_or(0) as u32;
            let output_tokens = event.payload["output_tokens"].as_u64().unwrap_or(0) as u32;
            let total_tokens = event.payload["total_tokens"].as_u64().unwrap_or(0) as u32;

            tracing::debug!(
                input = input_tokens,
                output = output_tokens,
                total = total_tokens,
                "Token usage"
            );

            // 1. Record into the framework's aggregate tracker (feeds UsageSnapshot).
            let usage = synaptic::core::TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
                input_details: None,
                output_details: None,
            };
            self.tracker.record_usage(&usage).await;

            // 2. Record into the multi-dimensional usage tracker (feeds dashboard & persistence).
            let model = event.payload["model"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let provider = event.payload["provider"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();

            let channel = event.payload["channel"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let agent_id = event.payload["agent_id"]
                .as_str()
                .unwrap_or("default")
                .to_string();

            self.usage_tracker
                .record(crate::gateway::usage::UsageRecord {
                    model,
                    provider,
                    channel,
                    agent_id,
                    session_key: event.metadata.request_id.clone().unwrap_or_default(),
                    input_tokens: input_tokens as u64,
                    output_tokens: output_tokens as u64,
                    total_tokens: total_tokens as u64,
                    cost_usd: 0.0, // cost is computed by the framework tracker
                    latency_ms: 0, // per-call latency not available here
                    timestamp_ms: event.metadata.timestamp,
                })
                .await;
        } else {
            tracing::debug!("Provider returned no usage data for this response");
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "CostTrackingSubscriber"
    }
}
