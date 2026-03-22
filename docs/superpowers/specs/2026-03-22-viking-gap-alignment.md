# Viking Gap Alignment: Auto-Recall, Auto-Capture, Progressive Loading

**Date:** 2026-03-22
**Status:** Draft
**Scope:** Business layer (synapse) — memory plugin enhancements
**Breaking:** No — additive changes only

## Problem

Viking integration has 3 critical gaps compared to OpenClaw's integration design (documented in `/Users/bytedance/code/github/OpenViking/docs/design/openclaw-integration.md`):

1. **Auto-recall is dead code** — `MemoryRecallSubscriber` exists but is never registered. Even with `auto_recall: true`, no memories are injected per-turn.
2. **Auto-capture is dead code** — `MemoryCaptureSubscriber` exists but is never registered. Conversation messages are never sent to Viking, so `commit()` has nothing to extract.
3. **No progressive loading** — Agent can only see L0-level search results. Viking's L0/L1/L2 content hierarchy is completely unused.

Secondary gaps: no `commit_memory` tool for active writes, no intent detection to skip greeting recall, no BeforeCompaction → commit trigger.

## Design Principle

**Observe via events, mutate via interceptors.**

| Operation | Nature | Pattern |
|-----------|--------|---------|
| Auto-recall (inject memories into prompt) | Request mutation | **Interceptor** — direct `&mut ModelRequest` |
| Auto-capture (record messages to Viking) | Observation | **EventSubscriber** — fire-and-forget |

This matches LangChain (callbacks for observation, middleware for transformation), OpenClaw (hooks that directly modify messages), and AutoGen (middleware for augmentation).

The existing `MemoryRecallSubscriber` uses the wrong pattern — it tries to mutate the prompt via event payload, but no framework code consumes the `memory_context` payload key. It should be an Interceptor.

## Design

### Shared Components (face `dyn MemoryProvider`)

#### MemoryRecallInterceptor

```rust
/// Interceptor that auto-recalls relevant memories before each model call.
/// Works with any MemoryProvider implementation (native LTM or Viking).
pub struct MemoryRecallInterceptor {
    provider: Arc<dyn MemoryProvider>,
    limit: usize,
    score_threshold: f64,
}

#[async_trait]
impl Interceptor for MemoryRecallInterceptor {
    async fn before_model(&self, req: &mut ModelRequest) -> Result<(), SynapticError> {
        // 1. Extract last user message from req.messages
        let query = extract_last_user_message(&req.messages);

        // 2. Intent detection — skip greetings and short messages
        if should_skip_recall(&query) {
            return Ok(());
        }

        // 3. Recall from provider
        let results = self.provider.recall(&query, self.limit).await?;

        // 4. Filter by score threshold
        let results: Vec<_> = results.into_iter()
            .filter(|r| r.score >= self.score_threshold)
            .collect();

        if results.is_empty() {
            return Ok(());
        }

        // 5. Inject into system prompt as <recalled_memories> section
        let recall_text = format_recall_results(&results);
        let section = format!(
            "\n\n<recalled_memories>\n{recall_text}\n</recalled_memories>"
        );
        match req.system_prompt {
            Some(ref mut prompt) => prompt.push_str(&section),
            None => req.system_prompt = Some(section),
        }

        Ok(())
    }
}

/// Skip recall for greetings and very short messages.
fn should_skip_recall(query: &str) -> bool {
    let trimmed = query.trim();
    // Use char count (not byte length) for correct CJK handling
    if trimmed.chars().count() <= 2 { return true; }
    const GREETINGS: &[&str] = &[
        "你好", "hi", "hello", "hey", "嗨", "哈喽", "早上好", "晚上好",
        "good morning", "good evening", "thanks", "谢谢", "ok",
    ];
    GREETINGS.iter().any(|g| trimmed.eq_ignore_ascii_case(g))
}

fn extract_last_user_message(messages: &[Message]) -> String {
    // Message API: role() returns &str ("human"/"assistant"/"system"/"tool"),
    // content() returns &str.
    messages.iter().rev()
        .find(|m| m.role() == "human")
        .map(|m| m.content().to_string())
        .unwrap_or_default()
}

fn format_recall_results(results: &[MemoryResult]) -> String {
    results.iter().enumerate()
        .map(|(i, r)| {
            let cat = r.category.as_deref().unwrap_or("general");
            format!("{}. [{}] {} (score: {:.2}, uri: {})",
                i + 1, cat, r.content, r.score, r.uri)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

#### MemoryCaptureSubscriber

```rust
/// EventSubscriber that auto-captures conversation turns to the memory provider.
/// Subscribes to: MessageReceived, AgentEnd (Parallel — observation only).
/// Also subscribes to: BeforeCompaction (Sequential — triggers commit before compact).
pub struct MemoryCaptureSubscriber {
    provider: Arc<dyn MemoryProvider>,
}

#[async_trait]
impl EventSubscriber for MemoryCaptureSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::AnyOf(vec![
            EventKind::MessageReceived,  // Parallel — record user message
            EventKind::AgentEnd,         // Parallel — record assistant response
            EventKind::BeforeCompaction, // Sequential — commit before compact
        ])]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let session_key = event.payload.get("session_key")
            .or_else(|| event.payload.get("sessionKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match event.kind {
            EventKind::MessageReceived => {
                if let Some(content) = event.payload.get("content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        let _ = self.provider.add_message(session_key, "user", content).await;
                    }
                }
            }
            EventKind::AgentEnd => {
                if let Some(content) = event.payload.get("response").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        let _ = self.provider.add_message(session_key, "assistant", content).await;
                    }
                }
            }
            EventKind::BeforeCompaction => {
                // Commit session to extract memories before context is compacted
                match self.provider.commit(session_key).await {
                    Ok(result) => {
                        tracing::info!(
                            session = session_key,
                            extracted = result.memories_extracted,
                            merged = result.memories_merged,
                            "committed memories before compaction"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "memory commit before compaction failed");
                    }
                }
            }
            _ => {}
        }

        Ok(EventAction::Continue)
    }
}
```

### Viking-Specific Components

#### VikingContentTool (L0/L1/L2 Progressive Loading)

Viking-specific because native LTM has no content hierarchy concept.

```rust
/// Tool for progressive content loading from Viking.
/// Agent uses after memory_search to drill into specific results.
pub struct VikingContentTool {
    provider: Arc<VikingMemoryProvider>,  // Concrete type — needs read_content()
}

impl Tool for VikingContentTool {
    fn name() -> "memory_read"
    fn description() -> "Read memory content at different detail levels.
        Use 'abstract' for quick summaries (~100 tokens),
        'overview' for navigation (~2k tokens),
        'full' for complete content.
        Use after memory_search to get more detail on specific results."
    fn parameters() -> {
        "uri": string (required) — "Viking URI from memory_search results",
        "level": enum["abstract", "overview", "full"] (default: "overview")
    }
    async fn call(args) -> provider.read_content(uri, level)
}
```

Requires extending `VikingMemoryProvider`:
```rust
impl VikingMemoryProvider {
    /// Read content at a specific detail level.
    /// Maps to: GET /api/v1/content/{level}?uri={uri}
    pub async fn read_content(&self, uri: &str, level: &str) -> Result<Value, SynapticError> {
        let endpoint = match level {
            "abstract" => "/api/v1/content/abstract",
            "overview" => "/api/v1/content/overview",
            "full" | _ => "/api/v1/content/read",
        };
        let url = self.url(endpoint);
        let resp = self.auth(self.client.get(&url).query(&[("uri", uri)]))
            .send().await.map_err(Self::map_err)?;
        let resp = Self::check_status(resp).await?;
        let data: serde_json::Value = resp.json().await.map_err(Self::map_err)?;
        Ok(data)
    }
}
```

#### VikingCommitMemoryTool (Active Memory Write)

```rust
/// Tool for the agent to actively save memories to Viking.
/// When user says "remember that I like dark mode", agent calls this.
pub struct VikingCommitMemoryTool {
    provider: Arc<VikingMemoryProvider>,
}

impl Tool for VikingCommitMemoryTool {
    fn name() -> "memory_commit"
    fn description() -> "Actively save important information to long-term memory.
        Use when the user asks you to remember something, or when you encounter
        information that should be preserved across sessions."
    fn parameters() -> {
        "content": string (required) — "The information to remember",
        "session_key": string (default: "default") — "Session identifier"
    }
    async fn call(args) -> {
        // Write as a user-initiated memory instruction so Viking's LLM extraction
        // treats it as high-priority content during commit().
        // The message is added as "user" role with explicit save-intent framing,
        // which Viking's extraction pipeline recognizes as something to preserve.
        // Followed by immediate commit() to ensure extraction happens now.
        let msg = format!("Please remember the following: {content}");
        provider.add_message(session_key, "user", &msg).await?;
        provider.commit(session_key).await?;
        Ok(json!({ "status": "saved", "content": content }))
    }
}
```

### Plugin Registration

#### memory-native plugin (updated)

```rust
async fn register(&self, api: &mut PluginApi<'_>) -> Result<(), SynapticError> {
    let provider = Arc::new(/* ... */);

    // 1. Memory slot
    api.register_memory(provider.clone());

    // 2. Tools
    api.register_tool(MemorySearchTool::new(provider.clone()));
    if let Some(ref ltm) = self.ltm {
        api.register_tool(MemoryGetTool::new(ltm.clone()));
        api.register_tool(MemorySaveTool::new(ltm.clone()));
        api.register_tool(MemoryForgetTool::new(ltm.clone()));
    }

    // 3. Auto-recall (only if real embeddings available)
    if self.has_embeddings {
        api.register_interceptor(Arc::new(MemoryRecallInterceptor::new(
            provider.clone(),
            5,     // recall_limit
            0.3,   // score_threshold — higher for native (keyword search is noisier)
        )));
    }

    // No auto-capture for native — add_message() is no-op
    Ok(())
}
```

#### memory-viking plugin (updated)

```rust
async fn register(&self, api: &mut PluginApi<'_>) -> Result<(), SynapticError> {
    let provider = Arc::new(VikingMemoryProvider::new(self.config.clone()));

    // 1. Memory slot
    api.register_memory(provider.clone());

    // 2. Search tool (shared, uses MemoryProvider trait)
    api.register_tool(MemorySearchTool::new(provider.clone()));

    // 3. Viking-specific tools
    api.register_tool(Arc::new(VikingContentTool::new(provider.clone())));
    api.register_tool(Arc::new(VikingCommitMemoryTool::new(provider.clone())));

    // 4. Auto-recall interceptor
    if self.config.auto_recall {
        api.register_interceptor(Arc::new(MemoryRecallInterceptor::new(
            provider.clone(),
            self.config.recall_limit,
            self.config.recall_score_threshold,
        )));
    }

    // 5. Auto-capture subscriber (record messages + commit on compaction)
    if self.config.auto_capture {
        api.register_event_subscriber(
            Arc::new(MemoryCaptureSubscriber::new(provider.clone())),
            0, // default priority
        );
    }

    // 6. Managed service
    api.register_service(Box::new(VikingService::new(self.config.clone())));

    Ok(())
}
```

### Migration / Cleanup

| Delete | Reason |
|--------|--------|
| `MemoryRecallSubscriber` in `subscribers.rs` | Wrong pattern, replaced by `MemoryRecallInterceptor` |
| `MemoryCaptureSubscriber` in `subscribers.rs` | Moved to `src/plugins/memory_capture.rs` with enhancements |

### New Files

| File | Content |
|------|---------|
| `src/plugins/memory_recall.rs` | `MemoryRecallInterceptor` + intent detection + formatting |
| `src/plugins/memory_capture.rs` | `MemoryCaptureSubscriber` + BeforeCompaction commit |
| `src/plugins/viking_tools.rs` | `VikingContentTool` + `VikingCommitMemoryTool` |

### Modified Files

| File | Change |
|------|--------|
| `src/plugins/memory_native.rs` | Add `has_embeddings` field, conditionally register recall interceptor |
| `src/plugins/memory_viking.rs` | Register recall/capture/viking tools |
| `src/memory/viking_provider.rs` | Add `pub async fn read_content(&self, uri, level)` method |
| `src/agent/subscribers.rs` | Delete `MemoryRecallSubscriber` + `MemoryCaptureSubscriber` |
| `src/plugins/mod.rs` | Add new submodules |

### NativeMemoryPlugin `has_embeddings` Detection

```rust
pub struct NativeMemoryPlugin {
    ltm: Option<Arc<LongTermMemory>>,
    has_embeddings: bool,
}

impl NativeMemoryPlugin {
    pub fn new(ltm: Option<Arc<LongTermMemory>>) -> Self {
        let has_embeddings = ltm.as_ref().map(|l| l.uses_embeddings()).unwrap_or(false);
        Self { ltm, has_embeddings }
    }
}
```

`LongTermMemory::uses_embeddings()` already exists — returns true when a real embedding provider (OpenAI, Jina, etc.) is configured vs the fallback `FakeEmbeddings`.

## YAGNI — Not Doing Now

- **C5: Skill memory injection** — BeforeToolCall hook to augment SKILL.md with usage stats. Stretch goal.
- **C6: Tool memory injection** — System prompt section with tool usage statistics. Stretch goal.
- **Viking directory tree injection** — Pre-inject `ov ls viking://` into system prompt. Nice-to-have, not critical.
- **Session creation API** — We use implicit session creation via `add_message`. Works fine.
- **`search/grep` and `search/glob`** — Viking has regex/glob search APIs. Not needed yet.
- **Resource management tools** — `add_resource`, `export/import`. Future feature.
