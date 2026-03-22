import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, ChevronDown, Check, X } from "lucide-react";
import { cn } from "../../../lib/cn";
import type { TraceRecord, Span } from "./types";
import { isModelCallSpan, isToolCallSpan } from "./types";

// ─── Utilities ──────────────────────────────────────────────────────────────

function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "\u2014";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function formatTokens(n: number): string {
  return n.toLocaleString();
}

// ─── Span Color Helpers ─────────────────────────────────────────────────────

function spanDotColor(span: Span): string {
  if (isModelCallSpan(span)) return "bg-indigo-400";
  if (span.tool === "task") return "bg-purple-400";
  return "bg-amber-400";
}

function spanBarColor(span: Span): string {
  if (isModelCallSpan(span)) return "bg-indigo-500/70";
  if (span.tool === "task") return "bg-purple-500/70";
  return "bg-amber-500/70";
}

// ─── Tree Node ──────────────────────────────────────────────────────────────

interface TreeNode {
  span: Span;
  children: TreeNode[];
  depth: number;
  modelIndex: number | null;
}

function buildTree(spans: Span[]): TreeNode[] {
  const nodes: TreeNode[] = [];
  let currentModel: TreeNode | null = null;
  let modelCounter = 0;

  for (const span of spans) {
    if (isModelCallSpan(span)) {
      modelCounter++;
      const node: TreeNode = { span, children: [], depth: 0, modelIndex: modelCounter };
      nodes.push(node);
      currentModel = node;
    } else {
      const node: TreeNode = { span, children: [], depth: 1, modelIndex: null };
      if (currentModel) {
        currentModel.children.push(node);
      } else {
        nodes.push(node);
      }
    }
  }
  return nodes;
}

// ─── Props ──────────────────────────────────────────────────────────────────

interface TimelineViewProps {
  trace: TraceRecord;
}

// ─── Time Ruler ─────────────────────────────────────────────────────────────

function TimeRuler({ durationMs }: { durationMs: number }) {
  const ticks = useMemo(() => {
    const count = Math.min(Math.max(2, Math.ceil(durationMs / 1000) + 1), 10);
    const step = durationMs / (count - 1);
    return Array.from({ length: count }, (_, i) => ({
      pct: (i / (count - 1)) * 100,
      label: formatDuration(Math.round(step * i)),
    }));
  }, [durationMs]);

  return (
    <div className="relative h-4 mb-1">
      {ticks.map((tick, i) => (
        <div
          key={i}
          className="absolute text-[7px] font-mono text-[var(--text-tertiary)]"
          style={{
            left: `${tick.pct}%`,
            transform: i === ticks.length - 1 ? "translateX(-100%)" : i === 0 ? "none" : "translateX(-50%)",
          }}
        >
          {tick.label}
        </div>
      ))}
    </div>
  );
}

// ─── Waterfall Row ──────────────────────────────────────────────────────────

function WaterfallRow({ node, traceStart, traceDuration, expanded, onToggle, isLast }: {
  node: TreeNode;
  traceStart: number;
  traceDuration: number;
  expanded: boolean;
  onToggle: () => void;
  isLast: boolean;
}) {
  const { t } = useTranslation();
  const span = node.span;
  const hasChildren = node.children.length > 0;
  const spanStart = new Date(span.start_time).getTime();
  const offsetMs = spanStart - traceStart;
  const leftPct = Math.max(0, (offsetMs / traceDuration) * 100);
  const widthPct = Math.max(0.5, ((span.duration_ms ?? 0) / traceDuration) * 100);

  const label = isModelCallSpan(span)
    ? t("traces.overview.modelN", { n: node.modelIndex ?? 0 })
    : (span.tool ?? "tool");

  const statusIcon = isToolCallSpan(span)
    ? span.error
      ? <X className="h-2.5 w-2.5 text-red-400" />
      : <Check className="h-2.5 w-2.5 text-emerald-400" />
    : null;

  return (
    <div className={cn(
      "flex items-center h-7 hover:bg-[var(--bg-grouped)] transition-colors text-[10px]",
      node.depth > 0 && "pl-5"
    )}>
      {/* Span name column */}
      <div className="w-[200px] flex-shrink-0 flex items-center gap-1 truncate pr-2">
        {node.depth > 0 && (
          <span className="text-[var(--text-tertiary)] font-mono text-[9px] w-3 text-center flex-shrink-0">
            {isLast ? "\u2570" : "\u2502"}
          </span>
        )}
        {hasChildren ? (
          <button onClick={onToggle} className="flex-shrink-0 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] cursor-pointer">
            {expanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
          </button>
        ) : (
          <span className="w-3 flex-shrink-0" />
        )}
        <span className={cn("w-2 h-2 rounded-full flex-shrink-0", spanDotColor(span))} />
        <span className="text-[var(--text-secondary)] truncate">{label}</span>
        {statusIcon}
      </div>

      {/* Duration column */}
      <div className="w-[80px] flex-shrink-0 text-right font-mono text-[var(--text-tertiary)] pr-2">
        {formatDuration(span.duration_ms)}
      </div>

      {/* Tokens column */}
      <div className="w-[70px] flex-shrink-0 text-right text-[var(--text-tertiary)] pr-2">
        {isModelCallSpan(span) ? formatTokens(span.total_tokens ?? 0) : "\u2014"}
      </div>

      {/* Time bar column */}
      <div className="flex-1 h-4 relative rounded-[var(--radius-sm)] bg-[var(--bg-grouped)]">
        <div
          className={cn("absolute top-0.5 bottom-0.5 rounded-[2px] min-w-[2px]", spanBarColor(span))}
          style={{ left: `${leftPct}%`, width: `${Math.min(widthPct, 100 - leftPct)}%` }}
        />
      </div>
    </div>
  );
}

// ─── Time Breakdown Bar ─────────────────────────────────────────────────────

function TimeBreakdownBar({ trace }: { trace: TraceRecord }) {
  const { t } = useTranslation();
  const totalDuration = trace.metadata.duration_ms ?? 1;

  const modelTime = trace.spans
    .filter(isModelCallSpan)
    .reduce((sum, s) => sum + (s.duration_ms ?? 0), 0);
  const toolTime = trace.spans
    .filter(isToolCallSpan)
    .reduce((sum, s) => sum + (s.duration_ms ?? 0), 0);
  const modelPct = Math.round((modelTime / totalDuration) * 100);
  const toolPct = Math.round((toolTime / totalDuration) * 100);
  const idlePct = Math.max(0, 100 - modelPct - toolPct);

  const segments = [
    { label: t("traces.timeline.model"), pct: modelPct, className: "bg-indigo-500/70 text-white" },
    { label: t("traces.timeline.tools"), pct: toolPct, className: "bg-amber-500/70 text-white" },
    { label: t("traces.timeline.idle"), pct: idlePct, className: "bg-[var(--text-tertiary)]/20 text-[var(--text-secondary)]" },
  ].filter(s => s.pct > 0);

  return (
    <div className="space-y-1.5">
      <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
        {t("traces.timeline.timeBreakdown")}
      </span>
      <div className="flex h-6 rounded-[var(--radius-sm)] overflow-hidden border border-[var(--border-subtle)]">
        {segments.map((seg, i) => (
          <div
            key={i}
            className={cn("flex items-center justify-center text-[8px] font-medium truncate px-1", seg.className)}
            style={{ width: `${seg.pct}%` }}
          >
            {seg.pct > 10 && `${seg.label} ${seg.pct}%`}
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── Main Component ─────────────────────────────────────────────────────────

export default function TimelineView({ trace }: TimelineViewProps) {
  const { t } = useTranslation();
  const tree = useMemo(() => buildTree(trace.spans), [trace.spans]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const traceStart = new Date(trace.start_time).getTime();
  const traceDuration = trace.metadata.duration_ms ?? 1;

  const toggleCollapse = (id: string) => {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  if (trace.spans.length === 0) {
    return (
      <div className="text-[11px] text-[var(--text-tertiary)] py-8 text-center">
        {t("traces.overview.noSpans")}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Waterfall */}
      <div className="rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-3">
        {/* Header row */}
        <div className="flex items-center h-6 text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)] border-b border-[var(--separator)] mb-1">
          <div className="w-[200px] flex-shrink-0">{t("traces.timeline.span")}</div>
          <div className="w-[80px] flex-shrink-0 text-right pr-2">{t("traces.span.duration")}</div>
          <div className="w-[70px] flex-shrink-0 text-right pr-2">{t("traces.timeline.tokens")}</div>
          <div className="flex-1">
            <TimeRuler durationMs={traceDuration} />
          </div>
        </div>

        {/* Rows */}
        {tree.map((node, nodeIdx) => {
          const isExpanded = !collapsed.has(node.span.id);
          return (
            <div key={node.span.id}>
              <WaterfallRow
                node={node}
                traceStart={traceStart}
                traceDuration={traceDuration}
                expanded={isExpanded}
                onToggle={() => toggleCollapse(node.span.id)}
                isLast={nodeIdx === tree.length - 1 && node.children.length === 0}
              />
              {isExpanded && node.children.map((child, childIdx) => (
                <WaterfallRow
                  key={child.span.id}
                  node={child}
                  traceStart={traceStart}
                  traceDuration={traceDuration}
                  expanded={false}
                  onToggle={() => {}}
                  isLast={childIdx === node.children.length - 1}
                />
              ))}
            </div>
          );
        })}
      </div>

      {/* Time Breakdown */}
      <TimeBreakdownBar trace={trace} />
    </div>
  );
}
