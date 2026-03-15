# Usage Tracking — 完整对齐 OpenClaw

6 维度 + 持久化 + 日粒度 + 延迟统计。

## 目标

```
模型调用 → Usage 归一化 → CostTracker 聚合 → Session 持久化 → RPC 查询 → Dashboard 展示
```

### 6 维度
1. **Model** — 每个模型的 token/cost
2. **Provider** — 每个 provider（OpenAI/Anthropic/Ark/...）
3. **Channel** — 每个频道（lark/telegram/discord/webchat/...）
4. **Agent** — 每个 agent（default/home/work/...）
5. **Session** — 每个会话
6. **Time** — 日粒度时间序列

### 追踪指标
- input_tokens / output_tokens / cache_read_tokens / cache_write_tokens / total_tokens
- input_cost / output_cost / total_cost
- request_count / message_count
- latency: avg_ms / p95_ms / min_ms / max_ms
- error_count

---

## Phase 1: Bot 频道 Token 回报（核心缺失）

### Step 1.1: AgentSession 回报 usage
**文件**: `src/channels/handler.rs`

当前 `handle_deep_agent` 调用 `agent.invoke()` 后只提取文本回复。
需要同时提取 token usage 并回报。

```rust
// agent.invoke() 返回 AgentResult，其中包含 usage 信息
let result = agent.invoke(initial_state).await?;
let final_state = result.into_state();

// 提取 usage（从最后的 AI message 的 metadata 中）
let usage = extract_usage_from_messages(&final_state.messages);

// 回报到 CostTracker（如果可用）
if let Some(ref tracker) = self.cost_tracker {
    tracker.record(UsageRecord {
        model: model_name.clone(),
        provider: provider_name.clone(),
        channel: envelope.delivery.channel.clone(),
        agent_id: agent_info.id.clone(),
        session_id: sid.clone(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_usd: usage.cost_usd,
        latency_ms: start.elapsed().as_millis() as u64,
        timestamp: chrono::Utc::now(),
    });
}
```

### Step 1.2: CostTracker 加入 AgentSession
**文件**: `src/channels/handler.rs`, `src/gateway/mod.rs`

- `AgentSession` 新增 `cost_tracker: Option<Arc<CostTracker>>` 字段
- Gateway 启动时把 `app_state.cost_tracker` 传入 AgentSession
- Bot 适配器和 WebSocket handler 都使用同一个 CostTracker

### Step 1.3: extract_usage_from_messages 辅助函数
**文件**: `src/channels/handler.rs`

从 agent 输出的 messages 中提取 usage metadata：
- synaptic 框架的 `Message` 类型在 AI response 中可能携带 `usage` 字段
- 如果没有，从 `ChatResponse` 的 `usage` 字段获取
- 需要确认 synaptic 框架是否在 deep agent 的 graph invoke 中传播 usage

---

## Phase 2: CostTracker 多维聚合

### Step 2.1: 扩展 CostTracker 数据结构
**文件**: `src/gateway/api/dashboard.rs` 或新建 `src/usage.rs`

```rust
pub struct UsageRecord {
    pub model: String,
    pub provider: String,
    pub channel: String,
    pub agent_id: String,
    pub session_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
    pub is_error: bool,
}

pub struct UsageAggregates {
    /// 总计
    pub totals: UsageTotals,
    /// 按模型
    pub by_model: Vec<ModelUsage>,
    /// 按 Provider
    pub by_provider: Vec<ProviderUsage>,
    /// 按频道
    pub by_channel: Vec<ChannelUsage>,
    /// 按 Agent
    pub by_agent: Vec<AgentUsage>,
    /// 按天
    pub daily: Vec<DailyUsage>,
    /// 延迟统计
    pub latency: LatencyStats,
}
```

### Step 2.2: 聚合逻辑
**文件**: `src/usage.rs`

```rust
impl CostTracker {
    pub fn record(&self, record: UsageRecord) { ... }

    pub fn snapshot(&self) -> UsageAggregates {
        // 从内存记录聚合各维度
    }

    pub fn snapshot_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> UsageAggregates {
        // 时间范围查询
    }
}
```

---

## Phase 3: 持久化

### Step 3.1: Usage 写入 Session JSONL
**文件**: `src/channels/handler.rs`

每次 agent 回复后，在 session transcript JSONL 中追加 usage 元数据行：
```json
{"type":"usage","model":"gpt-4o","provider":"openai","channel":"lark","agent":"home","input_tokens":150,"output_tokens":300,"cost_usd":0.002,"latency_ms":1200,"timestamp":"2026-03-15T12:00:00Z"}
```

### Step 3.2: 启动时从文件恢复
**文件**: `src/usage.rs`

Gateway 启动时扫描 session 目录，从 JSONL 文件中恢复 usage 记录到 CostTracker。
加 30s 缓存避免频繁 I/O。

---

## Phase 4: RPC 接口

### Step 4.1: 扩展 usage RPC
**文件**: `src/gateway/rpc/usage.rs` 或现有 dashboard API

新增/扩展端点：
- `usage.aggregates` — 返回完整 6 维聚合
  - 参数：`{ from?: string, to?: string, agent?: string, channel?: string }`
  - 返回：`UsageAggregates`

- `usage.daily` — 返回日粒度时间序列
  - 参数：`{ days?: number }`
  - 返回：`Vec<DailyUsage>`

### Step 4.2: 现有端点兼容
保持现有 `GET /api/dashboard/usage` 和 `GET /api/dashboard/stats` 正常工作。
新端点通过 RPC invoke 访问。

---

## Phase 5: Dashboard 展示

### Step 5.1: UsagePage 增强
**文件**: `web/src/components/dashboard/UsagePage.tsx`

新增卡片/图表：
- **按频道分布** — 饼图或柱状图（lark: 60%, webchat: 30%, telegram: 10%）
- **按 Agent 分布** — 柱状图（home: 70%, work: 30%）
- **延迟统计** — avg / p95 / min / max 卡片
- **按天趋势** — 已有折线图，确保数据源正确
- **缓存命中** — cache_read_tokens / total_tokens 比率

### Step 5.2: OverviewPage 增强
**文件**: `web/src/components/dashboard/OverviewPage.tsx`

概览页的统计卡片使用新的聚合数据：
- Token 总量从 CostTracker 获取（包含 bot 频道）
- 费用从 CostTracker 获取
- 新增 "活跃频道" 卡片

### Step 5.3: i18n
```json
{
  "usage": {
    "byChannel": "By Channel",
    "byAgent": "By Agent",
    "latency": "Latency",
    "avgLatency": "Avg",
    "p95Latency": "P95",
    "cacheHitRate": "Cache Hit Rate",
    "noChannelData": "No channel usage data"
  }
}
```

---

## Phase 6: 模型兼容性

### Step 6.1: 非标 Provider 验证
**文件**: synaptic 框架层

OpenClaw 刚修的 bug：非标 OpenAI-compatible endpoint 被强制关闭 `stream_options.include_usage`。
需要验证 synaptic 框架是否有同样问题：
- 检查 `stream_options: { include_usage: true }` 是否发送给所有 provider
- Ark/DashScope/DeepSeek/Groq 是否正确返回 usage chunk
- 如果有问题，在框架层修复

---

## 执行顺序

```
Phase 1 (Bot token 回报):          ← 最先做，解决 0 token 问题
  1.1 AgentSession 回报 usage
  1.2 CostTracker 加入 AgentSession
  1.3 extract_usage 辅助函数

Phase 2 (多维聚合):
  2.1 扩展数据结构
  2.2 聚合逻辑

Phase 3 (持久化):
  3.1 写入 Session JSONL
  3.2 启动恢复

Phase 4 (RPC):
  4.1 扩展 usage RPC
  4.2 现有端点兼容

Phase 5 (Dashboard):
  5.1 UsagePage 增强
  5.2 OverviewPage 增强
  5.3 i18n

Phase 6 (模型兼容):
  6.1 非标 Provider 验证
```

## 验收标准

- [ ] Lark 发消息后，用量页显示 token 消耗（不再是 0）
- [ ] 按频道分布图正确显示 lark / webchat 占比
- [ ] 按 Agent 分布图正确显示 default / home / work 占比
- [ ] 日粒度趋势图有真实数据
- [ ] 延迟统计卡片显示 avg / p95
- [ ] 重启后用量数据不丢失（从 JSONL 恢复）
- [ ] 非标 Provider（Ark/DashScope）正确报告 usage
- [ ] cargo clippy + eslint 通过
