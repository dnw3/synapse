import { z } from "zod";

// ---------------------------------------------------------------------------
// Shared / primitive
// ---------------------------------------------------------------------------

export const OkResponseSchema = z.object({ ok: z.boolean() });
export type OkResponse = z.infer<typeof OkResponseSchema>;

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

export const ServiceInfoSchema = z.object({
  id: z.string(),
  status: z.enum(["running", "stopped", "error", "unknown"]),
});
export type ServiceInfo = z.infer<typeof ServiceInfoSchema>;

export const PluginInfoSchema = z.object({
  name: z.string(),
  version: z.string(),
  description: z.string(),
  author: z.string().nullable().optional(),
  source: z.enum(["builtin", "workspace", "global"]),
  enabled: z.boolean(),
  slot: z.string().nullable().optional(),
  capabilities: z.array(z.string()),
  health: z.enum(["healthy", "degraded", "error", "unknown"]),
  tools: z.array(z.string()),
  interceptors: z.array(z.string()),
  subscribers: z.array(z.string()),
  services: z.array(ServiceInfoSchema),
});
export type PluginInfo = z.infer<typeof PluginInfoSchema>;

// ---------------------------------------------------------------------------
// Overview
// ---------------------------------------------------------------------------

export const ProviderInfoSchema = z.object({
  name: z.string(),
  base_url: z.string(),
  models: z.array(z.string()),
});
export type ProviderInfo = z.infer<typeof ProviderInfoSchema>;

export const StatsDataSchema = z.object({
  session_count: z.number(),
  total_messages: z.number(),
  total_input_tokens: z.number(),
  total_output_tokens: z.number(),
  total_cost_usd: z.number(),
  uptime_secs: z.number(),
  active_ws_sessions: z.number(),
});
export type StatsData = z.infer<typeof StatsDataSchema>;

export const UsageModelEntrySchema = z.object({
  model: z.string(),
  input_tokens: z.number(),
  output_tokens: z.number(),
  cost_usd: z.number(),
  requests: z.number(),
});
export type UsageModelEntry = z.infer<typeof UsageModelEntrySchema>;

export const UsageDataSchema = z.object({
  per_model: z.array(UsageModelEntrySchema),
  total_input_tokens: z.number(),
  total_output_tokens: z.number(),
  total_requests: z.number(),
  total_cost_usd: z.number(),
});
export type UsageData = z.infer<typeof UsageDataSchema>;

export const HealthConfigSummarySchema = z.object({
  model: z.string(),
  provider: z.string(),
  mcp_servers: z.number(),
  scheduled_jobs: z.number(),
  bot_channels: z.number(),
});
export type HealthConfigSummary = z.infer<typeof HealthConfigSummarySchema>;

export const HealthDataSchema = z.object({
  status: z.string(),
  uptime_secs: z.number(),
  auth_enabled: z.boolean(),
  memory_entries: z.number(),
  active_sessions: z.number(),
  session_count: z.number().nullable().optional(),
  config_summary: HealthConfigSummarySchema.nullable().optional(),
});
export type HealthData = z.infer<typeof HealthDataSchema>;

export const RequestEntrySchema = z.object({
  method: z.string(),
  path: z.string(),
  total_requests: z.number(),
  status_counts: z.record(z.string(), z.number()),
  avg_duration_secs: z.number().nullable(),
});
export type RequestEntry = z.infer<typeof RequestEntrySchema>;

export const RequestMetricsResponseSchema = z.object({
  endpoints: z.array(RequestEntrySchema),
  llm_durations: z.array(
    z.object({
      model: z.string(),
      count: z.number(),
      avg_duration_secs: z.number(),
    })
  ),
});
export type RequestMetricsResponse = z.infer<typeof RequestMetricsResponseSchema>;

export const IdentityInfoSchema = z.object({
  name: z.string().nullable(),
  emoji: z.string().nullable(),
  avatar_url: z.string().nullable(),
  theme_color: z.string().nullable(),
});
export type IdentityInfo = z.infer<typeof IdentityInfoSchema>;

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

export const SessionEntrySchema = z.object({
  key: z.string(),
  id: z.string().nullable().optional(),
  created_at: z.string(),
  updated_at: z.string().nullable().optional(),
  channel: z.string().nullable().optional(),
  kind: z.string().nullable().optional(),
  display_name: z.string().nullable().optional(),
  message_count: z.number(),
  token_count: z.number(),
  compaction_count: z.number().nullable().optional(),
  label: z.string().nullable().optional(),
  title: z.string().nullable().optional(),
  thinking_level: z.string().nullable().optional(),
  verbose_level: z.string().nullable().optional(),
  cost: z.number().nullable().optional(),
  model: z.string().nullable().optional(),
  input_tokens: z.number().nullable().optional(),
  output_tokens: z.number().nullable().optional(),
  cache_tokens: z.number().nullable().optional(),
});
export type SessionEntry = z.infer<typeof SessionEntrySchema>;

// ---------------------------------------------------------------------------
// Schedules
// ---------------------------------------------------------------------------

export const ScheduleEntrySchema = z.object({
  name: z.string(),
  prompt: z.string(),
  cron: z.string().nullable().optional(),
  interval_secs: z.number().nullable().optional(),
  enabled: z.boolean(),
  description: z.string().nullable().optional(),
});
export type ScheduleEntry = z.infer<typeof ScheduleEntrySchema>;

export const ScheduleRunEntrySchema = z.object({
  id: z.string(),
  schedule_name: z.string(),
  started_at: z.string(),
  finished_at: z.string().nullable(),
  status: z.enum(["success", "error", "running"]),
  result: z.string().nullable().optional(),
  error: z.string().nullable().optional(),
});
export type ScheduleRunEntry = z.infer<typeof ScheduleRunEntrySchema>;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export const ConfigDataSchema = z.object({
  content: z.string(),
  path: z.string(),
});
export type ConfigData = z.infer<typeof ConfigDataSchema>;

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

export const ChannelEntrySchema = z.object({
  name: z.string(),
  platform: z.string().nullable().optional(),
  enabled: z.boolean(),
  config: z.record(z.string(), z.string()),
});
export type ChannelEntry = z.infer<typeof ChannelEntrySchema>;

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

export const SkillEntrySchema = z.object({
  name: z.string(),
  description: z.string(),
  path: z.string(),
  user_invocable: z.boolean(),
  source: z.string().nullable().optional(),
  enabled: z.boolean().nullable().optional(),
  eligible: z.boolean().nullable().optional(),
  emoji: z.string().nullable().optional(),
  homepage: z.string().nullable().optional(),
  version: z.string().nullable().optional(),
  missing_env: z.array(z.string()).nullable().optional(),
  missing_bins: z.array(z.string()).nullable().optional(),
  has_install_specs: z.boolean().nullable().optional(),
});
export type SkillEntry = z.infer<typeof SkillEntrySchema>;

export const StoreSearchResultSchema = z.object({
  score: z.number().nullable().optional(),
  slug: z.string(),
  displayName: z.string().nullable().optional(),
  summary: z.string().nullable().optional(),
  version: z.string().nullable().optional(),
  updatedAt: z.number().nullable().optional(),
});
export type StoreSearchResult = z.infer<typeof StoreSearchResultSchema>;

export const StoreSkillItemSchema = z.object({
  slug: z.string(),
  displayName: z.string().nullable().optional(),
  summary: z.string().nullable().optional(),
  tags: z.union([z.array(z.string()), z.record(z.string(), z.unknown())]).nullable().optional(),
  stats: z
    .object({
      downloads: z.number().nullable().optional(),
      stars: z.number().nullable().optional(),
      versions: z.number().nullable().optional(),
      installsAllTime: z.number().nullable().optional(),
      installsCurrentVersion: z.number().nullable().optional(),
      comments: z.number().nullable().optional(),
    })
    .nullable().optional(),
  createdAt: z.number().nullable().optional(),
  updatedAt: z.number().nullable().optional(),
  latestVersion: z
    .object({
      version: z.string().nullable().optional(),
      createdAt: z.number().nullable().optional(),
      changelog: z.string().nullable().optional(),
      license: z.string().nullable().optional(),
    })
    .nullable().optional(),
  metadata: z
    .object({
      os: z.array(z.string()).nullable().optional(),
      systems: z.array(z.string()).nullable().optional(),
    })
    .nullable().optional(),
});
export type StoreSkillItem = z.infer<typeof StoreSkillItemSchema>;

export const StoreSkillDetailSchema = z.object({
  skill: StoreSkillItemSchema.extend({
    createdAt: z.number().nullable().optional(),
    updatedAt: z.number().nullable().optional(),
  }).nullable().optional(),
  owner: z
    .object({
      handle: z.string().nullable().optional(),
      image: z.string().nullable().optional(),
      displayName: z.string().nullable().optional(),
    })
    .nullable().optional(),
  latestVersion: z
    .object({
      version: z.string().nullable().optional(),
      createdAt: z.number().nullable().optional(),
      changelog: z.string().nullable().optional(),
      license: z.string().nullable().optional(),
    })
    .nullable().optional(),
  metadata: z
    .object({
      os: z.array(z.string()).nullable().optional(),
      systems: z.array(z.string()).nullable().optional(),
    })
    .nullable().optional(),
});
export type StoreSkillDetail = z.infer<typeof StoreSkillDetailSchema>;

export const StoreStatusSchema = z.object({
  configured: z.boolean(),
  installedCount: z.number(),
  installed: z.array(z.string()),
  source: z.string(),
});
export type StoreStatus = z.infer<typeof StoreStatusSchema>;

// ---------------------------------------------------------------------------
// MCP Servers
// ---------------------------------------------------------------------------

export const McpToolInfoSchema = z.object({
  name: z.string(),
  prefixedName: z.string(),
  description: z.string(),
  parameters: z.record(z.string(), z.unknown()).nullable().optional(),
});
export type McpToolInfo = z.infer<typeof McpToolInfoSchema>;

export const McpServerInfoSchema = z.object({
  name: z.string(),
  transport: z.enum(["stdio", "sse", "streamable-http"]),
  command: z.string().nullable().optional(),
  args: z.array(z.string()).nullable().optional(),
  env: z.record(z.string(), z.string()).nullable().optional(),
  url: z.string().nullable().optional(),
  headers: z.record(z.string(), z.string()).nullable().optional(),
  status: z.enum(["connected", "disconnected", "error", "unknown"]),
  tools: z.array(McpToolInfoSchema),
  lastChecked: z.string().nullable().optional(),
  error: z.string().nullable().optional(),
  transient: z.boolean(),
});
export type McpServerInfo = z.infer<typeof McpServerInfoSchema>;

export const McpTestResultSchema = z.object({
  success: z.boolean(),
  toolCount: z.number(),
  latencyMs: z.number(),
  error: z.string().nullable().optional(),
});
export type McpTestResult = z.infer<typeof McpTestResultSchema>;

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

export const WorkspaceFileEntrySchema = z.object({
  filename: z.string(),
  description: z.string(),
  category: z.string(),
  icon: z.string(),
  exists: z.boolean(),
  size_bytes: z.number().nullable(),
  modified: z.string().nullable(),
  preview: z.string().nullable(),
  is_template: z.boolean(),
});
export type WorkspaceFileEntry = z.infer<typeof WorkspaceFileEntrySchema>;

export const WorkspaceFileContentSchema = z.object({
  filename: z.string(),
  content: z.string(),
  is_template: z.boolean(),
});
export type WorkspaceFileContent = z.infer<typeof WorkspaceFileContentSchema>;

// ---------------------------------------------------------------------------
// Usage analytics
// ---------------------------------------------------------------------------

export const UsageTimeseriesEntrySchema = z.object({
  timestamp: z.string(),
  input_tokens: z.number(),
  output_tokens: z.number(),
  cost: z.number(),
  count: z.number(),
});
export type UsageTimeseriesEntry = z.infer<typeof UsageTimeseriesEntrySchema>;

export const UsageSessionEntrySchema = z.object({
  session_id: z.string(),
  input_tokens: z.number(),
  output_tokens: z.number(),
  cost: z.number(),
  request_count: z.number(),
});
export type UsageSessionEntry = z.infer<typeof UsageSessionEntrySchema>;

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

export const AgentEntrySchema = z.object({
  name: z.string(),
  id: z.string().nullable().optional(),
  model: z.string(),
  description: z.string().nullable().optional(),
  system_prompt: z.string().nullable().optional(),
  channels: z.array(z.string()).nullable().optional(),
  skills: z.array(z.string()).nullable().optional(),
  is_default: z.boolean(),
  workspace: z.string().nullable().optional(),
  dm_scope: z.string().nullable().optional(),
  group_session_scope: z.string().nullable().optional(),
  tool_allow: z.array(z.string()).nullable().optional(),
  tool_deny: z.array(z.string()).nullable().optional(),
  skills_dir: z.string().nullable().optional(),
});
export type AgentEntry = z.infer<typeof AgentEntrySchema>;

export const BindingEntrySchema = z.object({
  agent: z.string(),
  channel: z.string().nullable().optional(),
  account_id: z.string().nullable().optional(),
  peer: z
    .object({ kind: z.string(), id: z.string() })
    .nullable()
    .nullable().optional(),
  guild_id: z.string().nullable().optional(),
  team_id: z.string().nullable().optional(),
  roles: z.array(z.string()).nullable().optional(),
  comment: z.string().nullable().optional(),
});
export type BindingEntry = z.infer<typeof BindingEntrySchema>;

export const BroadcastGroupEntrySchema = z.object({
  name: z.string(),
  description: z.string().nullable().optional(),
  channel: z.string().nullable().optional(),
  peer_id: z.string().nullable().optional(),
  agents: z.array(z.string()),
  strategy: z.string(),
  timeout_secs: z.number(),
});
export type BroadcastGroupEntry = z.infer<typeof BroadcastGroupEntrySchema>;

// ---------------------------------------------------------------------------
// Tool catalog
// ---------------------------------------------------------------------------

export const ToolCatalogEntrySchema = z.object({
  name: z.string(),
  description: z.string(),
  source: z.string(),
});
export type ToolCatalogEntry = z.infer<typeof ToolCatalogEntrySchema>;

export const ToolCatalogGroupSchema = z.object({
  id: z.string(),
  label: z.string(),
  tools: z.array(ToolCatalogEntrySchema),
});
export type ToolCatalogGroup = z.infer<typeof ToolCatalogGroupSchema>;

// ---------------------------------------------------------------------------
// Debug
// ---------------------------------------------------------------------------

export const DebugInvokeRequestSchema = z.object({
  method: z.string(),
  params: z.record(z.string(), z.unknown()),
});
export type DebugInvokeRequest = z.infer<typeof DebugInvokeRequestSchema>;

export const DebugInvokeResponseSchema = z.object({
  ok: z.boolean(),
  result: z.unknown().nullable().optional(),
  error: z.string().nullable().optional(),
});
export type DebugInvokeResponse = z.infer<typeof DebugInvokeResponseSchema>;

export const DebugHealthResponseSchema = z.object({
  status: z.string(),
  uptime_secs: z.number(),
  memory_rss_mb: z.number().nullable().optional(),
  active_connections: z.number(),
  active_sessions: z.number(),
});
export type DebugHealthResponse = z.infer<typeof DebugHealthResponseSchema>;
