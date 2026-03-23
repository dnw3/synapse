import { useCallback, useMemo } from "react";
import type {
  StatsData,
  UsageData,
  ProviderInfo,
  HealthData,
  SessionEntry,
  ScheduleEntry,
  ScheduleRunEntry,
  ConfigData,
  ChannelEntry,
  SkillEntry,
  McpServerInfo,
  McpTestResult,
  RequestMetricsResponse,
  UsageTimeseriesEntry,
  UsageSessionEntry,
  AgentEntry,
  BindingEntry,
  BroadcastGroupEntry,
  ToolCatalogGroup,
  DebugInvokeRequest,
  DebugInvokeResponse,
  DebugHealthResponse,
  WorkspaceFileEntry,
  WorkspaceFileContent,
  IdentityInfo,
  StoreSearchResult,
  StoreSkillItem,
  StoreSkillDetail,
  StoreStatus,
  PluginInfo,
} from "../types/dashboard";

export function useDashboardAPI() {
  const fetchJSON = useCallback(async <T,>(path: string): Promise<T | null> => {
    try {
      const res = await fetch(`/api/dashboard${path}`);
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  const postJSON = useCallback(async <T,>(path: string, body?: unknown): Promise<T | null> => {
    try {
      const res = await fetch(`/api/dashboard${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
      });
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  const putJSON = useCallback(async <T,>(path: string, body: unknown): Promise<T | null> => {
    try {
      const res = await fetch(`/api/dashboard${path}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  const deleteJSON = useCallback(async (path: string): Promise<boolean> => {
    try {
      const res = await fetch(`/api/dashboard${path}`, { method: "DELETE" });
      return res.ok;
    } catch {
      return false;
    }
  }, []);

  const patchJSON = useCallback(async <T,>(path: string, body: unknown): Promise<T | null> => {
    try {
      const res = await fetch(`/api/dashboard${path}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  // Core endpoints
  const fetchStats = useCallback(() => fetchJSON<StatsData>("/stats"), [fetchJSON]);
  const fetchUsage = useCallback(() => fetchJSON<UsageData>("/usage"), [fetchJSON]);
  const fetchProviders = useCallback(() => fetchJSON<ProviderInfo[]>("/providers"), [fetchJSON]);
  const fetchHealth = useCallback(() => fetchJSON<HealthData>("/health"), [fetchJSON]);
  const fetchSessions = useCallback((params?: { limit?: number; offset?: number; sort?: string; order?: string }) => {
    const qs = new URLSearchParams();
    if (params?.limit) qs.set("limit", String(params.limit));
    if (params?.offset) qs.set("offset", String(params.offset));
    if (params?.sort) qs.set("sort", params.sort);
    if (params?.order) qs.set("order", params.order);
    const q = qs.toString();
    return fetchJSON<{ sessions: SessionEntry[]; total: number }>(`/sessions${q ? `?${q}` : ""}`);
  }, [fetchJSON]);
  const fetchSchedules = useCallback(() => fetchJSON<ScheduleEntry[]>("/schedules"), [fetchJSON]);
  const fetchConfig = useCallback(() => fetchJSON<ConfigData>("/config"), [fetchJSON]);
  const fetchChannels = useCallback(() => fetchJSON<ChannelEntry[]>("/channels"), [fetchJSON]);
  const fetchSkills = useCallback(() => fetchJSON<SkillEntry[]>("/skills"), [fetchJSON]);
  // ── MCP Servers ──────────────────────────────────────────────────────────

  const fetchMcpServers = useCallback(
    () => fetchJSON<McpServerInfo[]>("/mcp"),
    [fetchJSON]
  );

  const createMcpServer = useCallback(
    (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">) =>
      postJSON<McpServerInfo>("/mcp", server),
    [postJSON]
  );

  const updateMcpServer = useCallback(
    (name: string, server: Partial<McpServerInfo>) =>
      putJSON<McpServerInfo>(`/mcp/${encodeURIComponent(name)}`, server),
    [putJSON]
  );

  const deleteMcpServer = useCallback(
    (name: string) => deleteJSON(`/mcp/${encodeURIComponent(name)}`),
    [deleteJSON]
  );

  const testMcpServer = useCallback(
    (name: string) => postJSON<McpTestResult>(`/mcp/${encodeURIComponent(name)}/test`),
    [postJSON]
  );

  const persistMcpServer = useCallback(
    (name: string) => postJSON<McpServerInfo>(`/mcp/${encodeURIComponent(name)}/persist`),
    [postJSON]
  );

  const fetchRequests = useCallback(async () => {
    const resp = await fetchJSON<RequestMetricsResponse>("/requests");
    return resp?.endpoints ?? null;
  }, [fetchJSON]);
  const fetchLogs = useCallback(async (lines = 200, level?: string): Promise<{ lines: string[]; file?: string } | null> => {
    try {
      const qs = new URLSearchParams({ lines: String(lines) });
      if (level && level !== "all") qs.set("level", level);
      const res = await fetch(`/api/dashboard/logs?${qs}`);
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  // Config
  const saveConfig = useCallback(async (content: string): Promise<boolean> => {
    try {
      const res = await fetch("/api/dashboard/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ content }),
      });
      if (!res.ok) return false;
      const data = await res.json();
      return data.success === true;
    } catch {
      return false;
    }
  }, []);

  // Usage analytics (Phase 1)
  const fetchUsageTimeseries = useCallback((from?: string, to?: string, granularity?: string) => {
    const qs = new URLSearchParams();
    if (from) qs.set("from", from);
    if (to) qs.set("to", to);
    if (granularity) qs.set("granularity", granularity);
    const q = qs.toString();
    return fetchJSON<UsageTimeseriesEntry[]>(`/usage/timeseries${q ? `?${q}` : ""}`);
  }, [fetchJSON]);

  const fetchUsageSessions = useCallback((from?: string, to?: string, sort?: string, limit?: number, offset?: number) => {
    const qs = new URLSearchParams();
    if (from) qs.set("from", from);
    if (to) qs.set("to", to);
    if (sort) qs.set("sort", sort);
    if (limit) qs.set("limit", String(limit));
    if (offset) qs.set("offset", String(offset));
    const q = qs.toString();
    return fetchJSON<UsageSessionEntry[]>(`/usage/sessions${q ? `?${q}` : ""}`);
  }, [fetchJSON]);

  // Schedules CRUD (Phase 2)
  const createSchedule = useCallback((schedule: Partial<ScheduleEntry>) =>
    postJSON<ScheduleEntry>("/schedules", schedule), [postJSON]);
  const updateSchedule = useCallback((name: string, schedule: Partial<ScheduleEntry>) =>
    putJSON<ScheduleEntry>(`/schedules/${encodeURIComponent(name)}`, schedule), [putJSON]);
  const deleteSchedule = useCallback((name: string) =>
    deleteJSON(`/schedules/${encodeURIComponent(name)}`), [deleteJSON]);
  const triggerSchedule = useCallback((name: string) =>
    postJSON<{ ok: boolean }>(`/schedules/${encodeURIComponent(name)}/trigger`), [postJSON]);
  const toggleSchedule = useCallback((name: string) =>
    postJSON<{ enabled: boolean }>(`/schedules/${encodeURIComponent(name)}/toggle`), [postJSON]);
  const fetchScheduleRuns = useCallback((name: string) =>
    fetchJSON<ScheduleRunEntry[]>(`/schedules/${encodeURIComponent(name)}/runs`), [fetchJSON]);

  // Config advanced (Phase 3)
  const fetchConfigSchema = useCallback(() => fetchJSON<Record<string, unknown>>("/config/schema"), [fetchJSON]);
  const patchConfig = useCallback((fields: Record<string, unknown>) =>
    patchJSON<{ ok: boolean }>("/config", fields), [patchJSON]);
  const validateConfig = useCallback((content: string) =>
    postJSON<{ valid: boolean; errors?: string[] }>("/config/validate", { content }), [postJSON]);
  const reloadConfig = useCallback(() =>
    postJSON<{ ok: boolean }>("/config/reload"), [postJSON]);

  // Agents (Phase 4) + Tools catalog
  const fetchAgents = useCallback(() => fetchJSON<AgentEntry[]>("/agents"), [fetchJSON]);
  const fetchToolsCatalog = useCallback(() => fetchJSON<ToolCatalogGroup[]>("/tools"), [fetchJSON]);
  const createAgent = useCallback((agent: Partial<AgentEntry>) =>
    postJSON<AgentEntry>("/agents", agent), [postJSON]);
  const updateAgent = useCallback((name: string, agent: Partial<AgentEntry>) =>
    putJSON<AgentEntry>(`/agents/${encodeURIComponent(name)}`, agent), [putJSON]);
  const deleteAgent = useCallback((name: string) =>
    deleteJSON(`/agents/${encodeURIComponent(name)}`), [deleteJSON]);

  // Bindings & Broadcasts (multi-agent)
  const fetchBindings = useCallback(async () => {
    const resp = await postJSON<DebugInvokeResponse>("/debug/invoke" as never, { method: "bindings.list", params: {} });
    return (resp?.result as { bindings: BindingEntry[] } | undefined)?.bindings ?? [];
  }, [postJSON]);
  const fetchBroadcasts = useCallback(async () => {
    const resp = await postJSON<DebugInvokeResponse>("/debug/invoke" as never, { method: "broadcasts.list", params: {} });
    return (resp?.result as { broadcasts: BroadcastGroupEntry[] } | undefined)?.broadcasts ?? [];
  }, [postJSON]);

  // Sessions advanced (Phase 5)
  const deleteSession = useCallback((id: string) =>
    deleteJSON(`/sessions/${encodeURIComponent(id)}`), [deleteJSON]);
  const renameSession = useCallback((id: string, displayName: string) =>
    patchJSON<{ ok: boolean }>(`/sessions/${encodeURIComponent(id)}`, { display_name: displayName }), [patchJSON]);
  const patchSessionOverrides = useCallback((id: string, overrides: { label?: string; thinking?: string; verbose?: string }) =>
    patchJSON<{ ok: boolean }>(`/sessions/${encodeURIComponent(id)}`, overrides), [patchJSON]);
  const compactSession = useCallback((id: string) =>
    postJSON<{ ok: boolean }>(`/sessions/${encodeURIComponent(id)}/compact`), [postJSON]);

  // Channels (Phase 6)
  const toggleChannel = useCallback((name: string) =>
    postJSON<{ enabled: boolean }>(`/channels/${encodeURIComponent(name)}/toggle`), [postJSON]);
  const updateChannelConfig = useCallback((name: string, config: Record<string, string>) =>
    putJSON<{ ok: boolean }>(`/channels/${encodeURIComponent(name)}/config`, config), [putJSON]);

  // Skills (Phase 7)
  const toggleSkill = useCallback((name: string) =>
    postJSON<{ enabled: boolean }>(`/skills/${encodeURIComponent(name)}/toggle`), [postJSON]);
  const fetchSkillFiles = useCallback((path: string) =>
    fetchJSON<{ files: { name: string; size: number }[] }>(`/skills/files?path=${encodeURIComponent(path)}`), [fetchJSON]);
  const fetchSkillFileContent = useCallback((path: string) =>
    fetchJSON<{ content: string }>(`/skills/content?path=${encodeURIComponent(path)}`), [fetchJSON]);

  // Debug (Phase 8)
  const debugInvoke = useCallback((req: DebugInvokeRequest) =>
    postJSON<DebugInvokeResponse>("/debug/invoke" as never, req), [postJSON]);
  const fetchDebugHealth = useCallback(() =>
    fetchJSON<DebugHealthResponse>("/debug/health" as never), [fetchJSON]);

  // Workspace (supports optional ?agent= param for per-agent workspaces)
  const agentQs = (agent?: string) => agent ? `?agent=${encodeURIComponent(agent)}` : "";
  const fetchWorkspaceFiles = useCallback((agent?: string) =>
    fetchJSON<WorkspaceFileEntry[]>(`/workspace${agentQs(agent)}`), [fetchJSON]);
  const fetchWorkspaceFile = useCallback((filename: string, agent?: string) =>
    fetchJSON<WorkspaceFileContent>(`/workspace/${encodeURIComponent(filename)}${agentQs(agent)}`), [fetchJSON]);
  const saveWorkspaceFile = useCallback((filename: string, content: string, agent?: string) =>
    putJSON<{ ok: boolean }>(`/workspace/${encodeURIComponent(filename)}${agentQs(agent)}`, { content }), [putJSON]);
  const createWorkspaceFile = useCallback((filename: string, content: string, agent?: string) =>
    postJSON<{ ok: boolean }>(`/workspace/${encodeURIComponent(filename)}${agentQs(agent)}`, { content }), [postJSON]);
  const deleteWorkspaceFile = useCallback((filename: string, agent?: string) =>
    deleteJSON(`/workspace/${encodeURIComponent(filename)}${agentQs(agent)}`), [deleteJSON]);
  const resetWorkspaceFile = useCallback((filename: string, agent?: string) =>
    postJSON<{ ok: boolean }>(`/workspace/${encodeURIComponent(filename)}/reset${agentQs(agent)}`), [postJSON]);
  const fetchIdentity = useCallback((agent?: string) =>
    fetchJSON<IdentityInfo>(`/identity${agentQs(agent)}`), [fetchJSON]);

  // Skill Store
  const storeSearch = useCallback((q: string, limit = 20) =>
    fetchJSON<{ results: StoreSearchResult[]; source: string }>(`/store/search?q=${encodeURIComponent(q)}&limit=${limit}`), [fetchJSON]);
  const storeList = useCallback((limit = 20, sort?: string, offset?: number) => {
    let path = `/store/skills?limit=${limit}`;
    if (sort) path += `&sort=${sort}`;
    if (offset) path += `&cursor=${offset}`;
    return fetchJSON<{ items: StoreSkillItem[]; source: string }>(path);
  }, [fetchJSON]);
  const storeInstall = useCallback((slug: string, version?: string) =>
    postJSON<{ ok: boolean }>("/store/install", { slug, version }), [postJSON]);
  const storeDetail = useCallback((slug: string) =>
    fetchJSON<StoreSkillDetail>(`/store/skills/${encodeURIComponent(slug)}`), [fetchJSON]);
  const storeFiles = useCallback((slug: string) =>
    fetchJSON<{ files: { name: string; size: number }[]; skillMd: string | null }>(`/store/skills/${encodeURIComponent(slug)}/files`), [fetchJSON]);
  const storeFileContent = useCallback((slug: string, filePath: string) =>
    fetchJSON<{ content: string | null }>(`/store/skills/${encodeURIComponent(slug)}/files/${filePath.split('/').map(encodeURIComponent).join('/')}`), [fetchJSON]);
  const storeStatus = useCallback(() =>
    fetchJSON<StoreStatus>("/store/status"), [fetchJSON]);

  // Plugins (Phase 10)
  const fetchPlugins = useCallback(
    () => fetchJSON<{ plugins: PluginInfo[] }>("/plugins"),
    [fetchJSON]
  );

  const togglePlugin = useCallback(
    (name: string, enabled: boolean) =>
      postJSON<{ ok: boolean; name: string; enabled: boolean; message?: string }>("/plugins/toggle", { name, enabled }),
    [postJSON]
  );

  const controlService = useCallback(
    (plugin: string, service: string, action: "start" | "stop") =>
      postJSON<{ ok: boolean; service: string; status: string }>("/plugins/service-control", { plugin, service, action }),
    [postJSON]
  );

  const installPlugin = useCallback(
    (path: string) =>
      postJSON<{ ok: boolean; name?: string; message?: string }>("/plugins/install", { name: path }),
    [postJSON]
  );

  const removePlugin = useCallback(
    (name: string) =>
      deleteJSON(`/plugins/${encodeURIComponent(name)}`),
    [deleteJSON]
  );

  // Logs export (Phase 9)
  const exportLogs = useCallback(async (): Promise<Blob | null> => {
    try {
      const res = await fetch("/api/dashboard/logs/export");
      if (!res.ok) return null;
      return await res.blob();
    } catch {
      return null;
    }
  }, []);

  return useMemo(() => ({
    // Core
    fetchStats, fetchUsage, fetchProviders, fetchHealth,
    fetchSessions, fetchSchedules, fetchConfig, fetchChannels,
    fetchSkills, fetchRequests, fetchLogs, saveConfig,
    // MCP Servers
    fetchMcpServers, createMcpServer, updateMcpServer, deleteMcpServer, testMcpServer, persistMcpServer,
    // Usage
    fetchUsageTimeseries, fetchUsageSessions,
    // Schedules CRUD
    createSchedule, updateSchedule, deleteSchedule, triggerSchedule, toggleSchedule, fetchScheduleRuns,
    // Config advanced
    fetchConfigSchema, patchConfig, validateConfig, reloadConfig,
    // Agents + Tools + Bindings + Broadcasts
    fetchAgents, createAgent, updateAgent, deleteAgent, fetchToolsCatalog,
    fetchBindings, fetchBroadcasts,
    // Sessions advanced
    deleteSession, renameSession, patchSessionOverrides, compactSession,
    // Channels
    toggleChannel, updateChannelConfig,
    // Skills
    toggleSkill, fetchSkillFiles, fetchSkillFileContent,
    // Store
    storeSearch, storeList, storeDetail, storeFiles, storeFileContent, storeInstall, storeStatus,
    // Debug
    debugInvoke, fetchDebugHealth,
    // Logs
    exportLogs,
    // Workspace
    fetchWorkspaceFiles, fetchWorkspaceFile, saveWorkspaceFile,
    createWorkspaceFile, deleteWorkspaceFile, resetWorkspaceFile,
    fetchIdentity,
    // Plugins
    fetchPlugins, togglePlugin, controlService, installPlugin, removePlugin,
  }), [
    fetchStats, fetchUsage, fetchProviders, fetchHealth,
    fetchSessions, fetchSchedules, fetchConfig, fetchChannels,
    fetchSkills, fetchRequests, fetchLogs, saveConfig,
    fetchMcpServers, createMcpServer, updateMcpServer, deleteMcpServer, testMcpServer, persistMcpServer,
    fetchUsageTimeseries, fetchUsageSessions,
    createSchedule, updateSchedule, deleteSchedule, triggerSchedule, toggleSchedule, fetchScheduleRuns,
    fetchConfigSchema, patchConfig, validateConfig, reloadConfig,
    fetchAgents, createAgent, updateAgent, deleteAgent, fetchToolsCatalog,
    fetchBindings, fetchBroadcasts,
    deleteSession, renameSession, patchSessionOverrides, compactSession,
    toggleChannel, updateChannelConfig,
    toggleSkill, fetchSkillFiles, fetchSkillFileContent,
    storeSearch, storeList, storeDetail, storeFiles, storeFileContent, storeInstall, storeStatus,
    debugInvoke, fetchDebugHealth,
    exportLogs,
    fetchWorkspaceFiles, fetchWorkspaceFile, saveWorkspaceFile,
    createWorkspaceFile, deleteWorkspaceFile, resetWorkspaceFile,
    fetchIdentity,
    fetchPlugins, togglePlugin, controlService, installPlugin, removePlugin,
  ]);
}
