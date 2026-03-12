// Dashboard types shared across all dashboard page components

export interface StatsCardProps {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  sub?: string;
  trend?: { value: number; up: boolean };
  accent?: string;
  pulse?: boolean;
}

export interface ProviderInfo {
  name: string;
  base_url: string;
  models: string[];
}

export interface StatsData {
  session_count: number;
  total_messages: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_usd: number;
  uptime_secs: number;
  active_ws_sessions: number;
}

export interface UsageModelEntry {
  model: string;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  requests: number;
}

export interface UsageData {
  per_model: UsageModelEntry[];
  total_input_tokens: number;
  total_output_tokens: number;
  total_requests: number;
  total_cost_usd: number;
}

export interface HealthData {
  status: string;
  uptime_secs: number;
  auth_enabled: boolean;
  memory_entries: number;
  active_sessions: number;
}

export interface SessionEntry {
  id: string;
  created_at: string;
  message_count: number;
  token_count: number;
  compaction_count?: number;
  label?: string;
  title?: string;
  thinking_level?: string;
  verbose_level?: string;
  cost?: number;
  model?: string;
  updated_at?: string;
  input_tokens?: number;
  output_tokens?: number;
  cache_tokens?: number;
}

export interface ScheduleEntry {
  name: string;
  prompt: string;
  cron?: string;
  interval_secs?: number;
  enabled: boolean;
  description?: string;
}

export interface ScheduleRunEntry {
  id: string;
  schedule_name: string;
  started_at: string;
  finished_at: string | null;
  status: "success" | "error" | "running";
  result?: string;
  error?: string;
}

export interface ConfigData {
  content: string;
  path: string;
}

export interface ChannelEntry {
  name: string;
  platform: string;
  enabled: boolean;
  config: Record<string, string>;
}

export interface SkillEntry {
  name: string;
  description: string;
  path: string;
  user_invocable: boolean;
  source?: string;
  enabled?: boolean;
  eligible?: boolean;
  emoji?: string;
  homepage?: string;
  version?: string;
  missing_env?: string[];
  missing_bins?: string[];
  has_install_specs?: boolean;
}

export interface StoreSearchResult {
  score?: number;
  slug: string;
  displayName?: string;
  summary?: string;
  version?: string;
  updatedAt?: number;
}

export interface StoreSkillItem {
  slug: string;
  displayName?: string;
  summary?: string;
  tags?: string[] | Record<string, unknown>;
  stats?: {
    downloads?: number;
    stars?: number;
    versions?: number;
    installsAllTime?: number;
    installsCurrentVersion?: number;
    comments?: number;
  };
  createdAt?: number;
  updatedAt?: number;
  latestVersion?: {
    version?: string;
    createdAt?: number;
    changelog?: string;
    license?: string;
  };
  metadata?: {
    os?: string[];
    systems?: string[];
  };
}

export interface StoreSkillDetail {
  skill?: StoreSkillItem & {
    createdAt?: number;
    updatedAt?: number;
  };
  owner?: {
    handle?: string;
    image?: string;
    displayName?: string;
  };
  latestVersion?: {
    version?: string;
    createdAt?: number;
    changelog?: string;
    license?: string;
  };
  metadata?: {
    os?: string[];
    systems?: string[];
  };
}

export interface StoreStatus {
  configured: boolean;
  installedCount: number;
  installed: string[];
  source: string;
}

export interface McpEntry {
  name: string;
  transport: string;
  command?: string;
}

export interface RequestEntry {
  method: string;
  path: string;
  total_requests: number;
  status_counts: Record<string, number>;
  avg_duration_secs: number | null;
}

export interface RequestMetricsResponse {
  endpoints: RequestEntry[];
  llm_durations: { model: string; count: number; avg_duration_secs: number }[];
}

// Workspace types
export interface WorkspaceFileEntry {
  filename: string;
  description: string;
  category: string;
  icon: string;
  exists: boolean;
  size_bytes: number | null;
  modified: string | null;
  preview: string | null;
  is_template: boolean;
}

export interface WorkspaceFileContent {
  filename: string;
  content: string;
  is_template: boolean;
}

export interface IdentityInfo {
  name: string | null;
  emoji: string | null;
  avatar_url: string | null;
  theme_color: string | null;
}

// Tab system
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
  | "workspace"
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

// Usage analytics types
export interface UsageTimeseriesEntry {
  timestamp: string;
  input_tokens: number;
  output_tokens: number;
  cost: number;
  count: number;
}

export interface UsageSessionEntry {
  session_id: string;
  input_tokens: number;
  output_tokens: number;
  cost: number;
  request_count: number;
}

// Agent types
export interface AgentEntry {
  name: string;
  model: string;
  system_prompt?: string;
  channels?: string[] | null;
  skills?: string[] | null;
  is_default: boolean;
  workspace?: string;
}

// Tool catalog types
export interface ToolCatalogEntry {
  name: string;
  description: string;
  source: string;
}

export interface ToolCatalogGroup {
  id: string;
  label: string;
  tools: ToolCatalogEntry[];
}

// Debug types
export interface DebugInvokeRequest {
  method: string;
  params: Record<string, unknown>;
}

export interface DebugInvokeResponse {
  ok: boolean;
  result?: unknown;
  error?: string;
}

export interface DebugHealthResponse {
  status: string;
  uptime_secs: number;
  memory_rss_mb?: number;
  active_connections: number;
  active_sessions: number;
}
