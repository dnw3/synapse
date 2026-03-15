# Multi-Agent Architecture Implementation Plan

对齐 OpenClaw 的三层架构：Agent 定义 → 路由绑定 → Broadcast。

## 目标配置格式

```toml
# ====== Layer 1: Agent 定义 ======
[agents]
default = "home"           # 默认 agent（兜底）

[[agents.list]]
id = "home"
model = "claude-sonnet-4-20250514"
system_prompt = "你是一个家庭助手"
dm_scope = "per-channel-peer"
tool_allow = ["@coding", "read_file", "write_file"]
tool_deny = []
skills_dir = "~/.synapse/agents/home/skills"

[[agents.list]]
id = "work"
model = "gpt-4o"
system_prompt = "你是一个工作助手"
dm_scope = "per-peer"
tool_allow = ["@web", "@readonly"]
group_session_scope = "group_sender"

# ====== Layer 2: 路由绑定 ======
# 优先级：peer > guild/team > account > channel > default
[[bindings]]
agent = "home"
channel = "lark"
account_id = "personal"

[[bindings]]
agent = "work"
channel = "lark"
account_id = "biz"

[[bindings]]
agent = "work"
channel = "discord"
guild_id = "123456"
roles = ["dev", "admin"]

[[bindings]]
agent = "home"
channel = "discord"
peer = { kind = "direct", id = "alice" }

# ====== Layer 3: Broadcast ======
[[broadcasts]]
name = "code-review"
channel = "lark"
peer_id = "oc_review_group"
agents = ["home", "work"]
strategy = "parallel"        # parallel | sequential | aggregated
timeout_secs = 60
```

## 目录结构

```
~/.synapse/
  agents/
    home/
      workspace/          # SOUL.md, IDENTITY.md, AGENTS.md
      sessions/           # {session-key}.jsonl
      memory/             # LTM embeddings + index
      skills/             # per-agent skills
    work/
      workspace/
      sessions/
      memory/
      skills/
  pairing/                # DM pairing (共享，不分 agent)
  logs/                   # 日志 (共享)
```

## Session Key 格式

```
agent:{agent_id}:{channel}:{kind}:{peer_id}

示例:
agent:home:main                                  # 主 session (REPL/web)
agent:home:lark:dm:ou_abc123                     # Lark DM (per-peer)
agent:home:lark:personal:dm:ou_abc123            # Lark DM (per-account-channel-peer)
agent:work:lark:grp:oc_groupid                   # Lark 群聊
agent:work:discord:grp:channel123                # Discord 频道
agent:home:discord:grp:channel123:sender:user456 # Discord per-sender
```

---

## Phase 1: 基础设施（Config + Session Key + 目录）

### Step 1.1: Agent 定义配置
**文件**: `src/config/agent.rs`, `src/config/mod.rs`

新增:
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AgentsConfig {
    /// 默认 agent ID（兜底路由）
    pub default: String,
    /// Agent 定义列表
    #[serde(default)]
    pub list: Vec<AgentDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentDef {
    pub id: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    /// 工作区目录（默认 ~/.synapse/agents/{id}/workspace）
    pub workspace: Option<String>,
    /// DM session 隔离级别
    #[serde(default)]
    pub dm_scope: DmSessionScope,
    /// 群聊 session 隔离级别
    #[serde(default)]
    pub group_session_scope: Option<GroupSessionScope>,
    /// 工具白名单
    #[serde(default)]
    pub tool_allow: Vec<String>,
    /// 工具黑名单
    #[serde(default)]
    pub tool_deny: Vec<String>,
    /// Per-agent skills 目录
    pub skills_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DmSessionScope {
    /// 所有 DM 共享一个 session（不安全）
    Main,
    /// 每个 sender 独立 session
    PerPeer,
    /// 每个 channel + sender 独立（推荐）
    #[default]
    PerChannelPeer,
    /// 每个 account + channel + sender 独立（多账号场景）
    PerAccountChannelPeer,
}
```

在 `SynapseConfig` 中:
- 新增 `pub agents: Option<AgentsConfig>`
- 保留 `pub agent_routes: Vec<AgentRouteConfig>` 做向后兼容迁移
- 如果 `agents` 存在，忽略旧 `agent_routes`

### Step 1.2: Binding 配置
**文件**: `src/config/agent.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Binding {
    pub agent: String,
    pub channel: Option<String>,
    pub account_id: Option<String>,
    /// 精确 peer 绑定
    pub peer: Option<PeerMatch>,
    /// Discord guild ID
    pub guild_id: Option<String>,
    /// Slack team/workspace ID
    pub team_id: Option<String>,
    /// Discord roles (AND 逻辑)
    #[serde(default)]
    pub roles: Vec<String>,
    /// 说明
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerMatch {
    pub kind: PeerKind,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerKind {
    Direct,
    Group,
    Channel,
}
```

在 `SynapseConfig` 中新增 `pub bindings: Vec<Binding>`。

### Step 1.3: Broadcast 配置
**文件**: `src/config/agent.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastGroup {
    pub name: String,
    pub channel: Option<String>,
    pub peer_id: Option<String>,
    pub agents: Vec<String>,
    #[serde(default)]
    pub strategy: BroadcastStrategy,
    #[serde(default = "default_60")]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastStrategy {
    #[default]
    Parallel,
    Sequential,
    Aggregated,
}
```

在 `SynapseConfig` 中新增 `pub broadcasts: Vec<BroadcastGroup>`。

### Step 1.4: Agent 目录初始化
**文件**: `src/agent/workspace.rs`

```rust
pub fn ensure_agent_dirs(agent_id: &str) -> PathBuf {
    let base = dirs::home_dir().unwrap().join(".synapse/agents").join(agent_id);
    fs::create_dir_all(base.join("workspace")).ok();
    fs::create_dir_all(base.join("sessions")).ok();
    fs::create_dir_all(base.join("memory")).ok();
    base
}

pub fn agent_workspace_dir(agent_def: &AgentDef) -> PathBuf {
    agent_def.workspace.as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir().unwrap()
                .join(".synapse/agents")
                .join(&agent_def.id)
                .join("workspace")
        })
}

pub fn agent_sessions_dir(agent_id: &str) -> PathBuf {
    dirs::home_dir().unwrap()
        .join(".synapse/agents")
        .join(agent_id)
        .join("sessions")
}

pub fn agent_memory_dir(agent_id: &str) -> PathBuf {
    dirs::home_dir().unwrap()
        .join(".synapse/agents")
        .join(agent_id)
        .join("memory")
}
```

### Step 1.5: 统一 Session Key 计算
**文件**: 新建 `src/channels/session_key.rs`

```rust
pub struct SessionKeyParams<'a> {
    pub agent_id: &'a str,
    pub channel: &'a str,
    pub account_id: Option<&'a str>,
    pub chat_type: ChatType,
    pub peer_id: &'a str,       // chat_id or sender_id
    pub sender_id: Option<&'a str>,
    pub thread_id: Option<&'a str>,
    pub dm_scope: &'a DmSessionScope,
    pub group_scope: &'a GroupSessionScope,
}

pub enum ChatType { Dm, Group }

pub fn compute_session_key(p: &SessionKeyParams) -> String {
    let base = match p.chat_type {
        ChatType::Dm => match p.dm_scope {
            DmSessionScope::Main =>
                format!("agent:{}:main", p.agent_id),
            DmSessionScope::PerPeer =>
                format!("agent:{}:{}:dm:{}", p.agent_id, p.channel, p.peer_id),
            DmSessionScope::PerChannelPeer =>
                format!("agent:{}:{}:dm:{}", p.agent_id, p.channel, p.peer_id),
            DmSessionScope::PerAccountChannelPeer =>
                format!("agent:{}:{}:{}:dm:{}",
                    p.agent_id, p.channel,
                    p.account_id.unwrap_or("default"), p.peer_id),
        },
        ChatType::Group => {
            let grp = format!("agent:{}:{}:grp:{}", p.agent_id, p.channel, p.peer_id);
            match p.group_scope {
                GroupSessionScope::Group => grp,
                GroupSessionScope::GroupSender =>
                    format!("{}:sender:{}", grp, p.sender_id.unwrap_or("?")),
                GroupSessionScope::GroupTopic =>
                    format!("{}:topic:{}", grp, p.thread_id.unwrap_or("main")),
                GroupSessionScope::GroupTopicSender =>
                    format!("{}:topic:{}:sender:{}",
                        grp,
                        p.thread_id.unwrap_or("main"),
                        p.sender_id.unwrap_or("?")),
            }
        }
    };
    base
}
```

---

## Phase 2: 路由引擎

### Step 2.1: 重写 AgentRouter
**文件**: `src/router.rs`

替代当前打分制，改为 OpenClaw 的优先级链：

```rust
pub struct BindingRouter {
    agents: HashMap<String, AgentDef>,
    default_agent: String,
    bindings: Vec<Binding>,
    broadcasts: Vec<BroadcastGroup>,
}

pub struct RoutingContext {
    pub channel: String,
    pub account_id: Option<String>,
    pub peer_kind: PeerKind,      // Direct | Group | Channel
    pub peer_id: String,
    pub sender_id: Option<String>,
    pub guild_id: Option<String>,
    pub team_id: Option<String>,
    pub roles: Vec<String>,
    pub message: Option<String>,   // 消息内容 (for pattern matching)
}

pub enum RouteResult {
    Single(ResolvedAgent),
    Broadcast {
        group: BroadcastGroup,
        agents: Vec<ResolvedAgent>,
    },
}

pub struct ResolvedAgent {
    pub def: AgentDef,
    pub binding: Option<Binding>,  // 匹配的绑定规则（用于日志）
}
```

匹配逻辑：
```rust
impl BindingRouter {
    pub fn resolve(&self, ctx: &RoutingContext) -> RouteResult {
        // 1. 先查 broadcast
        if let Some(bg) = self.match_broadcast(ctx) {
            let agents = bg.agents.iter()
                .filter_map(|id| self.agents.get(id))
                .map(|def| ResolvedAgent { def: def.clone(), binding: None })
                .collect();
            return RouteResult::Broadcast { group: bg.clone(), agents };
        }

        // 2. 按优先级链匹配 binding
        //    peer > guild+roles > guild > team > account > channel > default
        let matched = self.bindings.iter()
            .filter(|b| self.binding_matches(b, ctx))
            .max_by_key(|b| self.binding_specificity(b));

        match matched {
            Some(b) => {
                let def = self.agents.get(&b.agent)
                    .unwrap_or_else(|| self.agents.get(&self.default_agent).unwrap());
                RouteResult::Single(ResolvedAgent {
                    def: def.clone(),
                    binding: Some(b.clone()),
                })
            }
            None => {
                let def = self.agents.get(&self.default_agent).unwrap();
                RouteResult::Single(ResolvedAgent {
                    def: def.clone(),
                    binding: None,
                })
            }
        }
    }

    fn binding_specificity(&self, b: &Binding) -> u32 {
        // peer: 100, guild+roles: 80, guild: 60, team: 50,
        // account: 30, channel: 10
        let mut score = 0;
        if b.peer.is_some() { score += 100; }
        if b.guild_id.is_some() { score += 60; }
        if !b.roles.is_empty() { score += 20; }
        if b.team_id.is_some() { score += 50; }
        if b.account_id.is_some() { score += 30; }
        if b.channel.is_some() { score += 10; }
        score
    }
}
```

### Step 2.2: 适配器填充 RoutingContext
**文件**: 所有 `src/channels/adapters/*.rs`

每个适配器在构造 `MessageEnvelope` 时，同时构造 `RoutingContext`：

- **Lark**: channel="lark", peer_kind=Direct/Group, peer_id=chat_id, sender_id=open_id, account_id=config.account_id
- **Discord**: +guild_id, +roles (从 member.roles 提取)
- **Slack**: +team_id (从 event.team 提取)
- **Telegram**: peer_kind 从 chat.type 推断
- 其余适配器类似

### Step 2.3: AgentSession 接入路由
**文件**: `src/channels/handler.rs`

```rust
pub struct AgentSession {
    router: Arc<BindingRouter>,
    // ... 其他字段
}

pub async fn handle_message(&self, envelope: MessageEnvelope) -> Result<...> {
    // 1. 构造 RoutingContext (从 envelope 提取)
    let ctx = RoutingContext::from_envelope(&envelope);

    // 2. 路由
    let result = self.router.resolve(&ctx);

    match result {
        RouteResult::Single(agent) => {
            // 3. 用 agent.def 的 model/prompt/tools 构建 deep agent
            // 4. session key 用 compute_session_key(agent_id=agent.def.id, ...)
            // 5. sessions 存到 agent_sessions_dir(agent.def.id)
            // 6. LTM 存到 agent_memory_dir(agent.def.id)
            self.handle_single_agent(envelope, agent).await
        }
        RouteResult::Broadcast { group, agents } => {
            self.handle_broadcast(envelope, group, agents).await
        }
    }
}
```

### Step 2.4: Per-agent Deep Agent 构建
**文件**: `src/agent/builder.rs`

新增:
```rust
pub fn build_deep_agent_for_route(
    agent_def: &AgentDef,
    global_config: &SynapseConfig,
    // ...
) -> DeepAgent {
    // model: agent_def.model 覆盖 global
    // system_prompt: agent_def.system_prompt 覆盖 global
    // workspace: agent_workspace_dir(agent_def) 的 SOUL.md 注入
    // tools: 应用 agent_def.tool_allow / tool_deny
    // skills: agent_def.skills_dir 覆盖 global
}
```

---

## Phase 3: Broadcast 实现

### Step 3.1: Broadcast 分发
**文件**: `src/channels/handler.rs`

```rust
async fn handle_broadcast(
    &self,
    envelope: MessageEnvelope,
    group: BroadcastGroup,
    agents: Vec<ResolvedAgent>,
) -> Result<Vec<String>, ...> {
    match group.strategy {
        BroadcastStrategy::Parallel => {
            let mut set = tokio::task::JoinSet::new();
            for agent in agents {
                let env = envelope.clone();
                let session = self.clone();
                set.spawn(async move {
                    session.handle_single_agent(env, agent).await
                });
            }
            // 收集所有回复，各自独立发送
            let mut replies = vec![];
            while let Some(result) = set.join_next().await {
                if let Ok(Ok(reply)) = result {
                    replies.push(reply);
                }
            }
            Ok(replies)
        }
        BroadcastStrategy::Sequential => {
            // 按顺序逐个处理
            for agent in agents {
                self.handle_single_agent(envelope.clone(), agent).await?;
            }
            Ok(vec![])
        }
        BroadcastStrategy::Aggregated => {
            // 并行处理，合并回复
            let timeout = Duration::from_secs(group.timeout_secs);
            let results = tokio::time::timeout(timeout, async {
                // ... parallel collect
            }).await;
            // 合并: "## {agent_name}\n\n{response}\n\n---"
        }
    }
}
```

### Step 3.2: Broadcast RPC
**文件**: 新建 `src/gateway/rpc/broadcasts.rs`

- `broadcasts.list` / `broadcasts.create` / `broadcasts.update` / `broadcasts.delete`
- 注册到 RPC router + scopes

---

## Phase 5: Web Dashboard 改造

### Step 5.1: AgentsPage 重构
**文件**: `web/src/components/dashboard/AgentsPage.tsx`

现状：展示 `agent_routes[]`，CRUD 单个 route（name + model + prompt）。
目标：展示三层架构（agents + bindings + broadcasts）。

**布局**：
```
┌─────────────────────────────────────────────────────┐
│ Agents                                              │
├──────────────┬──────────────────────────────────────┤
│ Agent 列表    │  Agent 详情                          │
│              │                                      │
│ 🤖 home ★    │  [概览] [工具] [绑定] [Skills] [文件]  │
│ 🧠 work      │                                      │
│              │  概览 Tab:                            │
│ + 创建 Agent │  ├─ ID: home                         │
│              │  ├─ Model: claude-sonnet-4            │
│              │  ├─ DM Scope: per-channel-peer        │
│              │  ├─ Group Scope: group                │
│              │  ├─ Workspace: ~/.synapse/agents/home/ │
│              │  └─ System Prompt: ...                │
│              │                                      │
│              │  绑定 Tab:                            │
│              │  ├─ lark / personal → home            │
│              │  ├─ discord / guild:xxx → home        │
│              │  └─ + 添加绑定                        │
│              │                                      │
│              │  Sessions Tab:                        │
│              │  └─ 该 agent 下的 session 列表         │
├──────────────┴──────────────────────────────────────┤
│ Broadcast Groups                                     │
│ ┌─────────────────────────────────────────────────┐ │
│ │ code-review  parallel  agents: [home, work]     │ │
│ │ peer: oc_review_group                           │ │
│ └─────────────────────────────────────────────────┘ │
│ + 创建 Broadcast                                     │
└─────────────────────────────────────────────────────┘
```

**新增 Tab**：
- **绑定 Tab** — 显示该 agent 的所有 bindings，支持增删
- **Sessions Tab** — 该 agent 的 session 列表（复用 SessionsPage 组件，传入 agent filter）

**Agent 概览 Tab 新增字段**：
- DM Scope 选择器（Main / Per Peer / Per Channel Peer / Per Account Channel Peer）
- Group Session Scope 选择器（Group / GroupSender / GroupTopic / GroupTopicSender）
- Workspace 路径（只读，显示实际路径）
- Tool Allow / Tool Deny 编辑（tag 输入）

**Agent 创建/编辑表单**：
- ID（创建时必填，编辑时只读）
- Model 下拉（从 providers API 获取可用模型列表）
- System Prompt 文本域
- DM Scope 下拉
- Tool Allow / Tool Deny tag 输入

**Broadcast 区域**：
- 列表展示所有 broadcast groups
- 每行显示：name, strategy badge, agents 标签, peer_id
- 内联创建/编辑/删除

### Step 5.2: SessionsPage 加 Agent 过滤
**文件**: `web/src/components/dashboard/SessionsPage.tsx`

**改动**：
- 在过滤栏新增 Agent 下拉选择器
- 从 session key 解析 agent_id（`agent:{id}:...` → 提取 id）
- 每行 session 显示 agent 标签（accent 色 badge，类似 DM pairing 的 channel badge）
- 统计卡片区域：显示每个 agent 的 session 数和消息数分布

**后端**：
- `sessions.list` RPC 新增 `agent` 过滤参数
- 或前端 client-side 过滤（session key prefix match）

### Step 5.3: ChannelsPage 显示绑定关系
**文件**: `web/src/components/dashboard/ChannelsPage.tsx`

**改动**：
- Bot Channels 列表中，每个频道行右侧显示绑定的 agent 名称
- 多账号频道：展开后每个 account 行显示 `account_id → agent_name` 映射
- 数据来源：调用 `bindings.list` RPC，在前端按 channel 分组

**示例**：
```
lark                                          🔵 ON
  ├─ personal  →  🤖 home
  └─ biz       →  🧠 work

discord                                       🔴 OFF
  └─ guild:xxx / roles:[dev]  →  🧠 work

slack                                         🔴 OFF
  └─ (no bindings)
```

### Step 5.4: OverviewPage 加 Agent 分布
**文件**: `web/src/components/dashboard/OverviewPage.tsx`

**改动**：
- 在"模型分布"卡片旁新增 "Agent 活跃度" 卡片
- 显示每个 agent 的 session 数 / 消息数 / 最近活跃时间
- 数据来源：从 sessions 列表按 agent 聚合

**布局**：
```
┌──────────────┐  ┌──────────────┐
│ 模型 Token   │  │ Agent 活跃度  │
│ 用量         │  │              │
│              │  │ home    12s  │
│ ...          │  │ work     3s  │
│              │  │              │
└──────────────┘  └──────────────┘
```

### Step 5.5: WorkspacePage 支持 Agent 切换
**文件**: `web/src/components/dashboard/WorkspacePage.tsx`

**改动**：
- 页面顶部新增 Agent 选择器下拉框（default / home / work / ...）
- 切换 agent 后，加载对应 agent 的 workspace 目录文件
- 文件 API 已支持 `?agent=xxx` 参数（`agentQs()` 函数），只需传入选中的 agent
- 文件列表从 `~/.synapse/agents/{agent_id}/workspace/` 加载

### Step 5.6: RPC 新增接口
**文件**: `src/gateway/rpc/agents.rs`, 新建 `src/gateway/rpc/broadcasts.rs`

**Agent RPC 调整**：
- `agents.list` — 返回 `agents.list[]` 定义（新格式），包含 dm_scope, tool_allow/deny, workspace 路径
- `agents.create` — 创建 agent 定义 + 初始化目录
- `agents.update` — 更新 agent 配置
- `agents.delete` — 删除 agent 定义（保留目录？或加 `purge` 参数）
- `agents.sessions` — 获取某个 agent 的 session 列表

**Bindings RPC**：
- `bindings.list` — 返回所有 bindings
- `bindings.list_for_agent` — 返回某个 agent 的 bindings
- `bindings.create` — 创建 binding
- `bindings.update` — 更新 binding（按索引）
- `bindings.delete` — 删除 binding

**Broadcasts RPC**：
- `broadcasts.list` / `broadcasts.create` / `broadcasts.update` / `broadcasts.delete`

### Step 5.7: i18n 新增
**文件**: `web/src/i18n/en.json`, `web/src/i18n/zh.json`

```json
// en.json 新增
{
  "agents": {
    "createAgent": "Create Agent",
    "editAgent": "Edit Agent",
    "agentId": "Agent ID",
    "defaultAgent": "Default Agent",
    "dmScope": "DM Session Scope",
    "dmScopeMain": "Shared (all DMs in one session)",
    "dmScopePerPeer": "Per sender",
    "dmScopePerChannelPeer": "Per channel + sender",
    "dmScopePerAccountChannelPeer": "Per account + channel + sender",
    "groupScope": "Group Session Scope",
    "toolAllow": "Allowed Tools",
    "toolDeny": "Denied Tools",
    "workspacePath": "Workspace Path",
    "tabBindings": "Bindings",
    "tabSessions": "Sessions",
    "addBinding": "Add Binding",
    "editBinding": "Edit Binding",
    "deleteBinding": "Delete Binding",
    "bindingChannel": "Channel",
    "bindingAccount": "Account",
    "bindingPeer": "Peer",
    "bindingPeerKind": "Peer Type",
    "bindingPeerDirect": "Direct (DM)",
    "bindingPeerGroup": "Group",
    "bindingPeerChannel": "Channel",
    "bindingGuild": "Guild (Discord)",
    "bindingTeam": "Team (Slack)",
    "bindingRoles": "Roles",
    "bindingComment": "Comment",
    "noBindings": "No bindings configured",
    "boundTo": "Bound to",
    "broadcasts": "Broadcast Groups",
    "createBroadcast": "Create Broadcast",
    "broadcastName": "Name",
    "broadcastAgents": "Agents",
    "broadcastStrategy": "Strategy",
    "broadcastParallel": "Parallel",
    "broadcastSequential": "Sequential",
    "broadcastAggregated": "Aggregated",
    "broadcastPeer": "Peer ID",
    "broadcastTimeout": "Timeout (s)",
    "noBroadcasts": "No broadcast groups configured",
    "agentActivity": "Agent Activity",
    "selectAgent": "Select Agent"
  }
}
```

```json
// zh.json 新增
{
  "agents": {
    "createAgent": "创建 Agent",
    "editAgent": "编辑 Agent",
    "agentId": "Agent ID",
    "defaultAgent": "默认 Agent",
    "dmScope": "DM 会话隔离",
    "dmScopeMain": "共享（所有 DM 同一会话）",
    "dmScopePerPeer": "按发送者隔离",
    "dmScopePerChannelPeer": "按渠道+发送者隔离",
    "dmScopePerAccountChannelPeer": "按账号+渠道+发送者隔离",
    "groupScope": "群聊会话隔离",
    "toolAllow": "允许的工具",
    "toolDeny": "禁止的工具",
    "workspacePath": "工作区路径",
    "tabBindings": "路由绑定",
    "tabSessions": "会话",
    "addBinding": "添加绑定",
    "editBinding": "编辑绑定",
    "deleteBinding": "删除绑定",
    "bindingChannel": "频道",
    "bindingAccount": "账号",
    "bindingPeer": "对话",
    "bindingPeerKind": "对话类型",
    "bindingPeerDirect": "私聊",
    "bindingPeerGroup": "群组",
    "bindingPeerChannel": "频道",
    "bindingGuild": "服务器 (Discord)",
    "bindingTeam": "工作区 (Slack)",
    "bindingRoles": "角色",
    "bindingComment": "备注",
    "noBindings": "暂无路由绑定",
    "boundTo": "绑定到",
    "broadcasts": "广播组",
    "createBroadcast": "创建广播组",
    "broadcastName": "名称",
    "broadcastAgents": "Agent 列表",
    "broadcastStrategy": "策略",
    "broadcastParallel": "并行",
    "broadcastSequential": "顺序",
    "broadcastAggregated": "聚合",
    "broadcastPeer": "对话 ID",
    "broadcastTimeout": "超时 (秒)",
    "noBroadcasts": "暂无广播组",
    "agentActivity": "Agent 活跃度",
    "selectAgent": "选择 Agent"
  }
}
```

---

## Phase 4: 迁移与兼容

### Step 4.1: 旧配置迁移
**文件**: `src/config/mod.rs`

```rust
impl SynapseConfig {
    /// 旧 [[agent_routes]] → 新 agents + bindings 自动转换
    pub fn migrate_legacy_routes(&mut self) {
        if self.agents.is_some() { return; } // 已用新格式
        if self.agent_routes.is_empty() { return; }

        let mut agents = AgentsConfig {
            default: "default".into(),
            list: vec![],
        };
        let mut bindings = vec![];

        for route in &self.agent_routes {
            agents.list.push(AgentDef {
                id: route.name.clone(),
                model: route.model.clone(),
                system_prompt: route.system_prompt.clone(),
                workspace: route.workspace.clone(),
                ..Default::default()
            });
            for ch in &route.channels {
                bindings.push(Binding {
                    agent: route.name.clone(),
                    channel: Some(ch.clone()),
                    ..Default::default()
                });
            }
        }

        self.agents = Some(agents);
        self.bindings = bindings;
    }
}
```

### Step 4.2: Session Key 迁移
**文件**: `src/channels/handler.rs`

resolve_session() 中：新 key 查不到时，回退查旧 key，找到则自动迁移映射。

---

## 执行顺序

```
Phase 1 (基础设施):
  1.1 Agent 定义 config         ← 先做，其他都依赖
  1.2 Binding config
  1.3 Broadcast config
  1.4 Agent 目录初始化
  1.5 统一 session key

Phase 2 (路由引擎):
  2.1 重写 BindingRouter         ← 核心
  2.2 适配器填充 RoutingContext
  2.3 AgentSession 接入路由
  2.4 Per-agent deep agent 构建

Phase 3 (Broadcast):
  3.1 Broadcast 分发逻辑
  3.2 Broadcast RPC

Phase 4 (迁移):
  4.1 旧配置自动迁移
  4.2 Session key 迁移

Phase 5 (Web Dashboard):           ← 可与 Phase 2-4 并行
  5.1 AgentsPage 重构              ← 最大改动
  5.2 SessionsPage 加 Agent 过滤
  5.3 ChannelsPage 显示绑定关系
  5.4 OverviewPage 加 Agent 分布
  5.5 WorkspacePage 支持 Agent 切换
  5.6 RPC 新增接口
  5.7 i18n 补全
```

## 依赖关系

```
1.1 ──→ 1.2 ──→ 1.3
 │                │
 ├──→ 1.4        │
 │                │
 ├──→ 1.5        │
 │                │
 ▼                ▼
2.1 ──→ 2.2 ──→ 2.3 ──→ 2.4
                  │
                  ▼
                 3.1 ──→ 3.2
                          │
4.1 (独立)                 │
4.2 (依赖 1.5)             │
                          ▼
5.6 (依赖 1.1-1.3) ──→ 5.1 ──→ 5.2
                        │       │
                        ├──→ 5.3
                        ├──→ 5.4
                        ├──→ 5.5
                        └──→ 5.7
```

## 验收标准

### 后端
- [ ] `[[agents.list]]` 定义多个 agent，各自独立目录 (`~/.synapse/agents/{id}/`)
- [ ] `[[bindings]]` 按 peer > guild > team > account > channel 优先级路由
- [ ] DM session 按 dm_scope 隔离（不同 sender 独立 session）
- [ ] 每个 agent 的 sessions/memory 存在独立目录下
- [ ] Per-agent tool_allow/deny 生效
- [ ] Per-agent workspace 文件 (SOUL.md, IDENTITY.md) 独立加载
- [ ] Broadcast 并行分发，多 agent 各自回复
- [ ] Aggregated 策略合并回复，带超时
- [ ] 旧 `[[agent_routes]]` 配置自动迁移到新格式
- [ ] 旧 session key 自动迁移
- [ ] `cargo clippy -- -D warnings` 通过

### Web Dashboard
- [ ] AgentsPage: 三层架构展示（agent 定义 + bindings + broadcasts）
- [ ] AgentsPage: Agent CRUD（创建/编辑/删除）
- [ ] AgentsPage: Binding CRUD（添加/编辑/删除路由绑定）
- [ ] AgentsPage: Broadcast CRUD（创建/编辑/删除广播组）
- [ ] AgentsPage: 每个 agent 的 Sessions Tab 显示关联 session
- [ ] AgentsPage: 文件 Tab 加载 agent 独立目录的 workspace 文件
- [ ] SessionsPage: Agent 下拉过滤 + 每行显示 agent 标签
- [ ] ChannelsPage: 频道行旁显示绑定的 agent 名称
- [ ] OverviewPage: Agent 活跃度卡片
- [ ] WorkspacePage: Agent 切换下拉框
- [ ] `cd web && npx eslint src` 通过
- [ ] i18n 完整 (en.json + zh.json 所有新增 key)
