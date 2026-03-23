import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Minimize2, X } from "lucide-react";
import type { Message, FileAttachment, Session } from "../../types";
import ChatHeader from "./ChatHeader";
import MessageList from "./MessageList";
import ChatInput from "./ChatInput";
import { formatTokens } from "./chatUtils";

export interface ApprovalRequest {
  tool_name: string;
  args_preview: string;
  risk_level: string;
}

interface Props {
  messages: Message[];
  loading: boolean;
  streaming?: boolean;
  approvalRequest?: ApprovalRequest | null;
  onSend: (content: string, attachments?: FileAttachment[]) => void;
  onCancel: () => void;
  onApprovalRespond?: (approved: boolean, allowAll?: boolean) => void;
  onNewChat?: () => void;
  onReset?: () => void;
  onResetSession?: () => void;
  onCompact?: () => void;
  onToggleFocus?: () => void;
  focusMode?: boolean;
  onSetModel?: (name: string) => void;
  onSetThinking?: (level: string) => void;
  onSetVerbose?: (level: string) => void;
  onSetFast?: (mode: string) => void;
  onShowStatus?: () => void;
  onExport?: () => void;
  onShowUsage?: () => void;
  onListAgents?: () => void;
  onRunSkill?: (name: string) => void;
  onClearMessages?: () => void;
  onRefreshMessages?: () => void;
  queueSize?: number;
  chatError?: string | null;
  onDismissError?: () => void;
  contextUsage?: { tokens: number; limit: number };
  onToolResultClick?: (content: string, toolName?: string) => void;
  /* Session selector (OpenClaw pattern) */
  sessions?: Session[];
  activeSessionKey?: string | null;
  onSelectSession?: (key: string) => void;
  modelName?: string | null;
}

export default function ChatPanel({
  messages,
  loading,
  streaming,
  approvalRequest,
  onSend,
  onCancel,
  onApprovalRespond,
  onNewChat,
  onReset,
  onResetSession,
  onCompact,
  onToggleFocus,
  focusMode,
  onSetModel,
  onSetThinking,
  onSetVerbose,
  onSetFast,
  onShowStatus,
  onExport,
  onShowUsage,
  onListAgents,
  onRunSkill,
  onClearMessages,
  onRefreshMessages,
  queueSize,
  chatError,
  onDismissError,
  contextUsage,
  onToolResultClick,
  sessions,
  activeSessionKey,
  onSelectSession,
  modelName,
}: Props) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState("");
  const [showSearch, setShowSearch] = useState(false);

  // Feature: Thinking/Tool output toggle
  const [showToolOutput, setShowToolOutput] = useState(() => {
    return localStorage.getItem("synapse:chat:showToolOutput") !== "false";
  });

  // Feature: System message toggle
  const [showSystem, setShowSystem] = useState(false);

  // Cmd+F / Ctrl+F -> open search; Escape -> exit focus mode
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        setShowSearch(true);
      }
      if (e.key === "Escape" && focusMode && !showSearch) {
        onToggleFocus?.();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [focusMode, showSearch, onToggleFocus]);

  // Context usage warning
  const contextWarning = (() => {
    if (!contextUsage || contextUsage.limit <= 0) return null;
    const pct = (contextUsage.tokens / contextUsage.limit) * 100;
    if (pct < 75) return null;
    const color = pct >= 90 ? "var(--error)" : "var(--warning)";
    return (
      <div className="mx-auto max-w-2xl px-5 py-1">
        <div className="flex items-center gap-2 text-xs" style={{ color }}>
          <AlertTriangle className="w-3.5 h-3.5 flex-shrink-0" />
          <span>
            {t("context.warning", {
              tokens: formatTokens(contextUsage.tokens),
              limit: formatTokens(contextUsage.limit),
              pct: Math.round(pct),
            })}
          </span>
        </div>
      </div>
    );
  })();

  const handleCloseSearch = () => {
    setShowSearch(false);
    setSearchQuery("");
  };

  return (
    <div className="flex flex-col flex-1 min-w-0 min-h-0">
      {/* Top bar: session dropdown + model + refresh (OpenClaw pattern) */}
      <ChatHeader
        sessions={sessions}
        activeSessionKey={activeSessionKey}
        onSelectSession={onSelectSession}
        modelName={modelName}
        showSystem={showSystem}
        onToggleSystem={() => setShowSystem((prev) => !prev)}
        showToolOutput={showToolOutput}
        onToggleToolOutput={() => {
          setShowToolOutput((prev) => {
            const next = !prev;
            localStorage.setItem("synapse:chat:showToolOutput", String(next));
            return next;
          });
        }}
        focusMode={focusMode}
        onToggleFocus={onToggleFocus}
        onRefreshMessages={onRefreshMessages}
      />

      <MessageList
        messages={messages}
        loading={loading}
        streaming={streaming}
        approvalRequest={approvalRequest}
        onApprovalRespond={onApprovalRespond}
        onToolResultClick={onToolResultClick}
        showToolOutput={showToolOutput}
        showSystem={showSystem}
        showSearch={showSearch}
        searchQuery={searchQuery}
        onSearchQueryChange={setSearchQuery}
        onCloseSearch={handleCloseSearch}
      />

      {contextWarning}

      {chatError && (
        <div className="mx-auto max-w-3xl px-5 py-2">
          <div className="flex items-center gap-2 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--error)]/10 border border-[var(--error)]/20 text-sm text-[var(--error)]">
            <span className="flex-1">{chatError}</span>
            <button onClick={onDismissError} className="shrink-0 p-0.5 hover:bg-[var(--error)]/10 rounded">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      )}

      {/* Input */}
      <ChatInput
        messages={messages}
        loading={loading}
        streaming={streaming}
        onSend={onSend}
        onCancel={onCancel}
        onNewChat={onNewChat}
        onReset={onReset}
        onResetSession={onResetSession}
        onCompact={onCompact}
        onToggleFocus={onToggleFocus}
        onSetModel={onSetModel}
        onSetThinking={onSetThinking}
        onSetVerbose={onSetVerbose}
        onSetFast={onSetFast}
        onShowStatus={onShowStatus}
        onExport={onExport}
        onShowUsage={onShowUsage}
        onListAgents={onListAgents}
        onRunSkill={onRunSkill}
        onClearMessages={onClearMessages}
        queueSize={queueSize}
      />

      <style>{`
        @keyframes fadeIn {
          from { opacity: 0; transform: translateY(4px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}

// Re-export focus exit button for use in App.tsx
export function FocusModeExitButton({ onExit }: { onExit: () => void }) {
  const { t } = useTranslation();
  return (
    <button
      onClick={onExit}
      className="fixed top-4 right-4 z-50 p-2 rounded-full bg-[var(--bg-content)] border border-[var(--separator)] shadow-md hover:bg-[var(--bg-hover)] transition-colors"
      title={t("focus.exit")}
    >
      <Minimize2 className="w-4 h-4 text-[var(--text-secondary)]" />
    </button>
  );
}
