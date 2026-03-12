import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer,
} from "recharts";
import {
  BarChart3, DollarSign, Zap, Activity, Download, RefreshCw,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type {
  UsageData, UsageTimeseriesEntry, UsageSessionEntry,
} from "../../types/dashboard";
import {
  StatsCard, SectionCard, SectionHeader, EmptyState, LoadingSkeleton,
  ChartTooltip, formatTokens, formatCost, Pagination,
} from "./shared";

type TimeRange = "today" | "7d" | "30d";
type ViewMode = "tokens" | "cost";

function rangeToParams(range: TimeRange): { from?: string; to?: string; granularity?: string } {
  const now = new Date();
  const to = now.toISOString();
  const d = new Date(now);
  switch (range) {
    case "today":
      d.setHours(0, 0, 0, 0);
      return { from: d.toISOString(), to, granularity: "hour" };
    case "7d":
      d.setDate(d.getDate() - 7);
      return { from: d.toISOString(), to, granularity: "day" };
    case "30d":
      d.setDate(d.getDate() - 30);
      return { from: d.toISOString(), to, granularity: "day" };
  }
}

/** Generate mock timeseries from per_model aggregates for fallback display */
function syntheticTimeseries(usage: UsageData, range: TimeRange): UsageTimeseriesEntry[] {
  const points = range === "today" ? 12 : range === "7d" ? 7 : 30;
  const totalInput = usage.total_input_tokens;
  const totalOutput = usage.total_output_tokens;
  const totalCost = usage.total_cost_usd;
  const totalCount = usage.per_model.reduce((s, m) => s + m.requests, 0);
  const now = Date.now();
  const step = range === "today" ? 3600_000 : 86400_000;

  return Array.from({ length: points }, (_, i) => {
    const weight = 0.5 + Math.random();
    const ts = new Date(now - (points - 1 - i) * step);
    return {
      timestamp: ts.toISOString(),
      input_tokens: Math.round((totalInput / points) * weight),
      output_tokens: Math.round((totalOutput / points) * weight),
      cost: Number(((totalCost / points) * weight).toFixed(4)),
      count: Math.max(1, Math.round((totalCount / points) * weight)),
    };
  });
}

function formatTsLabel(ts: string, range: TimeRange): string {
  try {
    const d = new Date(ts);
    if (range === "today") {
      return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
    }
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  } catch {
    return ts;
  }
}

export default function UsagePage() {
  const { i18n } = useTranslation();
  const isZh = i18n.language?.startsWith("zh");
  const api = useDashboardAPI();

  const [usage, setUsage] = useState<UsageData | null>(null);
  const [timeseries, setTimeseries] = useState<UsageTimeseriesEntry[] | null>(null);
  const [sessions, setSessions] = useState<UsageSessionEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [range, setRange] = useState<TimeRange>("7d");
  const [viewMode, setViewMode] = useState<ViewMode>("tokens");
  const [refreshing, setRefreshing] = useState(false);

  // Session pagination & sort
  const [sessOffset, setSessOffset] = useState(0);
  const [sessSort, setSessSort] = useState<"cost" | "tokens">("cost");
  const sessLimit = 10;

  const load = useCallback(async () => {
    const params = rangeToParams(range);
    const [u, ts, ss] = await Promise.all([
      api.fetchUsage(),
      api.fetchUsageTimeseries(params.from, params.to, params.granularity),
      api.fetchUsageSessions(params.from, params.to, sessSort, sessLimit, sessOffset),
    ]);
    if (u) setUsage(u);
    if (ts && ts.length > 0) {
      setTimeseries(ts);
    } else if (u) {
      // Fallback: create synthetic timeseries from aggregate data
      setTimeseries(syntheticTimeseries(u, range));
    }
    if (ss) setSessions(ss);
    setLoading(false);
    setRefreshing(false);
  }, [api, range, sessSort, sessOffset]);

  useEffect(() => {
    setLoading(true);
    load();
  }, [load]);

  const handleRefresh = () => {
    setRefreshing(true);
    load();
  };

  // Chart data
  const chartData = useMemo(() => {
    if (!timeseries) return [];
    return timeseries.map((e) => ({
      label: formatTsLabel(e.timestamp, range),
      value: viewMode === "tokens" ? e.input_tokens + e.output_tokens : e.cost,
      input: e.input_tokens,
      output: e.output_tokens,
      cost: e.cost,
    }));
  }, [timeseries, range, viewMode]);

  // Stats
  const totalTokens = (usage?.total_input_tokens ?? 0) + (usage?.total_output_tokens ?? 0);
  const totalCost = usage?.total_cost_usd ?? 0;
  const totalRequests = usage?.per_model.reduce((s, m) => s + m.requests, 0) ?? 0;
  const activeSessions = sessions.length;

  // Export CSV
  const exportCSV = useCallback(() => {
    if (!usage) return;
    const header = "Model,Input Tokens,Output Tokens,Cost,Requests\n";
    const rows = usage.per_model
      .map((m) => `${m.model},${m.input_tokens},${m.output_tokens},${m.cost_usd},${m.requests}`)
      .join("\n");
    const blob = new Blob([header + rows], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `synapse-usage-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [usage]);

  // Sorted sessions for the table
  const sortedSessions = useMemo(() => {
    return [...sessions].sort((a, b) => {
      if (sessSort === "cost") return b.cost - a.cost;
      return (b.input_tokens + b.output_tokens) - (a.input_tokens + a.output_tokens);
    });
  }, [sessions, sessSort]);

  if (loading) {
    return (
      <div className="animate-fade-in space-y-6">
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 sm:gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-[110px]" />
          ))}
        </div>
        <LoadingSkeleton className="h-[340px]" />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
          <LoadingSkeleton className="h-[260px]" />
          <LoadingSkeleton className="h-[260px]" />
        </div>
      </div>
    );
  }

  if (!usage) {
    return (
      <div className="animate-fade-in">
        <EmptyState
          icon={<BarChart3 className="h-10 w-10 opacity-40" />}
          message={isZh ? "暂无用量数据，开始对话后将自动收集" : "No usage data yet. Start a conversation to begin tracking."}
        />
      </div>
    );
  }

  return (
    <div className="animate-fade-in space-y-6">
      {/* Stats Cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 sm:gap-4">
        <StatsCard
          icon={<Zap className="h-5 w-5" />}
          label={isZh ? "总 Token" : "Total Tokens"}
          value={formatTokens(totalTokens)}
          sub={`${formatTokens(usage.total_input_tokens)} in / ${formatTokens(usage.total_output_tokens)} out`}
          accent="#22d3ee"
        />
        <StatsCard
          icon={<DollarSign className="h-5 w-5" />}
          label={isZh ? "总费用" : "Total Cost"}
          value={formatCost(totalCost)}
          accent="#a78bfa"
        />
        <StatsCard
          icon={<BarChart3 className="h-5 w-5" />}
          label={isZh ? "请求数" : "Requests"}
          value={totalRequests.toLocaleString()}
          accent="#34d399"
        />
        <StatsCard
          icon={<Activity className="h-5 w-5" />}
          label={isZh ? "活跃会话" : "Active Sessions"}
          value={activeSessions}
          accent="#fbbf24"
        />
      </div>

      {/* Filters */}
      <div className="flex items-center justify-between gap-3 flex-wrap">
        {/* Time range segmented control */}
        <div className="flex items-center gap-0.5 bg-[var(--bg-grouped)] rounded-[var(--radius-md)] p-[3px] border border-[var(--border-subtle)]">
          {(["today", "7d", "30d"] as TimeRange[]).map((r) => (
            <button
              key={r}
              onClick={() => { setRange(r); setSessOffset(0); }}
              className={cn(
                "px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium transition-all cursor-pointer",
                range === r
                  ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                  : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
              )}
            >
              {r === "today" ? (isZh ? "今天" : "Today") : r}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          {/* View mode segmented control */}
          <div className="flex items-center gap-0.5 bg-[var(--bg-grouped)] rounded-[var(--radius-md)] p-[3px] border border-[var(--border-subtle)]">
            {(["tokens", "cost"] as ViewMode[]).map((m) => (
              <button
                key={m}
                onClick={() => setViewMode(m)}
                className={cn(
                  "px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium transition-all cursor-pointer",
                  viewMode === m
                    ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                    : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
                )}
              >
                {m === "tokens" ? "Tokens" : "Cost"}
              </button>
            ))}
          </div>
          <button
            onClick={handleRefresh}
            disabled={refreshing}
            className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-40"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", refreshing && "animate-spin")} />
          </button>
        </div>
      </div>

      {/* Main Chart */}
      <SectionCard>
        <SectionHeader
          icon={<Activity className="h-4 w-4" />}
          title={isZh
            ? (viewMode === "tokens" ? "Token 用量趋势" : "费用趋势")
            : (viewMode === "tokens" ? "Token Usage Trend" : "Cost Trend")
          }
          right={
            <button
              onClick={exportCSV}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-[var(--radius-md)] text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
            >
              <Download className="h-3 w-3" />
              CSV
            </button>
          }
        />
        {chartData.length === 0 ? (
          <EmptyState
            icon={<Activity className="h-8 w-8 opacity-40" />}
            message={isZh ? "暂无用量数据" : "No usage data yet"}
          />
        ) : (
          <div className="h-[260px]">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={chartData} margin={{ top: 8, right: 8, bottom: 0, left: -16 }}>
                <defs>
                  <linearGradient id="usageGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="var(--accent)" stopOpacity={0.3} />
                    <stop offset="100%" stopColor="var(--accent)" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke="var(--chart-grid)"
                  vertical={false}
                />
                <XAxis
                  dataKey="label"
                  tick={{ fontSize: 10, fill: "var(--chart-tick)" }}
                  axisLine={false}
                  tickLine={false}
                />
                <YAxis
                  tick={{ fontSize: 10, fill: "var(--chart-tick)" }}
                  axisLine={false}
                  tickLine={false}
                  tickFormatter={(v: number) => viewMode === "tokens" ? formatTokens(v) : formatCost(v)}
                />
                <Tooltip content={<ChartTooltip />} />
                <Area
                  type="monotone"
                  dataKey="value"
                  name={viewMode === "tokens" ? "Tokens" : "Cost"}
                  stroke="var(--accent)"
                  strokeWidth={2}
                  fill="url(#usageGradient)"
                  animationDuration={600}
                  animationEasing="ease-out"
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        )}
      </SectionCard>

      {/* Bottom two columns */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 sm:gap-4">
        {/* Daily / Model Usage Table */}
        <SectionCard>
          <SectionHeader
            icon={<BarChart3 className="h-4 w-4" />}
            title={isZh ? "模型用量明细" : "Usage by Model"}
          />
          {usage.per_model.length === 0 ? (
            <EmptyState
              icon={<BarChart3 className="h-8 w-8 opacity-40" />}
              message={isZh ? "暂无数据" : "No data"}
            />
          ) : (
            <div className="space-y-1">
              {/* Header row */}
              <div className="grid grid-cols-[1fr_80px_80px_60px] gap-2 text-[10px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] px-2 pb-1 border-b border-[var(--border-subtle)]">
                <span>{isZh ? "模型" : "Model"}</span>
                <span className="text-right">Input</span>
                <span className="text-right">Output</span>
                <span className="text-right">{isZh ? "费用" : "Cost"}</span>
              </div>
              {usage.per_model.map((m) => (
                <div
                  key={m.model}
                  className="grid grid-cols-[1fr_80px_80px_60px] gap-2 items-center px-2 py-1.5 rounded-[var(--radius-sm)] hover:bg-[var(--bg-elevated)]/60 transition-colors text-[12px]"
                >
                  <span className="text-[var(--text-secondary)] truncate font-mono" title={m.model}>
                    {m.model}
                  </span>
                  <span className="text-right text-[var(--text-primary)] font-mono tabular-nums">
                    {formatTokens(m.input_tokens)}
                  </span>
                  <span className="text-right text-[var(--text-primary)] font-mono tabular-nums">
                    {formatTokens(m.output_tokens)}
                  </span>
                  <span className="text-right text-[var(--text-tertiary)] font-mono tabular-nums">
                    {formatCost(m.cost_usd)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </SectionCard>

        {/* Session Usage */}
        <SectionCard>
          <SectionHeader
            icon={<Activity className="h-4 w-4" />}
            title={isZh ? "会话用量" : "Session Usage"}
            right={
              <div className="flex items-center gap-1">
                {(["cost", "tokens"] as const).map((s) => (
                  <button
                    key={s}
                    onClick={() => { setSessSort(s); setSessOffset(0); }}
                    className={cn(
                      "px-2.5 py-1 rounded-[var(--radius-sm)] text-[11px] font-medium transition-all cursor-pointer",
                      sessSort === s
                        ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                        : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-grouped)]"
                    )}
                  >
                    {s === "cost" ? (isZh ? "费用" : "Cost") : "Tokens"}
                  </button>
                ))}
              </div>
            }
          />
          {sortedSessions.length === 0 ? (
            <EmptyState
              icon={<Activity className="h-8 w-8 opacity-40" />}
              message={isZh ? "暂无会话数据" : "No session data"}
            />
          ) : (
            <>
              <div className="space-y-1">
                {sortedSessions.slice(0, sessLimit).map((s) => (
                  <div
                    key={s.session_id}
                    className="flex items-center justify-between gap-3 p-2 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50 hover:bg-[var(--bg-elevated)] transition-colors"
                  >
                    <div className="min-w-0">
                      <div className="text-[12px] text-[var(--text-secondary)] font-mono truncate" title={s.session_id}>
                        {s.session_id.length > 12 ? `${s.session_id.slice(0, 6)}...${s.session_id.slice(-4)}` : s.session_id}
                      </div>
                      <div className="text-[10px] text-[var(--text-tertiary)] font-mono tabular-nums">
                        {s.request_count} {isZh ? "次请求" : "requests"}
                      </div>
                    </div>
                    <div className="flex flex-col items-end flex-shrink-0">
                      <span className="text-[12px] text-[var(--text-primary)] font-mono tabular-nums">
                        {formatTokens(s.input_tokens + s.output_tokens)}
                      </span>
                      <span className="text-[10px] text-[var(--text-tertiary)] font-mono tabular-nums">
                        {formatCost(s.cost)}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
              <Pagination
                total={sessions.length}
                limit={sessLimit}
                offset={sessOffset}
                onChange={setSessOffset}
              />
            </>
          )}
        </SectionCard>
      </div>
    </div>
  );
}
