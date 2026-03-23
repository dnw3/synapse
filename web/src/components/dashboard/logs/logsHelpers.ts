import {
  AlertTriangle, AlertCircle, Info, Bug, Radio,
} from "lucide-react";

// ─── Types ───────────────────────────────────────────────────────────────────

export interface LogEntry {
  ts: string;
  level: string;
  request_id: string | null;
  target: string;
  message: string;
  fields: Record<string, unknown>;
}

export type LogLevel = "ALL" | "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE";
export type TimeRange = "5m" | "15m" | "30m" | "1h" | "all" | "custom";

export const LOG_LEVELS: LogLevel[] = ["ALL", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

export const TIME_RANGES: { value: TimeRange; label: string; minutes: number }[] = [
  { value: "5m", label: "5 min", minutes: 5 },
  { value: "15m", label: "15 min", minutes: 15 },
  { value: "30m", label: "30 min", minutes: 30 },
  { value: "1h", label: "1 hour", minutes: 60 },
  { value: "all", label: "All", minutes: 0 },
];

export const LOGID_TIME_OFFSETS = [
  { label: "+10min", minutes: 10 },
  { label: "+20min", minutes: 20 },
  { label: "+30min", minutes: 30 },
];

// ─── LogID Time Decoder ──────────────────────────────────────────────────────

export function parseLogIdTime(rid: string): Date | null {
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

export function parseLogIdMachine(rid: string): string | null {
  const match = rid.match(/^\d{13}([0-9a-f]{6})/i);
  return match ? match[1] : null;
}

export function isLogId(s: string): boolean {
  const t = s.trim();
  if (/^\d{13}[0-9a-f]{10}$/i.test(t)) return true;
  if (/^req-[0-9a-f]{20}$/i.test(t)) return true;
  return false;
}

export function formatTimeShort(d: Date): string {
  return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
    + "." + String(d.getMilliseconds()).padStart(3, "0");
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

export const LEVEL_CONFIG: Record<string, {
  color: string; bg: string; border: string; icon: typeof Info; barColor: string;
}> = {
  ERROR: { color: "text-[var(--error)]", bg: "bg-[var(--error)]/10", border: "border-[var(--error)]/20", icon: AlertCircle, barColor: "bg-[var(--error)]" },
  WARN:  { color: "text-[var(--warning)]", bg: "bg-[var(--warning)]/10", border: "border-[var(--warning)]/20", icon: AlertTriangle, barColor: "bg-[var(--warning)]" },
  INFO:  { color: "text-[var(--accent-light)]", bg: "bg-[var(--accent)]/10", border: "border-[var(--accent)]/15", icon: Info, barColor: "bg-[var(--accent)]" },
  DEBUG: { color: "text-[var(--text-tertiary)]", bg: "bg-[var(--text-tertiary)]/8", border: "border-[var(--text-tertiary)]/15", icon: Bug, barColor: "bg-[var(--text-tertiary)]" },
  TRACE: { color: "text-[var(--text-tertiary)]", bg: "bg-[var(--text-tertiary)]/5", border: "border-[var(--text-tertiary)]/10", icon: Radio, barColor: "bg-[var(--text-tertiary)]" },
};

export function getLevelConfig(level: string) {
  return LEVEL_CONFIG[level.toUpperCase()] || LEVEL_CONFIG.DEBUG;
}

export function formatTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
      + "." + String(d.getMilliseconds()).padStart(3, "0");
  } catch {
    return ts;
  }
}

export function formatFullTime(ts: string): string {
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

export function truncateTarget(target: string): string {
  const parts = target.split("::");
  if (parts.length > 2) return parts.slice(-2).join("::");
  return target;
}

export function renderFieldValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

// ─── Field Classification ────────────────────────────────────────────────────

/** Fields that contain content (long text) — shown as preview lines, not chips */
export const CONTENT_FIELDS = new Set([
  "system_prompt", "user_message", "args", "result", "response", "tools",
  "error", "content",
]);

/** Fields that are redundant in tracing mode (same for all entries) */
export const CONTEXT_FIELDS = new Set(["conn_id", "conversation_id"]);

/** Metric fields — shown as inline badges */
export const METRIC_PRIORITY = [
  "duration_ms", "tool", "input_tokens", "output_tokens", "total_tokens",
  "tool_calls", "result_len", "content_len", "message_count", "tool_count",
  "has_thinking",
];

// ─── API ─────────────────────────────────────────────────────────────────────

export async function fetchStructuredLogs(params: {
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
