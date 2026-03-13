import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Send, Square, Loader2, Paperclip, X } from "lucide-react";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";
import type { Message, FileAttachment } from "../types";
import { api } from "../api";
import MessageBubble from "./MessageBubble";
import ApprovalDialog from "./ApprovalDialog";

export interface ApprovalRequest {
  tool_name: string;
  args_preview: string;
  risk_level: string;
}

interface SlashCommand {
  name: string;
  description: string;
  descriptionZh: string;
  action: () => void;
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
  queueSize?: number;
  chatError?: string | null;
  onDismissError?: () => void;
}

export default function ChatPanel({ messages, loading, streaming, approvalRequest, onSend, onCancel, onApprovalRespond, onNewChat, onReset, queueSize, chatError, onDismissError }: Props) {
  const { t, i18n } = useTranslation();
  const [input, setInput] = useState("");
  const [showCommands, setShowCommands] = useState(false);
  const [selectedCommandIndex, setSelectedCommandIndex] = useState(0);
  const [attachments, setAttachments] = useState<FileAttachment[]>([]);
  const [uploading, setUploading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollAreaRef = useRef<HTMLDivElement>(null);
  const userScrolledUp = useRef(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const commandsRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const isZh = i18n.language?.startsWith("zh");

  const commands: SlashCommand[] = useMemo(() => [
    {
      name: "stop",
      description: "Cancel the current agent execution",
      descriptionZh: "取消当前 Agent 执行",
      action: () => onCancel(),
    },
    {
      name: "new",
      description: "Create a new conversation",
      descriptionZh: "创建新对话",
      action: () => onNewChat?.(),
    },
    {
      name: "reset",
      description: "Reset the current conversation",
      descriptionZh: "重置当前对话",
      action: () => onReset?.(),
    },
  ], [onCancel, onNewChat, onReset]);

  const filteredCommands = useMemo(() => {
    if (!showCommands) return [];
    const filter = input.slice(1).toLowerCase();
    return commands.filter((cmd) => cmd.name.startsWith(filter));
  }, [showCommands, input, commands]);

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    setInput(value);

    if (value.startsWith("/") && !value.includes(" ")) {
      setShowCommands(true);
      setSelectedCommandIndex(0);
    } else {
      setShowCommands(false);
    }
  }, []);

  const executeCommand = useCallback((cmd: SlashCommand) => {
    setInput("");
    setShowCommands(false);
    cmd.action();
  }, []);

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

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Keep selected index in bounds when filtered list changes
  useEffect(() => {
    if (selectedCommandIndex >= filteredCommands.length) {
      setSelectedCommandIndex(Math.max(0, filteredCommands.length - 1));
    }
  }, [filteredCommands.length, selectedCommandIndex]);

  const handleSubmit = () => {
    const content = input.trim();
    if (!content && attachments.length === 0) return;
    setInput("");
    const atts = attachments.length > 0 ? [...attachments] : undefined;
    setAttachments([]);
    onSend(content || "(attached files)", atts);
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;

    setUploading(true);
    try {
      for (let i = 0; i < files.length; i++) {
        const result = await api.uploadFile(files[i]);
        setAttachments((prev) => [
          ...prev,
          { id: result.id, filename: result.filename, mime_type: result.mime_type, url: result.url },
        ]);
      }
    } catch (err) {
      console.error("Upload failed:", err);
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
  };

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  };

  const isImageMime = (mime: string) => mime.startsWith("image/");

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (showCommands && filteredCommands.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedCommandIndex((prev) =>
          prev < filteredCommands.length - 1 ? prev + 1 : 0
        );
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedCommandIndex((prev) =>
          prev > 0 ? prev - 1 : filteredCommands.length - 1
        );
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        executeCommand(filteredCommands[selectedCommandIndex]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setShowCommands(false);
        return;
      }
    }

    if (e.key === "Escape" && showCommands) {
      e.preventDefault();
      setShowCommands(false);
      return;
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const visibleMessages = messages.filter((m) => m.role !== "system");

  // Group consecutive assistant + tool messages into "turns" so they render
  // under a single avatar instead of multiple separate bubbles.
  const turns: Array<{ type: "human"; messages: Message[] } | { type: "assistant"; messages: Message[] }> = [];
  for (const msg of visibleMessages) {
    if (msg.role === "human") {
      turns.push({ type: "human", messages: [msg] });
    } else {
      // assistant or tool — merge into the current assistant turn
      const last = turns[turns.length - 1];
      if (last && last.type === "assistant") {
        last.messages.push(msg);
      } else {
        turns.push({ type: "assistant", messages: [msg] });
      }
    }
  }

  return (
    <div className="flex flex-col flex-1 min-w-0 min-h-0">
      <ScrollArea ref={scrollAreaRef} className="flex-1 min-h-0">
        <div className="max-w-2xl mx-auto px-5 py-5 space-y-5" style={{ background: "var(--bg-window)" }}>
          {visibleMessages.length === 0 && (
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

          {turns.map((turn, i) =>
            turn.type === "human" ? (
              <MessageBubble key={i} message={turn.messages[0]} />
            ) : (
              <MessageBubble key={i} turn={turn.messages} />
            )
          )}

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
      <div className="border-t border-[var(--separator)] p-3 bg-[var(--bg-grouped)] backdrop-blur-md">
        <div className="max-w-3xl mx-auto relative">
          {/* Slash command palette */}
          {showCommands && filteredCommands.length > 0 && (
            <div
              ref={commandsRef}
              className="absolute bottom-full left-0 mb-1 w-72 max-h-[200px] overflow-y-auto rounded-[var(--radius-lg)] border border-[var(--separator)] bg-[var(--bg-content)] animate-fade-in z-50"
              style={{ animation: "fadeIn 120ms ease-out", boxShadow: "var(--shadow-md)" }}
            >
              {filteredCommands.map((cmd, i) => (
                <button
                  key={cmd.name}
                  type="button"
                  className={`w-full text-left px-3 py-2 flex items-baseline gap-2 text-sm transition-colors duration-75 ${
                    i === selectedCommandIndex
                      ? "bg-[var(--accent)]/10 text-[var(--text-primary)]"
                      : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                  }`}
                  onMouseEnter={() => setSelectedCommandIndex(i)}
                  onMouseDown={(e) => {
                    e.preventDefault(); // keep focus on textarea
                    executeCommand(cmd);
                  }}
                >
                  <span className="font-semibold text-[var(--text-primary)] shrink-0">
                    /{cmd.name}
                  </span>
                  <span className="text-xs text-[var(--text-tertiary)] truncate">
                    {isZh ? cmd.descriptionZh : cmd.description}
                  </span>
                </button>
              ))}
            </div>
          )}

          {/* Attachment chips */}
          {attachments.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mb-2">
              {attachments.map((att) => (
                <div
                  key={att.id}
                  className="flex items-center gap-1.5 px-2 py-1 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--separator)] text-xs text-[var(--text-secondary)] max-w-[200px]"
                >
                  {isImageMime(att.mime_type) && (
                    <img
                      src={att.url}
                      alt={att.filename}
                      className="w-6 h-6 rounded object-cover shrink-0"
                    />
                  )}
                  <span className="truncate">{att.filename}</span>
                  <button
                    type="button"
                    onClick={() => removeAttachment(att.id)}
                    className="shrink-0 p-0.5 rounded hover:bg-[var(--bg-elevated)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="flex gap-2 items-end">
            <input
              ref={fileInputRef}
              type="file"
              multiple
              accept="image/*,.txt,.md,.csv,.json,.xml,.yaml,.yml,.toml,.log,.pdf,.rs,.py,.js,.ts,.go,.java,.c,.cpp,.h,.rb,.sh"
              onChange={handleFileSelect}
              className="hidden"
            />
            <Button
              variant="ghost"
              size="icon"
              onClick={() => fileInputRef.current?.click()}
              disabled={loading || uploading}
              className="h-[44px] w-[44px] rounded-[var(--radius-md)] shrink-0 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
              title={t("chat.attachFile")}
            >
              {uploading ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Paperclip className="h-4 w-4" />
              )}
            </Button>
            <textarea
              ref={inputRef}
              value={input}
              onChange={handleInputChange}
              onKeyDown={handleKeyDown}
              placeholder={t("chat.placeholder")}
              className="flex-1 resize-none bg-[var(--bg-content)] border border-[var(--separator)] rounded-[var(--radius-xl)] px-4 py-2.5 text-sm text-[var(--text-primary)] placeholder-[var(--text-tertiary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent)]/30 focus:border-[var(--accent)]/40 min-h-[44px] max-h-[140px] transition-all duration-150"
              rows={1}
            />
            {loading ? (
              <div className="flex gap-1.5">
                <Button variant="destructive" size="icon" onClick={onCancel} className="h-7 w-7 rounded-full" title={t("chat.stop")}>
                  <Square className="h-3.5 w-3.5" />
                </Button>
                {/* Queue button: allow sending even when busy (OpenClaw pattern) */}
                <Button
                  size="icon"
                  onClick={handleSubmit}
                  disabled={!input.trim() && attachments.length === 0}
                  className="h-7 w-7 rounded-full bg-[var(--accent)] hover:bg-[var(--accent-light)] relative"
                  title={t("chat.queue")}
                >
                  <Send className="h-3.5 w-3.5" />
                  {(queueSize ?? 0) > 0 && (
                    <span className="absolute -top-1 -right-1 min-w-[16px] h-4 flex items-center justify-center text-[9px] font-bold rounded-full bg-[var(--accent)] text-white">
                      {queueSize}
                    </span>
                  )}
                </Button>
              </div>
            ) : (
              <Button
                size="icon"
                onClick={handleSubmit}
                disabled={!input.trim() && attachments.length === 0}
                className="h-7 w-7 rounded-full bg-[var(--accent)] hover:bg-[var(--accent-light)]"
              >
                <Send className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
        </div>
      </div>

      <style>{`
        @keyframes fadeIn {
          from { opacity: 0; transform: translateY(4px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}
