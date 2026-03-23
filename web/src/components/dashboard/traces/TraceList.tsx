import { useTranslation } from "react-i18next";
import { useState } from "react";
import {
  Search, RefreshCw, ChevronDown, Clock,
  Cpu, Wrench, Coins, Activity, Copy, Check,
} from "lucide-react";
import { cn } from "../../../lib/cn";
import { EmptyState, LoadingSpinner } from "../shared";
import type { TraceRecord, TraceListParams } from "./types";

interface TraceListProps {
  traces: TraceRecord[];
  loading: boolean;
  onSelectTrace: (requestId: string) => void;
  filters: TraceListParams;
  onFilterChange: (filters: TraceListParams) => void;
  onRefresh: () => void;
}

function formatDuration(ms: number | null): string {
  if (ms === null) return "\u2014";
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
}

function formatTokens(n: number): string {
  return n.toLocaleString();
}

function formatRelativeTime(isoStr: string): string {
  const diff = Date.now() - new Date(isoStr).getTime();
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function truncateId(id: string, len = 12): string {
  if (id.length <= len) return id;
  return `...${id.slice(-len)}`;
}

function CopyableLogId({ id, truncated }: { id: string; truncated?: boolean }) {
  const [copied, setCopied] = useState(false);
  const { t } = useTranslation();

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const display = truncated ? truncateId(id) : id;

  return (
    <button
      onClick={handleCopy}
      title={copied ? t("logid.copied") : `${id}\n${t("logid.tooltip")}`}
      className="inline-flex items-center gap-1 text-[12px] font-mono font-medium text-[var(--text-primary)] tabular-nums hover:text-[var(--accent)] transition-colors cursor-pointer"
    >
      {copied ? <Check className="h-3 w-3 text-green-500" /> : <Copy className="h-3 w-3 opacity-40" />}
      <span>{copied ? t("logid.copied") : display}</span>
    </button>
  );
}

const STATUS_BORDER: Record<string, string> = {
  success: "border-l-green-500",
  error: "border-l-red-500",
  running: "border-l-blue-500",
};

const STATUS_BADGE: Record<string, { bg: string; text: string }> = {
  success: { bg: "bg-green-500/10", text: "text-green-500" },
  error: { bg: "bg-red-500/10", text: "text-red-500" },
  running: { bg: "bg-blue-500/10", text: "text-blue-500" },
};

// Build flat list with tree info for sub-agent nesting
interface TraceListItem {
  trace: TraceRecord;
  depth: number;
  isChild: boolean;
}

function buildTreeList(traces: TraceRecord[]): TraceListItem[] {
  const byId = new Map(traces.map((t) => [t.request_id, t]));
  const items: TraceListItem[] = [];

  // Build parent→children mapping from metadata.parent_request_id
  const childrenOf = new Map<string, TraceRecord[]>();
  for (const trace of traces) {
    const parentId = trace.metadata.parent_request_id;
    if (parentId) {
      const siblings = childrenOf.get(parentId) ?? [];
      siblings.push(trace);
      childrenOf.set(parentId, siblings);
    }
  }

  for (const trace of traces) {
    // Skip children at top level — they'll be inserted after their parent
    const parentId = trace.metadata.parent_request_id;
    if (parentId && byId.has(parentId)) continue;

    const isChild = !!parentId;
    items.push({ trace, depth: 0, isChild });

    // Insert children right after parent
    const children = childrenOf.get(trace.request_id) ?? [];
    for (const child of children) {
      items.push({ trace: child, depth: 1, isChild: true });
    }
  }

  return items;
}

export function TraceList({
  traces, loading, onSelectTrace, filters, onFilterChange, onRefresh,
}: TraceListProps) {
  const { t } = useTranslation();

  const handleKeywordChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    onFilterChange({ ...filters, keyword: e.target.value || undefined });
  };

  const handleStatusChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    onFilterChange({ ...filters, status: e.target.value || undefined });
  };

  const treeItems = buildTreeList(traces);

  return (
    <div className="flex flex-col gap-3 p-4">
      {/* Search bar */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
        <input
          type="text"
          value={filters.keyword ?? ""}
          onChange={handleKeywordChange}
          placeholder={t("traces.search")}
          className="w-full pl-9 pr-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--separator)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
        />
      </div>

      {/* Filter row */}
      <div className="flex items-center gap-2">
        <div className="relative">
          <select
            value={filters.status ?? ""}
            onChange={handleStatusChange}
            className="appearance-none text-[11px] bg-[var(--bg-grouped)] border border-[var(--separator)] rounded-[var(--radius-sm)] pl-2.5 pr-7 py-1.5 text-[var(--text-secondary)] focus:outline-none focus:border-[var(--accent)] cursor-pointer transition-colors"
          >
            <option value="">{t("traces.filter.allStatuses")}</option>
            <option value="running">{t("traces.status.running")}</option>
            <option value="success">{t("traces.status.success")}</option>
            <option value="error">{t("traces.status.error")}</option>
          </select>
          <ChevronDown className="absolute right-2 top-1/2 -translate-y-1/2 h-3 w-3 text-[var(--text-tertiary)] pointer-events-none" />
        </div>

        <div className="flex-1" />

        <button
          onClick={onRefresh}
          className="flex items-center justify-center w-8 h-8 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-all cursor-pointer"
          title={t("traces.refresh")}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
        </button>
      </div>

      {/* List */}
      {loading && traces.length === 0 ? (
        <LoadingSpinner />
      ) : treeItems.length === 0 ? (
        <EmptyState
          icon={<Activity className="h-8 w-8" />}
          message={filters.keyword || filters.status ? t("traces.noTraces") : t("traces.empty")}
        />
      ) : (
        <div className="flex flex-col gap-1.5">
          {treeItems.map(({ trace, depth, isChild }) => {
            const statusBorder = isChild ? "border-l-purple-500" : (STATUS_BORDER[trace.status] ?? "");
            const badge = isChild
              ? { bg: "bg-purple-500/10", text: "text-purple-500" }
              : (STATUS_BADGE[trace.status] ?? STATUS_BADGE.success);

            return (
              <button
                key={trace.request_id}
                onClick={() => onSelectTrace(trace.request_id)}
                className={cn(
                  "relative w-full text-left rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-3 transition-all hover:border-[var(--separator)] hover:shadow-[var(--shadow-sm)] cursor-pointer",
                  "border-l-3",
                  statusBorder
                )}
                style={{ marginLeft: depth * 24 }}
              >
                {/* Connector line for children */}
                {depth > 0 && (
                  <div
                    className="absolute -left-[13px] top-0 bottom-1/2 w-px bg-[var(--separator)]"
                    style={{ height: "50%" }}
                  />
                )}

                {/* Header: request_id + status + time */}
                <div className="flex items-center justify-between gap-2 mb-1.5">
                  <div className="flex items-center gap-2 min-w-0">
                    <CopyableLogId id={trace.request_id} truncated />
                    <span className={cn(
                      "inline-flex items-center px-1.5 py-[1px] rounded-[var(--radius-sm)] text-[10px] font-semibold uppercase tracking-wider",
                      badge.bg, badge.text
                    )}>
                      {isChild ? t("traces.subAgent.label") : t(`traces.status.${trace.status}`)}
                    </span>
                  </div>
                  <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)] whitespace-nowrap">
                    <Clock className="h-2.5 w-2.5" />
                    {formatRelativeTime(trace.start_time)}
                  </span>
                </div>

                {/* Preview text */}
                {trace.metadata.user_message_preview && (
                  <p className="text-[11px] text-[var(--text-secondary)] truncate mb-1.5">
                    {trace.metadata.user_message_preview}
                  </p>
                )}

                {/* Stats row */}
                <div className="flex items-center gap-3 text-[10px] text-[var(--text-tertiary)] font-mono tabular-nums">
                  <span className="flex items-center gap-1" title={t("traces.summary.duration")}>
                    <Clock className="h-2.5 w-2.5" />
                    {formatDuration(trace.metadata.duration_ms)}
                  </span>
                  <span className="flex items-center gap-1" title={t("traces.summary.modelCalls")}>
                    <Cpu className="h-2.5 w-2.5" />
                    {trace.metadata.model_calls}
                  </span>
                  <span className="flex items-center gap-1" title={t("traces.summary.toolCalls")}>
                    <Wrench className="h-2.5 w-2.5" />
                    {trace.metadata.tool_calls}
                  </span>
                  <span className="flex items-center gap-1" title={t("traces.summary.tokens")}>
                    <Coins className="h-2.5 w-2.5" />
                    {formatTokens(trace.metadata.total_tokens)}
                  </span>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
