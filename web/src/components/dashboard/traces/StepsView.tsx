import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check, ChevronDown, ChevronRight, AlertCircle, Bot, User, Wrench, MessageSquare, Cpu } from "lucide-react";
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

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })
    + "." + String(d.getMilliseconds()).padStart(3, "0");
}

// ─── Copy Button ────────────────────────────────────────────────────────────

function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      onClick={handleCopy}
      className="inline-flex items-center gap-0.5 text-[9px] text-[var(--accent)] cursor-pointer hover:underline opacity-0 group-hover:opacity-100 transition-opacity"
    >
      {copied ? <Check className="h-2.5 w-2.5" /> : <Copy className="h-2.5 w-2.5" />}
      {copied ? "OK" : t("traces.span.copy")}
    </button>
  );
}

// ─── Content Block (collapsible with scroll) ────────────────────────────────

function ContentBlock({
  content,
  maxHeight = 120,
  className,
  mono = true,
}: {
  content: string;
  maxHeight?: number;
  className?: string;
  mono?: boolean;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const needsCollapse = content.length > 300;

  return (
    <div className="group">
      <div
        className={cn(
          "relative rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-[var(--bg-grouped)] transition-all",
          className
        )}
        style={!expanded && needsCollapse ? { maxHeight, overflow: "hidden" } : expanded ? { maxHeight: 400, overflow: "auto" } : undefined}
      >
        <pre className={cn(
          "whitespace-pre-wrap break-words text-[11px] leading-relaxed p-3 text-[var(--text-primary)]",
          mono && "font-mono"
        )}>
          {content}
        </pre>
        {!expanded && needsCollapse && (
          <div
            className="sticky bottom-0 left-0 right-0 h-10 rounded-b-[var(--radius-sm)] pointer-events-none"
            style={{ background: "linear-gradient(to top, var(--bg-grouped), transparent)" }}
          />
        )}
      </div>
      {needsCollapse && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 text-[10px] text-[var(--accent)] cursor-pointer hover:underline mt-2 relative z-10"
        >
          {expanded ? (
            <><ChevronDown className="h-3 w-3" />{t("traces.span.collapse")}</>
          ) : (
            <><ChevronRight className="h-3 w-3" />{t("traces.span.showFull")} ({content.length.toLocaleString()} chars)</>
          )}
        </button>
      )}
    </div>
  );
}

// ─── Section with label + copy ──────────────────────────────────────────────

function Section({
  icon,
  label,
  children,
  copyText,
  className,
}: {
  icon: React.ReactNode;
  label: string;
  children: React.ReactNode;
  copyText?: string;
  className?: string;
}) {
  return (
    <div className={cn("group space-y-1.5", className)}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span className="text-[var(--text-tertiary)]">{icon}</span>
          <span className="text-[10px] font-semibold uppercase tracking-[0.06em] text-[var(--text-tertiary)]">
            {label}
          </span>
        </div>
        {copyText && <CopyButton text={copyText} />}
      </div>
      {children}
    </div>
  );
}

// ─── Message Bubble ─────────────────────────────────────────────────────────

interface MessageItem {
  role: string;
  content: string;
}

function MessageBubble({ msg, index }: { msg: MessageItem; index: number }) {
  const [expanded, setExpanded] = useState(false);
  const isHuman = msg.role === "human";
  const isAssistant = msg.role === "assistant";
  const isTool = msg.role === "tool";
  const isSystem = msg.role === "system";

  const roleIcon = isHuman ? <User className="h-3 w-3" />
    : isAssistant ? <Bot className="h-3 w-3" />
    : isTool ? <Wrench className="h-3 w-3" />
    : <Cpu className="h-3 w-3" />;

  const roleLabel = isHuman ? "Human"
    : isAssistant ? "Assistant"
    : isTool ? "Tool"
    : isSystem ? "System"
    : msg.role;

  const roleBg = isHuman ? "bg-blue-500/10 border-blue-500/20"
    : isAssistant ? "bg-emerald-500/10 border-emerald-500/20"
    : isTool ? "bg-amber-500/10 border-amber-500/20"
    : "bg-[var(--bg-grouped)] border-[var(--border-subtle)]";

  const roleTextColor = isHuman ? "text-blue-400"
    : isAssistant ? "text-emerald-400"
    : isTool ? "text-amber-400"
    : "text-[var(--text-tertiary)]";

  const needsCollapse = msg.content.length > 200;
  const displayContent = !expanded && needsCollapse
    ? msg.content.substring(0, 200) + "..."
    : msg.content;

  return (
    <div className={cn("rounded-[var(--radius-sm)] border p-2.5 group", roleBg)}>
      <div className="flex items-center justify-between mb-1.5">
        <div className={cn("flex items-center gap-1.5 text-[9px] font-semibold uppercase tracking-wider", roleTextColor)}>
          {roleIcon}
          {roleLabel}
          <span className="text-[var(--text-tertiary)] font-normal">#{index + 1}</span>
        </div>
        <CopyButton text={msg.content} />
      </div>
      <pre className="whitespace-pre-wrap break-words text-[10px] font-mono leading-relaxed text-[var(--text-primary)]">
        {displayContent}
      </pre>
      {needsCollapse && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-[9px] text-[var(--accent)] cursor-pointer hover:underline mt-1"
        >
          {expanded ? "▲ collapse" : `▼ show full (${msg.content.length.toLocaleString()} chars)`}
        </button>
      )}
    </div>
  );
}

// ─── Messages List ──────────────────────────────────────────────────────────

function MessagesList({ messages }: { messages: MessageItem[] }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  if (messages.length === 0) return null;

  // Show last 3 messages by default, expand to show all
  const visibleMessages = expanded ? messages : messages.slice(-3);
  const hiddenCount = messages.length - 3;

  return (
    <Section
      icon={<MessageSquare className="h-3 w-3" />}
      label={t("traces.span.context") + ` (${messages.length})`}
    >
      {!expanded && hiddenCount > 0 && (
        <button
          onClick={() => setExpanded(true)}
          className="w-full text-center text-[10px] text-[var(--accent)] cursor-pointer hover:underline py-1.5 rounded-[var(--radius-sm)] border border-dashed border-[var(--border-subtle)] hover:border-[var(--accent)] transition-colors mb-1"
        >
          ▼ {hiddenCount} earlier messages hidden — click to show all
        </button>
      )}
      <div
        className={cn("space-y-1.5", expanded && "max-h-[400px] overflow-y-auto rounded-[var(--radius-sm)] border border-[var(--border-subtle)] p-2")}
      >
        {visibleMessages.map((msg, i) => {
          const globalIndex = expanded ? i : messages.length - 3 + i;
          return <MessageBubble key={globalIndex} msg={msg} index={globalIndex < 0 ? i : globalIndex} />;
        })}
      </div>
      {expanded && hiddenCount > 0 && (
        <button
          onClick={() => setExpanded(false)}
          className="text-[9px] text-[var(--accent)] cursor-pointer hover:underline mt-1"
        >
          ▲ show only last 3
        </button>
      )}
    </Section>
  );
}

// ─── Stat Chip ──────────────────────────────────────────────────────────────

function StatChip({ label, value }: { label: string; value: string }) {
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-[var(--bg-grouped)] text-[9px] border border-[var(--border-subtle)]">
      <span className="text-[var(--text-tertiary)]">{label}:</span>
      <span className="text-[var(--text-secondary)] font-mono">{value}</span>
    </span>
  );
}

// ─── Model Call Card ────────────────────────────────────────────────────────

function ModelCallCard({ span, index }: { span: Span; index: number }) {
  const { t } = useTranslation();

  // Parse messages from span data
  const messages: MessageItem[] = (() => {
    const raw = (span as unknown as Record<string, unknown>).messages;
    if (!raw) return [];
    if (Array.isArray(raw)) {
      return raw.map((m: Record<string, unknown>) => ({
        role: String(m.role ?? "unknown"),
        content: String(m.content ?? ""),
      }));
    }
    // May be a JSON string
    if (typeof raw === "string") {
      try {
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) {
          return parsed.map((m: Record<string, unknown>) => ({
            role: String(m.role ?? "unknown"),
            content: String(m.content ?? ""),
          }));
        }
      } catch { /* ignore */ }
    }
    return [];
  })();

  return (
    <div className="rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] overflow-hidden shadow-sm">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-indigo-500/10 border-b border-indigo-500/20">
        <div className="flex items-center gap-2">
          <Cpu className="h-3.5 w-3.5 text-indigo-400" />
          <span className="text-[12px] font-semibold text-indigo-400">
            {t("traces.steps.modelCallN", { n: index })}
          </span>
        </div>
        <div className="flex items-center gap-3 text-[10px] text-[var(--text-tertiary)] font-mono">
          <span>{formatDuration(span.duration_ms)}</span>
          <span>{formatTokens(span.total_tokens ?? 0)} tokens</span>
        </div>
      </div>

      {/* Body */}
      <div className="p-4 space-y-4">
        {/* System Prompt */}
        {span.system_prompt && (
          <Section
            icon={<Cpu className="h-3 w-3" />}
            label={t("traces.span.systemPrompt")}
            copyText={span.system_prompt}
          >
            <ContentBlock content={span.system_prompt} maxHeight={100} />
          </Section>
        )}

        {/* Messages (conversation history) */}
        {messages.length > 0 ? (
          <MessagesList messages={messages} />
        ) : span.user_message ? (
          /* Fallback: single user message if no messages array */
          <Section
            icon={<User className="h-3 w-3" />}
            label={t("traces.span.userMessage")}
            copyText={span.user_message}
          >
            <ContentBlock content={span.user_message} maxHeight={80} />
          </Section>
        ) : null}

        {/* Response */}
        {span.response && (
          <Section
            icon={<Bot className="h-3 w-3" />}
            label={t("traces.span.response")}
            copyText={span.response}
          >
            <ContentBlock
              content={span.response}
              maxHeight={120}
              className="border-emerald-500/20 bg-emerald-500/5"
            />
          </Section>
        )}
      </div>

      {/* Footer: stat chips */}
      <div className="flex flex-wrap items-center gap-1.5 px-4 py-2.5 border-t border-[var(--separator)] bg-[var(--bg-grouped)]/30">
        <StatChip label={t("traces.token.input")} value={formatTokens(span.input_tokens ?? 0)} />
        <StatChip label={t("traces.token.output")} value={formatTokens(span.output_tokens ?? 0)} />
        <StatChip label={t("traces.span.messages")} value={String(span.message_count ?? 0)} />
        <StatChip
          label={t("traces.span.thinking")}
          value={span.has_thinking ? t("traces.overview.thinkingOn") : t("traces.overview.thinkingOff")}
        />
        {(span.tool_calls_in_response ?? 0) > 0 && (
          <StatChip label={t("traces.span.toolsTriggered")} value={span.tools ?? String(span.tool_calls_in_response)} />
        )}
      </div>
    </div>
  );
}

// ─── Tool Call Card ─────────────────────────────────────────────────────────

function ToolCallCard({ span }: { span: Span }) {
  const { t } = useTranslation();

  return (
    <div className="rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] overflow-hidden shadow-sm">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-amber-500/10 border-b border-amber-500/20">
        <div className="flex items-center gap-2">
          <Wrench className="h-3.5 w-3.5 text-amber-400" />
          <span className="text-[12px] font-semibold text-amber-400">{span.tool ?? "tool"}</span>
        </div>
        <div className="flex items-center gap-2 text-[10px] text-[var(--text-tertiary)] font-mono">
          {span.error && (
            <span className="text-red-400 flex items-center gap-0.5">
              <AlertCircle className="h-3 w-3" />
              {t("traces.span.error")}
            </span>
          )}
          <span>{formatDuration(span.duration_ms)}</span>
        </div>
      </div>

      {/* Body */}
      <div className="p-4 space-y-4">
        {span.args != null && String(span.args) !== "null" && (
          <Section icon={<ChevronRight className="h-3 w-3" />} label={t("traces.span.args")}>
            <ContentBlock
              content={typeof span.args === "string" ? span.args : JSON.stringify(span.args, null, 2)}
              maxHeight={100}
            />
          </Section>
        )}

        {span.result != null && String(span.result) !== "null" && (
          <Section
            icon={<Check className="h-3 w-3" />}
            label={t("traces.span.result")}
            copyText={span.result}
          >
            <ContentBlock content={span.result} maxHeight={100} />
          </Section>
        )}

        {span.error && (
          <Section icon={<AlertCircle className="h-3 w-3" />} label={t("traces.span.error")}>
            <ContentBlock
              content={span.error}
              className="border-red-500/20 bg-red-500/5"
            />
          </Section>
        )}
      </div>
    </div>
  );
}

// ─── Span Dot Colors ────────────────────────────────────────────────────────

function spanDotColor(span: Span): string {
  if (isModelCallSpan(span)) return "bg-indigo-500 ring-indigo-500/30";
  if (span.tool === "task") return "bg-purple-500 ring-purple-500/30";
  return "bg-amber-500 ring-amber-500/30";
}

// ─── Main Component ─────────────────────────────────────────────────────────

interface StepsViewProps {
  trace: TraceRecord;
  onNavigateToTrace: (id: string) => void;
}

export default function StepsView({ trace, onNavigateToTrace: _onNavigateToTrace }: StepsViewProps) {
  const { t } = useTranslation();

  if (trace.spans.length === 0) {
    return (
      <div className="text-[12px] text-[var(--text-tertiary)] py-12 text-center">
        {t("traces.steps.noSteps")}
      </div>
    );
  }

  let modelCounter = 0;

  return (
    <div className="relative pl-7">
      {/* Vertical timeline line */}
      <div
        className="absolute left-[9px] top-3 bottom-3 w-[2px] rounded-full bg-[var(--border-subtle)]"
      />

      <div className="space-y-4">
        {trace.spans.map((span) => {
          const isModel = isModelCallSpan(span);
          if (isModel) modelCounter++;

          return (
            <div key={span.id} className="relative">
              {/* Timeline dot */}
              <div className={cn(
                "absolute -left-7 top-3.5 w-3 h-3 rounded-full ring-2 z-10",
                spanDotColor(span)
              )} />

              {/* Card */}
              {isModel ? (
                <ModelCallCard span={span} index={modelCounter} />
              ) : (
                <ToolCallCard span={span} />
              )}

              {/* Timestamp */}
              <div className="text-[9px] text-[var(--text-tertiary)] font-mono mt-1 ml-1">
                {formatTime(span.start_time)}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
