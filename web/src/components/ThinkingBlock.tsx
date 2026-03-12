import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, Brain } from "lucide-react";

interface Props {
  content: string;
}

export default function ThinkingBlock({ content }: Props) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  if (!content) return null;

  // Preview: first line or first 80 chars
  const firstLine = content.split("\n")[0];
  const preview = firstLine.length > 80 ? firstLine.slice(0, 80) + "..." : firstLine;

  return (
    <div className="rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-[var(--bg-surface)]/60 text-sm">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 w-full px-3 py-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors cursor-pointer select-none"
      >
        <ChevronRight
          className={`h-3.5 w-3.5 transition-transform ${expanded ? "rotate-90" : ""}`}
        />
        <Brain className="h-3.5 w-3.5" />
        <span className="font-medium text-xs">{t("reasoning.title")}</span>
        {!expanded && (
          <span className="text-xs text-[var(--text-tertiary)]/60 truncate ml-1">
            {preview}
          </span>
        )}
      </button>
      {expanded && (
        <div className="px-3 pb-2 pt-0">
          <pre className="text-xs text-[var(--text-secondary)] font-mono whitespace-pre-wrap break-words max-h-64 overflow-auto leading-relaxed">
            {content}
          </pre>
        </div>
      )}
    </div>
  );
}
