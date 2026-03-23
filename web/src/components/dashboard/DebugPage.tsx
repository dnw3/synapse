import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Terminal, Play, Heart, Cpu, Activity, Clock, Trash2,
  Database, ChevronDown, ChevronRight,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import {
  SectionCard, SectionHeader, EmptyState, LoadingSkeleton, StatsCard,
} from "./shared";
import { formatUptime } from "../../lib/format";
import type { DebugHealthResponse, DebugInvokeResponse } from "../../types/dashboard";

interface HistoryItem {
  id: number;
  timestamp: string;
  method: string;
  params: Record<string, unknown>;
  response: DebugInvokeResponse;
}

/** Simple JSON syntax highlighting using spans */
function highlightJSON(json: string): React.ReactNode[] {
  const lines = json.split("\n");

  return lines.map((line, i) => {
    const highlighted = line
      .replace(/"([^"\\]|\\.)*"\s*:/g, (m) => `\x01KEY${m}\x01END`)
      .replace(/"([^"\\]|\\.)*"/g, (m) => `\x01STR${m}\x01END`)
      .replace(/\b(true|false)\b/g, (m) => `\x01BOOL${m}\x01END`)
      .replace(/\b(null)\b/g, (m) => `\x01NULL${m}\x01END`)
      .replace(/\b(\d+\.?\d*)\b/g, (m) => `\x01NUM${m}\x01END`);

    const segments = highlighted.split("\x01");

    return (
      <div key={i} className="leading-5">
        {segments.map((seg, j) => {
          if (seg.startsWith("KEY")) {
            return <span key={j} className="text-[var(--code-key)]">{seg.slice(3).replace(/END$/, "")}</span>;
          }
          if (seg.startsWith("STR")) {
            return <span key={j} className="text-[var(--code-string)]">{seg.slice(3).replace(/END$/, "")}</span>;
          }
          if (seg.startsWith("BOOL")) {
            return <span key={j} className="text-[var(--code-bool)]">{seg.slice(4).replace(/END$/, "")}</span>;
          }
          if (seg.startsWith("NULL")) {
            return <span key={j} className="text-[var(--code-null)]">{seg.slice(4).replace(/END$/, "")}</span>;
          }
          if (seg.startsWith("NUM")) {
            return <span key={j} className="text-[var(--code-number)]">{seg.slice(3).replace(/END$/, "")}</span>;
          }
          if (seg === "END") return null;
          return <span key={j}>{seg}</span>;
        })}
      </div>
    );
  });
}

const COMMON_METHODS = [
  "health",
  "cost_snapshot",
  "stats",
  "usage",
  "sessions",
  "config",
  "agents",
  "skills",
  "channels",
  "providers",
  "schedules",
  "models.list",
  "mcp",
  "requests",
  "version",
];

export default function DebugPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();

  const [health, setHealth] = useState<DebugHealthResponse | null>(null);
  const [healthLoading, setHealthLoading] = useState(true);

  // Invoke form
  const [method, setMethod] = useState("health");
  const [paramsText, setParamsText] = useState("{}");
  const [executing, setExecuting] = useState(false);
  const [response, setResponse] = useState<DebugInvokeResponse | null>(null);

  // History
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const nextIdRef = useRef(0);

  // Snapshots (collapsible JSON sections)
  const [snapshots, setSnapshots] = useState<Record<string, unknown>>({});
  const [snapshotOpen, setSnapshotOpen] = useState<Record<string, boolean>>({});

  // Health polling
  const pollHealth = useCallback(async () => {
    const h = await api.fetchDebugHealth();
    if (h) {
      setHealth(h);
      setSnapshots(prev => ({ ...prev, health: h }));
    }
    setHealthLoading(false);
  }, [api]);

  useEffect(() => {
    pollHealth();
    const interval = setInterval(pollHealth, 10_000);
    return () => clearInterval(interval);
  }, [pollHealth]);

  // Load additional snapshots (stats, usage, providers)
  useEffect(() => {
    const loadSnapshots = async () => {
      const [stats, providers] = await Promise.all([
        api.fetchStats(),
        api.fetchProviders(),
      ]);
      setSnapshots(prev => ({
        ...prev,
        ...(stats ? { status: stats } : {}),
        ...(providers ? { providers } : {}),
      }));
    };
    loadSnapshots();
  }, [api]);

  // Execute method
  const handleExecute = async () => {
    let params: Record<string, unknown>;
    try {
      params = JSON.parse(paramsText);
    } catch {
      setResponse({ ok: false, error: "Invalid JSON params" });
      return;
    }

    setExecuting(true);
    const result = await api.debugInvoke({ method, params });
    const resp = result ?? { ok: false, error: "Request failed" };
    setResponse(resp);

    const item: HistoryItem = {
      id: nextIdRef.current++,
      timestamp: new Date().toLocaleTimeString(),
      method,
      params,
      response: resp,
    };
    setHistory((prev) => [item, ...prev].slice(0, 50));
    setExecuting(false);
  };

  const clearHistory = () => {
    setHistory([]);
    setResponse(null);
  };

  const replayItem = (item: HistoryItem) => {
    setMethod(item.method);
    setParamsText(JSON.stringify(item.params, null, 2));
  };

  const responseJSON = response
    ? JSON.stringify(response, null, 2)
    : null;

  return (
    <div className="animate-fade-in flex flex-col h-full min-h-0 gap-6">
      {/* Health Stats Row */}
      {healthLoading ? (
        <div className="grid grid-cols-2 lg:grid-cols-5 gap-3 sm:gap-4">
          {Array.from({ length: 5 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-[110px]" />
          ))}
        </div>
      ) : health ? (
        <div className="grid grid-cols-2 lg:grid-cols-5 gap-3 sm:gap-4">
          <StatsCard
            icon={<Heart className="h-5 w-5" />}
            label={t("debug.status")}
            value={health.status}
            accent={health.status === "ok" ? "var(--success)" : "var(--error)"}
            pulse={health.status === "ok"}
          />
          <StatsCard
            icon={<Clock className="h-5 w-5" />}
            label={t("debug.uptime")}
            value={formatUptime(Math.floor(health.uptime_secs))}
            accent="var(--warning)"
          />
          <StatsCard
            icon={<Cpu className="h-5 w-5" />}
            label={t("debug.memory")}
            value={health.memory_rss_mb != null ? `${health.memory_rss_mb.toFixed(1)} MB` : "—"}
            accent="var(--chart-2)"
          />
          <StatsCard
            icon={<Activity className="h-5 w-5" />}
            label={t("debug.connections")}
            value={health.active_connections}
            accent="var(--chart-1)"
          />
          <StatsCard
            icon={<Terminal className="h-5 w-5" />}
            label={t("debug.sessions")}
            value={health.active_sessions}
            accent="var(--chart-8)"
          />
        </div>
      ) : null}

      {/* Snapshots Row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {Object.entries(snapshots).map(([key, data]) => (
          <SectionCard key={key} className="overflow-hidden">
            <button
              onClick={() => setSnapshotOpen(prev => ({ ...prev, [key]: !prev[key] }))}
              className="w-full flex items-center justify-between cursor-pointer group"
            >
              <div className="flex items-center gap-2">
                <Database className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
                <span className="text-[12px] font-medium text-[var(--text-primary)] capitalize">{key}</span>
              </div>
              {snapshotOpen[key] ? (
                <ChevronDown className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
              ) : (
                <ChevronRight className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
              )}
            </button>
            {snapshotOpen[key] && (
              <div className="mt-3 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] overflow-auto max-h-[280px]">
                <pre className="text-[11px] font-mono text-[var(--text-secondary)] leading-5 whitespace-pre-wrap">
                  {highlightJSON(JSON.stringify(data, null, 2))}
                </pre>
              </div>
            )}
          </SectionCard>
        ))}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-4 flex-1 min-h-0">
        {/* Left: Invoke Area + Response */}
        <div className="space-y-4">
          {/* Method Invoke */}
          <SectionCard>
            <SectionHeader
              icon={<Terminal className="h-4 w-4" />}
              title={t("debug.methodInvoke")}
            />
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <input
                    value={method}
                    onChange={(e) => setMethod(e.target.value)}
                    list="method-suggestions"
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] font-mono outline-none focus:border-[var(--accent)] transition-colors"
                    placeholder={t("debug.methodName")}
                  />
                  <datalist id="method-suggestions">
                    {COMMON_METHODS.map((m) => (
                      <option key={m} value={m} />
                    ))}
                  </datalist>
                </div>
                <button
                  onClick={handleExecute}
                  disabled={executing || !method.trim()}
                  className="flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[12px] font-medium hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)] transition-all cursor-pointer disabled:opacity-40"
                >
                  <Play className="h-3.5 w-3.5" />
                  {executing ? t("debug.running") : t("debug.execute")}
                </button>
              </div>
              <div>
                <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
                  {t("debug.paramsJson")}
                </label>
                <textarea
                  value={paramsText}
                  onChange={(e) => setParamsText(e.target.value)}
                  rows={4}
                  className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] font-mono outline-none focus:border-[var(--accent)] transition-colors resize-none"
                  placeholder="{}"
                />
              </div>
            </div>
          </SectionCard>

          {/* Response Viewer */}
          <SectionCard>
            <SectionHeader
              icon={<Activity className="h-4 w-4" />}
              title={t("debug.response")}
              right={
                response && (
                  <span className={cn(
                    "px-2 py-0.5 rounded-full text-[10px] font-bold",
                    response.ok
                      ? "bg-[var(--success)]/10 text-[var(--success)]"
                      : "bg-[var(--error)]/10 text-[var(--error)]"
                  )}>
                    {response.ok ? "OK" : "ERROR"}
                  </span>
                )
              }
            />
            {responseJSON ? (
              <div className="p-3 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] overflow-x-auto max-h-[400px] overflow-y-auto">
                <pre className="text-[12px] font-mono text-[var(--text-secondary)] leading-5">
                  {highlightJSON(responseJSON)}
                </pre>
              </div>
            ) : (
              <EmptyState
                icon={<Terminal className="h-8 w-8 opacity-40" />}
                message={t("debug.executeToSeeResponse")}
              />
            )}
          </SectionCard>
        </div>

        {/* Right: Request History */}
        <SectionCard className="h-fit">
          <SectionHeader
            icon={<Clock className="h-4 w-4" />}
            title={t("debug.requestHistory")}
            right={
              history.length > 0 && (
                <button
                  onClick={clearHistory}
                  className="flex items-center gap-1 px-2 py-1 rounded-[var(--radius-md)] text-[10px] text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/5 transition-colors cursor-pointer"
                >
                  <Trash2 className="h-3 w-3" />
                  {t("debug.clear")}
                </button>
              )
            }
          />
          {history.length === 0 ? (
            <EmptyState
              icon={<Clock className="h-8 w-8 opacity-40" />}
              message={t("debug.noInvocations")}
            />
          ) : (
            <div className="space-y-1.5 max-h-[500px] overflow-y-auto">
              {history.map((item) => (
                <button
                  key={item.id}
                  onClick={() => replayItem(item)}
                  className="w-full flex items-center justify-between gap-2 p-2 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50 hover:bg-[var(--bg-content)] transition-colors cursor-pointer text-left"
                >
                  <div className="min-w-0">
                    <div className="text-[12px] text-[var(--text-secondary)] font-mono truncate">
                      {item.method}
                    </div>
                    <div className="text-[10px] text-[var(--text-tertiary)]">
                      {item.timestamp}
                    </div>
                  </div>
                  <span className={cn(
                    "px-1.5 py-0.5 rounded-full text-[9px] font-bold flex-shrink-0",
                    item.response.ok
                      ? "bg-[var(--success)]/10 text-[var(--success)]"
                      : "bg-[var(--error)]/10 text-[var(--error)]"
                  )}>
                    {item.response.ok ? "OK" : "ERR"}
                  </span>
                </button>
              ))}
            </div>
          )}
        </SectionCard>
      </div>
    </div>
  );
}
