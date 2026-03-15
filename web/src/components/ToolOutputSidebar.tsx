import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { useCodeTheme } from "../hooks/useCodeTheme";
import { X } from "lucide-react";

interface ToolOutputSidebarProps {
  open: boolean;
  content: string;
  toolName?: string;
  onClose: () => void;
}

const MIN_WIDTH = 240;
const MAX_WIDTH = 800;
const DEFAULT_WIDTH = 350;

export default function ToolOutputSidebar({ open, content, toolName, onClose }: ToolOutputSidebarProps) {
  const { t } = useTranslation();
  const codeTheme = useCodeTheme();
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const [raw, setRaw] = useState(false);
  const isDragging = useRef(false);
  const startX = useRef(0);
  const startWidth = useRef(DEFAULT_WIDTH);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    isDragging.current = true;
    startX.current = e.clientX;
    startWidth.current = width;
    e.preventDefault();
  }, [width]);

  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      if (!isDragging.current) return;
      const delta = startX.current - e.clientX;
      const newWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidth.current + delta));
      setWidth(newWidth);
    };
    const onMouseUp = () => {
      isDragging.current = false;
    };
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
    };
  }, []);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  // Try to parse content as JSON for rendered mode
  const parsedJson = useMemo(() => {
    if (!content) return null;
    const trimmed = content.trim();
    if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return null;
    try {
      return JSON.parse(trimmed) as unknown;
    } catch {
      return null;
    }
  }, [content]);

  if (!open) return null;

  return (
    <div
      className="flex flex-shrink-0 border-l border-[var(--separator)] bg-[var(--bg-content)] flex-col h-full relative"
      style={{ width }}
    >
      {/* Drag handle */}
      <div
        onMouseDown={onMouseDown}
        className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize z-10 hover:bg-[var(--accent)]/30 transition-colors"
      />

      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2.5 border-b border-[var(--separator)] flex-shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-xs font-semibold text-[var(--text-primary)] truncate">
            {toolName || t("toolSidebar.title")}
          </span>
        </div>
        <div className="flex items-center gap-1 flex-shrink-0">
          {/* Raw / Rendered toggle */}
          <div className="flex items-center rounded-[var(--radius-sm)] border border-[var(--separator)] overflow-hidden text-[10px]">
            <button
              onClick={() => setRaw(false)}
              className={`px-2 py-0.5 transition-colors ${
                !raw
                  ? "bg-[var(--accent)]/10 text-[var(--accent-light)]"
                  : "text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]"
              }`}
            >
              {t("toolSidebar.rendered")}
            </button>
            <button
              onClick={() => setRaw(true)}
              className={`px-2 py-0.5 transition-colors ${
                raw
                  ? "bg-[var(--accent)]/10 text-[var(--accent-light)]"
                  : "text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]"
              }`}
            >
              {t("toolSidebar.raw")}
            </button>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
            title={t("workspace.close")}
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-auto p-3 min-h-0">
        {raw ? (
          <pre className="text-xs font-mono text-[var(--text-secondary)] whitespace-pre-wrap break-words leading-relaxed">
            {content || "(empty)"}
          </pre>
        ) : parsedJson !== null ? (
          <RenderedJson data={parsedJson} />
        ) : (
          <div className="synapse-prose prose max-w-none prose-p:leading-[1.75] prose-headings:text-[var(--text-primary)] prose-a:text-[var(--accent-light)] prose-strong:text-[var(--text-primary)] text-sm">
            <ReactMarkdown
              components={{
                code(props) {
                  const { children, className, ...rest } = props;
                  const match = /language-(\w+)/.exec(className || "");
                  const isMultiline = String(children).includes("\n");
                  const inline = !match && !isMultiline;
                  return inline ? (
                    <code
                      className="px-1 py-0.5 bg-[var(--bg-grouped)] border border-[var(--border-subtle)] rounded text-[var(--accent-light)] text-[0.85em] font-mono"
                      {...rest}
                    >
                      {children}
                    </code>
                  ) : (
                    <SyntaxHighlighter
                      style={codeTheme}
                      language={match?.[1] || "text"}
                      PreTag="div"
                      className="!rounded-[var(--radius-md)] !text-[12px] !leading-relaxed !border !border-[var(--border-subtle)] !my-2"
                    >
                      {String(children).replace(/\n$/, "")}
                    </SyntaxHighlighter>
                  );
                },
              }}
            >
              {content || "(empty)"}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}

/* ---------- JSON rendered views ---------- */

/** Check if an array looks like a file listing (objects with name + is_dir) */
function isFileListing(data: unknown): data is Array<{ name: string; is_dir: boolean; size?: number }> {
  if (!Array.isArray(data) || data.length === 0) return false;
  return data.every(
    (item) =>
      typeof item === "object" &&
      item !== null &&
      "name" in item &&
      "is_dir" in item,
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function FileListingView({ items }: { items: Array<{ name: string; is_dir: boolean; size?: number }> }) {
  // Sort: directories first, then alphabetical
  const sorted = [...items].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.localeCompare(b.name);
  });

  return (
    <div className="space-y-0.5">
      {sorted.map((item) => (
        <div
          key={item.name}
          className="flex items-center gap-2 px-2 py-1 rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] transition-colors text-xs font-mono"
        >
          <span className="flex-shrink-0 w-4 text-center">
            {item.is_dir ? "\u{1F4C1}" : "\u{1F4C4}"}
          </span>
          <span className="text-[var(--text-primary)] truncate flex-1">
            {item.name}{item.is_dir ? "/" : ""}
          </span>
          {item.size != null && (
            <span className="text-[var(--text-tertiary)] flex-shrink-0 text-right" style={{ minWidth: "5em" }}>
              {formatSize(item.size)}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

function RenderedJson({ data }: { data: unknown }) {
  const codeTheme = useCodeTheme();

  // File listing: array of {name, is_dir, size?}
  if (isFileListing(data)) {
    return <FileListingView items={data} />;
  }

  // Generic JSON: pretty-printed with syntax highlighting
  const formatted = JSON.stringify(data, null, 2);
  return (
    <SyntaxHighlighter
      style={codeTheme}
      language="json"
      PreTag="div"
      className="!rounded-[var(--radius-md)] !text-[12px] !leading-relaxed !border !border-[var(--border-subtle)]"
      customStyle={{ margin: 0 }}
    >
      {formatted}
    </SyntaxHighlighter>
  );
}
