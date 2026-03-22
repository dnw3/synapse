import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Copy } from "lucide-react";
import { cn } from "../../../lib/cn";
import type { TraceRecord, Span } from "./types";
import { isModelCallSpan } from "./types";

// ─── Utilities ──────────────────────────────────────────────────────────────

function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "\u2014";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function formatTokens(n: number): string {
  return n.toLocaleString();
}

// ─── Collapsible Text ───────────────────────────────────────────────────────

function CollapsibleText({ content, maxHeight = 80, className, fadeBg = "var(--bg-grouped)" }: {
  content: string;
  maxHeight?: number;
  className?: string;
  fadeBg?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation();
  const needsCollapse = content.length > 200;

  return (
    <div>
      <div className={cn("relative overflow-hidden", className)} style={!expanded && needsCollapse ? { maxHeight } : undefined}>
        <pre className="whitespace-pre-wrap break-words text-[10px] font-mono leading-relaxed text-[var(--text-primary)]">{content}</pre>
        {!expanded && needsCollapse && (
          <div
            className="absolute bottom-0 left-0 right-0 h-8"
            style={{ background: `linear-gradient(to top, ${fadeBg}, transparent)` }}
          />
        )}
      </div>
      <div className="flex items-center gap-3 mt-1">
        {needsCollapse && (
          <button onClick={() => setExpanded(!expanded)} className="text-[9px] text-[var(--accent)] cursor-pointer hover:underline">
            {expanded ? t("traces.span.collapse") + " \u2191" : t("traces.span.showFull") + " \u2193"}
          </button>
        )}
        <button onClick={() => navigator.clipboard.writeText(content)} className="text-[9px] text-[var(--accent)] cursor-pointer hover:underline inline-flex items-center gap-0.5">
          <Copy className="h-2.5 w-2.5" />
          {t("traces.span.copy")}
        </button>
      </div>
    </div>
  );
}

// ─── Span Color Helpers ─────────────────────────────────────────────────────

function spanDotColor(span: Span): string {
  if (isModelCallSpan(span)) return "bg-indigo-400";
  if (span.tool === "task") return "bg-purple-400";
  return "bg-amber-400";
}

function spanBarColor(span: Span): string {
  if (isModelCallSpan(span)) return "bg-indigo-500/60";
  if (span.tool === "task") return "bg-purple-500/60";
  return "bg-amber-500/60";
}

// ─── Props ──────────────────────────────────────────────────────────────────

interface OverviewViewProps {
  trace: TraceRecord;
  onNavigateToTrace: (id: string) => void;
}

// ─── Token Distribution Bar ────────────────────────────────────────────────

function TokenDistributionBar({ trace }: { trace: TraceRecord }) {
  const { t } = useTranslation();
  const modelSpans = trace.spans.filter(isModelCallSpan);
  const totalTokens = modelSpans.reduce((sum, s) => sum + (s.total_tokens ?? 0), 0);

  if (modelSpans.length === 0 || totalTokens === 0) return null;

  const shades = ["bg-indigo-400", "bg-indigo-500", "bg-indigo-600", "bg-indigo-700", "bg-indigo-300"];

  return (
    <div className="space-y-1.5">
      <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
        {t("traces.token.distribution")}
      </span>
      <div className="flex h-6 rounded-[var(--radius-sm)] overflow-hidden border border-[var(--border-subtle)]">
        {modelSpans.map((span, i) => {
          const pct = ((span.total_tokens ?? 0) / totalTokens) * 100;
          if (pct < 1) return null;
          const tokenK = ((span.total_tokens ?? 0) / 1000).toFixed(1);
          return (
            <div
              key={span.id}
              className={cn("flex items-center justify-center text-[8px] font-medium text-white truncate px-1", shades[i % shades.length])}
              style={{ width: `${pct}%` }}
              title={`${t("traces.overview.modelN", { n: i + 1 })}: ${formatTokens(span.total_tokens ?? 0)} tokens`}
            >
              {pct > 8 && `${t("traces.overview.modelN", { n: i + 1 })} \u00b7 ${tokenK}k`}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ─── Compact Waterfall ──────────────────────────────────────────────────────

function CompactWaterfall({ trace, selectedSpanId, onSelectSpan }: {
  trace: TraceRecord;
  selectedSpanId: string | null;
  onSelectSpan: (id: string) => void;
}) {
  const { t } = useTranslation();
  const traceStart = new Date(trace.start_time).getTime();
  const traceDuration = trace.metadata.duration_ms ?? 1;

  if (trace.spans.length === 0) {
    return (
      <div className="text-[11px] text-[var(--text-tertiary)] py-4 text-center">
        {t("traces.overview.noSpans")}
      </div>
    );
  }

  // Group tool calls under their preceding model call
  const rows: { span: Span; indent: boolean }[] = [];
  let lastModelIndex = -1;
  for (const span of trace.spans) {
    if (isModelCallSpan(span)) {
      rows.push({ span, indent: false });
      lastModelIndex = rows.length - 1;
    } else {
      rows.push({ span, indent: lastModelIndex >= 0 });
    }
  }

  return (
    <div className="space-y-0">
      {/* Time ruler */}
      <div className="flex items-center justify-between text-[8px] text-[var(--text-tertiary)] font-mono mb-1 px-1">
        <span>0s</span>
        <span>{formatDuration(traceDuration)}</span>
      </div>

      {rows.map(({ span, indent }) => {
        const spanStart = new Date(span.start_time).getTime();
        const offsetMs = spanStart - traceStart;
        const leftPct = Math.max(0, (offsetMs / traceDuration) * 100);
        const widthPct = Math.max(1, ((span.duration_ms ?? 0) / traceDuration) * 100);
        const isSelected = selectedSpanId === span.id;
        const label = isModelCallSpan(span) ? t("traces.span.model") : (span.tool ?? "tool");

        return (
          <div
            key={span.id}
            onClick={() => onSelectSpan(span.id)}
            className={cn(
              "flex items-center gap-2 py-1 px-1 cursor-pointer rounded-[var(--radius-sm)] transition-colors",
              isSelected ? "bg-[var(--accent)]/10" : "hover:bg-[var(--bg-grouped)]",
              indent && "pl-5"
            )}
          >
            {/* Left: dot + name */}
            <div className="flex items-center gap-1.5 w-[140px] flex-shrink-0 truncate">
              <div className={cn("w-2 h-2 rounded-full flex-shrink-0", spanDotColor(span))} />
              <span className="text-[10px] text-[var(--text-secondary)] truncate">{label}</span>
            </div>
            {/* Right: bar */}
            <div className="flex-1 h-4 relative rounded-[var(--radius-sm)] bg-[var(--bg-grouped)]">
              <div
                className={cn("absolute top-0.5 bottom-0.5 rounded-[2px]", spanBarColor(span))}
                style={{ left: `${leftPct}%`, width: `${Math.min(widthPct, 100 - leftPct)}%` }}
              />
            </div>
            <span className="text-[9px] text-[var(--text-tertiary)] font-mono w-[50px] text-right flex-shrink-0">
              {formatDuration(span.duration_ms)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

// ─── Token Donut (SVG) ──────────────────────────────────────────────────────

function TokenDonut({ input, output }: { input: number; output: number }) {
  const { t } = useTranslation();
  const total = input + output;
  if (total === 0) return null;

  const inputPct = input / total;
  const r = 36;
  const circumference = 2 * Math.PI * r;
  const inputArc = circumference * inputPct;
  const outputArc = circumference * (1 - inputPct);

  return (
    <div className="flex flex-col items-center gap-2">
      <svg width="96" height="96" viewBox="0 0 96 96">
        <circle cx="48" cy="48" r={r} fill="none" stroke="var(--border-subtle)" strokeWidth="8" opacity="0.3" />
        <circle
          cx="48" cy="48" r={r}
          fill="none" stroke="#818cf8" strokeWidth="8"
          strokeDasharray={`${inputArc} ${circumference}`}
          strokeDashoffset="0"
          transform="rotate(-90 48 48)"
          strokeLinecap="round"
        />
        <circle
          cx="48" cy="48" r={r}
          fill="none" stroke="#34d399" strokeWidth="8"
          strokeDasharray={`${outputArc} ${circumference}`}
          strokeDashoffset={`${-inputArc}`}
          transform="rotate(-90 48 48)"
          strokeLinecap="round"
        />
        <text x="48" y="46" textAnchor="middle" className="fill-[var(--text-primary)] text-[11px] font-bold">{formatTokens(total)}</text>
        <text x="48" y="58" textAnchor="middle" className="fill-[var(--text-tertiary)] text-[8px]">tokens</text>
      </svg>
      <div className="flex gap-3 text-[9px]">
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded-full bg-indigo-400" />
          <span className="text-[var(--text-secondary)]">{t("traces.token.input")}: {formatTokens(input)}</span>
        </span>
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded-full bg-emerald-400" />
          <span className="text-[var(--text-secondary)]">{t("traces.token.output")}: {formatTokens(output)}</span>
        </span>
      </div>
    </div>
  );
}

// ─── Span Detail Panel ──────────────────────────────────────────────────────

function SpanDetailPanel({ span }: {
  span: Span;
}) {
  const { t } = useTranslation();

  if (isModelCallSpan(span)) {
    return (
      <div className="grid grid-cols-1 lg:grid-cols-[1fr_200px] gap-4 p-3 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)]">
        {/* Left column: prompts + response */}
        <div className="space-y-3 min-w-0">
          {span.system_prompt && (
            <div className="space-y-1">
              <span className="text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                {t("traces.span.systemPrompt")}
              </span>
              <CollapsibleText content={span.system_prompt} maxHeight={80} fadeBg="var(--bg-content)" />
            </div>
          )}
          {span.user_message && (
            <div className="space-y-1">
              <span className="text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                {t("traces.span.userMessage")}
              </span>
              <pre className="whitespace-pre-wrap break-words text-[10px] font-mono leading-relaxed text-[var(--text-primary)]">
                {span.user_message}
              </pre>
            </div>
          )}
          {span.response && (
            <div className="space-y-1">
              <span className="text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                {t("traces.span.response")}
              </span>
              <CollapsibleText content={span.response} maxHeight={120} fadeBg="var(--bg-content)" className="text-emerald-300/90" />
            </div>
          )}
        </div>

        {/* Right column: donut + stats */}
        <div className="flex flex-col items-center gap-3 pt-2">
          <TokenDonut input={span.input_tokens ?? 0} output={span.output_tokens ?? 0} />
          <div className="space-y-1.5 text-[9px] w-full">
            <div className="flex justify-between text-[var(--text-secondary)]">
              <span>{t("traces.span.messages")}</span>
              <span className="font-mono">{span.message_count ?? 0}</span>
            </div>
            <div className="flex justify-between text-[var(--text-secondary)]">
              <span>{t("traces.span.thinking")}</span>
              <span className="font-mono">{span.has_thinking ? t("traces.overview.thinkingOn") : t("traces.overview.thinkingOff")}</span>
            </div>
            {(span.tool_calls_in_response ?? 0) > 0 && (
              <div className="flex justify-between text-[var(--text-secondary)]">
                <span>{t("traces.span.toolsTriggered")}</span>
                <span className="font-mono">{span.tool_calls_in_response}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  }

  // Tool call detail
  return (
    <div className="p-3 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] space-y-3">
      {span.args != null && (
        <div className="space-y-1">
          <span className="text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
            {t("traces.span.args")}
          </span>
          <pre className="whitespace-pre-wrap break-words text-[10px] font-mono leading-relaxed text-[var(--text-primary)] bg-[var(--bg-grouped)] p-2 rounded-[var(--radius-sm)]">
            {typeof span.args === "string" ? span.args : JSON.stringify(span.args, null, 2)}
          </pre>
        </div>
      )}
      {span.result != null && (
        <div className="space-y-1">
          <span className="text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
            {t("traces.span.result")}
          </span>
          <CollapsibleText content={span.result} maxHeight={100} fadeBg="var(--bg-content)" />
        </div>
      )}
      {span.error && (
        <div className="space-y-1">
          <span className="text-[9px] font-medium uppercase tracking-[0.08em] text-red-400">
            {t("traces.span.error")}
          </span>
          <pre className="whitespace-pre-wrap break-words text-[10px] font-mono leading-relaxed text-red-400">
            {span.error}
          </pre>
        </div>
      )}
    </div>
  );
}

// ─── Main Component ─────────────────────────────────────────────────────────

export default function OverviewView({ trace, onNavigateToTrace: _onNavigateToTrace }: OverviewViewProps) {
  const { t } = useTranslation();
  const [selectedSpanId, setSelectedSpanId] = useState<string | null>(null);

  const selectedSpan = useMemo(() => {
    if (!selectedSpanId) return null;
    return trace.spans.find(s => s.id === selectedSpanId) ?? null;
  }, [selectedSpanId, trace.spans]);

  return (
    <div className="space-y-4">
      {/* Summary Stats */}
      <div className="grid grid-cols-4 gap-3">
        {[
          { label: t("traces.summary.duration"), value: formatDuration(trace.metadata.duration_ms) },
          { label: t("traces.summary.tokens"), value: formatTokens(trace.metadata.total_tokens) },
          { label: t("traces.summary.modelCalls"), value: String(trace.metadata.model_calls) },
          { label: t("traces.summary.toolCalls"), value: String(trace.metadata.tool_calls) },
        ].map(({ label, value }) => (
          <div key={label} className="rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-3 text-center">
            <div className="text-[9px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">{label}</div>
            <div className="text-[18px] font-bold text-[var(--text-primary)] tabular-nums mt-0.5">{value}</div>
          </div>
        ))}
      </div>

      {/* Token Distribution */}
      <TokenDistributionBar trace={trace} />

      {/* Compact Waterfall */}
      <CompactWaterfall
        trace={trace}
        selectedSpanId={selectedSpanId}
        onSelectSpan={setSelectedSpanId}
      />

      {/* Span Detail Panel */}
      {selectedSpan ? (
        <SpanDetailPanel
          span={selectedSpan}
        />
      ) : (
        <div className="text-[11px] text-[var(--text-tertiary)] text-center py-6 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)]">
          {t("traces.overview.selectSpan")}
        </div>
      )}
    </div>
  );
}
