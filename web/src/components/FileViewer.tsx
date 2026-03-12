import { useTranslation } from "react-i18next";
import { FileCode } from "lucide-react";
import Editor from "@monaco-editor/react";

interface Props {
  path: string | null;
  content: string;
  theme?: "light" | "dark";
  onNavigate?: (dirPath: string) => void;
}

function getLanguage(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase();
  const map: Record<string, string> = {
    rs: "rust", ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript",
    py: "python", json: "json", toml: "ini", yaml: "yaml", yml: "yaml",
    md: "markdown", html: "html", css: "css", sh: "shell", bash: "shell",
    sql: "sql", go: "go", java: "java", c: "c", h: "c", cpp: "cpp", hpp: "cpp",
    xml: "xml", svg: "xml", dockerfile: "dockerfile", makefile: "makefile",
    env: "ini", ini: "ini", conf: "ini", cfg: "ini",
  };
  return map[ext || ""] || "plaintext";
}

export default function FileViewer({ path, content, theme = "dark", onNavigate }: Props) {
  const { t } = useTranslation();

  if (!path) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-sm text-[var(--text-tertiary)] gap-3">
        <FileCode className="h-8 w-8 text-[var(--text-tertiary)]/40" />
        {t("files.selectFile")}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-1.5 text-xs font-mono border-b border-[var(--separator)] bg-[var(--bg-elevated)]/60 flex items-center gap-0.5 text-[var(--text-secondary)]">
        {(() => {
          const parts = path.split("/");
          const fileName = parts.pop()!;
          return (
            <>
              {onNavigate && (
                <button
                  onClick={() => onNavigate(".")}
                  className="text-[var(--accent-light)] hover:text-[var(--accent)] transition-colors flex-shrink-0"
                >
                  ~
                </button>
              )}
              {parts.map((seg, i) => {
                const segPath = parts.slice(0, i + 1).join("/");
                return (
                  <span key={segPath} className="flex items-center gap-0.5">
                    <span className="text-[var(--text-tertiary)] mx-0.5">›</span>
                    {onNavigate ? (
                      <button
                        onClick={() => onNavigate(segPath)}
                        className="text-[var(--accent-light)] hover:text-[var(--accent)] transition-colors"
                      >
                        {seg}
                      </button>
                    ) : (
                      <span className="text-[var(--text-tertiary)]">{seg}</span>
                    )}
                  </span>
                );
              })}
              <span className="text-[var(--text-tertiary)] mx-0.5">›</span>
              <span className="text-[var(--text-primary)] font-medium truncate">{fileName}</span>
            </>
          );
        })()}
      </div>
      <div className="flex-1">
        <Editor
          height="100%"
          language={getLanguage(path)}
          value={content}
          theme={theme === "light" ? "light" : "vs-dark"}
          options={{
            readOnly: true,
            minimap: { enabled: false },
            fontSize: 12,
            lineNumbers: "on",
            scrollBeyondLastLine: false,
            wordWrap: "on",
            padding: { top: 8 },
            renderLineHighlight: "none",
          }}
        />
      </div>
    </div>
  );
}
