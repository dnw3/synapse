# Synapse

Personal AI Agent assistant powered by the [Synaptic](../synaptic) framework. Multi-channel bot adapters, web dashboard, multi-agent routing, long-term memory, and real-time streaming.

## Architecture

```
                          ┌─────────────────────────────────────────────────┐
                          │                  Synapse                        │
                          │                                                 │
  Lark ──┐                │  ┌──────────┐    ┌──────────────┐              │
  Slack ──┤  MessageEnvelope │ Binding   │    │ AgentSession │              │
  TG   ──┤──────────────────→│ Router   │───→│ (Deep Agent) │              │
  Discord─┤                │  └──────────┘    └──────┬───────┘              │
  Web  ──┘                │                          │                      │
                          │         ┌────────────────┼────────────────┐     │
                          │         ▼                ▼                ▼     │
                          │  ┌──────────┐    ┌──────────┐    ┌──────────┐  │
                          │  │  Tools   │    │  Memory  │    │  Session │  │
                          │  │ MCP/PDF/ │    │  LTM +   │    │  JSONL   │  │
                          │  │ Firecrawl│    │ Embeddings│   │ Persist  │  │
                          │  └──────────┘    └──────────┘    └──────────┘  │
                          │                                                 │
                          │  ┌──────────────────────────────────────────┐   │
                          │  │           Gateway (Axum)                 │   │
                          │  │  WebSocket v3 │ REST API │ RPC (70+)    │   │
                          │  └──────────────────────────────────────────┘   │
                          │                                                 │
                          │  ┌──────────────────────────────────────────┐   │
                          │  │        Web Dashboard (React 19)         │   │
                          │  │  Chat │ Agents │ Channels │ Usage │ ... │   │
                          │  └──────────────────────────────────────────┘   │
                          └─────────────────────────────────────────────────┘
                                              │
                          ┌───────────────────┴───────────────────┐
                          │         Synaptic Framework             │
                          │  ChatModel │ Graph │ Tools │ Store     │
                          │  Session │ MCP │ Middleware │ Message IR│
                          └───────────────────────────────────────┘
```

## Quick Start

```bash
# Unified launch script
./start.sh dev              # Dev: backend :3000 + Vite HMR :5173
./start.sh serve            # Production gateway (release build)
./start.sh repl             # Interactive REPL
./start.sh bot lark         # Bot adapter (lark/telegram/discord/slack/...)
./start.sh build            # Full production build
./start.sh stop             # Kill all processes

# Direct cargo
cargo build --release --features full
cargo test
```

## Configuration

Search order: `-c` flag → `./synapse.toml` → `~/.synapse/config.toml`

```toml
# Model
[model]
provider = "openai"
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"

# Agent personality
[agent]
system_prompt = "You are Synapse, a helpful AI assistant."
max_turns = 50

# Multi-agent definitions
[[agents.list]]
id = "home"
model = "claude-sonnet-4-20250514"
dm_scope = "per_channel_peer"
tool_allow = ["@coding"]

[[agents.list]]
id = "work"
model = "gpt-4o"
tool_allow = ["@web", "@readonly"]

# Route bindings (priority: peer > guild > team > account > channel)
[[bindings]]
agent = "home"
channel = "lark"
account_id = "personal"

[[bindings]]
agent = "work"
channel = "discord"
guild_id = "123456"
roles = ["dev"]

# Broadcast: fan out to multiple agents
[[broadcasts]]
name = "code-review"
peer_id = "oc_review_group"
agents = ["home", "work"]
strategy = "parallel"

# Bot channels (multi-account)
[[lark]]
enabled = true
app_id = "cli_xxx"
app_secret_env = "LARK_APP_SECRET"
dm_policy = "pairing"
streaming = true
render_mode = "card"

# Memory
[memory]
embeddings_provider = "voyage"
auto_compact_threshold = 8000

# Schedules
[[schedule]]
name = "morning"
cron = "0 9 * * *"
prompt = "Check calendar and summarize today"
```

## Source Structure

```
src/
  main.rs + cli.rs              CLI entry (clap): chat, serve, bot, connect, init, pairing, ...
  router.rs                     BindingRouter: multi-agent routing with priority-chain matching
  usage.rs                      UsageTracker: 6-dimension tracking + JSONL persistence

  agent/                        Deep Agent construction
    builder.rs                  build_deep_agent(): tools + 12 middleware layers + checkpointer
    model.rs                    Model resolution with failover
    tracing_mw.rs               Full-text structured logging (never truncated)
    callbacks.rs                Safety: BotSafetyCallback, InteractiveApproval, WebSocketApproval
    mcp.rs                      MCP server discovery + tool loading
    tool_policy.rs              Per-agent tool allow/deny (glob patterns)

  channels/                     Bot adapters + message handling
    handler.rs                  AgentSession: unified handler for all adapters
                                  - Deep Agent mode (tools) + Simple chat (fallback)
                                  - Multi-agent routing via BindingRouter
                                  - Broadcast dispatch (parallel/sequential/aggregated)
                                  - Usage tracking + cost tracking
    formatter.rs                Message IR integration: parse → chunk → render per-platform
    session_key.rs              Deterministic session key: agent:{id}:{channel}:{kind}:{peer}
    dm.rs                       DM pairing: FileDmPolicyEnforcer + approval flow
    dedup.rs                    LRU message deduplication
    adapters/                   23 platform adapters:
      lark.rs                     Lark/Feishu (card streaming, pbbp2 binary protocol)
      telegram.rs                 Telegram (long polling)
      discord.rs                  Discord (gateway WebSocket, guild/role routing)
      slack.rs                    Slack (Bolt framework)
      dingtalk.rs                 DingTalk
      teams.rs                    Microsoft Teams
      whatsapp.rs                 WhatsApp (webhook)
      signal.rs                   Signal (signal-cli)
      imessage.rs                 iMessage
      line.rs                     LINE
      googlechat.rs               Google Chat
      wechat.rs                   WeChat/WeCom
      matrix.rs                   Matrix
      mattermost.rs               Mattermost
      irc.rs                      IRC
      webchat.rs                  WebChat (REST)
      twitch.rs                   Twitch
      nostr.rs                    Nostr
      nextcloud.rs                Nextcloud Talk
      synology.rs                 Synology Chat
      tlon.rs                     Tlon (Urbit)
      zalo.rs                     Zalo

  gateway/                      Web server (Axum)
    mod.rs                      Server startup + channel adapter spawning
    state.rs                    AppState: model, sessions, config, cost_tracker, usage_tracker, ...
    ws.rs                       WebSocket v3 protocol (streaming tokens, tool calls, reasoning)
    api/                        REST: conversations, messages, files, dashboard, upload
    rpc/                        70+ RPC methods:
      agents.rs                   agents.list/create/update/delete
      sessions.rs                 sessions.list/get/patch/delete/compact
      channels.rs                 channels.status/logout
      config_rpc.rs               config.get/set/schema/validate/reload
      schedules.rs                cron.list/add/update/remove/run
      nodes.rs                    node.list/register/invoke
      exec_approvals.rs           approval.request/resolve/waitDecision
      bindings_rpc.rs             bindings.list
      broadcasts_rpc.rs           broadcasts.list
      dm_pairing.rs               dm.pairing.list/approve/remove/channels
      usage.rs                    usage.aggregates/records
      ...
    messages/                   MessageEnvelope, RoutingMeta, delivery routing, channel registry
    nodes/                      Device pairing, QR codes, bootstrap tokens

  config/                       SynapseConfig extending framework's SynapticAgentConfig
    agent.rs                    AgentsConfig, AgentDef, Binding, DmSessionScope, BroadcastGroup
    bot.rs                      23 bot configs + DmPolicy, GroupPolicy, GroupSessionScope
    models.rs                   Model catalog, provider definitions
    security.rs                 Auth, SSRF guard, secret masking
    memory.rs                   LTM config: embeddings, hybrid search, decay, reflection

  memory/                       Long-term memory
    ltm.rs                      LongTermMemory: embeddings + keyword hybrid search + decay
    embeddings.rs               Embedding providers (Voyage, Jina, Cohere, Nomic)

  tools/                        Built-in tools
    pdf.rs                      ReadPdfTool
    firecrawl.rs                FirecrawlTool (web crawling)
    media_tool.rs               AnalyzeImageTool, TranscribeAudioTool
    memory_tool.rs              MemoryGetTool, MemorySearchTool
    patch.rs                    ApplyPatchTool (unified diffs)
    session_tool.rs             SessionsList/History/Send/Spawn tools
    pruning.rs                  Tool result truncation

  repl/                         Interactive REPL
    mod.rs                      REPL loop + single-shot mode
    commands.rs                 Slash command parsing
    skills.rs                   Skill resolution (/name [args])

  session/                      Session persistence (JSONL transcripts in .sessions/)

web/                            React 19 + Vite 6 + Tailwind v4
  src/
    App.tsx                     Root: sidebar mode switching, agent selector, theme
    components/
      ChatPanel.tsx             Chat UI with streaming, tool calls, thinking blocks
      MessageBubble.tsx         Markdown rendering + syntax highlighting (theme-aware)
      Dashboard.tsx             Tab router: 16 dashboard pages
      Toolbar.tsx               Session/model/agent controls
      ToolOutputSidebar.tsx     JSON formatter + file listing + syntax highlighting
      dashboard/
        OverviewPage.tsx        System stats, uptime, providers, API requests
        ChannelsPage.tsx        Live status, bot channels, DM pairing management
        SessionsPage.tsx        Session list with agent filter + agent badges
        UsagePage.tsx           Token trends, cost breakdown, channel/agent distribution
        AgentsPage.tsx          Agent CRUD, bindings, broadcasts, MD preview, cron
        ConfigPage.tsx          TOML editor with section filtering per settings page
        SkillsPage.tsx          Local skills + ClawHub store
        LogsPage.tsx            Real-time log tail, LogID tracing, message stream
        SchedulesPage.tsx       Cron job management
        NodesPage.tsx           Device pairing, QR codes
        InstancesPage.tsx       Connected instances
        DebugPage.tsx           Raw RPC invocation
    hooks/
      useGatewayWS.ts           WebSocket connection management
      useDashboardAPI.ts        RPC method calls (50+ endpoints)
      useCodeTheme.ts           Light/dark syntax highlighting theme
      useTheme.ts               macOS Sequoia design system theme
    i18n/
      en.json + zh.json         All UI text (200+ keys each)
```

## Key Concepts

### Multi-Agent Routing

Messages are routed to agents via **bindings** with a priority chain:

```
peer match (100) > guild+roles (80) > guild (60) > team (50) > account (30) > channel (10) > default (0)
```

Each agent has isolated: workspace, sessions, memory (LTM), tool policy, model.

### Session Key Format

```
agent:{agent_id}:{channel}:{kind}:{peer_id}[:{extras}]

Examples:
  agent:home:lark:dm:ou_abc123              # Lark DM (per-channel-peer)
  agent:work:discord:grp:channel456         # Discord group
  agent:default:main                        # Web/REPL main session
```

### DM Session Isolation

4 levels controlling how DM sessions are keyed:

| Level | Session Key | Use Case |
|-------|-------------|----------|
| `main` | `agent:X:main` | All DMs share one session (unsafe) |
| `per_peer` | `agent:X:channel:dm:sender` | Each sender gets own session |
| `per_channel_peer` | `agent:X:channel:dm:sender` | Per channel+sender (default) |
| `per_account_channel_peer` | `agent:X:channel:account:dm:sender` | Full isolation |

### Message IR (Intermediate Representation)

```
AI Output (Markdown) → parse_markdown() → MessageIR → chunk_ir(limit) → render(target) → Platform
```

Targets: Markdown (Discord) | Lark Card JSON | Slack mrkdwn | Telegram HTML | Plain Text

### Middleware Stack (12 layers)

```
Request → Tracing → Skills → Thinking → Verbose → Fallback → LoopDetection
       → SSRF Guard → CircuitBreaker → Security → SecretMasking → Condenser → CostTracking → Model
```

### Usage Tracking (6 dimensions)

| Dimension | Example |
|-----------|---------|
| Model | gpt-4o, claude-sonnet-4 |
| Provider | openai, anthropic, ark |
| Channel | lark, telegram, webchat |
| Agent | home, work, default |
| Session | agent:home:lark:dm:ou_xxx |
| Time | daily granularity |

Persisted to `~/.synapse/usage/records.jsonl`, queryable via `usage.aggregates` RPC.

### Dashboard (16 pages, 3 sections)

```
Control:  Overview | Channels | Instances | Sessions | Usage | Schedules | Nodes
Agent:    Agents | Skills
Settings: General | Communications | Automation | Infrastructure | AI & Agents | Logs | Debug
```

## Directory Layout

```
~/.synapse/
  config.toml                   Global config
  workspace/                    Default agent workspace (SOUL.md, IDENTITY.md, ...)
  agents/
    {agent_id}/
      workspace/                Per-agent workspace
      sessions/                 Per-agent session transcripts
      memory/                   Per-agent LTM
  pairing/                      DM pairing data
  usage/
    records.jsonl               Usage tracking persistence
  logs/
    synapse.log.YYYY-MM-DD      Daily log files
```

## Development

```bash
# Type check
cargo check --features web,plugins,bot-lark
cargo clippy --features "web,plugins,bot-telegram,bot-discord,bot-slack,bot-lark" -- -D warnings

# Tests
cargo test --features web,plugins,bot-lark

# Frontend
cd web && npm run dev           # Vite HMR :5173
cd web && npx tsc --noEmit      # Type check
cd web && npx eslint src        # Lint

# Vite proxy: /api → :3000, /ws → ws://:3000
```

## Conventions

- **Rust**: `rustfmt`, `snake_case` functions, `PascalCase` types, feature-gate with `#[cfg(feature = "...")]`
- **TypeScript**: 2-space indent, `PascalCase` components, `camelCase` helpers
- **i18n**: All user-visible text must use `t("key")` — update both `en.json` and `zh.json`
- **Commits**: Conventional Commits — `feat(scope): summary`, `fix(scope): summary`
- **Logging**: `tracing::info!/warn!/error!` with structured fields, never truncate content
