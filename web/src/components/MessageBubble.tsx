import { useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { useCodeTheme } from "../hooks/useCodeTheme";
import { User, Bot, Copy, Check, Trash2, Volume2, VolumeX } from "lucide-react";
import type { Message } from "../types";
import ToolCallCard from "./ToolCallCard";
import ThinkingBlock from "./ThinkingBlock";
import { useIdentity } from "../App";

interface Props {
  /** Single message (human) */
  message?: Message;
  /** Grouped assistant turn: multiple assistant + tool messages rendered under one avatar */
  turn?: Message[];
  /** Called when user deletes this message group */
  onDelete?: () => void;
  /** Called when a tool result is clicked — passes full content + tool name */
  onToolResultClick?: (content: string, toolName?: string) => void;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function shortenModel(model: string): string {
  return model
    .replace(/^(openai\/|anthropic\/|google\/)/, "")
    .replace(/-\d{8}$/, "");
}

function copyAsMarkdown(msgs: Message[]) {
  const md = msgs
    .map((msg) => {
      if (msg.role === "assistant" && msg.content) return msg.content;
      if (msg.role === "tool") return `> Tool: ${msg.content.slice(0, 200)}`;
      return "";
    })
    .filter(Boolean)
    .join("\n\n");
  navigator.clipboard.writeText(md).catch(() => {});
}

function LogIdBadge({ requestId }: { requestId: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  // Decode timestamp from LogID: first 13 chars are milliseconds
  const ts = parseInt(requestId.slice(0, 13), 10);
  const timeStr = !isNaN(ts) ? new Date(ts).toLocaleTimeString() : "";

  const handleCopy = () => {
    navigator.clipboard.writeText(requestId).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <button
      onClick={handleCopy}
      title={t("logid.tooltip")}
      className="inline-flex items-center gap-1 mt-1.5 px-2 py-0.5 rounded-full text-[10px] font-mono text-[var(--text-tertiary)] bg-[var(--bg-content)]/60 border border-[var(--border-subtle)] hover:border-[var(--accent)]/30 hover:text-[var(--text-secondary)] transition-all cursor-pointer select-none"
    >
      {copied ? (
        <Check className="h-2.5 w-2.5 text-[var(--success)]" />
      ) : (
        <Copy className="h-2.5 w-2.5" />
      )}
      <span className={copied ? "text-[var(--success)]" : ""}>
        {copied ? t("logid.copied") : `${requestId.slice(0, 8)}...${requestId.slice(-4)}`}
      </span>
      {timeStr && !copied && (
        <span className="text-[var(--text-tertiary)] ml-0.5">{timeStr}</span>
      )}
    </button>
  );
}

function MarkdownContent({ content }: { content: string }) {
  const codeTheme = useCodeTheme();

  return (
    <div className="synapse-prose prose max-w-none prose-p:leading-[1.75] prose-li:leading-[1.75] prose-headings:mt-6 prose-headings:mb-3 prose-headings:tracking-tight prose-h2:text-lg prose-h2:border-b prose-h2:border-[var(--border-subtle)] prose-h2:pb-2 prose-pre:bg-[var(--bg-window)] prose-pre:border prose-pre:border-[var(--border-subtle)] prose-headings:text-[var(--text-primary)] prose-a:text-[var(--accent-light)] prose-strong:text-[var(--text-primary)]">
      <ReactMarkdown
        components={{
          code(props) {
            const { children, className, ...rest } = props;
            const match = /language-(\w+)/.exec(className || "");
            const isMultiline = String(children).includes("\n");
            const inline = !match && !isMultiline;
            return inline ? (
              <code
                className="px-1.5 py-0.5 bg-[var(--bg-grouped)] border border-[var(--border-subtle)] rounded-[var(--radius-sm)] text-[var(--accent-light)] text-[0.875em] font-mono"
                {...rest}
              >
                {children}
              </code>
            ) : (
              <SyntaxHighlighter
                style={codeTheme}
                language={match?.[1] || "text"}
                PreTag="div"
                className="!rounded-[var(--radius-md)] !text-[13px] !leading-relaxed !border !border-[var(--border-subtle)] !my-3"
                customStyle={{
                  padding: "1em",
                  margin: 0,
                  overflow: "auto",
                }}
              >
                {String(children).replace(/\n$/, "")}
              </SyntaxHighlighter>
            );
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}

function ToolResultSnippet({ content, onClick }: { content: string; onClick?: () => void }) {
  const { t } = useTranslation();
  // Try to unescape JSON-encoded strings (e.g. "\"[agent]\\nmax_turns..." → real content)
  let display = content ?? "";
  if (display.startsWith('"') && display.endsWith('"')) {
    try {
      display = JSON.parse(display);
    } catch {
      // keep original
    }
  }
  const truncated = display.length > 500;
  const shown = truncated ? display.slice(0, 500) + "\n..." : display || "(empty)";

  return (
    <pre
      onClick={onClick}
      title={onClick ? t("toolSidebar.title") : undefined}
      className={`text-xs text-[var(--text-tertiary)] font-mono bg-[var(--bg-content)]/80 rounded-[var(--radius-sm)] px-3 py-2 max-h-24 overflow-auto border border-[var(--border-subtle)] whitespace-pre-wrap break-words ${onClick ? "cursor-pointer hover:border-[var(--accent)]/40 hover:bg-[var(--bg-hover)] transition-colors" : ""}`}
    >
      {shown}
    </pre>
  );
}

export default function MessageBubble({ message, turn, onDelete, onToolResultClick }: Props) {
  const { t } = useTranslation();
  const identity = useIdentity();
  const [copyDone, setCopyDone] = useState(false);
  const [ttsPlaying, setTtsPlaying] = useState(false);

  const handleTtsPlay = (text: string) => {
    if (window.speechSynthesis.speaking) {
      window.speechSynthesis.cancel();
      setTtsPlaying(false);
      return;
    }
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.onend = () => setTtsPlaying(false);
    utterance.onerror = () => setTtsPlaying(false);
    setTtsPlaying(true);
    window.speechSynthesis.speak(utterance);
  };

  // Single human message
  if (message && message.role === "human") {
    return (
      <div className="flex gap-3 justify-end animate-message-in-right">
        <div
          className="max-w-[65%] px-4 py-2.5 text-white text-[15px] leading-[1.75]"
          style={{
            background: "var(--accent)",
            borderRadius: "16px 16px 4px 16px",
            boxShadow: "var(--shadow-sm)",
          }}
        >
          {message.content}
        </div>
        <div className="w-7 h-7 rounded-full bg-[var(--bg-content)] border border-[var(--separator)] flex items-center justify-center flex-shrink-0">
          <User className="h-3.5 w-3.5 text-[var(--text-secondary)]" />
        </div>
      </div>
    );
  }

  // Grouped assistant turn (multiple messages under one avatar)
  const msgs = turn ?? (message ? [message] : []);
  if (msgs.length === 0) return null;

  // Find the last request_id with text content for LogID badge
  const msgsWithRid = msgs.filter((m) => m.role === "assistant" && m.content && m.request_id);
  const lastRequestId = msgsWithRid.length > 0 ? msgsWithRid[msgsWithRid.length - 1].request_id : undefined;

  // Find the last assistant message for metadata footer
  const lastAssistantMsg = [...msgs].reverse().find((m) => m.role === "assistant");

  const handleCopy = () => {
    copyAsMarkdown(msgs);
    setCopyDone(true);
    setTimeout(() => setCopyDone(false), 1500);
  };

  return (
    <div className="flex gap-3 animate-message-in-left group/turn">
      <div className="w-7 h-7 rounded-full bg-[var(--accent-glow)] border border-[var(--accent)]/15 flex items-center justify-center flex-shrink-0 overflow-hidden">
        {identity?.avatar_url ? (
          <img src={identity.avatar_url} alt="" className="w-full h-full object-cover" />
        ) : identity?.emoji ? (
          <span className="text-sm leading-none">{identity.emoji}</span>
        ) : (
          <Bot className="h-3.5 w-3.5 text-[var(--accent-light)]" />
        )}
      </div>
      <div className="flex-1 min-w-0 space-y-2 relative">
        {/* Hover action buttons */}
        <div className="absolute -top-2 right-0 hidden group-hover/turn:flex items-center gap-0.5 bg-[var(--bg-content)] border border-[var(--separator)] rounded-[var(--radius-md)] px-0.5 py-0.5 shadow-sm z-10">
          <button
            onClick={handleCopy}
            title={t("chat.copyMarkdown")}
            className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
          >
            {copyDone ? <Check className="w-3 h-3 text-[var(--success)]" /> : <Copy className="w-3 h-3" />}
          </button>
          <button
            onClick={() => {
              const allContent = msgs
                .filter((m) => m.role === "assistant" && m.content)
                .map((m) => m.content)
                .join("\n\n");
              handleTtsPlay(allContent);
            }}
            title={ttsPlaying ? t("tts.stop") : t("tts.play")}
            className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
          >
            {ttsPlaying ? <VolumeX className="w-3 h-3" /> : <Volume2 className="w-3 h-3" />}
          </button>
          {onDelete && (
            <button
              onClick={onDelete}
              title={t("chat.deleteMessage")}
              className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--error)] transition-colors"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          )}
        </div>

        {msgs.map((msg, i) => {
          if (msg.role === "tool") {
            return (
              <ToolResultSnippet
                key={i}
                content={msg.content}
                onClick={onToolResultClick ? () => onToolResultClick(msg.content) : undefined}
              />
            );
          }
          // assistant message
          const hasToolCalls = (msg.tool_calls ?? []).length > 0;
          const hasContent = !!msg.content;
          return (
            <div key={i} className="space-y-2">
              {msg.reasoning && <ThinkingBlock content={msg.reasoning} />}
              {hasContent && (
                <div
                  className="max-w-[70%] px-4 py-2.5 text-[var(--text-primary)]"
                  style={{
                    background: "var(--bg-elevated)",
                    borderRadius: "16px 16px 16px 4px",
                  }}
                >
                  <MarkdownContent content={msg.content} />
                </div>
              )}
              {hasToolCalls && (
                <div className="flex flex-col gap-1.5">
                  {msg.tool_calls.map((tc, j) => (
                    <ToolCallCard key={j} name={tc.name} args={tc.arguments} />
                  ))}
                </div>
              )}
            </div>
          );
        })}

        {/* Metadata footer */}
        {lastAssistantMsg?.usage && (
          <div className="flex items-center gap-2 mt-1 text-[10px] text-[var(--text-tertiary)] font-mono flex-wrap">
            {lastAssistantMsg.usage.input_tokens != null && (
              <span>↑{formatTokens(lastAssistantMsg.usage.input_tokens)}</span>
            )}
            {lastAssistantMsg.usage.output_tokens != null && (
              <span>↓{formatTokens(lastAssistantMsg.usage.output_tokens)}</span>
            )}
            {lastAssistantMsg.usage.cost_usd != null && (
              <span>${lastAssistantMsg.usage.cost_usd.toFixed(4)}</span>
            )}
            {lastAssistantMsg.model && (
              <span>{shortenModel(lastAssistantMsg.model)}</span>
            )}
          </div>
        )}

        {lastRequestId && <LogIdBadge requestId={lastRequestId} />}
      </div>
    </div>
  );
}
