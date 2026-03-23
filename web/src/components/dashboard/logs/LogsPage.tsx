import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  ScrollText, Search, Download, Play, Pause, RefreshCw,
  X, Link2, Clock, Timer, Activity,
} from "lucide-react";
import { EmptyState, LoadingSpinner } from "../shared";
import { TracesPage } from "../traces/TracesPage";
import { cn } from "../../../lib/cn";
import type { LogEntry, LogLevel, TimeRange } from "./logsHelpers";
import {
  LOG_LEVELS, TIME_RANGES, LOGID_TIME_OFFSETS,
  parseLogIdTime, parseLogIdMachine, isLogId, formatTimeShort,
  getLevelConfig, fetchStructuredLogs,
} from "./logsHelpers";
import type { ColumnId } from "./logsColumns";
import { loadColumnWidths, ResizeHandle } from "./logsColumns";
import { LogRow } from "./LogRow";

// ─── Tab type ─────────────────────────────────────────────────────────────────

type LogsViewTab = "logs" | "traces";

// ─── Main Page ───────────────────────────────────────────────────────────────

export default function LogsPage() {
  const { t } = useTranslation();

  const [activeTab, setActiveTab] = useState<LogsViewTab>("logs");
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [levelFilter, setLevelFilter] = useState<LogLevel>("ALL");
  const [search, setSearch] = useState("");
  const [requestIdFilter, setRequestIdFilter] = useState<string | null>(null);
  const [logidDecodedTime, setLogidDecodedTime] = useState<Date | null>(null);
  const [logidOffsetMin, setLogidOffsetMin] = useState(10);
  const [timeRange, setTimeRange] = useState<TimeRange>("all");
  const [autoFollow, setAutoFollow] = useState(true);
  const [limit, setLimit] = useState(200);
  const [colWidths, setColWidths] = useState<Record<ColumnId, number>>(loadColumnWidths);

  const scrollRef = useRef<HTMLDivElement>(null);
  const pollRef = useRef<ReturnType<typeof setInterval>>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // ─── Data loading ─────────────────────────────────────────────────────────

  const loadLogs = useCallback(async () => {
    // Compute time range at call-time (not render-time) to avoid impure render
    let apiFrom: string | undefined;
    let apiTo: string | undefined;
    if (requestIdFilter && logidDecodedTime) {
      const from = new Date(logidDecodedTime.getTime() - 5000);
      const to = new Date(logidDecodedTime.getTime() + logidOffsetMin * 60 * 1000);
      apiFrom = from.toISOString();
      apiTo = to.toISOString();
    } else if (timeRange !== "all" && timeRange !== "custom") {
      const r = TIME_RANGES.find(t => t.value === timeRange);
      if (r && r.minutes > 0) {
        const from = new Date(Date.now() - r.minutes * 60 * 1000);
        apiFrom = from.toISOString();
        apiTo = undefined;
      }
    }

    // When tracing a LogID, don't pass the LogID string as keyword —
    // the search box may still show it, but we only filter by request_id.
    const keyword = requestIdFilter ? undefined : (search || undefined);
    const data = await fetchStructuredLogs({
      limit,
      level: levelFilter !== "ALL" ? levelFilter : undefined,
      request_id: requestIdFilter || undefined,
      from: apiFrom,
      to: apiTo,
      keyword,
    });
    if (data) {
      setEntries(data.entries);
    }
    setLoading(false);
  }, [limit, levelFilter, requestIdFilter, logidDecodedTime, logidOffsetMin, timeRange, search]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    loadLogs();
  }, [loadLogs]);

  // Polling
  useEffect(() => {
    if (!autoFollow) {
      if (pollRef.current) clearInterval(pollRef.current);
      return;
    }
    pollRef.current = setInterval(loadLogs, 3000);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, [autoFollow, loadLogs]);

  // Auto-scroll
  useEffect(() => {
    if (autoFollow && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [entries, autoFollow]);

  // ─── Smart search: detect LogID ──────────────────────────────────────────

  const handleSearchChange = (value: string) => {
    setSearch(value);
    const trimmed = value.trim();
    if (isLogId(trimmed)) {
      const clean = trimmed.startsWith("req-") ? trimmed.slice(4) : trimmed;
      setRequestIdFilter(clean);
      const decoded = parseLogIdTime(clean);
      setLogidDecodedTime(decoded);
      setLogidOffsetMin(10);
      setAutoFollow(false);
    } else if (!value.trim()) {
      setRequestIdFilter(null);
      setLogidDecodedTime(null);
    }
  };

  const handleFilterRequestId = (rid: string) => {
    setRequestIdFilter(rid);
    const decoded = parseLogIdTime(rid);
    setLogidDecodedTime(decoded);
    setLogidOffsetMin(10);
    setAutoFollow(false);
    setSearch("");
  };

  const clearLogidFilter = () => {
    setRequestIdFilter(null);
    setLogidDecodedTime(null);
    setSearch("");
  };

  // ─── Export ───────────────────────────────────────────────────────────────

  const handleExport = () => {
    const lines = entries.map((e) =>
      `${e.ts} [${e.level}] ${e.request_id || "-"} ${e.target} ${e.message} ${JSON.stringify(e.fields)}`
    );
    const blob = new Blob([lines.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `synapse-logs-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  // ─── Count by level ───────────────────────────────────────────────────────

  const levelCounts: Record<string, number> = {};
  for (const e of entries) {
    const l = e.level.toUpperCase();
    levelCounts[l] = (levelCounts[l] || 0) + 1;
  }

  const isTracing = !!requestIdFilter;

  if (loading && entries.length === 0 && activeTab === "logs") return <LoadingSpinner />;

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="rounded-[var(--radius-xl)] bg-[var(--bg-content)] border border-[var(--separator)] overflow-hidden flex flex-col flex-1 min-h-0">

        {/* ── Header bar ── */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-[var(--separator)]">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-[var(--accent)]/10">
              {activeTab === "traces"
                ? <Activity className="h-4 w-4 text-[var(--accent-light)]" />
                : <ScrollText className="h-4 w-4 text-[var(--accent-light)]" />
              }
            </div>
            <div>
              <h3 className="text-[14px] font-semibold text-[var(--text-primary)] tracking-[-0.01em]">
                {activeTab === "traces" ? t("traces.title") : t("dashboard.logs", "Log Explorer")}
              </h3>
              <span className="text-[11px] font-mono tabular-nums text-[var(--text-tertiary)]">
                {activeTab === "logs" && (
                  <>
                    {entries.length} entries
                    {isTracing && logidDecodedTime && (
                      <span className="ml-2 text-[var(--accent-light)] opacity-70">
                        tracing from {formatTimeShort(logidDecodedTime)}
                      </span>
                    )}
                  </>
                )}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {/* Tab switcher */}
            <div className="flex items-center gap-0.5 bg-[var(--bg-grouped)] rounded-[var(--radius-md)] p-[3px] border border-[var(--separator)]">
              <button
                onClick={() => setActiveTab("logs")}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-[5px] rounded-md text-[11px] font-semibold transition-all cursor-pointer",
                  activeTab === "logs"
                    ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                    : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                )}
              >
                <ScrollText className="h-3 w-3" />
                {t("logs.logsTab")}
              </button>
              <button
                onClick={() => setActiveTab("traces")}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-[5px] rounded-md text-[11px] font-semibold transition-all cursor-pointer",
                  activeTab === "traces"
                    ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                    : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                )}
              >
                <Activity className="h-3 w-3" />
                {t("traces.tracesTab")}
              </button>
            </div>

            {activeTab === "logs" && (
              <>
                <div className="w-px h-5 bg-[var(--border-subtle)]/50 mx-1" />

                <select
                  value={limit}
                  onChange={(e) => setLimit(Number(e.target.value))}
                  className="text-[11px] bg-[var(--bg-grouped)] border border-[var(--separator)] rounded-[var(--radius-sm)] px-2.5 py-1.5 text-[var(--text-secondary)] focus:outline-none focus:border-[var(--accent)] cursor-pointer transition-colors"
                >
                  <option value={50}>50</option>
                  <option value={100}>100</option>
                  <option value={200}>200</option>
                  <option value={500}>500</option>
                  <option value={1000}>1000</option>
                </select>

                <div className="w-px h-5 bg-[var(--border-subtle)]/50 mx-1" />

                <button
                  onClick={() => loadLogs()}
                  className="flex items-center justify-center w-8 h-8 rounded-lg text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-all cursor-pointer"
                  title="Refresh"
                >
                  <RefreshCw className="h-3.5 w-3.5" />
                </button>

                <button
                  onClick={() => setAutoFollow((v) => !v)}
                  className={cn(
                    "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-semibold transition-all cursor-pointer border",
                    autoFollow
                      ? "bg-[var(--success)]/15 text-[var(--success)] border-[var(--success)]/25 shadow-[0_0_8px_-2px_var(--success)]/20"
                      : "text-[var(--text-tertiary)] border-[var(--border-subtle)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                  )}
                >
                  {autoFollow ? <Play className="h-3 w-3" /> : <Pause className="h-3 w-3" />}
                  {autoFollow ? t("logs.live") : t("logs.paused2")}
                </button>

                <button
                  onClick={handleExport}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-medium text-[var(--text-tertiary)] border border-[var(--border-subtle)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-all cursor-pointer"
                >
                  <Download className="h-3 w-3" />
                  {t("logs.export")}
                </button>
              </>
            )}
          </div>
        </div>

        {/* ── Traces tab view ── */}
        {activeTab === "traces" && (
          <div className="flex-1 min-h-0 overflow-y-auto">
            <TracesPage />
          </div>
        )}

        {/* ── Logs tab view ── */}
        {activeTab === "logs" && <>

        {/* ── Filter bar ── */}
        <div className="px-5 py-3 space-y-2.5 border-b border-[var(--separator)] bg-[var(--bg-grouped)]/40">
          <div className="flex flex-wrap items-center gap-3">
            {/* Level pills */}
            <div className="flex items-center gap-0.5 bg-[var(--bg-grouped)] rounded-[var(--radius-md)] p-[3px] border border-[var(--separator)]">
              {LOG_LEVELS.map((level) => {
                const active = levelFilter === level;
                const count = level === "ALL" ? entries.length : (levelCounts[level] || 0);
                const lc = level !== "ALL" ? getLevelConfig(level) : null;
                return (
                  <button
                    key={level}
                    onClick={() => setLevelFilter(level)}
                    className={cn(
                      "px-2.5 py-[5px] rounded-md text-[11px] font-bold tracking-wide transition-all cursor-pointer flex items-center gap-1.5",
                      active
                        ? lc ? cn(lc.bg, lc.color) : "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                        : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                    )}
                  >
                    {level}
                    {count > 0 && (
                      <span className={cn(
                        "text-[9px] font-mono tabular-nums px-1 py-[1px] rounded",
                        active ? "opacity-70" : "opacity-40"
                      )}>
                        {count}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>

            {/* Search */}
            <div className="relative flex-1 min-w-[240px] max-w-[420px]">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
              <input
                ref={searchInputRef}
                type="text"
                value={search}
                onChange={(e) => handleSearchChange(e.target.value)}
                placeholder="Search logs or paste LogID to trace"
                className="w-full pl-9 pr-9 py-2 rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--separator)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent-glow)] transition-all"
              />
              {search && (
                <button
                  onClick={() => { setSearch(""); clearLogidFilter(); }}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] cursor-pointer transition-colors"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          </div>

          {/* Row 2: Time range OR LogID tracing panel */}
          {isTracing && logidDecodedTime ? (
            <div className="flex items-center gap-3 py-1.5 px-3 rounded-lg bg-[var(--accent)]/[0.06] border border-[var(--accent)]/15">
              <Link2 className="h-3.5 w-3.5 text-[var(--accent-light)] shrink-0" />
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-[11px] font-mono text-[var(--accent-light)] truncate max-w-[220px]">{requestIdFilter}</span>
                <span className="text-[10px] text-[var(--accent-light)] opacity-60 shrink-0">
                  @ {formatTimeShort(logidDecodedTime)}
                </span>
                {requestIdFilter && parseLogIdMachine(requestIdFilter) && (
                  <span className="text-[10px] text-[var(--accent-light)] opacity-40 shrink-0">
                    node:{parseLogIdMachine(requestIdFilter)}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1 ml-auto shrink-0">
                <Timer className="h-3 w-3 text-[var(--accent-light)] opacity-60" />
                {LOGID_TIME_OFFSETS.map((offset) => (
                  <button
                    key={offset.minutes}
                    onClick={() => setLogidOffsetMin(offset.minutes)}
                    className={cn(
                      "px-2 py-0.5 rounded text-[10px] font-semibold transition-all cursor-pointer",
                      logidOffsetMin === offset.minutes
                        ? "bg-[var(--accent)]/20 text-[var(--accent-light)]"
                        : "text-[var(--accent-light)] opacity-50 hover:opacity-80 hover:bg-[var(--accent)]/10"
                    )}
                  >
                    {offset.label}
                  </button>
                ))}
                <div className="w-px h-4 bg-[var(--accent)]/20 mx-1" />
                <button
                  onClick={clearLogidFilter}
                  className="text-[var(--accent-light)] opacity-50 hover:text-[var(--error)] transition-colors cursor-pointer"
                  title="Clear trace"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <Clock className="h-3 w-3 text-[var(--text-tertiary)] shrink-0" />
              <div className="flex items-center gap-0.5">
                {TIME_RANGES.map((tr) => (
                  <button
                    key={tr.value}
                    onClick={() => { setTimeRange(tr.value); if (tr.value !== "all") setAutoFollow(false); }}
                    className={cn(
                      "px-2 py-1 rounded text-[10px] font-semibold transition-all cursor-pointer",
                      timeRange === tr.value
                        ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                        : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
                    )}
                  >
                    {tr.label}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* ── Column headers (resizable, drag border to resize) ── */}
        <div className="flex items-center gap-3 pl-4 pr-3 py-2 text-[10px] font-bold uppercase tracking-[0.08em] text-[var(--text-tertiary)] border-b border-[var(--separator)] bg-[var(--bg-grouped)]/30">
          <span className="w-3.5 shrink-0" />
          {!isTracing && (
            <span className="shrink-0 relative select-none" style={{ width: colWidths.traceId }}>
              Trace ID
              <ResizeHandle columnId="traceId" widths={colWidths} setWidths={setColWidths} />
            </span>
          )}
          <span className="shrink-0 relative select-none" style={{ width: colWidths.time }}>
            Time
            <ResizeHandle columnId="time" widths={colWidths} setWidths={setColWidths} />
          </span>
          <span className="shrink-0 relative select-none" style={{ width: colWidths.level }}>
            Level
            <ResizeHandle columnId="level" widths={colWidths} setWidths={setColWidths} />
          </span>
          <span className="shrink-0 relative select-none" style={{ width: colWidths.target }}>
            Target
            <ResizeHandle columnId="target" widths={colWidths} setWidths={setColWidths} />
          </span>
          <span className="flex-1">Message</span>
        </div>

        {/* ── Log entries ── */}
        {entries.length === 0 ? (
          <div className="py-16">
            <EmptyState
              icon={<ScrollText className="h-6 w-6" />}
              message={
                isTracing
                  ? `No entries for ${requestIdFilter?.slice(0, 20)}... in ${logidOffsetMin}min window`
                  : search
                  ? `No entries matching "${search}"`
                  : "No log entries captured yet"
              }
            />
          </div>
        ) : (
          <div
            ref={scrollRef}
            className="flex-1 min-h-0 overflow-y-auto overscroll-contain"
            style={{ scrollbarGutter: "stable" }}
          >
            {entries.map((entry, i) => (
              <LogRow
                key={`${entry.ts}-${i}`}
                entry={entry}
                index={i}
                onFilterRequestId={handleFilterRequestId}
                isTracing={isTracing}
                searchQuery={search}
                colWidths={colWidths}
              />
            ))}
          </div>
        )}

        {/* ── Footer ── */}
        <div className="flex items-center justify-between px-5 py-2.5 text-[10px] text-[var(--text-tertiary)] border-t border-[var(--separator)] bg-[var(--bg-grouped)]/30">
          <div className="flex items-center gap-4 font-mono tabular-nums">
            <span>{entries.length} {t("logs.shown").replace("{{count}} ", "")}</span>
            {Object.entries(levelCounts).filter(([, v]) => v > 0).map(([k, v]) => {
              const lc = getLevelConfig(k);
              return (
                <span key={k} className={lc.color} style={{ opacity: 0.6 }}>
                  {k} {v}
                </span>
              );
            })}
          </div>
          <div className="flex items-center gap-2">
            <span className={cn(
              "w-1.5 h-1.5 rounded-full",
              autoFollow ? "bg-[var(--success)] animate-pulse" : "bg-[var(--text-tertiary)]"
            )} />
            <span>{autoFollow ? t("logs.livePolling") : t("logs.paused")}</span>
          </div>
        </div>

        </>}
      </div>
    </div>
  );
}
