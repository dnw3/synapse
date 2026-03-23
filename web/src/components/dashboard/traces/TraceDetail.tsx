import { useTranslation } from "react-i18next";
import { useState } from "react";
import { ArrowLeft, Clock, Coins, Cpu, Wrench, ChevronRight, Copy, Check } from "lucide-react";
import { cn } from "../../../lib/cn";
import type { TraceRecord, TraceSubView } from "./types";
import OverviewView from "./OverviewView";
import TimelineView from "./TimelineView";
import StepsView from "./StepsView";

interface TraceDetailProps {
  trace: TraceRecord;
  subView: TraceSubView;
  onSubViewChange: (view: TraceSubView) => void;
  onBack: () => void;
  onNavigateToTrace: (requestId: string) => void;
}

function formatDuration(ms: number | null): string {
  if (ms === null) return "\u2014";
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
}

function formatTokens(n: number): string {
  return n.toLocaleString();
}

function truncateId(id: string, len = 12): string {
  if (id.length <= len) return id;
  return `...${id.slice(-len)}`;
}

function CopyableLogId({ id }: { id: string }) {
  const [copied, setCopied] = useState(false);
  const { t } = useTranslation();

  const handleCopy = () => {
    navigator.clipboard.writeText(id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      onClick={handleCopy}
      title={copied ? t("logid.copied") : t("logid.tooltip")}
      className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-[var(--radius-sm)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[12px] font-mono text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)] transition-colors cursor-pointer"
    >
      {copied ? <Check className="h-3 w-3 text-green-500" /> : <Copy className="h-3 w-3 opacity-50" />}
      <span>{copied ? t("logid.copied") : id}</span>
    </button>
  );
}

const SUB_VIEWS: TraceSubView[] = ["overview", "timeline", "steps"];

export function TraceDetail({
  trace, subView, onSubViewChange, onBack, onNavigateToTrace,
}: TraceDetailProps) {
  const { t } = useTranslation();

  const summaryItems = [
    {
      label: t("traces.summary.duration"),
      value: formatDuration(trace.metadata.duration_ms),
      icon: <Clock className="h-3.5 w-3.5" />,
    },
    {
      label: t("traces.summary.tokens"),
      value: formatTokens(trace.metadata.total_tokens),
      icon: <Coins className="h-3.5 w-3.5" />,
    },
    {
      label: t("traces.summary.modelCalls"),
      value: String(trace.metadata.model_calls),
      icon: <Cpu className="h-3.5 w-3.5" />,
    },
    {
      label: t("traces.summary.toolCalls"),
      value: String(trace.metadata.tool_calls),
      icon: <Wrench className="h-3.5 w-3.5" />,
    },
  ];

  return (
    <div className="flex flex-col gap-4 p-4 overflow-y-auto flex-1 min-h-0">
      {/* Back button */}
      <button
        onClick={onBack}
        className="inline-flex items-center gap-1.5 text-[12px] text-[var(--text-tertiary)] hover:text-[var(--accent)] transition-colors cursor-pointer self-start"
      >
        <ArrowLeft className="h-3.5 w-3.5" />
        {t("traces.detail.backToList")}
      </button>

      {/* LogID — always visible, copyable */}
      <div className="flex items-center gap-2">
        <span className="text-[10px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)]">LogID</span>
        <CopyableLogId id={trace.request_id} />
      </div>

      {/* Breadcrumb (only when trace has a parent) */}
      {trace.metadata.parent_request_id && (
        <div className="flex items-center gap-1.5 text-[11px] text-[var(--text-tertiary)] font-mono">
          <button
            onClick={onBack}
            className="hover:text-[var(--accent)] transition-colors cursor-pointer"
          >
            {t("traces.breadcrumb.allTraces")}
          </button>
          <ChevronRight className="h-3 w-3" />
          <button
            onClick={() => onNavigateToTrace(trace.metadata.parent_request_id!)}
            className="hover:text-[var(--accent)] transition-colors cursor-pointer"
          >
            {truncateId(trace.metadata.parent_request_id)} <span className="text-[var(--text-tertiary)] opacity-60">({t("traces.breadcrumb.parent")})</span>
          </button>
          <ChevronRight className="h-3 w-3" />
          <span className="text-[var(--text-tertiary)] opacity-60">{t("traces.breadcrumb.task")}</span>
          <ChevronRight className="h-3 w-3" />
          <span className="text-[var(--text-primary)]">
            {truncateId(trace.request_id)}
          </span>
        </div>
      )}

      {/* Summary bar */}
      <div className="grid grid-cols-4 gap-3">
        {summaryItems.map((item) => (
          <div
            key={item.label}
            className="rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-3 flex flex-col gap-1"
          >
            <div className="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)]">
              <span className="text-[var(--text-tertiary)] opacity-60">{item.icon}</span>
              {item.label}
            </div>
            <span className="text-[20px] font-bold tracking-[-0.02em] text-[var(--text-primary)] tabular-nums font-mono">
              {item.value}
            </span>
          </div>
        ))}
      </div>

      {/* Sub-tab switcher */}
      <div className="flex items-center gap-0 border-b border-[var(--separator)]">
        {SUB_VIEWS.map((view) => (
          <button
            key={view}
            onClick={() => onSubViewChange(view)}
            className={cn(
              "px-4 py-2 text-[12px] font-semibold transition-colors cursor-pointer border-b-2 -mb-px",
              subView === view
                ? "border-[var(--accent)] text-[var(--text-primary)]"
                : "border-transparent text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
            )}
          >
            {t(`traces.detail.${view}`)}
          </button>
        ))}
      </div>

      {/* Content area */}
      {subView === "overview" && <OverviewView trace={trace} onNavigateToTrace={onNavigateToTrace} />}
      {subView === "timeline" && <TimelineView trace={trace} />}
      {subView === "steps" && <StepsView trace={trace} onNavigateToTrace={onNavigateToTrace} />}
    </div>
  );
}
