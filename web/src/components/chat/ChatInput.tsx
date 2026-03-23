import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Send, Square, Loader2, Paperclip, X, Plus, Download } from "lucide-react";
import { Button } from "../ui/button";
import type { Message, FileAttachment } from "../../types";
import { exportToMarkdown } from "./chatUtils";

interface SlashCommand {
  name: string;
  description: string;
  descriptionZh: string;
  args?: string;
  action: (arg?: string) => void;
}

interface ChatInputProps {
  messages: Message[];
  loading: boolean;
  streaming?: boolean;
  onSend: (content: string, attachments?: FileAttachment[]) => void;
  onCancel: () => void;
  onNewChat?: () => void;
  onReset?: () => void;
  onResetSession?: () => void;
  onCompact?: () => void;
  onToggleFocus?: () => void;
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
  queueSize?: number;
}

export default function ChatInput({
  messages,
  loading,
  onSend,
  onCancel,
  onNewChat,
  onReset,
  onResetSession,
  onCompact,
  onToggleFocus,
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
  queueSize,
}: ChatInputProps) {
  const { t, i18n } = useTranslation();
  const [input, setInput] = useState("");
  const [showCommands, setShowCommands] = useState(false);
  const [selectedCommandIndex, setSelectedCommandIndex] = useState(0);
  const [attachments, setAttachments] = useState<FileAttachment[]>([]);
  const [uploading, setUploading] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const commandsRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Input history
  const inputHistoryRef = useRef<string[]>([]);
  const historyIndexRef = useRef(-1);

  const isZh = i18n.language?.startsWith("zh");

  const handleExport = useCallback(() => {
    if (onExport) {
      onExport();
      return;
    }
    const md = exportToMarkdown(messages);
    const blob = new Blob([md], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `conversation-${Date.now()}.md`;
    a.click();
    URL.revokeObjectURL(url);
  }, [messages, onExport]);

  const handleClearMessages = useCallback(() => {
    onClearMessages?.();
  }, [onClearMessages]);

  const showHelpMessage = useCallback(() => {
    // Build a help message injected as a system-like info
    // We call onSend with a special prefix. Since we can't inject without backend,
    // we open an info overlay — for now we post to chat as human message
    const helpText = commands.map((c) => {
      const argHint = c.args ? ` ${c.args}` : "";
      const desc = isZh ? c.descriptionZh : c.description;
      return `\`/${c.name}${argHint}\` — ${desc}`;
    }).join("\n");
    onSend(`[Help]\n${helpText}`);
  }, [isZh]); // eslint-disable-line react-hooks/exhaustive-deps

  const commands: SlashCommand[] = useMemo(() => [
    {
      name: "stop",
      description: t("commands.stop"),
      descriptionZh: t("commands.stop"),
      action: () => onCancel(),
    },
    {
      name: "new",
      description: t("commands.new"),
      descriptionZh: t("commands.new"),
      action: () => onNewChat?.(),
    },
    {
      name: "reset",
      description: t("commands.reset"),
      descriptionZh: t("commands.reset"),
      action: () => onReset?.(),
    },
    {
      name: "compact",
      description: t("commands.compact"),
      descriptionZh: t("commands.compact"),
      action: () => onCompact?.(),
    },
    {
      name: "focus",
      description: t("commands.focus"),
      descriptionZh: t("commands.focus"),
      action: () => onToggleFocus?.(),
    },
    {
      name: "model",
      description: t("commands.model"),
      descriptionZh: t("commands.model"),
      args: "<name>",
      action: (arg?: string) => arg && onSetModel?.(arg),
    },
    {
      name: "think",
      description: t("commands.think"),
      descriptionZh: t("commands.think"),
      args: "<off|low|medium|high>",
      action: (arg?: string) => arg && onSetThinking?.(arg),
    },
    {
      name: "verbose",
      description: t("commands.verbose"),
      descriptionZh: t("commands.verbose"),
      args: "<on|off|full>",
      action: (arg?: string) => arg && onSetVerbose?.(arg),
    },
    {
      name: "fast",
      description: t("commands.fast"),
      descriptionZh: t("commands.fast"),
      args: "<on|off>",
      action: (arg?: string) => arg && onSetFast?.(arg),
    },
    {
      name: "help",
      description: t("commands.help"),
      descriptionZh: t("commands.help"),
      action: () => showHelpMessage(),
    },
    {
      name: "status",
      description: t("commands.status"),
      descriptionZh: t("commands.status"),
      action: () => onShowStatus?.(),
    },
    {
      name: "export",
      description: t("commands.export"),
      descriptionZh: t("commands.export"),
      action: () => handleExport(),
    },
    {
      name: "usage",
      description: t("commands.usage"),
      descriptionZh: t("commands.usage"),
      action: () => onShowUsage?.(),
    },
    {
      name: "agents",
      description: t("commands.agents"),
      descriptionZh: t("commands.agents"),
      action: () => onListAgents?.(),
    },
    {
      name: "skill",
      description: t("commands.skill"),
      descriptionZh: t("commands.skill"),
      args: "<name>",
      action: (arg?: string) => arg && onRunSkill?.(arg),
    },
    {
      name: "clear",
      description: t("commands.clear"),
      descriptionZh: t("commands.clear"),
      action: () => handleClearMessages(),
    },
  ], [onCancel, onNewChat, onReset, onCompact, onToggleFocus, onSetModel, onSetThinking, onSetVerbose, onSetFast, onShowStatus, onShowUsage, onListAgents, onRunSkill, handleExport, handleClearMessages, t, showHelpMessage]);

  const filteredCommands = useMemo(() => {
    if (!showCommands) return [];
    // Extract command name portion (before any space)
    const filter = input.slice(1).split(" ")[0].toLowerCase();
    return commands.filter((cmd) => cmd.name.startsWith(filter));
  }, [showCommands, input, commands]);

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    setInput(value);
    // Reset history navigation on manual edit
    historyIndexRef.current = -1;

    if (value.startsWith("/") && !value.includes("\n")) {
      setShowCommands(true);
      setSelectedCommandIndex(0);
    } else {
      setShowCommands(false);
    }
  }, []);

  const executeCommand = useCallback((cmd: SlashCommand, rawInput: string) => {
    setInput("");
    setShowCommands(false);
    // Parse arg from input: everything after "/name "
    const spaceIdx = rawInput.indexOf(" ");
    const arg = spaceIdx !== -1 ? rawInput.slice(spaceIdx + 1).trim() : undefined;
    cmd.action(arg);
  }, []);

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

    // Push to input history
    if (content) {
      inputHistoryRef.current = [content, ...inputHistoryRef.current.slice(0, 99)];
      historyIndexRef.current = -1;
    }

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
        const formData = new FormData();
        formData.append("file", files[i]);
        const res = await fetch("/api/upload", { method: "POST", body: formData });
        if (!res.ok) throw new Error(`Upload failed: ${res.status}`);
        const result = await res.json() as { id: string; filename: string; mime_type: string; url: string };
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

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
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
        executeCommand(filteredCommands[selectedCommandIndex], input);
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

    // Input history navigation (up/down when not in command mode)
    if (!showCommands) {
      const textarea = e.currentTarget;
      if (e.key === "ArrowUp") {
        // Only navigate history when cursor is on first line
        const selStart = textarea.selectionStart;
        const textBefore = textarea.value.slice(0, selStart);
        const isFirstLine = !textBefore.includes("\n");
        if (isFirstLine && inputHistoryRef.current.length > 0) {
          e.preventDefault();
          const nextIndex = historyIndexRef.current + 1;
          if (nextIndex < inputHistoryRef.current.length) {
            historyIndexRef.current = nextIndex;
            setInput(inputHistoryRef.current[nextIndex]);
          }
          return;
        }
      }
      if (e.key === "ArrowDown" && historyIndexRef.current >= 0) {
        e.preventDefault();
        const nextIndex = historyIndexRef.current - 1;
        if (nextIndex < 0) {
          historyIndexRef.current = -1;
          setInput("");
        } else {
          historyIndexRef.current = nextIndex;
          setInput(inputHistoryRef.current[nextIndex]);
        }
        return;
      }
    }

    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleResetSession = useCallback(() => {
    if (onResetSession) {
      if (messages.length > 0 && !window.confirm(t("chat.resetConfirm"))) return;
      onResetSession();
    }
  }, [onResetSession, messages.length, t]);

  return (
    <div className="border-t border-[var(--separator)] p-3 bg-[var(--bg-grouped)] backdrop-blur-md">
      <div className="max-w-3xl mx-auto relative">
        {/* Slash command palette */}
        {showCommands && filteredCommands.length > 0 && (
          <div
            ref={commandsRef}
            className="absolute bottom-full left-0 mb-1 w-80 max-h-[260px] overflow-y-auto rounded-[var(--radius-lg)] border border-[var(--separator)] bg-[var(--bg-content)] animate-fade-in z-50"
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
                  executeCommand(cmd, input);
                }}
              >
                <span className="font-semibold text-[var(--text-primary)] shrink-0 font-mono text-xs">
                  /{cmd.name}{cmd.args ? ` ${cmd.args}` : ""}
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
          {/* Reset session (+ button, OpenClaw pattern) */}
          <Button
            variant="ghost"
            size="icon"
            onClick={handleResetSession}
            className="h-7 w-7 rounded-full shrink-0 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
            title={t("chat.resetSession")}
          >
            <Plus className="h-3.5 w-3.5" />
          </Button>
          {/* Export button */}
          <Button
            variant="ghost"
            size="icon"
            onClick={handleExport}
            className="h-7 w-7 rounded-full shrink-0 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
            title={t("chat.export")}
          >
            <Download className="h-3.5 w-3.5" />
          </Button>
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
  );
}
