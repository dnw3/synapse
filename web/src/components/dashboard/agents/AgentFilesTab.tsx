import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { FileText, Save, Eye, Pencil } from "lucide-react";
import { cn } from "../../../lib/cn";
import { fetchJSON, putJSON } from "../../../lib/api";

interface AgentFilesTabProps {
  agentName: string;
}

export default function AgentFilesTab({ agentName }: AgentFilesTabProps) {
  const { t } = useTranslation();

  const [workspaceFiles, setWorkspaceFiles] = useState<{ name: string; size?: number }[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState("");
  const [fileLoading, setFileLoading] = useState(false);
  const [fileSaving, setFileSaving] = useState(false);
  const [fileSaved, setFileSaved] = useState(false);
  const [filePreview, setFilePreview] = useState(true);

  const agentParam = agentName !== "default" ? agentName : undefined;

  const loadWorkspaceFiles = useCallback(async () => {
    try {
      const qs = agentParam ? `?agent=${encodeURIComponent(agentParam)}` : "";
      const data = await fetchJSON<{ filename: string; size_bytes?: number }[]>(`/workspace/files${qs}`);
      if (data) setWorkspaceFiles(data.map((f) => ({ name: f.filename, size: f.size_bytes ?? 0 })));
    } catch { /* ignore */ }
  }, [agentParam]);

  useEffect(() => {
    loadWorkspaceFiles();
    setSelectedFile(null);
    setFileContent("");
  }, [agentName, loadWorkspaceFiles]);

  const loadFile = useCallback(async (filename: string) => {
    setSelectedFile(filename);
    setFileContent("");
    setFileSaved(false);
    setFileLoading(true);
    try {
      const qs = agentParam ? `?agent=${encodeURIComponent(agentParam)}` : "";
      const data = await fetchJSON<{ content: string }>(`/workspace/files/${encodeURIComponent(filename)}${qs}`);
      setFileContent(data?.content ?? "");
    } catch {
      setFileContent("");
    } finally {
      setFileLoading(false);
    }
  }, [agentParam]);

  const saveFile = useCallback(async () => {
    if (!selectedFile) return;
    setFileSaving(true);
    try {
      const qs = agentParam ? `?agent=${encodeURIComponent(agentParam)}` : "";
      await putJSON(`/workspace/files/${encodeURIComponent(selectedFile)}${qs}`, { content: fileContent });
      setFileSaved(true);
      setTimeout(() => setFileSaved(false), 2000);
    } catch { /* ignore */ }
    finally {
      setFileSaving(false);
    }
  }, [selectedFile, fileContent, agentParam]);

  return (
    <div className="space-y-3">
      {/* File buttons — dynamic from workspace API */}
      <div className="flex flex-wrap gap-2">
        {workspaceFiles.map((f) => (
          <button
            key={f.name}
            onClick={() => loadFile(f.name)}
            className={cn(
              "flex items-center gap-1.5 px-2.5 py-1.5 rounded-[var(--radius-md)] text-[12px] font-mono border transition-colors cursor-pointer",
              selectedFile === f.name
                ? "bg-[var(--accent)]/10 border-[var(--accent)]/40 text-[var(--accent)]"
                : "bg-[var(--bg-content)]/50 border-[var(--border-subtle)] text-[var(--text-secondary)] hover:border-[var(--separator)]"
            )}
          >
            <FileText className="h-3 w-3" />
            {f.name}
          </button>
        ))}
        {workspaceFiles.length === 0 && (
          <span className="text-[11px] text-[var(--text-tertiary)]">
            {t("agents.noChannels")}
          </span>
        )}
      </div>

      {/* Editor / Preview */}
      {fileLoading ? (
        <div className="text-[12px] text-[var(--text-tertiary)] py-4">
          {t("agentFiles.loading")}
        </div>
      ) : selectedFile ? (
        <div className="space-y-2">
          {/* Toolbar: Edit/Preview toggle + Save */}
          <div className="flex items-center justify-between">
            <div className="flex items-center rounded-[var(--radius-md)] border border-[var(--border-subtle)] overflow-hidden">
              <button
                onClick={() => setFilePreview(true)}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium transition-colors cursor-pointer",
                  filePreview
                    ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-content)]"
                )}
              >
                <Eye className="h-3 w-3" />
                Preview
              </button>
              <button
                onClick={() => setFilePreview(false)}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium transition-colors cursor-pointer border-l border-[var(--border-subtle)]",
                  !filePreview
                    ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-content)]"
                )}
              >
                <Pencil className="h-3 w-3" />
                Edit
              </button>
            </div>
            <div className="flex items-center gap-2">
              {fileSaved && (
                <span className="text-[11px] text-[var(--success)]">
                  {t("agentFiles.saved")}
                </span>
              )}
              <button
                onClick={saveFile}
                disabled={fileSaving || filePreview}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[11px] font-medium hover:brightness-110 active:scale-[0.97] transition-all cursor-pointer disabled:opacity-40"
              >
                <Save className="h-3 w-3" />
                {fileSaving ? t("agentFiles.loading") : t("agentFiles.save")}
              </button>
            </div>
          </div>

          {/* Content area */}
          {filePreview ? (
            <div className="px-4 py-3 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] min-h-[300px] max-h-[500px] overflow-y-auto prose prose-sm max-w-none text-[13px]">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {fileContent || "*Empty file*"}
              </ReactMarkdown>
            </div>
          ) : (
            <textarea
              value={fileContent}
              onChange={(e) => { setFileContent(e.target.value); setFileSaved(false); }}
              rows={16}
              className="w-full px-4 py-3 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] font-mono text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors resize-y leading-relaxed"
              spellCheck={false}
            />
          )}
        </div>
      ) : (
        <div className="text-[12px] text-[var(--text-tertiary)] py-4">
          {t("agentFiles.noAgent")}
        </div>
      )}
    </div>
  );
}
