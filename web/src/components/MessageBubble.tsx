import { useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { User, Bot, Copy, Check } from "lucide-react";
import type { Message } from "../types";
import ToolCallCard from "./ToolCallCard";
import ThinkingBlock from "./ThinkingBlock";
import { useIdentity } from "../App";

interface Props {
  /** Single message (human) */
  message?: Message;
  /** Grouped assistant turn: multiple assistant + tool messages rendered under one avatar */
  turn?: Message[];
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
        <span className="text-[var(--text-tertiary)]/60 ml-0.5">{timeStr}</span>
      )}
    </button>
  );
}

function MarkdownContent({ content }: { content: string }) {
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
                style={oneDark}
                language={match?.[1] || "text"}
                PreTag="div"
                className="!rounded-[var(--radius-md)] !text-[13px] !leading-relaxed !border !border-[var(--border-subtle)] !my-3 !bg-[var(--bg-window)]"
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

function ToolResultSnippet({ content }: { content: string }) {
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
    <pre className="text-xs text-[var(--text-tertiary)] font-mono bg-[var(--bg-content)]/80 rounded-[var(--radius-sm)] px-3 py-2 max-h-24 overflow-auto border border-[var(--border-subtle)] whitespace-pre-wrap break-words">
      {shown}
    </pre>
  );
}

export default function MessageBubble({ message, turn }: Props) {
  const identity = useIdentity();

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

  return (
    <div className="flex gap-3 animate-message-in-left">
      <div className="w-7 h-7 rounded-full bg-[var(--accent-glow)] border border-[var(--accent)]/15 flex items-center justify-center flex-shrink-0 overflow-hidden">
        {identity?.avatar_url ? (
          <img src={identity.avatar_url} alt="" className="w-full h-full object-cover" />
        ) : identity?.emoji ? (
          <span className="text-sm leading-none">{identity.emoji}</span>
        ) : (
          <Bot className="h-3.5 w-3.5 text-[var(--accent-light)]" />
        )}
      </div>
      <div className="flex-1 min-w-0 space-y-2">
        {msgs.map((msg, i) => {
          if (msg.role === "tool") {
            return <ToolResultSnippet key={i} content={msg.content} />;
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

        {lastRequestId && <LogIdBadge requestId={lastRequestId} />}
      </div>
    </div>
  );
}
