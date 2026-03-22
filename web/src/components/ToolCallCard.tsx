import { useTranslation } from "react-i18next";
import type { ToolDisplay } from "../types";

interface Props {
  name: string;
  args: Record<string, unknown>;
  display?: ToolDisplay;
}

export default function ToolCallCard({ name, args, display }: Props) {
  const { t } = useTranslation();

  const emoji = display?.emoji ?? "\u{1f9e9}";
  const label = display?.label ?? t(`tools.${name}`, { defaultValue: name });
  const detail = display?.detail ?? formatFallbackDetail(args);

  return (
    <div
      className="flex items-center gap-2.5 px-3 py-1.5 rounded-[var(--radius-md)] text-xs border transition-colors"
      style={{
        backgroundColor: "color-mix(in srgb, var(--text-secondary) 6%, transparent)",
        borderColor: "color-mix(in srgb, var(--text-secondary) 12%, transparent)",
      }}
    >
      <span className="flex-shrink-0 text-sm">{emoji}</span>
      <span className="font-bold text-[var(--text-secondary)]">{label}</span>
      {detail && (
        <span className="font-mono text-[var(--text-tertiary)] truncate">
          {detail}
        </span>
      )}
    </div>
  );
}

function formatFallbackDetail(args: Record<string, unknown>): string {
  const keys = [
    "command", "path", "file_path", "url", "query",
    "pattern", "name", "description",
  ];
  for (const key of keys) {
    const val = args[key];
    if (typeof val === "string" && val.length > 0) {
      return val.length > 60 ? val.slice(0, 57) + "..." : val;
    }
  }
  return "";
}
