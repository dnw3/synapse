import { useTranslation } from "react-i18next";
import { ChevronLeft, Folder, FileText, FileCode } from "lucide-react";
import { ScrollArea } from "./ui/scroll-area";
import { cn } from "../lib/cn";
import type { FileEntry } from "../types";

interface Props {
  currentPath: string;
  entries: FileEntry[];
  onSelect: (path: string, isDir: boolean) => void;
  onNavigateUp: () => void;
}

function getFileIcon(name: string) {
  const ext = name.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "rs":
    case "ts":
    case "tsx":
    case "js":
    case "jsx":
    case "py":
    case "go":
    case "java":
      return <FileCode className="h-3.5 w-3.5 text-[var(--accent-light)]" />;
    default:
      return <FileText className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />;
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
}

export default function FileTree({
  currentPath,
  entries,
  onSelect,
  onNavigateUp,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="text-xs">
      <div className="flex items-center gap-1 px-3 py-2 border-b border-[var(--border-subtle)] min-h-[32px]">
        {currentPath !== "." && (
          <button
            onClick={onNavigateUp}
            className="p-0.5 rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors flex-shrink-0"
            title={t("files.navigateUp")}
          >
            <ChevronLeft className="h-3.5 w-3.5" />
          </button>
        )}
        <div className="flex items-center gap-0.5 font-mono text-[var(--text-secondary)] truncate">
          <button
            onClick={() => onSelect(".", true)}
            className="hover:text-[var(--accent-light)] transition-colors flex-shrink-0"
          >
            ~
          </button>
          {currentPath !== "." &&
            currentPath.split("/").map((seg, i, arr) => {
              const segPath = arr.slice(0, i + 1).join("/");
              return (
                <span key={segPath} className="flex items-center gap-0.5">
                  <span className="text-[var(--text-tertiary)]">/</span>
                  <button
                    onClick={() => onSelect(segPath, true)}
                    className="hover:text-[var(--accent-light)] transition-colors"
                  >
                    {seg}
                  </button>
                </span>
              );
            })}
        </div>
      </div>

      <ScrollArea className="h-full">
        <div className="py-0.5">
          {entries.map((entry) => {
            const fullPath =
              currentPath === "." ? entry.name : `${currentPath}/${entry.name}`;

            return (
              <button
                key={entry.name}
                onClick={() => onSelect(fullPath, entry.is_dir)}
                className={cn(
                  "w-full flex items-center gap-2 px-3 py-1.5 text-left transition-colors",
                  "hover:bg-[var(--bg-hover)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                )}
              >
                {entry.is_dir ? (
                  <Folder className="h-3.5 w-3.5 text-[var(--warning)]/80" />
                ) : (
                  getFileIcon(entry.name)
                )}
                <span className="truncate flex-1">{entry.name}</span>
                {!entry.is_dir && entry.size != null && (
                  <span className="text-[var(--text-tertiary)] flex-shrink-0 font-mono">
                    {formatSize(entry.size)}
                  </span>
                )}
              </button>
            );
          })}

          {entries.length === 0 && (
            <div className="px-3 py-8 text-[var(--text-tertiary)] text-center">
              {t("files.emptyDir")}
            </div>
          )}
        </div>
      </ScrollArea>
    </div>
  );
}
