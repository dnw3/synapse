import { useState, useEffect, useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  Play, Pause, X, Filter,
  ArrowDownLeft, ArrowUpRight, MessageSquare,
} from "lucide-react";
import { EmptyState } from "../shared";
import { cn } from "../../../lib/cn";
import { useMessageStream, getChannelColors, MAX_MESSAGES } from "./useMessageStream";

// ─── Messages Tab ────────────────────────────────────────────────────────────

export function MessagesTab() {
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
