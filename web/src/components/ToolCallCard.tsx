import { useTranslation } from "react-i18next";
import {
  FileText,
  FilePlus,
  FileEdit,
  Terminal,
  FolderOpen,
  Search,
  GitBranch,
  Wrench,
} from "lucide-react";
interface Props {
  name: string;
  args: Record<string, unknown>;
}

const TOOL_CONFIG: Record<
  string,
  { icon: typeof FileText; colorVar: string }
> = {
  read_file: { icon: FileText, colorVar: "--tool-read" },
  write_file: { icon: FilePlus, colorVar: "--tool-write" },
  edit_file: { icon: FileEdit, colorVar: "--tool-edit" },
  execute: { icon: Terminal, colorVar: "--tool-exec" },
  ls: { icon: FolderOpen, colorVar: "--tool-read" },
  glob: { icon: Search, colorVar: "--tool-read" },
  grep: { icon: Search, colorVar: "--tool-read" },
  task: { icon: GitBranch, colorVar: "--tool-task" },
};

export default function ToolCallCard({ name, args }: Props) {
  const { t } = useTranslation();
  const config = TOOL_CONFIG[name] || { icon: Wrench, colorVar: "--text-secondary" };
  const Icon = config.icon;
  const label = t(`tools.${name}`, { defaultValue: name });
  const summary = formatToolSummary(name, args);
  const colorVal = `var(${config.colorVar})`;

  return (
    <div
      className="flex items-center gap-2.5 px-3 py-1.5 rounded-[var(--radius-md)] text-xs border transition-colors"
      style={{
        backgroundColor: `color-mix(in srgb, ${colorVal} 8%, transparent)`,
        borderColor: `color-mix(in srgb, ${colorVal} 15%, transparent)`,
      }}
    >
      {/* Color dot */}
      <span
        className="flex-shrink-0 rounded-full"
        style={{ width: 8, height: 8, background: colorVal }}
      />
      <span className="font-bold" style={{ color: colorVal }}>{label}</span>
      <span className="font-mono text-[var(--text-tertiary)] truncate">{summary}</span>
    </div>
  );
}

function formatToolSummary(name: string, args: Record<string, unknown>): string {
  switch (name) {
    case "read_file":
      return String(args.path || "");
    case "write_file": {
      const path = String(args.path || "");
      const content = String(args.content || "");
      const lines = content.split("\n").length;
      return `${path} (${lines} lines)`;
    }
    case "edit_file":
      return String(args.path || "");
    case "execute":
      return String(args.command || "").slice(0, 60);
    case "ls":
      return String(args.path || ".");
    case "glob":
      return String(args.pattern || "");
    case "grep":
      return String(args.pattern || "");
    case "task":
      return String(args.task || "").slice(0, 60);
    default:
      return JSON.stringify(args).slice(0, 60);
  }
}
