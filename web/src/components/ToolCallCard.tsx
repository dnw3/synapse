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
      className="flex items-center gap-2.5 px-3 py-2 rounded-[var(--radius-sm)] text-xs font-mono border transition-colors"
      style={{
        color: colorVal,
        backgroundColor: `color-mix(in srgb, ${colorVal} 6%, transparent)`,
        borderColor: `color-mix(in srgb, ${colorVal} 12%, transparent)`,
      }}
    >
      <Icon className="h-3.5 w-3.5 flex-shrink-0" />
      <span className="font-medium">{label}</span>
      <span className="text-[var(--text-tertiary)] truncate">{summary}</span>
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
