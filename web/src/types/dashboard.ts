// Dashboard types shared across all dashboard page components
//
// Data types (API responses) are defined as Zod schemas in schemas/dashboard.ts
// and re-exported here. UI-only types remain as plain interfaces.

// --- Data types (from Zod schemas) ---
export type {
  OkResponse,
  ServiceInfo,
  PluginInfo,
  ProviderInfo,
  StatsData,
  UsageModelEntry,
  UsageData,
  HealthConfigSummary,
  HealthData,
  RequestEntry,
  RequestMetricsResponse,
  IdentityInfo,
  SessionEntry,
  ScheduleEntry,
  ScheduleRunEntry,
  ConfigData,
  ChannelEntry,
  SkillEntry,
  StoreSearchResult,
  StoreSkillItem,
  StoreSkillDetail,
  StoreStatus,
  McpToolInfo,
  McpServerInfo,
  McpTestResult,
  McpInvokeResult,
  WorkspaceFileEntry,
  WorkspaceFileContent,
  UsageTimeseriesEntry,
  UsageSessionEntry,
  AgentEntry,
  BindingEntry,
  BroadcastGroupEntry,
  ToolCatalogEntry,
  ToolCatalogGroup,
  DebugInvokeRequest,
  DebugInvokeResponse,
  DebugHealthResponse,
} from "../schemas/dashboard";

// --- UI-only types (not schematizable — contain React.ReactNode) ---

export interface StatsCardProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  sub?: string;
  trend?: { value: number; up: boolean };
  accent?: string;
  pulse?: boolean;
}

export type TabKey =
  | "overview"
  | "usage"
  | "sessions"
  | "logs"
  | "schedules"
  | "config"
  | "skills"
  | "channels"
  | "agents"
  | "instances"
  | "nodes"
  | "plugins"
  | "sandbox"
  | "mcp-servers"
  | "communications"
  | "automation"
  | "infrastructure"
  | "ai-agents"
  | "debug";

export interface TabDef {
  key: TabKey;
  labelZh: string;
  labelEn: string;
  icon: React.ReactNode;
}

export interface SidebarSection {
  labelZh: string;
  labelEn: string;
  keys: TabKey[];
}
