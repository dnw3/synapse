import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Heart, User, Palette, Bot, Wrench, Rocket, Clock,
  FileText, Plus, RotateCcw, Trash2, Save, FilePlus, ChevronDown,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import { LoadingSpinner, useToast, ToastContainer, useInlineConfirm } from "./shared";
import type { WorkspaceFileEntry, IdentityInfo, AgentEntry } from "../../types/dashboard";

const ICON_MAP: Record<string, React.ReactNode> = {
  heart: <Heart className="h-4 w-4" />,
  user: <User className="h-4 w-4" />,
  palette: <Palette className="h-4 w-4" />,
  bot: <Bot className="h-4 w-4" />,
  wrench: <Wrench className="h-4 w-4" />,
  rocket: <Rocket className="h-4 w-4" />,
  clock: <Clock className="h-4 w-4" />,
  "file-text": <FileText className="h-4 w-4" />,
};

const CATEGORY_ORDER = ["personality", "profile", "session", "tools", "bootstrap", "custom"];

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export default function WorkspacePage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const toast = useToast();

  const [files, setFiles] = useState<WorkspaceFileEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);

  // Agent selection for per-agent workspace
  const [agents, setAgents] = useState<AgentEntry[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<string | undefined>(undefined);
  const [agentDropdownOpen, setAgentDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Editor state
  const [editingFile, setEditingFile] = useState<string | null>(null);
  const [editContent, setEditContent] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [saving, setSaving] = useState(false);

  // New file modal
  const [creating, setCreating] = useState(false);
  const [newFilename, setNewFilename] = useState("");

  // Load agents list
  useEffect(() => {
    api.fetchAgents().then(data => {
      if (data) setAgents(data);
    });
  }, [api]);

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setAgentDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const agentParam = selectedAgent === "default" ? undefined : selectedAgent;

  const loadFiles = useCallback(async () => {
    const data = await api.fetchWorkspaceFiles(agentParam);
    if (data) setFiles(data);
    setLoading(false);
  }, [api, agentParam]);

  const loadIdentity = useCallback(async () => {
    const data = await api.fetchIdentity(agentParam);
    if (data) setIdentity(data);
  }, [api, agentParam]);

  useEffect(() => {
    setEditingFile(null);
    setLoading(true);
    loadFiles();
    loadIdentity();
  }, [loadFiles, loadIdentity]);

  const openEditor = useCallback(async (filename: string) => {
    const data = await api.fetchWorkspaceFile(filename, agentParam);
    if (data) {
      setEditingFile(filename);
      setEditContent(data.content);
      setOriginalContent(data.content);
    }
  }, [api, agentParam]);

  const handleCreateFromTemplate = useCallback(async (filename: string) => {
    const tmpl = files.find(f => f.filename === filename && f.is_template);
    if (!tmpl) return;
    const result = await api.resetWorkspaceFile(filename, agentParam);
    if (result?.ok) {
      toast.addToast(t("workspace.created", { name: filename }), "success");
      await loadFiles();
      await loadIdentity();
      openEditor(filename);
    }
  }, [api, agentParam, files, loadFiles, loadIdentity, openEditor, toast, t]);

  const handleSave = useCallback(async () => {
    if (!editingFile) return;
    setSaving(true);
    const fileExists = files.find(f => f.filename === editingFile)?.exists;
    const result = fileExists
      ? await api.saveWorkspaceFile(editingFile, editContent, agentParam)
      : await api.createWorkspaceFile(editingFile, editContent, agentParam);
    setSaving(false);
    if (result?.ok) {
      toast.addToast(t("workspace.saved"), "success");
      setOriginalContent(editContent);
      await loadFiles();
      await loadIdentity();
    } else {
      toast.addToast(t("workspace.saveFailed"), "error");
    }
  }, [editingFile, editContent, files, api, agentParam, loadFiles, loadIdentity, toast, t]);

  const handleReset = useCallback(async () => {
    if (!editingFile) return;
    const result = await api.resetWorkspaceFile(editingFile, agentParam);
    if (result?.ok) {
      toast.addToast(t("workspace.resetDone"), "success");
      const data = await api.fetchWorkspaceFile(editingFile, agentParam);
      if (data) {
        setEditContent(data.content);
        setOriginalContent(data.content);
      }
      await loadFiles();
      await loadIdentity();
    }
  }, [editingFile, api, agentParam, loadFiles, loadIdentity, toast, t]);

  const deleteConfirm = useInlineConfirm();
  const handleDelete = useCallback(async () => {
    if (!editingFile) return;
    const ok = await api.deleteWorkspaceFile(editingFile, agentParam);
    if (ok) {
      toast.addToast(t("workspace.deleted"), "success");
      setEditingFile(null);
      deleteConfirm.reset();
      await loadFiles();
      await loadIdentity();
    }
  }, [editingFile, api, agentParam, loadFiles, loadIdentity, toast, t, deleteConfirm]);

  const handleCreateNew = useCallback(async () => {
    const fname = newFilename.endsWith(".md") ? newFilename : `${newFilename}.md`;
    if (!fname || fname === ".md") return;
    const result = await api.createWorkspaceFile(fname, `# ${fname.replace(".md", "")}\n\n`, agentParam);
    if (result?.ok) {
      toast.addToast(t("workspace.created", { name: fname }), "success");
      setCreating(false);
      setNewFilename("");
      await loadFiles();
      openEditor(fname);
    } else {
      toast.addToast(t("workspace.createFailed"), "error");
    }
  }, [newFilename, api, agentParam, loadFiles, openEditor, toast, t]);

  // Group files by category
  const grouped = CATEGORY_ORDER
    .map(cat => ({
      category: cat,
      label: t(`workspace.category.${cat}`),
      items: files.filter(f => f.category === cat),
    }))
    .filter(g => g.items.length > 0);

  if (loading) return <LoadingSpinner />;

  const hasChanges = editContent !== originalContent;
  const editingMeta = files.find(f => f.filename === editingFile);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between mb-4 flex-shrink-0">
        <div className="flex items-center gap-3">
          {identity?.emoji && (
            <span className="text-2xl">{identity.emoji}</span>
          )}
          <div>
            <h3 className="text-lg font-semibold text-[var(--text-primary)]">
              {t("workspace.title")}
            </h3>
            <p className="text-xs text-[var(--text-tertiary)]">
              {t("workspace.subtitle")}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {/* Agent selector */}
          {agents.length > 1 && (
            <div className="relative" ref={dropdownRef}>
              <button
                onClick={() => setAgentDropdownOpen(!agentDropdownOpen)}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
              >
                <Bot className="h-3.5 w-3.5" />
                {selectedAgent || "default"}
                <ChevronDown className="h-3 w-3" />
              </button>
              {agentDropdownOpen && (
                <div className="absolute right-0 top-full mt-1 z-50 min-w-[160px] bg-[var(--bg-elevated)] border border-[var(--separator)] rounded-[var(--radius-md)] shadow-[var(--shadow-lg)] py-1">
                  {agents.map(agent => (
                    <button
                      key={agent.name}
                      onClick={() => {
                        setSelectedAgent(agent.name === "default" ? undefined : agent.name);
                        setAgentDropdownOpen(false);
                      }}
                      className={cn(
                        "w-full text-left px-3 py-1.5 text-xs hover:bg-[var(--bg-hover)] transition-colors",
                        (selectedAgent || "default") === agent.name
                          ? "text-[var(--accent)] font-medium"
                          : "text-[var(--text-secondary)]"
                      )}
                    >
                      <div className="flex items-center justify-between">
                        <span>{agent.name}</span>
                        {agent.is_default && (
                          <span className="text-[10px] text-[var(--text-tertiary)]">
                            {t("agents.default")}
                          </span>
                        )}
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
          <button
            onClick={() => setCreating(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)] transition-all"
          >
            <Plus className="h-3.5 w-3.5" />
            {t("workspace.newFile")}
          </button>
        </div>
      </div>

      {/* Identity preview bar */}
      {identity && (identity.name || identity.emoji || identity.theme_color) && (
        <div className="flex items-center gap-3 px-4 py-2.5 rounded-[var(--radius-lg)] bg-[var(--bg-elevated)]/70 border border-[var(--border-subtle)] mb-4 flex-shrink-0">
          {identity.avatar_url ? (
            <img src={identity.avatar_url} alt="" className="w-8 h-8 rounded-full object-cover" />
          ) : identity.emoji ? (
            <span className="text-xl">{identity.emoji}</span>
          ) : null}
          <div className="flex-1 min-w-0">
            <span className="text-sm font-medium text-[var(--text-primary)]">
              {identity.name || "Synapse"}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)] ml-2">
              {t("workspace.currentIdentity")}
            </span>
          </div>
          {identity.theme_color && (
            <span
              className="w-5 h-5 rounded-full border border-[var(--separator)]"
              style={{ background: identity.theme_color }}
              title={`Theme: ${identity.theme_color}`}
            />
          )}
        </div>
      )}

      {/* Main: left file list + right editor */}
      <div className="flex gap-4 flex-1 min-h-0">
        {/* Left: file list */}
        <div className="w-64 flex-shrink-0 overflow-y-auto pr-1">
          {files.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-40 text-[var(--text-tertiary)]">
              <FileText className="h-8 w-8 mb-2 opacity-40" />
              <span className="text-xs">{t("workspace.noFiles")}</span>
            </div>
          ) : (
            <div className="space-y-4">
              {grouped.map(group => (
                <div key={group.category}>
                  <h4 className="text-[10px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider mb-1.5 px-2">
                    {group.label}
                  </h4>
                  <div className="space-y-0.5">
                    {group.items.map(file => (
                      <button
                        key={file.filename}
                        onClick={() => file.exists ? openEditor(file.filename) : handleCreateFromTemplate(file.filename)}
                        className={cn(
                          "w-full text-left rounded-[var(--radius-md)] px-2.5 py-2 transition-all duration-150 group",
                          editingFile === file.filename
                            ? "bg-[var(--accent)]/10 text-[var(--accent-light)]"
                            : file.exists
                              ? "hover:bg-[var(--bg-hover)] text-[var(--text-primary)]"
                              : "hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]",
                        )}
                      >
                        <div className="flex items-center gap-2">
                          <span className={cn(
                            "flex-shrink-0",
                            editingFile === file.filename
                              ? "text-[var(--accent)]"
                              : file.exists
                                ? "text-[var(--text-tertiary)]"
                                : "text-[var(--text-tertiary)]"
                          )}>
                            {ICON_MAP[file.icon] || <FileText className="h-4 w-4" />}
                          </span>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-1.5">
                              <span className="text-[12px] font-medium truncate">
                                {file.filename}
                              </span>
                              {!file.exists && (
                                <FilePlus className="h-3 w-3 opacity-0 group-hover:opacity-100 transition-opacity text-[var(--accent)]" />
                              )}
                            </div>
                            {file.exists && (
                              <div className="text-[10px] text-[var(--text-tertiary)] font-mono mt-0.5">
                                {file.size_bytes != null && formatBytes(file.size_bytes)}
                                {file.modified && ` · ${timeAgo(file.modified)}`}
                              </div>
                            )}
                            {!file.exists && (
                              <div className="text-[10px] text-[var(--text-tertiary)] mt-0.5">
                                {t("workspace.notCreated")}
                              </div>
                            )}
                          </div>
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Right: editor */}
        <div className="flex-1 min-w-0 flex flex-col rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-elevated)]/50">
          {editingFile ? (
            <>
              {/* Editor header */}
              <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-subtle)] flex-shrink-0">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="flex-shrink-0 text-[var(--accent)]">
                    {ICON_MAP[editingMeta?.icon || ""] || <FileText className="h-4 w-4" />}
                  </span>
                  <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
                    {editingFile}
                  </span>
                  {hasChanges && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--warning)]/10 text-[var(--warning)] flex-shrink-0">
                      {t("workspace.unsaved")}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1.5 flex-shrink-0">
                  {editingMeta?.is_template && (
                    <button
                      onClick={handleReset}
                      title={t("workspace.resetDefault")}
                      className="p-1.5 rounded-[var(--radius-md)] text-[var(--warning)] hover:bg-[var(--warning)]/10 transition-colors"
                    >
                      <RotateCcw className="h-3.5 w-3.5" />
                    </button>
                  )}
                  <button
                    onClick={() => {
                      if (deleteConfirm.confirming === editingFile) {
                        handleDelete();
                      } else {
                        deleteConfirm.requestConfirm(editingFile);
                      }
                    }}
                    title={deleteConfirm.confirming === editingFile ? t("workspace.deleteConfirm") : t("workspace.delete")}
                    className={cn(
                      "p-1.5 rounded-[var(--radius-md)] transition-colors",
                      deleteConfirm.confirming === editingFile
                        ? "bg-[var(--error)]/10 text-[var(--error)]"
                        : "text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/10"
                    )}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>

              {/* Description */}
              {editingMeta?.description && (
                <div className="px-4 py-2 text-[11px] text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]/50 flex-shrink-0">
                  {editingMeta.description}
                </div>
              )}

              {/* Textarea */}
              <div className="flex-1 min-h-0 p-3">
                <textarea
                  value={editContent}
                  onChange={e => setEditContent(e.target.value)}
                  className="w-full h-full p-3 text-[13px] font-mono leading-relaxed bg-[var(--bg-content)] border border-[var(--border-subtle)] rounded-[var(--radius-md)] resize-none focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/30 text-[var(--text-primary)] placeholder-[var(--text-tertiary)]"
                  spellCheck={false}
                />
              </div>

              {/* Save bar */}
              <div className="flex items-center gap-2 px-4 py-3 border-t border-[var(--border-subtle)] flex-shrink-0">
                <button
                  onClick={handleSave}
                  disabled={saving || !hasChanges}
                  className={cn(
                    "flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] transition-all",
                    hasChanges
                      ? "bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]"
                      : "bg-[var(--bg-content)] text-[var(--text-tertiary)] cursor-not-allowed"
                  )}
                >
                  <Save className="h-3.5 w-3.5" />
                  {saving ? t("workspace.saving") : t("workspace.save")}
                </button>
                <kbd className="text-[10px] text-[var(--text-tertiary)] font-mono">
                  {navigator.platform?.includes("Mac") ? "⌘S" : "Ctrl+S"}
                </kbd>
              </div>
            </>
          ) : (
            /* Empty state */
            <div className="flex-1 flex flex-col items-center justify-center text-[var(--text-tertiary)]">
              <FileText className="h-10 w-10 mb-3 opacity-30" />
              <p className="text-sm">{t("workspace.selectFile") || "Select a file to edit"}</p>
              <p className="text-[11px] mt-1">{t("workspace.subtitle")}</p>
            </div>
          )}
        </div>
      </div>

      {/* New file modal */}
      {creating && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={() => setCreating(false)}>
          <div className="bg-[var(--bg-elevated)] border border-[var(--separator)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] p-6 w-96 max-w-[90vw]" onClick={e => e.stopPropagation()}>
            <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
              {t("workspace.newFileTitle")}
            </h3>
            <input
              value={newFilename}
              onChange={e => setNewFilename(e.target.value)}
              placeholder="CUSTOM.md"
              className="w-full px-3 py-2 text-sm bg-[var(--bg-content)] border border-[var(--border-subtle)] rounded-[var(--radius-md)] focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/30 text-[var(--text-primary)] placeholder-[var(--text-tertiary)]"
              autoFocus
              onKeyDown={e => e.key === "Enter" && handleCreateNew()}
            />
            <p className="text-[11px] text-[var(--text-tertiary)] mt-1.5">
              {t("workspace.newFileHint")}
            </p>
            <div className="flex justify-end gap-2 mt-4">
              <button
                onClick={() => setCreating(false)}
                className="px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] bg-[var(--bg-content)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
              >
                {t("workspace.cancel")}
              </button>
              <button
                onClick={handleCreateNew}
                disabled={!newFilename}
                className="px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)] transition-all disabled:opacity-50"
              >
                {t("workspace.create")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Toast */}
      <ToastContainer toasts={toast.toasts} />
    </div>
  );
}
