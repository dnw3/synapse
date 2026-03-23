import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2, X, Search, ChevronDown, Copy, Trash2 } from "lucide-react";
import { ScrollArea } from "../ui/scroll-area";
import type { Message } from "../../types";
import MessageBubble from "../MessageBubble";
import ApprovalDialog from "../ApprovalDialog";
import type { ApprovalRequest } from "./ChatPanel";
import { MessageDivider, formatSeparatorTime, turnTimestamp, TIME_GAP_MS } from "./chatUtils";

interface MessageListProps {
  messages: Message[];
  loading: boolean;
  streaming?: boolean;
  approvalRequest?: ApprovalRequest | null;
  onApprovalRespond?: (approved: boolean, allowAll?: boolean) => void;
  onToolResultClick?: (content: string, toolName?: string) => void;
  showToolOutput: boolean;
  showSystem: boolean;
  focusMode?: boolean;
  showSearch: boolean;
  searchQuery: string;
  onSearchQueryChange: (query: string) => void;
  onCloseSearch: () => void;
}

export default function MessageList({
  messages,
  loading,
  streaming,
  approvalRequest,
  onApprovalRespond,
  onToolResultClick,
  showToolOutput,
  showSystem,
  showSearch,
  searchQuery,
  onSearchQueryChange,
  onCloseSearch,
}: MessageListProps) {
  const { t } = useTranslation();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollAreaRef = useRef<HTMLDivElement>(null);
  const userScrolledUp = useRef(false);

  // Feature: Tool calls folding
  const [toolsExpanded, setToolsExpanded] = useState<Record<string, boolean>>({});

  // Feature: Deleted messages tracking
  const [deletedMsgKeys, setDeletedMsgKeys] = useState<Set<string>>(new Set());

  // Track whether user has scrolled up (away from bottom)
  useEffect(() => {
    const el = scrollAreaRef.current?.querySelector("[data-radix-scroll-area-viewport]") as HTMLElement | null;
    if (!el) return;
    const handleScroll = () => {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
      userScrolledUp.current = !atBottom;
    };
    el.addEventListener("scroll", handleScroll, { passive: true });
    return () => el.removeEventListener("scroll", handleScroll);
  }, []);

  // Only auto-scroll if user is near the bottom
  useEffect(() => {
    if (!userScrolledUp.current) {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages]);

  // Message filtering: system toggle, tool output toggle, deleted messages
  const visibleMessages = messages.filter((m) => {
    if (m.role === "system" && !showSystem) return false;
    if (m.role === "tool" && !showToolOutput) return false;
    const key = `${m.role}:${m.content.slice(0, 100)}`;
    if (deletedMsgKeys.has(key)) return false;
    return true;
  });

  // Apply search filter
  const displayMessages = searchQuery
    ? visibleMessages.filter((m) =>
        m.content.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : visibleMessages;

  // Group consecutive assistant + tool messages into "turns" so they render
  // under a single avatar instead of multiple separate bubbles.
  const turns: Array<{ type: "human"; messages: Message[] } | { type: "assistant"; messages: Message[] } | { type: "system"; messages: Message[] }> = [];
  for (const msg of displayMessages) {
    if (msg.role === "human") {
      turns.push({ type: "human", messages: [msg] });
    } else if (msg.role === "system") {
      turns.push({ type: "system", messages: [msg] });
    } else {
      // assistant or tool — merge into the current assistant turn
      // When tool output hidden, filter out tool_calls from assistant messages
      const filteredMsg = !showToolOutput && msg.role === "assistant" && msg.tool_calls?.length
        ? { ...msg, tool_calls: [] }
        : msg;
      const last = turns[turns.length - 1];
      if (last && last.type === "assistant") {
        last.messages.push(filteredMsg);
      } else {
        turns.push({ type: "assistant", messages: [filteredMsg] });
      }
    }
  }

  // Helper: delete a message from display
  const handleDeleteMessage = useCallback((msgContent: string, msgRole: string) => {
    const key = `${msgRole}:${msgContent.slice(0, 100)}`;
    setDeletedMsgKeys((prev) => {
      const next = new Set(prev);
      next.add(key);
      return next;
    });
  }, []);

  // Helper: copy a single message as markdown
  const handleCopyMessage = useCallback((content: string) => {
    navigator.clipboard.writeText(content).catch(() => {});
  }, []);

  // Helper: toggle tool group expansion
  const toggleToolGroup = useCallback((groupId: string) => {
    setToolsExpanded((prev) => ({ ...prev, [groupId]: !prev[groupId] }));
  }, []);

  return (
    <>
      {/* Message search bar */}
      {showSearch && (
        <div className="flex items-center gap-2 px-4 py-2 border-b border-[var(--separator)] bg-[var(--bg-grouped)]">
          <Search className="w-4 h-4 text-[var(--text-tertiary)] flex-shrink-0" />
          <input
            autoFocus
            value={searchQuery}
            onChange={(e) => onSearchQueryChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                onCloseSearch();
              }
            }}
            placeholder={t("chat.searchPlaceholder")}
            className="flex-1 bg-transparent text-sm text-[var(--text-primary)] outline-none placeholder-[var(--text-tertiary)]"
          />
          <button
            onClick={onCloseSearch}
            className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      <ScrollArea ref={scrollAreaRef} className="flex-1 min-h-0">
        <div className="max-w-2xl mx-auto px-5 py-5 space-y-5" style={{ background: "var(--bg-window)" }}>
          {displayMessages.length === 0 && !searchQuery && (
            <div className="flex items-center justify-center min-h-[60vh]">
              <div className="text-center space-y-4">
                <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-[var(--accent)]/10 to-[var(--accent)]/5 border border-[var(--accent)]/15 flex items-center justify-center mx-auto shadow-[0_0_24px_var(--accent-glow)]">
                  <span className="text-xl font-semibold bg-gradient-to-b from-[var(--accent-light)] to-[var(--accent)] bg-clip-text text-transparent">S</span>
                </div>
                <h2 className="text-xl font-semibold text-[var(--text-primary)] tracking-[-0.02em]">
                  {t("chat.emptyTitle")}
                </h2>
                <p className="text-sm text-[var(--text-secondary)] max-w-xs">
                  {t("chat.emptySubtitle")}
                </p>
                <p className="text-xs text-[var(--text-tertiary)]">
                  {t("chat.emptyHint")}
                </p>
              </div>
            </div>
          )}

          {displayMessages.length === 0 && searchQuery && (
            <div className="flex items-center justify-center min-h-[40vh]">
              <p className="text-sm text-[var(--text-tertiary)]">{t("chat.searchPlaceholder")}</p>
            </div>
          )}

          {turns.map((turn, i) => {
            // System messages with distinct styling
            if (turn.type === "system") {
              return (
                <div key={i} className="group relative">
                  <div className="px-4 py-2 rounded-[var(--radius-md)] bg-[var(--bg-grouped)]/60 border border-[var(--border-subtle)] text-[13px] italic text-[var(--text-tertiary)] font-mono leading-relaxed">
                    {turn.messages[0].content}
                  </div>
                  {/* Message actions for system messages */}
                  <div className="absolute right-2 top-2 hidden group-hover:flex items-center gap-0.5 bg-[var(--bg-content)] border border-[var(--separator)] rounded-[var(--radius-sm)] px-0.5 py-0.5 shadow-sm z-10">
                    <button
                      onClick={() => handleCopyMessage(turn.messages[0].content)}
                      title={t("chat.copyMarkdown")}
                      className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                    >
                      <Copy className="w-3 h-3" />
                    </button>
                  </div>
                </div>
              );
            }

            // Count consecutive tool calls/results in an assistant turn for folding
            const toolMessages = turn.type === "assistant"
              ? turn.messages.filter((m) => m.role === "tool" || (m.role === "assistant" && m.tool_calls?.length > 0 && !m.content))
              : [];
            const nonToolMessages = turn.type === "assistant"
              ? turn.messages.filter((m) => !(m.role === "tool" || (m.role === "assistant" && m.tool_calls?.length > 0 && !m.content)))
              : turn.messages;
            const hasToolGroup = toolMessages.length > 1;
            const toolGroupId = `turn-${i}`;
            const toolGroupExpanded = toolsExpanded[toolGroupId] ?? false;
            const toolNames = toolMessages
              .filter((m) => m.role === "assistant" && m.tool_calls?.length)
              .map((m) => m.tool_calls[0]?.name)
              .filter(Boolean)
              .slice(0, 3)
              .join(", ");

            // Time separator: insert when gap > 5 minutes between turns
            const currentTs = turnTimestamp(turn.messages);
            const prevTs = i > 0 ? turnTimestamp(turns[i - 1].messages) : undefined;
            const showTimeSep = currentTs && prevTs && (currentTs - prevTs > TIME_GAP_MS);

            return (
              <div key={i}>
                {showTimeSep && (
                  <MessageDivider label={formatSeparatorTime(currentTs)} />
                )}
                {turn.type === "human" ? (
                  <div className="group relative">
                    <MessageBubble message={turn.messages[0]} />
                    {/* Message actions for human messages */}
                    <div className="absolute right-10 top-0 hidden group-hover:flex items-center gap-0.5 bg-[var(--bg-content)] border border-[var(--separator)] rounded-[var(--radius-sm)] px-0.5 py-0.5 shadow-sm z-10">
                      <button
                        onClick={() => handleCopyMessage(turn.messages[0].content)}
                        title={t("chat.copyMarkdown")}
                        className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                      >
                        <Copy className="w-3 h-3" />
                      </button>
                      <button
                        onClick={() => handleDeleteMessage(turn.messages[0].content, turn.messages[0].role)}
                        title={t("chat.deleteMessage")}
                        className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--error)] transition-colors"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  </div>
                ) : hasToolGroup ? (
                  // Render assistant turn with collapsible tool group
                  <div className="space-y-2">
                    {/* Non-tool messages render normally */}
                    {nonToolMessages.length > 0 && (
                      <MessageBubble
                        turn={nonToolMessages}
                        onToolResultClick={onToolResultClick}
                      />
                    )}
                    {/* Collapsible tool group */}
                    <div className="ml-10">
                      <button
                        onClick={() => toggleToolGroup(toolGroupId)}
                        className="flex items-center gap-1.5 px-2.5 py-1 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] bg-[var(--bg-grouped)]/80 border border-[var(--border-subtle)] hover:bg-[var(--bg-hover)] transition-colors"
                      >
                        <ChevronDown className={`w-3 h-3 transition-transform ${toolGroupExpanded ? "" : "-rotate-90"}`} />
                        <span>{t("chat.toolsCollapsed", { count: toolMessages.length })}</span>
                        {toolNames && <span className="text-[var(--text-tertiary)] truncate max-w-[200px]">({toolNames})</span>}
                      </button>
                      {toolGroupExpanded && (
                        <div className="mt-1.5 space-y-1.5 animate-fade-in">
                          <MessageBubble
                            turn={toolMessages}
                            onToolResultClick={onToolResultClick}
                          />
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  <MessageBubble
                    turn={turn.messages}
                    onToolResultClick={onToolResultClick}
                  />
                )}
              </div>
            );
          })}

          {approvalRequest && onApprovalRespond && (
            <ApprovalDialog
              request={approvalRequest}
              onRespond={onApprovalRespond}
            />
          )}

          {loading && !streaming && !approvalRequest && (
            <div className="flex gap-3 animate-fade-in">
              <div className="w-7 h-7 rounded-full bg-[var(--accent-glow)] border border-[var(--accent)]/15 flex items-center justify-center flex-shrink-0">
                <Loader2 className="h-3.5 w-3.5 animate-spin text-[var(--accent-light)]" />
              </div>
              <div className="flex items-center text-[var(--text-secondary)] text-sm">
                <span>{t("chat.thinking")}</span>
              </div>
            </div>
          )}

          <div ref={messagesEndRef} />
        </div>
      </ScrollArea>
    </>
  );
}
