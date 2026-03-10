import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip,
  ResponsiveContainer, PieChart, Pie, Cell,
} from "recharts";
import {
  MessageSquare, Zap, Wifi, Clock, Server,
  Globe, Activity, Cpu, Database, Shield,
  Link2, Key, Copy, Check, RefreshCw,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type {
  StatsData, UsageData, ProviderInfo, HealthData, RequestEntry,
} from "../../types/dashboard";
import {
  StatsCard, StatusDot, SectionCard, SectionHeader,
  EmptyState, LoadingSkeleton, ChartTooltip,
  formatTokens, formatCost, formatUptime,
} from "./shared";

const MODEL_COLORS = [
  "var(--chart-1)", "var(--chart-2)", "var(--chart-3)", "var(--chart-4)",
  "var(--chart-5)", "var(--chart-6)", "var(--chart-7)", "var(--chart-8)",
];

// Module-level cache: survives component unmount/remount across tab switches
const uptimeCache = { base: 0, fetchedAt: 0 };

function getCachedUptime(): number {
  if (uptimeCache.fetchedAt === 0) return 0;
  const elapsed = Math.floor((Date.now() - uptimeCache.fetchedAt) / 1000);
  return uptimeCache.base + elapsed;
}

function setCachedUptime(serverUptime: number) {
  uptimeCache.base = serverUptime;
  uptimeCache.fetchedAt = Date.now();
}

interface OverviewPageProps {
  connected: boolean;
  conversationCount: number;
  messageCount: number;
}

export default function OverviewPage({ connected, conversationCount, messageCount }: OverviewPageProps) {
  const { t } = useTranslation();

  const api = useDashboardAPI();

  const [stats, setStats] = useState<StatsData | null>(null);
  const [usage, setUsage] = useState<UsageData | null>(null);
  const [providers, setProviders] = useState<ProviderInfo[] | null>(null);
  const [health, setHealth] = useState<HealthData | null>(null);
  const [requests, setRequests] = useState<RequestEntry[] | null>(null);
  const [loading, setLoading] = useState(true);

  // Live uptime counter — initialized from module-level cache to survive tab switches
  const [liveUptime, setLiveUptime] = useState(() => getCachedUptime());

  const loadOverview = useCallback(async () => {
    const [s, u, p, h, r] = await Promise.all([
      api.fetchStats(),
      api.fetchUsage(),
      api.fetchProviders(),
      api.fetchHealth(),
      api.fetchRequests(),
    ]);
    if (s) {
      setStats(s);
      setCachedUptime(s.uptime_secs);
      setLiveUptime(s.uptime_secs);
    }
    if (u) setUsage(u);
    if (p) setProviders(p);
    if (h) setHealth(h);
    if (r) setRequests(r);
    setLoading(false);
  }, [api]);

  // Initial load
  useEffect(() => {
    loadOverview();
  }, [loadOverview]);

  // Auto-refresh every 30s
  useEffect(() => {
    const interval = setInterval(loadOverview, 30_000);
    return () => clearInterval(interval);
  }, [loadOverview]);

  // Live uptime ticker (1s) — reads from module cache for continuity
  useEffect(() => {
    const interval = setInterval(() => {
      setLiveUptime(getCachedUptime());
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  // Gateway access info (must be before any early return)
  const [copied, setCopied] = useState<string | null>(null);
  const copyToClipboard = (text: string, key: string) => {
    navigator.clipboard.writeText(text);
    setCopied(key);
    setTimeout(() => setCopied(null), 2000);
  };

  // Derive gateway access info from current page location
  const wsUrl = `ws://${window.location.hostname}:${window.location.port || "3000"}/ws`;
  const apiBaseUrl = `${window.location.protocol}//${window.location.hostname}:${window.location.port || "3000"}/api`;

  if (loading) {
    return (
      <div className="animate-fade-in space-y-6">
        {/* Stats skeleton */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 sm:gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-[110px]" />
          ))}
        </div>
        {/* Charts skeleton */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
          <LoadingSkeleton className="h-[300px]" />
          <LoadingSkeleton className="h-[300px]" />
        </div>
        {/* Sections skeleton */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
          <LoadingSkeleton className="h-[200px]" />
          <LoadingSkeleton className="h-[200px]" />
        </div>
      </div>
    );
  }

  const totalTokens = (stats?.total_input_tokens ?? 0) + (stats?.total_output_tokens ?? 0);
  const modelData = usage?.per_model ?? [];

  // Pie chart data for model distribution
  const pieData = modelData.map((m) => ({
    name: m.model,
    value: m.input_tokens + m.output_tokens,
  }));

  // Bar chart data
  const barData = modelData.map((m) => ({
    model: m.model.length > 16 ? m.model.slice(0, 14) + "..." : m.model,
    input: m.input_tokens,
    output: m.output_tokens,
  }));

  const totalRequests = requests?.reduce((sum, r) => sum + r.total_requests, 0) ?? 0;

  return (
    <div className="animate-fade-in space-y-6">
      {/* Stats Cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 sm:gap-4">
        <StatsCard
          icon={<MessageSquare className="h-5 w-5" />}
          label={t("overview.sessions")}
          value={stats?.session_count ?? conversationCount}
          sub={t("overview.messageCount", { count: stats?.total_messages ?? messageCount })}
          accent="var(--chart-1)"
        />
        <StatsCard
          icon={<Zap className="h-5 w-5" />}
          label={t("overview.tokens")}
          value={formatTokens(totalTokens)}
          sub={`${formatCost(stats?.total_cost_usd ?? usage?.total_cost_usd ?? 0)}`}
          accent="var(--chart-2)"
        />
        <StatsCard
          icon={<Wifi className="h-5 w-5" />}
          label={t("overview.activeWs")}
          value={stats?.active_ws_sessions ?? 0}
          accent="var(--success)"
          pulse={connected}
        />
        <StatsCard
          icon={<Clock className="h-5 w-5" />}
          label={t("overview.uptime")}
          value={stats ? formatUptime(liveUptime) : "--"}
          accent="var(--warning)"
        />
      </div>

      {/* Gateway Access + Snapshot Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
        {/* Gateway Access */}
        <SectionCard>
          <SectionHeader
            icon={<Link2 className="h-4 w-4" />}
            title={t("overview.gatewayAccess")}
          />
          <div className="space-y-3">
            <GatewayField
              label="WebSocket URL"
              value={wsUrl}
              copyKey="ws"
              copied={copied}
              onCopy={copyToClipboard}
            />
            <GatewayField
              label="API Base URL"
              value={apiBaseUrl}
              copyKey="api"
              copied={copied}
              onCopy={copyToClipboard}
            />
            <GatewayField
              label={t("overview.gatewayToken")}
              value={health?.auth_enabled ? "••••••••" : t("overview.authNotEnabled")}
              copyKey="token"
              copied={copied}
              onCopy={copyToClipboard}
              masked
            />
          </div>
        </SectionCard>

        {/* Snapshot */}
        <SectionCard>
          <SectionHeader
            icon={<Activity className="h-4 w-4" />}
            title={t("overview.snapshot")}
            right={
              <button
                onClick={loadOverview}
                className="flex items-center gap-1 px-2 py-1 rounded-[var(--radius-md)] text-[10px] text-[var(--text-tertiary)] hover:text-[var(--accent)] hover:bg-[var(--accent)]/5 transition-colors cursor-pointer"
              >
                <RefreshCw className="h-3 w-3" />
                {t("overview.refresh")}
              </button>
            }
          />
          <div className="grid grid-cols-2 gap-3">
            <SnapshotItem
              label={t("overview.status")}
              value={connected ? "OK" : "Offline"}
              accent={connected ? "var(--success)" : "var(--error)"}
            />
            <SnapshotItem
              label={t("overview.uptime")}
              value={formatUptime(liveUptime)}
            />
            <SnapshotItem
              label={t("overview.version")}
              value={stats ? "v0.2.0" : "—"}
            />
            <SnapshotItem
              label={t("overview.activeWs")}
              value={String(stats?.active_ws_sessions ?? 0)}
            />
            <SnapshotItem
              label={t("overview.memoryEntries")}
              value={String(health?.memory_entries ?? 0)}
            />
            <SnapshotItem
              label={t("overview.auth")}
              value={health?.auth_enabled ? t("overview.enabled") : t("overview.disabled")}
              accent={health?.auth_enabled ? "var(--success)" : "var(--text-tertiary)"}
            />
          </div>
        </SectionCard>
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
        {/* Token Usage by Model */}
        <SectionCard>
          <SectionHeader
            icon={<Activity className="h-4 w-4" />}
            title={t("overview.tokenUsageByModel")}
          />
          {barData.length === 0 ? (
            <EmptyState
              icon={<Activity className="h-8 w-8 opacity-40" />}
              message={t("overview.noUsageData")}
            />
          ) : (
            <div className="h-[220px]">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={barData} margin={{ top: 8, right: 8, bottom: 0, left: -16 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--border-subtle)" vertical={false} />
                  <XAxis
                    dataKey="model"
                    tick={{ fontSize: 11, fill: "var(--text-tertiary)" }}
                    axisLine={false}
                    tickLine={false}
                  />
                  <YAxis
                    tick={{ fontSize: 11, fill: "var(--text-tertiary)" }}
                    axisLine={false}
                    tickLine={false}
                    tickFormatter={(v: number) => formatTokens(v)}
                  />
                  <Tooltip content={<ChartTooltip />} />
                  <Bar dataKey="input" name="Input" stackId="a" fill="var(--chart-1)" radius={[0, 0, 0, 0]} />
                  <Bar dataKey="output" name="Output" stackId="a" fill="var(--chart-2)" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          )}
        </SectionCard>

        {/* Model Distribution */}
        <SectionCard>
          <SectionHeader
            icon={<Cpu className="h-4 w-4" />}
            title={t("overview.modelDistribution")}
          />
          {pieData.length === 0 ? (
            <EmptyState
              icon={<Cpu className="h-8 w-8 opacity-40" />}
              message={t("overview.noModelData")}
            />
          ) : (
            <div className="flex items-center gap-4">
              <div className="h-[220px] flex-1">
                <ResponsiveContainer width="100%" height="100%">
                  <PieChart>
                    <Pie
                      data={pieData}
                      cx="50%"
                      cy="50%"
                      innerRadius={55}
                      outerRadius={85}
                      paddingAngle={2}
                      dataKey="value"
                    >
                      {pieData.map((_, i) => (
                        <Cell key={i} fill={MODEL_COLORS[i % MODEL_COLORS.length]} />
                      ))}
                    </Pie>
                    <Tooltip content={<ChartTooltip />} />
                  </PieChart>
                </ResponsiveContainer>
              </div>
              <div className="flex flex-col gap-1.5 min-w-[120px]">
                {pieData.map((entry, i) => (
                  <div key={entry.name} className="flex items-center gap-2 text-[11px]">
                    <span
                      className="w-2.5 h-2.5 rounded-full flex-shrink-0"
                      style={{ background: MODEL_COLORS[i % MODEL_COLORS.length] }}
                    />
                    <span className="text-[var(--text-secondary)] truncate max-w-[100px]" title={entry.name}>
                      {entry.name}
                    </span>
                    <span className="ml-auto text-[var(--text-tertiary)] font-mono tabular-nums">
                      {formatTokens(entry.value)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </SectionCard>
      </div>

      {/* Providers & Requests Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
        {/* Providers */}
        <SectionCard>
          <SectionHeader
            icon={<Globe className="h-4 w-4" />}
            title={t("overview.providers")}
            right={
              <span className="text-[11px] text-[var(--text-tertiary)] font-mono tabular-nums">
                {providers?.length ?? 0}
              </span>
            }
          />
          {!providers || providers.length === 0 ? (
            <EmptyState
              icon={<Globe className="h-8 w-8 opacity-40" />}
              message={t("overview.noProviders")}
            />
          ) : (
            <div className="space-y-2.5">
              {providers.map((p) => (
                <div
                  key={p.name}
                  className="flex items-start justify-between gap-3 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50 hover:bg-[var(--bg-surface)] transition-colors"
                >
                  <div className="flex items-start gap-2.5 min-w-0">
                    <StatusDot status="online" />
                    <div className="min-w-0">
                      <div className="text-[13px] font-medium text-[var(--text-primary)]">{p.name}</div>
                      <div className="text-[11px] text-[var(--text-tertiary)] font-mono truncate">{p.base_url}</div>
                    </div>
                  </div>
                  <span className="text-[11px] text-[var(--text-tertiary)] font-mono tabular-nums flex-shrink-0">
                    {p.models.length} {t("overview.models")}
                  </span>
                </div>
              ))}
            </div>
          )}
        </SectionCard>

        {/* API Request Stats */}
        <SectionCard>
          <SectionHeader
            icon={<Server className="h-4 w-4" />}
            title={t("overview.apiRequestStats")}
            right={
              <span className="text-[11px] text-[var(--text-tertiary)] font-mono tabular-nums">
                {totalRequests} {t("overview.total")}
              </span>
            }
          />
          {!requests || requests.length === 0 ? (
            <EmptyState
              icon={<Server className="h-8 w-8 opacity-40" />}
              message={t("overview.noRequestData")}
            />
          ) : (
            <div className="space-y-1.5 max-h-[240px] overflow-y-auto">
              {requests.map((r) => {
                const successCount = r.status_counts["2xx"] ?? 0;
                const errorCount = Object.entries(r.status_counts)
                  .filter(([k]) => k.startsWith("4") || k.startsWith("5"))
                  .reduce((sum, [, v]) => sum + v, 0);

                return (
                  <div
                    key={`${r.method}-${r.path}`}
                    className="flex items-center justify-between gap-3 p-2 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50 hover:bg-[var(--bg-surface)] transition-colors"
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      <span className={cn(
                        "text-[10px] font-bold px-1.5 py-0.5 rounded-[var(--radius-sm)]",
                        r.method === "GET" && "bg-[var(--method-get)]/10 text-[var(--method-get)]",
                        r.method === "POST" && "bg-[var(--method-post)]/10 text-[var(--method-post)]",
                        r.method === "PUT" && "bg-[var(--method-put)]/10 text-[var(--method-put)]",
                        r.method === "DELETE" && "bg-[var(--method-delete)]/10 text-[var(--method-delete)]",
                        !["GET", "POST", "PUT", "DELETE"].includes(r.method) && "bg-[var(--bg-surface)] text-[var(--text-tertiary)]",
                      )}>
                        {r.method}
                      </span>
                      <span className="text-[12px] text-[var(--text-secondary)] font-mono truncate">
                        {r.path}
                      </span>
                    </div>
                    <div className="flex items-center gap-3 flex-shrink-0">
                      <span className="text-[11px] text-[var(--text-primary)] font-mono tabular-nums">
                        {r.total_requests}
                      </span>
                      {errorCount > 0 && (
                        <span className="text-[10px] text-[var(--error)] font-mono tabular-nums">
                          {errorCount} err
                        </span>
                      )}
                      {r.avg_duration_secs != null && (
                        <span className="text-[10px] text-[var(--text-tertiary)] font-mono tabular-nums">
                          {(r.avg_duration_secs * 1000).toFixed(0)}ms
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </SectionCard>
      </div>

      {/* System Info Grid */}
      <SectionCard>
        <SectionHeader
          icon={<Database className="h-4 w-4" />}
          title={t("overview.systemInfo")}
        />
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3">
          <InfoItem
            icon={<Shield className="h-3.5 w-3.5" />}
            label={t("overview.auth")}
            value={health?.auth_enabled ? t("overview.enabled") : t("overview.disabled")}
            accent={health?.auth_enabled ? "var(--success)" : "var(--text-tertiary)"}
          />
          <InfoItem
            icon={<Database className="h-3.5 w-3.5" />}
            label={t("overview.memoryEntries")}
            value={String(health?.memory_entries ?? 0)}
          />
          <InfoItem
            icon={<Wifi className="h-3.5 w-3.5" />}
            label={t("overview.activeSessions")}
            value={String(health?.active_sessions ?? 0)}
          />
          <InfoItem
            icon={<Activity className="h-3.5 w-3.5" />}
            label={t("overview.healthStatus")}
            value={health?.status ?? "unknown"}
            accent={health?.status === "ok" ? "var(--success)" : "var(--warning)"}
          />
        </div>
      </SectionCard>
    </div>
  );
}

function InfoItem({
  icon, label, value, accent,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  accent?: string;
}) {
  return (
    <div className="flex items-center gap-2.5 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50">
      <span className="text-[var(--text-tertiary)]">{icon}</span>
      <div className="min-w-0">
        <div className="text-[10px] uppercase tracking-[0.06em] text-[var(--text-tertiary)]">{label}</div>
        <div
          className="text-[13px] font-medium tabular-nums truncate"
          style={{ color: accent || "var(--text-primary)" }}
        >
          {value}
        </div>
      </div>
    </div>
  );
}

function GatewayField({
  label, value, copyKey, copied, onCopy, masked,
}: {
  label: string;
  value: string;
  copyKey: string;
  copied: string | null;
  onCopy: (text: string, key: string) => void;
  masked?: boolean;
}) {
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 min-w-0">
        <div className="text-[10px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] mb-1">{label}</div>
        <div className="flex items-center gap-2">
          <input
            type={masked ? "password" : "text"}
            readOnly
            value={value}
            className="flex-1 min-w-0 px-2.5 py-1.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[12px] text-[var(--text-secondary)] font-mono outline-none truncate"
          />
          <button
            onClick={() => onCopy(value, copyKey)}
            className="flex-shrink-0 p-1.5 rounded-[var(--radius-md)] hover:bg-[var(--bg-surface)] transition-colors cursor-pointer text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
            title="Copy"
          >
            {copied === copyKey ? <Check className="h-3.5 w-3.5 text-[var(--success)]" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
        </div>
      </div>
    </div>
  );
}

function SnapshotItem({
  label, value, accent,
}: {
  label: string;
  value: string;
  accent?: string;
}) {
  return (
    <div className="p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50">
      <div className="text-[10px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] mb-0.5">{label}</div>
      <div
        className="text-[13px] font-medium tabular-nums"
        style={{ color: accent || "var(--text-primary)" }}
      >
        {value}
      </div>
    </div>
  );
}
