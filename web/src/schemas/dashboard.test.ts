import { describe, it, expect } from "vitest";
import {
  OkResponseSchema,
  ServiceInfoSchema,
  PluginInfoSchema,
  ProviderInfoSchema,
  StatsDataSchema,
  UsageModelEntrySchema,
  UsageDataSchema,
  HealthConfigSummarySchema,
  HealthDataSchema,
  RequestEntrySchema,
  RequestMetricsResponseSchema,
  IdentityInfoSchema,
  SessionEntrySchema,
  ScheduleEntrySchema,
  ScheduleRunEntrySchema,
  ConfigDataSchema,
  ChannelEntrySchema,
  SkillEntrySchema,
  StoreSearchResultSchema,
  StoreSkillItemSchema,
  StoreSkillDetailSchema,
  StoreStatusSchema,
  McpToolInfoSchema,
  McpServerInfoSchema,
  McpTestResultSchema,
  WorkspaceFileEntrySchema,
  WorkspaceFileContentSchema,
  UsageTimeseriesEntrySchema,
  UsageSessionEntrySchema,
  AgentEntrySchema,
  BindingEntrySchema,
  BroadcastGroupEntrySchema,
  ToolCatalogEntrySchema,
  ToolCatalogGroupSchema,
  DebugInvokeRequestSchema,
  DebugInvokeResponseSchema,
  DebugHealthResponseSchema,
} from "./dashboard";

// ===========================================================================
// Fixtures
// ===========================================================================

const validOkResponse = { ok: true };

const validServiceInfo = { id: "svc-1", status: "running" as const };

const validPluginInfo = {
  name: "memory",
  version: "1.0.0",
  description: "Long-term memory plugin",
  source: "builtin" as const,
  enabled: true,
  capabilities: ["store", "recall"],
  health: "healthy" as const,
  tools: ["memory_store"],
  interceptors: [],
  subscribers: ["on_message"],
  services: [validServiceInfo],
};

const validProviderInfo = {
  name: "openai",
  base_url: "https://api.openai.com/v1",
  models: ["gpt-4o", "gpt-4o-mini"],
};

const validStatsData = {
  session_count: 5,
  total_messages: 100,
  total_input_tokens: 50000,
  total_output_tokens: 25000,
  total_cost_usd: 1.5,
  uptime_secs: 3600,
  active_ws_sessions: 2,
};

const validUsageModelEntry = {
  model: "gpt-4o",
  input_tokens: 10000,
  output_tokens: 5000,
  cost_usd: 0.25,
  requests: 42,
};

const validUsageData = {
  per_model: [validUsageModelEntry],
  total_input_tokens: 10000,
  total_output_tokens: 5000,
  total_requests: 42,
  total_cost_usd: 0.25,
};

const validHealthConfigSummary = {
  model: "gpt-4o",
  provider: "openai",
  mcp_servers: 3,
  scheduled_jobs: 1,
  bot_channels: 2,
};

const validHealthData = {
  status: "ok",
  uptime_secs: 3600,
  auth_enabled: false,
  memory_entries: 128,
  active_sessions: 3,
};

const validRequestEntry = {
  method: "GET",
  path: "/api/health",
  total_requests: 500,
  status_counts: { "200": 490, "500": 10 },
  avg_duration_secs: 0.012,
};

const validRequestMetricsResponse = {
  endpoints: [validRequestEntry],
  llm_durations: [{ model: "gpt-4o", count: 10, avg_duration_secs: 2.5 }],
};

const validIdentityInfo = {
  name: "Synapse",
  emoji: null,
  avatar_url: null,
  theme_color: "#6366f1",
};

const validSessionEntry = {
  key: "session:abc123",
  id: "abc123",
  created_at: "2026-03-23T10:00:00Z",
  message_count: 15,
  token_count: 8000,
};

const validScheduleEntry = {
  name: "daily-digest",
  prompt: "Summarize today's activity",
  enabled: true,
};

const validScheduleRunEntry = {
  id: "run-001",
  schedule_name: "daily-digest",
  started_at: "2026-03-23T08:00:00Z",
  finished_at: "2026-03-23T08:01:00Z",
  status: "success" as const,
};

const validConfigData = {
  content: '[agent]\nmodel = "gpt-4o"',
  path: "/home/user/.synapse/synapse.toml",
};

const validChannelEntry = {
  name: "lark-main",
  platform: "lark",
  enabled: true,
  config: { app_id: "cli_xxx", app_secret: "***" },
};

const validSkillEntry = {
  name: "commit",
  description: "Create a git commit",
  path: "/home/user/.claude/skills/commit",
  user_invocable: true,
};

const validStoreSearchResult = {
  slug: "my-skill",
};

const validStoreSkillItem = {
  slug: "awesome-skill",
};

const validStoreSkillDetail = {};

const validStoreStatus = {
  configured: true,
  installedCount: 3,
  installed: ["skill-a", "skill-b", "skill-c"],
  source: "clawhub",
};

const validMcpToolInfo = {
  name: "read_file",
  prefixedName: "fs__read_file",
  description: "Read a file from disk",
};

const validMcpServerInfo = {
  name: "filesystem",
  transport: "stdio" as const,
  status: "connected" as const,
  tools: [validMcpToolInfo],
  transient: false,
};

const validMcpTestResult = {
  success: true,
  toolCount: 5,
  latencyMs: 120,
};

const validWorkspaceFileEntry = {
  filename: "synapse.toml",
  description: "Main configuration file",
  category: "config",
  icon: "cog",
  exists: true,
  size_bytes: 1024,
  modified: "2026-03-23T10:00:00Z",
  preview: "[agent]",
  is_template: false,
};

const validWorkspaceFileContent = {
  filename: "synapse.toml",
  content: '[agent]\nmodel = "gpt-4o"',
  is_template: false,
};

const validUsageTimeseriesEntry = {
  timestamp: "2026-03-23T10:00:00Z",
  input_tokens: 1000,
  output_tokens: 500,
  cost: 0.02,
  count: 3,
};

const validUsageSessionEntry = {
  session_id: "sess-001",
  input_tokens: 5000,
  output_tokens: 2500,
  cost: 0.1,
  request_count: 8,
};

const validAgentEntry = {
  name: "default",
  model: "gpt-4o",
  is_default: true,
};

const validBindingEntry = {
  agent: "default",
};

const validBroadcastGroupEntry = {
  name: "ops-team",
  agents: ["agent-a", "agent-b"],
  strategy: "round-robin",
  timeout_secs: 30,
};

const validToolCatalogEntry = {
  name: "memory_store",
  description: "Store a memory entry",
  source: "builtin",
};

const validToolCatalogGroup = {
  id: "memory",
  label: "Memory Tools",
  tools: [validToolCatalogEntry],
};

const validDebugInvokeRequest = {
  method: "health.check",
  params: { verbose: true },
};

const validDebugInvokeResponse = {
  ok: true,
};

const validDebugHealthResponse = {
  status: "ok",
  uptime_secs: 3600,
  active_connections: 2,
  active_sessions: 5,
};

// ===========================================================================
// Shared / Primitive
// ===========================================================================

describe("OkResponseSchema", () => {
  it("parses valid data", () => {
    expect(OkResponseSchema.parse(validOkResponse)).toEqual(validOkResponse);
  });
  it("rejects missing required field", () => {
    expect(() => OkResponseSchema.parse({})).toThrow();
  });
  it("strips unknown fields", () => {
    const result = OkResponseSchema.parse({ ...validOkResponse, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Plugins
// ===========================================================================

describe("ServiceInfoSchema", () => {
  it("parses valid data", () => {
    expect(ServiceInfoSchema.parse(validServiceInfo)).toEqual(validServiceInfo);
  });
  it("rejects missing required field", () => {
    const { id: _, ...rest } = validServiceInfo;
    expect(() => ServiceInfoSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ServiceInfoSchema.parse({ ...validServiceInfo, extra: 1 });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("PluginInfoSchema", () => {
  it("parses valid data", () => {
    expect(PluginInfoSchema.parse(validPluginInfo)).toEqual(validPluginInfo);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validPluginInfo;
    expect(() => PluginInfoSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = PluginInfoSchema.parse({ ...validPluginInfo, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Overview
// ===========================================================================

describe("ProviderInfoSchema", () => {
  it("parses valid data", () => {
    expect(ProviderInfoSchema.parse(validProviderInfo)).toEqual(validProviderInfo);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validProviderInfo;
    expect(() => ProviderInfoSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ProviderInfoSchema.parse({ ...validProviderInfo, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("StatsDataSchema", () => {
  it("parses valid data", () => {
    expect(StatsDataSchema.parse(validStatsData)).toEqual(validStatsData);
  });
  it("rejects missing required field", () => {
    const { session_count: _, ...rest } = validStatsData;
    expect(() => StatsDataSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = StatsDataSchema.parse({ ...validStatsData, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("UsageModelEntrySchema", () => {
  it("parses valid data", () => {
    expect(UsageModelEntrySchema.parse(validUsageModelEntry)).toEqual(validUsageModelEntry);
  });
  it("rejects missing required field", () => {
    const { model: _, ...rest } = validUsageModelEntry;
    expect(() => UsageModelEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = UsageModelEntrySchema.parse({ ...validUsageModelEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("UsageDataSchema", () => {
  it("parses valid data", () => {
    expect(UsageDataSchema.parse(validUsageData)).toEqual(validUsageData);
  });
  it("rejects missing required field", () => {
    const { per_model: _, ...rest } = validUsageData;
    expect(() => UsageDataSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = UsageDataSchema.parse({ ...validUsageData, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("HealthConfigSummarySchema", () => {
  it("parses valid data", () => {
    expect(HealthConfigSummarySchema.parse(validHealthConfigSummary)).toEqual(validHealthConfigSummary);
  });
  it("rejects missing required field", () => {
    const { model: _, ...rest } = validHealthConfigSummary;
    expect(() => HealthConfigSummarySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = HealthConfigSummarySchema.parse({ ...validHealthConfigSummary, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("HealthDataSchema", () => {
  it("parses valid data", () => {
    expect(HealthDataSchema.parse(validHealthData)).toEqual(validHealthData);
  });
  it("rejects missing required field", () => {
    const { status: _, ...rest } = validHealthData;
    expect(() => HealthDataSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = HealthDataSchema.parse({ ...validHealthData, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("RequestEntrySchema", () => {
  it("parses valid data", () => {
    expect(RequestEntrySchema.parse(validRequestEntry)).toEqual(validRequestEntry);
  });
  it("rejects missing required field", () => {
    const { method: _, ...rest } = validRequestEntry;
    expect(() => RequestEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = RequestEntrySchema.parse({ ...validRequestEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("RequestMetricsResponseSchema", () => {
  it("parses valid data", () => {
    expect(RequestMetricsResponseSchema.parse(validRequestMetricsResponse)).toEqual(validRequestMetricsResponse);
  });
  it("rejects missing required field", () => {
    const { endpoints: _, ...rest } = validRequestMetricsResponse;
    expect(() => RequestMetricsResponseSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = RequestMetricsResponseSchema.parse({ ...validRequestMetricsResponse, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("IdentityInfoSchema", () => {
  it("parses valid data", () => {
    expect(IdentityInfoSchema.parse(validIdentityInfo)).toEqual(validIdentityInfo);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validIdentityInfo;
    expect(() => IdentityInfoSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = IdentityInfoSchema.parse({ ...validIdentityInfo, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Sessions
// ===========================================================================

describe("SessionEntrySchema", () => {
  it("parses valid data", () => {
    expect(SessionEntrySchema.parse(validSessionEntry)).toEqual(validSessionEntry);
  });
  it("rejects missing required field", () => {
    const { key: _, ...rest } = validSessionEntry;
    expect(() => SessionEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = SessionEntrySchema.parse({ ...validSessionEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Schedules
// ===========================================================================

describe("ScheduleEntrySchema", () => {
  it("parses valid data", () => {
    expect(ScheduleEntrySchema.parse(validScheduleEntry)).toEqual(validScheduleEntry);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validScheduleEntry;
    expect(() => ScheduleEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ScheduleEntrySchema.parse({ ...validScheduleEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("ScheduleRunEntrySchema", () => {
  it("parses valid data", () => {
    expect(ScheduleRunEntrySchema.parse(validScheduleRunEntry)).toEqual(validScheduleRunEntry);
  });
  it("rejects missing required field", () => {
    const { id: _, ...rest } = validScheduleRunEntry;
    expect(() => ScheduleRunEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ScheduleRunEntrySchema.parse({ ...validScheduleRunEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Config
// ===========================================================================

describe("ConfigDataSchema", () => {
  it("parses valid data", () => {
    expect(ConfigDataSchema.parse(validConfigData)).toEqual(validConfigData);
  });
  it("rejects missing required field", () => {
    const { content: _, ...rest } = validConfigData;
    expect(() => ConfigDataSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ConfigDataSchema.parse({ ...validConfigData, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Channels
// ===========================================================================

describe("ChannelEntrySchema", () => {
  it("parses valid data", () => {
    expect(ChannelEntrySchema.parse(validChannelEntry)).toEqual(validChannelEntry);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validChannelEntry;
    expect(() => ChannelEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ChannelEntrySchema.parse({ ...validChannelEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Skills
// ===========================================================================

describe("SkillEntrySchema", () => {
  it("parses valid data", () => {
    expect(SkillEntrySchema.parse(validSkillEntry)).toEqual(validSkillEntry);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validSkillEntry;
    expect(() => SkillEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = SkillEntrySchema.parse({ ...validSkillEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("StoreSearchResultSchema", () => {
  it("parses valid data", () => {
    expect(StoreSearchResultSchema.parse(validStoreSearchResult)).toEqual(validStoreSearchResult);
  });
  it("rejects missing required field", () => {
    const { slug: _, ...rest } = validStoreSearchResult;
    expect(() => StoreSearchResultSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = StoreSearchResultSchema.parse({ ...validStoreSearchResult, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("StoreSkillItemSchema", () => {
  it("parses valid data", () => {
    expect(StoreSkillItemSchema.parse(validStoreSkillItem)).toEqual(validStoreSkillItem);
  });
  it("rejects missing required field", () => {
    const { slug: _, ...rest } = validStoreSkillItem;
    expect(() => StoreSkillItemSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = StoreSkillItemSchema.parse({ ...validStoreSkillItem, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("StoreSkillDetailSchema", () => {
  it("parses valid data", () => {
    expect(StoreSkillDetailSchema.parse(validStoreSkillDetail)).toEqual(validStoreSkillDetail);
  });
  it("parses with all optional nested objects", () => {
    const full = {
      skill: { slug: "test", createdAt: 1000, updatedAt: 2000 },
      owner: { handle: "user1", image: "img.png", displayName: "User One" },
      latestVersion: { version: "1.0.0", createdAt: 1000 },
      metadata: { os: ["linux"], systems: ["x86"] },
    };
    const result = StoreSkillDetailSchema.parse(full);
    expect(result.skill?.slug).toBe("test");
    expect(result.owner?.handle).toBe("user1");
  });
  it("strips unknown fields", () => {
    const result = StoreSkillDetailSchema.parse({ ...validStoreSkillDetail, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("StoreStatusSchema", () => {
  it("parses valid data", () => {
    expect(StoreStatusSchema.parse(validStoreStatus)).toEqual(validStoreStatus);
  });
  it("rejects missing required field", () => {
    const { configured: _, ...rest } = validStoreStatus;
    expect(() => StoreStatusSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = StoreStatusSchema.parse({ ...validStoreStatus, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// MCP Servers
// ===========================================================================

describe("McpToolInfoSchema", () => {
  it("parses valid data", () => {
    expect(McpToolInfoSchema.parse(validMcpToolInfo)).toEqual(validMcpToolInfo);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validMcpToolInfo;
    expect(() => McpToolInfoSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = McpToolInfoSchema.parse({ ...validMcpToolInfo, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("McpServerInfoSchema", () => {
  it("parses valid data", () => {
    expect(McpServerInfoSchema.parse(validMcpServerInfo)).toEqual(validMcpServerInfo);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validMcpServerInfo;
    expect(() => McpServerInfoSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = McpServerInfoSchema.parse({ ...validMcpServerInfo, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("McpTestResultSchema", () => {
  it("parses valid data", () => {
    expect(McpTestResultSchema.parse(validMcpTestResult)).toEqual(validMcpTestResult);
  });
  it("rejects missing required field", () => {
    const { success: _, ...rest } = validMcpTestResult;
    expect(() => McpTestResultSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = McpTestResultSchema.parse({ ...validMcpTestResult, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Workspace
// ===========================================================================

describe("WorkspaceFileEntrySchema", () => {
  it("parses valid data", () => {
    expect(WorkspaceFileEntrySchema.parse(validWorkspaceFileEntry)).toEqual(validWorkspaceFileEntry);
  });
  it("rejects missing required field", () => {
    const { filename: _, ...rest } = validWorkspaceFileEntry;
    expect(() => WorkspaceFileEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = WorkspaceFileEntrySchema.parse({ ...validWorkspaceFileEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("WorkspaceFileContentSchema", () => {
  it("parses valid data", () => {
    expect(WorkspaceFileContentSchema.parse(validWorkspaceFileContent)).toEqual(validWorkspaceFileContent);
  });
  it("rejects missing required field", () => {
    const { filename: _, ...rest } = validWorkspaceFileContent;
    expect(() => WorkspaceFileContentSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = WorkspaceFileContentSchema.parse({ ...validWorkspaceFileContent, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Usage Analytics
// ===========================================================================

describe("UsageTimeseriesEntrySchema", () => {
  it("parses valid data", () => {
    expect(UsageTimeseriesEntrySchema.parse(validUsageTimeseriesEntry)).toEqual(validUsageTimeseriesEntry);
  });
  it("rejects missing required field", () => {
    const { timestamp: _, ...rest } = validUsageTimeseriesEntry;
    expect(() => UsageTimeseriesEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = UsageTimeseriesEntrySchema.parse({ ...validUsageTimeseriesEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("UsageSessionEntrySchema", () => {
  it("parses valid data", () => {
    expect(UsageSessionEntrySchema.parse(validUsageSessionEntry)).toEqual(validUsageSessionEntry);
  });
  it("rejects missing required field", () => {
    const { session_id: _, ...rest } = validUsageSessionEntry;
    expect(() => UsageSessionEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = UsageSessionEntrySchema.parse({ ...validUsageSessionEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Agents
// ===========================================================================

describe("AgentEntrySchema", () => {
  it("parses valid data", () => {
    expect(AgentEntrySchema.parse(validAgentEntry)).toEqual(validAgentEntry);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validAgentEntry;
    expect(() => AgentEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = AgentEntrySchema.parse({ ...validAgentEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("BindingEntrySchema", () => {
  it("parses valid data", () => {
    expect(BindingEntrySchema.parse(validBindingEntry)).toEqual(validBindingEntry);
  });
  it("rejects missing required field", () => {
    const { agent: _, ...rest } = validBindingEntry;
    expect(() => BindingEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = BindingEntrySchema.parse({ ...validBindingEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("BroadcastGroupEntrySchema", () => {
  it("parses valid data", () => {
    expect(BroadcastGroupEntrySchema.parse(validBroadcastGroupEntry)).toEqual(validBroadcastGroupEntry);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validBroadcastGroupEntry;
    expect(() => BroadcastGroupEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = BroadcastGroupEntrySchema.parse({ ...validBroadcastGroupEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Tool Catalog
// ===========================================================================

describe("ToolCatalogEntrySchema", () => {
  it("parses valid data", () => {
    expect(ToolCatalogEntrySchema.parse(validToolCatalogEntry)).toEqual(validToolCatalogEntry);
  });
  it("rejects missing required field", () => {
    const { name: _, ...rest } = validToolCatalogEntry;
    expect(() => ToolCatalogEntrySchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ToolCatalogEntrySchema.parse({ ...validToolCatalogEntry, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("ToolCatalogGroupSchema", () => {
  it("parses valid data", () => {
    expect(ToolCatalogGroupSchema.parse(validToolCatalogGroup)).toEqual(validToolCatalogGroup);
  });
  it("rejects missing required field", () => {
    const { id: _, ...rest } = validToolCatalogGroup;
    expect(() => ToolCatalogGroupSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = ToolCatalogGroupSchema.parse({ ...validToolCatalogGroup, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

// ===========================================================================
// Debug
// ===========================================================================

describe("DebugInvokeRequestSchema", () => {
  it("parses valid data", () => {
    expect(DebugInvokeRequestSchema.parse(validDebugInvokeRequest)).toEqual(validDebugInvokeRequest);
  });
  it("rejects missing required field", () => {
    const { method: _, ...rest } = validDebugInvokeRequest;
    expect(() => DebugInvokeRequestSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = DebugInvokeRequestSchema.parse({ ...validDebugInvokeRequest, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("DebugInvokeResponseSchema", () => {
  it("parses valid data", () => {
    expect(DebugInvokeResponseSchema.parse(validDebugInvokeResponse)).toEqual(validDebugInvokeResponse);
  });
  it("rejects missing required field", () => {
    const { ok: _, ...rest } = validDebugInvokeResponse;
    expect(() => DebugInvokeResponseSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = DebugInvokeResponseSchema.parse({ ...validDebugInvokeResponse, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});

describe("DebugHealthResponseSchema", () => {
  it("parses valid data", () => {
    expect(DebugHealthResponseSchema.parse(validDebugHealthResponse)).toEqual(validDebugHealthResponse);
  });
  it("rejects missing required field", () => {
    const { status: _, ...rest } = validDebugHealthResponse;
    expect(() => DebugHealthResponseSchema.parse(rest)).toThrow();
  });
  it("strips unknown fields", () => {
    const result = DebugHealthResponseSchema.parse({ ...validDebugHealthResponse, extra: true });
    expect(result).not.toHaveProperty("extra");
  });
});
