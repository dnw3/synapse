import { useTranslation } from "react-i18next";
import { ChevronDown, Eye, EyeOff, Brain, Maximize2, RefreshCw } from "lucide-react";
import type { Session } from "../../types";
import { formatRelativeTime, truncateLabel } from "./chatUtils";

interface ChatHeaderProps {
  sessions?: Session[];
  activeSessionKey?: string | null;
  onSelectSession?: (key: string) => void;
  modelName?: string | null;
  showSystem: boolean;
  onToggleSystem: () => void;
  showToolOutput: boolean;
  onToggleToolOutput: () => void;
  focusMode?: boolean;
  onToggleFocus?: () => void;
  onRefreshMessages?: () => void;
}

export default function ChatHeader({
  sessions,
  activeSessionKey,
  onSelectSession,
  modelName,
  showSystem,
  onToggleSystem,
  showToolOutput,
  onToggleToolOutput,
  focusMode,
  onToggleFocus,
  onRefreshMessages,
}: ChatHeaderProps) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-3 px-4 h-[40px] flex-shrink-0 border-b border-[var(--separator)] bg-[var(--bg-window)]/80 backdrop-blur-[20px]">
      {sessions && sessions.length > 0 && (
        <div className="relative">
          <select
            value={activeSessionKey ?? ""}
            onChange={(e) => onSelectSession?.(e.target.value)}
            className="appearance-none pl-2 pr-6 py-1 text-[12px] font-medium bg-[var(--bg-grouped)] text-[var(--text-primary)] rounded-[var(--radius-sm)] border border-[var(--border-subtle)] outline-none cursor-pointer max-w-[260px] truncate"
          >
            {sessions.map((s) => {
              const label = s.displayName || (s.channel === "web" ? "main" : s.sessionKey.slice(0, 12));
              const channel = s.channel || "web";
              const kind = s.kind && s.kind !== "web" && s.kind !== channel ? `:${s.kind}` : "";
              const channelTag = `[${channel}${kind}]`;
              const timeAgo = s.createdAt ? formatRelativeTime(s.createdAt) : "";
              const displayLabel = truncateLabel(label, 24);
              return (
                <option key={s.sessionKey} value={s.sessionKey}>
                  {displayLabel} {channelTag}{timeAgo ? ` ${timeAgo}` : ""}
                </option>
              );
            })}
          </select>
          <ChevronDown className="absolute right-1.5 top-1/2 -translate-y-1/2 w-3 h-3 text-[var(--text-tertiary)] pointer-events-none" />
        </div>
      )}
      {modelName && (
        <span className="text-[11px] font-mono text-[var(--text-tertiary)] truncate max-w-[180px]">
          {modelName}
        </span>
      )}
      <div className="flex-1" />
      {/* System message toggle */}
      <button
        onClick={onToggleSystem}
        className={`p-1.5 rounded-[var(--radius-sm)] transition-colors ${showSystem ? "text-[var(--accent)] hover:bg-[var(--bg-hover)]" : "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"}`}
        title={showSystem ? t("chat.hideSystem") : t("chat.showSystem")}
      >
        {showSystem ? <Eye className="w-3.5 h-3.5" /> : <EyeOff className="w-3.5 h-3.5" />}
      </button>
      {/* Tool output toggle */}
      <button
        onClick={onToggleToolOutput}
        className={`p-1.5 rounded-[var(--radius-sm)] transition-colors ${showToolOutput ? "text-[var(--accent)] hover:bg-[var(--bg-hover)]" : "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"}`}
        title={t("chat.toggleToolOutput")}
      >
        <Brain className="w-3.5 h-3.5" />
      </button>
      {/* Focus mode toggle */}
      <button
        onClick={() => onToggleFocus?.()}
        className={`p-1.5 rounded-[var(--radius-sm)] transition-colors ${focusMode ? "text-[var(--accent)] hover:bg-[var(--bg-hover)]" : "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"}`}
        title={t("chat.focusMode")}
      >
        <Maximize2 className="w-3.5 h-3.5" />
      </button>
      <button
        onClick={() => onRefreshMessages?.()}
        className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
        title={t("chat.refresh")}
      >
        <RefreshCw className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}
