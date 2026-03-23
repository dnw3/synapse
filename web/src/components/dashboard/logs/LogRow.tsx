import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "../../../lib/cn";
import type { LogEntry } from "./logsHelpers";
import type { ColumnId } from "./logsColumns";
import {
  getLevelConfig, formatTime, formatFullTime, truncateTarget,
  renderFieldValue, CONTENT_FIELDS, CONTEXT_FIELDS, METRIC_PRIORITY,
} from "./logsHelpers";

// ─── Log Row ─────────────────────────────────────────────────────────────────

export function LogRow({
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
