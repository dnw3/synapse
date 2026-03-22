import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  ScrollText, Search, Download, Play, Pause, RefreshCw,
  ChevronDown, ChevronRight, X, Link2, Clock, Timer,
  AlertTriangle, AlertCircle, Info, Bug, Radio,
  ArrowDownLeft, ArrowUpRight, MessageSquare, Filter, Activity,
} from "lucide-react";
import { EmptyState, LoadingSpinner } from "./shared";
import { TracesPage } from "./traces/TracesPage";
import { cn } from "../../lib/cn";

// ─── Types ───────────────────────────────────────────────────────────────────

interface LogEntry {
  ts: string;
  level: string;
  request_id: string | null;
  target: string;
  message: string;
  fields: Record<string, unknown>;
}

type LogLevel = "ALL" | "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE";
type TimeRange = "5m" | "15m" | "30m" | "1h" | "all" | "custom";

// ─── Resizable Columns ──────────────────────────────────────────────────────

type ColumnId = "time" | "level" | "traceId" | "target";

const COLUMN_DEFAULTS: Record<ColumnId, { min: number; max: number; default: number }> = {
  time:    { min: 60, max: 180, default: 86 },
  level:   { min: 44, max: 100, default: 52 },
  traceId: { min: 60, max: 200, default: 90 },
  target:  { min: 60, max: 300, default: 100 },
};

const STORAGE_KEY = "synapse-log-col-widths";

function loadColumnWidths(): Record<ColumnId, number> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      const result = {} as Record<ColumnId, number>;
      for (const [k, def] of Object.entries(COLUMN_DEFAULTS)) {
        const v = parsed[k];
        result[k as ColumnId] = typeof v === "number" ? Math.max(def.min, Math.min(def.max, v)) : def.default;
      }
      return result;
    }
  } catch { /* ignore */ }
  const result = {} as Record<ColumnId, number>;
  for (const [k, def] of Object.entries(COLUMN_DEFAULTS)) {
    result[k as ColumnId] = def.default;
  }
  return result;
}

function saveColumnWidths(widths: Record<ColumnId, number>) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(widths)); } catch { /* ignore */ }
}

/** Drag handle for column resize */
function ResizeHandle({ columnId, widths, setWidths }: {
  columnId: ColumnId;
  widths: Record<ColumnId, number>;
  setWidths: React.Dispatch<React.SetStateAction<Record<ColumnId, number>>>;
}) {
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = widths[columnId];
    const { min, max } = COLUMN_DEFAULTS[columnId];

    const onMove = (ev: MouseEvent) => {
      const delta = ev.clientX - startX;
      const newW = Math.max(min, Math.min(max, startW + delta));
      setWidths(prev => {
        const next = { ...prev, [columnId]: newW };
        saveColumnWidths(next);
        return next;
      });
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [columnId, widths, setWidths]);

  return (
    <div
      onMouseDown={handleMouseDown}
      className="absolute right-0 top-0 bottom-0 w-[5px] cursor-col-resize z-10 group/handle hover:bg-[var(--accent)]/20 active:bg-[var(--accent)]/30 transition-colors"
    >
      <div className="absolute right-[2px] top-[4px] bottom-[4px] w-[1px] bg-[var(--border-subtle)]/0 group-hover/handle:bg-[var(--accent)]/40 transition-colors" />
    </div>
  );
}

// ─── Message Stream Types ────────────────────────────────────────────────────

type MessageDirection = "in" | "out";

interface ChannelMessage {
  id: string;
  direction: MessageDirection;
  channel: string;
  sessionKey?: string;
  contentPreview: string;
  timestampMs: number;
  requestId?: string;
  to?: string;
}

/** Channel → badge color classes */
const CHANNEL_COLORS: Record<string, { bg: string; text: string; border: string }> = {
  slack:     { bg: "bg-purple-500/15",   text: "text-purple-400",  border: "border-purple-500/25" },
  telegram:  { bg: "bg-blue-500/15",     text: "text-blue-400",    border: "border-blue-500/25" },
  lark:      { bg: "bg-green-500/15",    text: "text-green-400",   border: "border-green-500/25" },
  discord:   { bg: "bg-indigo-500/15",   text: "text-indigo-400",  border: "border-indigo-500/25" },
  webchat:   { bg: "bg-[var(--bg-content)]/60", text: "text-[var(--text-secondary)]", border: "border-[var(--border-subtle)]/40" },
  dingtalk:  { bg: "bg-orange-500/15",   text: "text-orange-400",  border: "border-orange-500/25" },
  whatsapp:  { bg: "bg-emerald-500/15",  text: "text-emerald-400", border: "border-emerald-500/25" },
  line:      { bg: "bg-lime-500/15",     text: "text-lime-400",    border: "border-lime-500/25" },
  mattermost:{ bg: "bg-cyan-500/15",     text: "text-cyan-400",    border: "border-cyan-500/25" },
  wechat:    { bg: "bg-green-600/15",    text: "text-green-500",   border: "border-green-600/25" },
};

function getChannelColors(channel: string) {
  const key = channel.toLowerCase();
  return CHANNEL_COLORS[key] ?? { bg: "bg-[var(--accent)]/10", text: "text-[var(--accent-light)]", border: "border-[var(--accent)]/20" };
}

const MAX_MESSAGES = 200;

/** Ring-buffer push: keep last N items */
function ringPush<T>(arr: T[], item: T, maxLen: number): T[] {
  const next = [...arr, item];
  return next.length > maxLen ? next.slice(next.length - maxLen) : next;
}

/** Hook: subscribe to message.received / message.sent via a dashboard WebSocket. */
function useMessageStream() {
  const [messages, setMessages] = useState<ChannelMessage[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const unmountedRef = useRef(false);

  const connect = useCallback(() => {
    if (unmountedRef.current) return;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/_dashboard_events`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      if (!unmountedRef.current) setConnected(true);
    };

    ws.onclose = () => {
      if (unmountedRef.current) return;
      setConnected(false);
      reconnectRef.current = setTimeout(connect, 3000);
    };

    ws.onerror = () => { /* onclose fires after */ };

    ws.onmessage = (evt) => {
      if (unmountedRef.current) return;
      try {
        const frame = JSON.parse(evt.data as string);
        if (frame.type !== "event") return;
        const { event, payload } = frame as { event: string; payload: Record<string, unknown> };

        if (event === "message.received") {
          const msg: ChannelMessage = {
            id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            direction: "in",
            channel: String(payload.channel ?? "unknown"),
            sessionKey: payload.session_key ? String(payload.session_key) : undefined,
            contentPreview: String(payload.content_preview ?? ""),
            timestampMs: typeof payload.timestamp_ms === "number" ? payload.timestamp_ms : Date.now(),
            requestId: payload.request_id ? String(payload.request_id) : undefined,
            to: payload.to ? String(payload.to) : undefined,
          };
          setMessages(prev => ringPush(prev, msg, MAX_MESSAGES));
        } else if (event === "message.sent") {
          const msg: ChannelMessage = {
            id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            direction: "out",
            channel: String(payload.channel ?? "unknown"),
            sessionKey: undefined,
            contentPreview: payload.message_id ? `[msg:${String(payload.message_id)}]` : "(sent)",
            timestampMs: typeof payload.timestamp_ms === "number" ? payload.timestamp_ms : Date.now(),
            requestId: payload.request_id ? String(payload.request_id) : undefined,
            to: payload.to ? String(payload.to) : undefined,
          };
          setMessages(prev => ringPush(prev, msg, MAX_MESSAGES));
        }
      } catch { /* ignore */ }
    };
  }, []);

  useEffect(() => {
    unmountedRef.current = false;
    connect();
    return () => {
      unmountedRef.current = true;
      if (reconnectRef.current !== null) {
        clearTimeout(reconnectRef.current);
        reconnectRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [connect]);

  const clearMessages = useCallback(() => setMessages([]), []);

  return { messages, connected, clearMessages };
}

// ─── Messages Tab ────────────────────────────────────────────────────────────

function MessagesTab() {
  const { t } = useTranslation();
  const { messages, connected, clearMessages } = useMessageStream();
  const [channelFilter, setChannelFilter] = useState("all");
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoFollow, setAutoFollow] = useState(true);

  // Collect unique channels for filter dropdown
  const allChannels = useMemo(() => {
    const seen = new Set<string>();
    for (const m of messages) seen.add(m.channel.toLowerCase());
    return Array.from(seen).sort();
  }, [messages]);

  // Filtered view
  const filtered = useMemo(() => {
    if (channelFilter === "all") return messages;
    return messages.filter(m => m.channel.toLowerCase() === channelFilter);
  }, [messages, channelFilter]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoFollow && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [filtered, autoFollow]);

  function formatTs(ms: number): string {
    const d = new Date(ms);
    return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
      + "." + String(d.getMilliseconds()).padStart(3, "0");
  }

  function truncateSessionKey(key?: string): string {
    if (!key) return "—";
    // session keys can be long like "lark:user:xxx" — show last 16 chars
    return key.length > 20 ? "…" + key.slice(-16) : key;
  }

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-5 py-3 border-b border-[var(--separator)] bg-[var(--bg-grouped)]/40">
        {/* Channel filter */}
        <div className="flex items-center gap-1.5">
          <Filter className="h-3.5 w-3.5 text-[var(--text-tertiary)] shrink-0" />
          <select
            value={channelFilter}
            onChange={e => setChannelFilter(e.target.value)}
            className="text-[11px] bg-[var(--bg-grouped)] border border-[var(--separator)] rounded-[var(--radius-sm)] px-2.5 py-1.5 text-[var(--text-secondary)] focus:outline-none focus:border-[var(--accent)] cursor-pointer transition-colors"
          >
            <option value="all">{t("logs.allChannels")}</option>
            {allChannels.map(ch => (
              <option key={ch} value={ch}>{ch}</option>
            ))}
          </select>
        </div>

        <div className="w-px h-5 bg-[var(--border-subtle)]/50 mx-1" />

        {/* Live/Paused toggle */}
        <button
          onClick={() => setAutoFollow(v => !v)}
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
          onClick={clearMessages}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-medium text-[var(--text-tertiary)] border border-[var(--border-subtle)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-all cursor-pointer"
        >
          <X className="h-3 w-3" />
          {t("debug.clear")}
        </button>

        <div className="ml-auto flex items-center gap-2">
          <span className="text-[11px] font-mono tabular-nums text-[var(--text-tertiary)]">
            {filtered.length} / {MAX_MESSAGES}
          </span>
          <div className="flex items-center gap-1.5">
            <span className={cn(
              "w-1.5 h-1.5 rounded-full",
              connected ? "bg-[var(--success)] animate-pulse" : "bg-[var(--text-tertiary)]"
            )} />
            <span className="text-[10px] text-[var(--text-tertiary)]">
              {connected ? "Live" : "Reconnecting..."}
            </span>
          </div>
        </div>
      </div>

      {/* Column headers */}
      <div className="flex items-center gap-3 px-4 py-2 text-[10px] font-bold uppercase tracking-[0.08em] text-[var(--text-tertiary)] border-b border-[var(--separator)] bg-[var(--bg-grouped)]/30 shrink-0">
        <span className="w-[86px] shrink-0">Time</span>
        <span className="w-[72px] shrink-0">Channel</span>
        <span className="w-[40px] shrink-0">Dir</span>
        <span className="w-[140px] shrink-0">Session</span>
        <span className="flex-1">Content Preview</span>
      </div>

      {/* Message rows */}
      {filtered.length === 0 ? (
        <div className="py-16">
          <EmptyState
            icon={<MessageSquare className="h-6 w-6" />}
            message={t("logs.noMessages")}
          />
        </div>
      ) : (
        <div
          ref={scrollRef}
          className="flex-1 min-h-0 overflow-y-auto overscroll-contain"
          style={{ scrollbarGutter: "stable" }}
        >
          {filtered.map((msg, i) => {
            const colors = getChannelColors(msg.channel);
            const isIn = msg.direction === "in";
            return (
              <div
                key={msg.id}
                className={cn(
                  "flex items-center gap-3 px-4 py-2 border-b border-[var(--border-subtle)]/20 transition-colors",
                  i % 2 === 0 ? "bg-[var(--bg-elevated)]/20" : "bg-transparent",
                  "hover:bg-[var(--bg-hover)]/40",
                  isIn
                    ? "border-l-2 border-l-[var(--accent)]/30"
                    : "border-l-2 border-l-[var(--success)]/30",
                )}
              >
                {/* Timestamp */}
                <span className="text-[11px] font-mono tabular-nums text-[var(--text-tertiary)] shrink-0 w-[86px]">
                  {formatTs(msg.timestampMs)}
                </span>

                {/* Channel badge */}
                <span className={cn(
                  "inline-flex items-center px-1.5 py-[1px] rounded-full text-[10px] font-semibold border shrink-0 w-[72px] justify-center truncate",
                  colors.bg, colors.text, colors.border,
                )}>
                  {msg.channel}
                </span>

                {/* Direction */}
                <span className={cn(
                  "flex items-center gap-0.5 text-[10px] font-bold shrink-0 w-[40px]",
                  isIn ? "text-[var(--accent-light)]" : "text-[var(--success)]",
                )}>
                  {isIn
                    ? <ArrowDownLeft className="h-3 w-3" />
                    : <ArrowUpRight className="h-3 w-3" />
                  }
                  {isIn ? t("logs.inbound").slice(0, 2) : t("logs.outbound").slice(0, 2)}
                </span>

                {/* Session key */}
                <span
                  className="text-[11px] font-mono text-[var(--text-tertiary)] shrink-0 w-[140px] truncate"
                  title={msg.sessionKey}
                >
                  {truncateSessionKey(msg.sessionKey)}
                </span>

                {/* Content preview */}
                <span className="flex-1 min-w-0 text-[12px] text-[var(--text-primary)] truncate leading-snug">
                  {msg.contentPreview || <span className="text-[var(--text-tertiary)] italic">(empty)</span>}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {/* Footer */}
      <div className="flex items-center justify-between px-5 py-2.5 text-[10px] text-[var(--text-tertiary)] border-t border-[var(--separator)] bg-[var(--bg-grouped)]/30">
        <span className="font-mono tabular-nums">{filtered.length} {t("logs.messagesTab").toLowerCase()}</span>
        <span>Ring buffer · max {MAX_MESSAGES}</span>
      </div>
    </div>
  );
}

// ─── Tab type ─────────────────────────────────────────────────────────────────

type LogsViewTab = "logs" | "traces";

const LOG_LEVELS: LogLevel[] = ["ALL", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

const TIME_RANGES: { value: TimeRange; label: string; minutes: number }[] = [
  { value: "5m", label: "5 min", minutes: 5 },
  { value: "15m", label: "15 min", minutes: 15 },
  { value: "30m", label: "30 min", minutes: 30 },
  { value: "1h", label: "1 hour", minutes: 60 },
  { value: "all", label: "All", minutes: 0 },
];

const LOGID_TIME_OFFSETS = [
  { label: "+10min", minutes: 10 },
  { label: "+20min", minutes: 20 },
  { label: "+30min", minutes: 30 },
];

// ─── LogID Time Decoder ──────────────────────────────────────────────────────

function parseLogIdTime(rid: string): Date | null {
  const newMatch = rid.match(/^(\d{13})[0-9a-f]{10}$/i);
  if (newMatch) {
    const ms = parseInt(newMatch[1], 10);
    const d = new Date(ms);
    if (d.getFullYear() >= 2020 && d.getFullYear() <= 2035) return d;
  }
  const legacyMatch = rid.match(/^(?:req-)?([0-9a-f]{12})[0-9a-f]{8}$/i);
  if (legacyMatch) {
    const ms = parseInt(legacyMatch[1], 16);
    const d = new Date(ms);
    if (d.getFullYear() >= 2020 && d.getFullYear() <= 2035) return d;
  }
  return null;
}

function parseLogIdMachine(rid: string): string | null {
  const match = rid.match(/^\d{13}([0-9a-f]{6})/i);
  return match ? match[1] : null;
}

function isLogId(s: string): boolean {
  const t = s.trim();
  if (/^\d{13}[0-9a-f]{10}$/i.test(t)) return true;
  if (/^req-[0-9a-f]{20}$/i.test(t)) return true;
  return false;
}

function formatTimeShort(d: Date): string {
  return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
    + "." + String(d.getMilliseconds()).padStart(3, "0");
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

const LEVEL_CONFIG: Record<string, {
  color: string; bg: string; border: string; icon: typeof Info; barColor: string;
}> = {
  ERROR: { color: "text-[var(--error)]", bg: "bg-[var(--error)]/10", border: "border-[var(--error)]/20", icon: AlertCircle, barColor: "bg-[var(--error)]" },
  WARN:  { color: "text-[var(--warning)]", bg: "bg-[var(--warning)]/10", border: "border-[var(--warning)]/20", icon: AlertTriangle, barColor: "bg-[var(--warning)]" },
  INFO:  { color: "text-[var(--accent-light)]", bg: "bg-[var(--accent)]/10", border: "border-[var(--accent)]/15", icon: Info, barColor: "bg-[var(--accent)]" },
  DEBUG: { color: "text-[var(--text-tertiary)]", bg: "bg-[var(--text-tertiary)]/8", border: "border-[var(--text-tertiary)]/15", icon: Bug, barColor: "bg-[var(--text-tertiary)]" },
  TRACE: { color: "text-[var(--text-tertiary)]", bg: "bg-[var(--text-tertiary)]/5", border: "border-[var(--text-tertiary)]/10", icon: Radio, barColor: "bg-[var(--text-tertiary)]" },
};

function getLevelConfig(level: string) {
  return LEVEL_CONFIG[level.toUpperCase()] || LEVEL_CONFIG.DEBUG;
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
      + "." + String(d.getMilliseconds()).padStart(3, "0");
  } catch {
    return ts;
  }
}

function formatFullTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleString("en-US", {
      year: "numeric", month: "short", day: "numeric",
      hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false,
    }) + "." + String(d.getMilliseconds()).padStart(3, "0");
  } catch {
    return ts;
  }
}

function truncateTarget(target: string): string {
  const parts = target.split("::");
  if (parts.length > 2) return parts.slice(-2).join("::");
  return target;
}

function renderFieldValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

// ─── Field Classification ────────────────────────────────────────────────────

/** Fields that contain content (long text) — shown as preview lines, not chips */
const CONTENT_FIELDS = new Set([
  "system_prompt", "user_message", "args", "result", "response", "tools",
  "error", "content",
]);

/** Fields that are redundant in tracing mode (same for all entries) */
const CONTEXT_FIELDS = new Set(["conn_id", "conversation_id"]);

/** Metric fields — shown as inline badges */
const METRIC_PRIORITY = [
  "duration_ms", "tool", "input_tokens", "output_tokens", "total_tokens",
  "tool_calls", "result_len", "content_len", "message_count", "tool_count",
  "has_thinking",
];

// ─── API ─────────────────────────────────────────────────────────────────────

async function fetchStructuredLogs(params: {
  limit?: number;
  level?: string;
  request_id?: string;
  from?: string;
  to?: string;
  keyword?: string;
}): Promise<{ entries: LogEntry[]; total: number } | null> {
  try {
    const qs = new URLSearchParams();
    if (params.limit) qs.set("limit", String(params.limit));
    if (params.level && params.level !== "ALL") qs.set("level", params.level);
    if (params.request_id) qs.set("request_id", params.request_id);
    if (params.from) qs.set("from", params.from);
    if (params.to) qs.set("to", params.to);
    if (params.keyword) qs.set("keyword", params.keyword);
    const res = await fetch(`/api/logs?${qs}`);
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  }
}

// ─── Log Row ─────────────────────────────────────────────────────────────────

function LogRow({
  entry,
  index,
  onFilterRequestId,
  isTracing,
  searchQuery: _searchQuery,
  colWidths,
}: {
  entry: LogEntry;
  index: number;
  onFilterRequestId: (rid: string) => void;
  isTracing: boolean;
  searchQuery: string;
  colWidths: Record<ColumnId, number>;
}) {
  const [expanded, setExpanded] = useState(false);
  const fields = entry.fields || {};
  const fieldKeys = Object.keys(fields);
  const lc = getLevelConfig(entry.level);
  const LevelIcon = lc.icon;

  // Classify fields
  const contentFields: [string, string][] = [];
  const metricFields: [string, string][] = [];
  const otherFields: [string, string][] = [];

  for (const k of fieldKeys) {
    const v = renderFieldValue(fields[k]);
    if (CONTENT_FIELDS.has(k) && v.length > 0 && v !== "\"\"" && v !== "") {
      contentFields.push([k, v]);
    } else if (isTracing && CONTEXT_FIELDS.has(k)) {
      // Hide in tracing mode — only show on expand
      otherFields.push([k, v]);
    } else {
      metricFields.push([k, v]);
    }
  }

  // Sort metric fields by priority
  metricFields.sort((a, b) => {
    const ai = METRIC_PRIORITY.indexOf(a[0]);
    const bi = METRIC_PRIORITY.indexOf(b[0]);
    if (ai !== -1 && bi !== -1) return ai - bi;
    if (ai !== -1) return -1;
    if (bi !== -1) return 1;
    return 0;
  });

  const hasExpandable = contentFields.length > 0 || otherFields.length > 0 || metricFields.length > 3;
  const inlineMetrics = metricFields.slice(0, 3);

  // Format duration badge
  const durationMs = fields.duration_ms;
  const hasDuration = typeof durationMs === "number";

  return (
    <div
      className={cn(
        "group relative transition-colors border-b border-[var(--border-subtle)]/20",
        isTracing && "bg-[var(--accent)]/[0.02]",
        index % 2 === 0 ? "bg-[var(--bg-elevated)]/20" : "bg-transparent",
        "hover:bg-[var(--bg-hover)]/40",
      )}
    >
      {/* Level color bar */}
      <div className={cn("absolute left-0 top-0 bottom-0 w-[3px] rounded-r-full opacity-50", lc.barColor)} />

      {/* Main row */}
      <div
        className="flex items-start gap-3 pl-4 pr-3 py-2 cursor-pointer select-none"
        onClick={() => hasExpandable && setExpanded(!expanded)}
      >
        {/* Expand indicator */}
        <div className="w-3.5 pt-[3px] shrink-0 flex items-center justify-center">
          {hasExpandable ? (
            expanded
              ? <ChevronDown className="h-3 w-3 text-[var(--text-tertiary)]" />
              : <ChevronRight className="h-3 w-3 text-[var(--text-tertiary)] opacity-50 group-hover:opacity-80 transition-opacity" />
          ) : <span className="w-3" />}
        </div>

        {/* Trace ID — first column, hidden in tracing mode */}
        {!isTracing && (
          <div className="shrink-0 truncate" style={{ width: colWidths.traceId }}>
            {entry.request_id ? (
              <button
                onClick={(e) => { e.stopPropagation(); onFilterRequestId(entry.request_id!); }}
                className="font-mono text-[10px] text-[var(--accent-light)] opacity-60 hover:opacity-100 hover:bg-[var(--bg-hover)]/60 rounded px-1 -mx-1 transition-all cursor-pointer truncate block w-full text-left"
                title={`Trace ${entry.request_id}`}
              >
                {entry.request_id.slice(-8)}
              </button>
            ) : (
              <span className="text-[10px] text-[var(--text-tertiary)]">—</span>
            )}
          </div>
        )}

        {/* Timestamp */}
        <span
          className="text-[12px] font-mono tabular-nums text-[var(--text-tertiary)] shrink-0 pt-[1px] truncate"
          style={{ width: colWidths.time }}
          title={formatFullTime(entry.ts)}
        >
          {formatTime(entry.ts)}
        </span>

        {/* Level badge */}
        <span
          className={cn(
            "inline-flex items-center gap-1 px-1.5 py-[1px] rounded text-[10px] font-bold tracking-wider border shrink-0",
            lc.bg, lc.color, lc.border,
          )}
          style={{ width: colWidths.level }}
        >
          <LevelIcon className="h-3 w-3" />
          {entry.level.toUpperCase().slice(0, 4)}
        </span>

        {/* Target */}
        <span
          className="text-[11px] font-mono text-[var(--text-tertiary)] shrink-0 pt-[2px] truncate"
          style={{ width: colWidths.target }}
          title={entry.target}
        >
          {truncateTarget(entry.target)}
        </span>

        {/* Message + inline metrics */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            {/* Message */}
            <span className={cn(
              "text-[13px] leading-snug font-medium",
              entry.level === "ERROR" ? "text-[var(--error)]" :
              entry.level === "WARN" ? "text-[var(--warning)]" :
              "text-[var(--text-primary)]",
            )}>
              {entry.message}
            </span>

            {/* Duration badge — always prominent */}
            {hasDuration && (
              <span className={cn(
                "text-[11px] font-mono tabular-nums px-1.5 py-[1px] rounded shrink-0",
                Number(durationMs) > 5000
                  ? "bg-[var(--warning)]/15 text-[var(--warning)] border border-[var(--warning)]/20"
                  : "bg-[var(--bg-content)]/60 text-[var(--text-secondary)] border border-[var(--border-subtle)]/30",
              )}>
                {Number(durationMs).toLocaleString()}ms
              </span>
            )}

            {/* Inline metric chips (skip duration_ms, already shown) */}
            <span className="hidden md:flex items-center gap-1.5 ml-auto shrink-0">
              {inlineMetrics
                .filter(([k]) => k !== "duration_ms")
                .slice(0, 3)
                .map(([k, v]) => (
                  <span
                    key={k}
                    className="text-[10px] font-mono px-1.5 py-[1px] rounded bg-[var(--bg-content)]/50 border border-[var(--border-subtle)]/30 text-[var(--text-tertiary)] truncate max-w-[140px]"
                  >
                    <span className="opacity-40">{k.replace(/_/g, " ")}=</span>{v}
                  </span>
                ))}
            </span>
          </div>

          {/* Content preview (first content field, single line) */}
          {!expanded && contentFields.length > 0 && (
            <div className="mt-1 text-[11px] font-mono text-[var(--text-tertiary)] truncate leading-snug">
              {contentFields[0][0] === "error" ? (
                <span className="text-[var(--error)]/80">{contentFields[0][1]}</span>
              ) : contentFields[0][0] === "tools" ? (
                <span className="text-[var(--accent-light)]/60">{contentFields[0][1]}</span>
              ) : (
                contentFields[0][1]
              )}
            </div>
          )}
        </div>
      </div>

      {/* Expanded panel */}
      {expanded && (
        <div className="pl-[52px] pr-4 pb-3">
          <div className="rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--separator)] overflow-hidden">
            {/* Content fields — shown as preformatted blocks */}
            {contentFields.length > 0 && (
              <div className="border-b border-[var(--border-subtle)]/30">
                {contentFields.map(([k, v]) => (
                  <div key={k} className="px-3 py-2 border-b border-[var(--border-subtle)]/20 last:border-b-0">
                    <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{k.replace(/_/g, " ")}</span>
                    <pre className={cn(
                      "mt-1 text-[11px] font-mono leading-relaxed whitespace-pre-wrap break-all max-h-[200px] overflow-y-auto",
                      k === "error" ? "text-[var(--error)]" :
                      k === "tools" ? "text-[var(--accent-light)]" :
                      "text-[var(--text-secondary)]"
                    )}>
                      {v}
                    </pre>
                  </div>
                ))}
              </div>
            )}

            {/* Metric + context fields — grid layout */}
            {(metricFields.length > 0 || otherFields.length > 0) && (
              <div className="px-3 py-2">
                <div className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1">
                  {[...metricFields, ...otherFields].map(([k, v]) => (
                    <div key={k} className="contents text-[11px] font-mono">
                      <span className="text-[var(--text-tertiary)] font-medium">{k}</span>
                      <span className={cn(
                        "break-all select-all",
                        k === "duration_ms" ? "text-[var(--warning)]" :
                        k === "error" ? "text-[var(--error)]" :
                        "text-[var(--text-secondary)]"
                      )}>
                        {v}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

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

  // ─── Compute from/to for API ──────────────────────────────────────────────

  const { apiFrom, apiTo } = useMemo(() => {
    if (requestIdFilter && logidDecodedTime) {
      const from = new Date(logidDecodedTime.getTime() - 5000);
      const to = new Date(logidDecodedTime.getTime() + logidOffsetMin * 60 * 1000);
      return { apiFrom: from.toISOString(), apiTo: to.toISOString() };
    }
    if (timeRange !== "all" && timeRange !== "custom") {
      const r = TIME_RANGES.find(t => t.value === timeRange);
      if (r && r.minutes > 0) {
        const from = new Date(Date.now() - r.minutes * 60 * 1000);
        return { apiFrom: from.toISOString(), apiTo: undefined };
      }
    }
    return { apiFrom: undefined, apiTo: undefined };
  }, [requestIdFilter, logidDecodedTime, logidOffsetMin, timeRange]);

  // ─── Data loading ─────────────────────────────────────────────────────────

  const loadLogs = useCallback(async () => {
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
  }, [limit, levelFilter, requestIdFilter, apiFrom, apiTo, search]);

  useEffect(() => {
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
