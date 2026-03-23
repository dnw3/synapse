import { useState, useMemo, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Search, FolderOpen, ChevronDown, ChevronRight,
  EyeOff, Package, Zap, ExternalLink,
  X, Loader2, CheckCircle2,
  FileText, Copy, Check,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { useCodeTheme } from "../../../hooks/useCodeTheme";
import {
  useSkills, useToggleSkill,
} from "../../../hooks/queries/useSkillsQueries";
import { fetchJSON } from "../../../lib/api";
import type { SkillEntry } from "../../../types/dashboard";
import {
  EmptyState,
  LoadingSkeleton,
} from "../shared";
import { useToast } from "../../ui/toast";
import { cn } from "../../../lib/cn";
import {
  type SourceGroup,
  SOURCE_ORDER,
  SOURCE_STYLE,
  resolveSource,
  extToLang,
  SkillCard,
} from "./skillsConstants";

// ===========================================================================
// LOCAL TAB
// ===========================================================================

export function LocalSkillsTab({ toast: _toast }: { toast: ReturnType<typeof useToast>["toast"] }) {
  const { t } = useTranslation();

  const skillsQ = useSkills();
  const toggleMut = useToggleSkill();

  const skillsData = skillsQ.data;
  const loading = skillsQ.isPending;

  const [search, setSearch] = useState("");
  const [collapsed, setCollapsed] = useState<Record<SourceGroup, boolean>>({
    project: false,
    personal: false,
    "built-in": false,
  });
  const [detailSkill, setDetailSkill] = useState<SkillEntry | null>(null);

  const filtered = useMemo(() => {
    const skills = skillsData ?? [];
    if (!search.trim()) return skills;
    const q = search.toLowerCase();
    return skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q)
    );
  }, [skillsData, search]);

  const grouped = useMemo(() => {
    const map: Record<SourceGroup, SkillEntry[]> = { project: [], personal: [], "built-in": [] };
    for (const skill of filtered) {
      map[resolveSource(skill)].push(skill);
    }
    return map;
  }, [filtered]);

  const toggleGroup = (group: SourceGroup) => {
    setCollapsed((prev) => ({ ...prev, [group]: !prev[group] }));
  };

  const handleToggleSkill = async (skill: SkillEntry) => {
    toggleMut.mutate(skill.name);
  };

  if (loading) {
    return (
      <div className="space-y-4">
        <LoadingSkeleton className="h-10 w-full max-w-[400px]" />
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-32" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div>
      {/* Search bar */}
      <div className="relative mb-5 max-w-[400px]">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t("dashboard.searchSkills", "Search skills...")}
          className="w-full pl-9 pr-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-colors"
        />
      </div>

      {filtered.length === 0 ? (
        <EmptyState
          icon={<FolderOpen className="h-5 w-5" />}
          message={
            search
              ? t("dashboard.noSkillsMatch", "No skills match your search")
              : t("dashboard.noSkills", "No skills configured")
          }
        />
      ) : (
        <div className="space-y-4">
          {SOURCE_ORDER.map((source) => {
            const items = grouped[source];
            if (items.length === 0) return null;
            const isCollapsed = collapsed[source];
            const groupStyle = SOURCE_STYLE[source];
            const GroupIcon = groupStyle.icon;

            return (
              <div key={source}>
                <button
                  onClick={() => toggleGroup(source)}
                  className="flex items-center gap-2.5 mb-3 cursor-pointer group"
                >
                  {isCollapsed ? (
                    <ChevronRight className="h-3.5 w-3.5 text-[var(--text-tertiary)] group-hover:text-[var(--text-secondary)] transition-colors" />
                  ) : (
                    <ChevronDown className="h-3.5 w-3.5 text-[var(--text-tertiary)] group-hover:text-[var(--text-secondary)] transition-colors" />
                  )}
                  <GroupIcon className="h-3.5 w-3.5" style={{ color: groupStyle.color }} />
                  <span className="text-[12px] font-semibold uppercase tracking-[0.06em]" style={{ color: groupStyle.color }}>
                    {t(groupStyle.labelKey)}
                  </span>
                  <span className="px-2 py-0.5 rounded-full text-[10px] font-mono tabular-nums border" style={{
                    color: groupStyle.color,
                    background: `color-mix(in srgb, ${groupStyle.color} 8%, transparent)`,
                    borderColor: `color-mix(in srgb, ${groupStyle.color} 20%, transparent)`,
                  }}>
                    {items.length}
                  </span>
                  <div className="flex-1 h-px bg-[var(--border-subtle)]/50 ml-1" />
                </button>

                {!isCollapsed && (
                  <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                    {items.map((skill) => (
                      <SkillCard
                        key={skill.name}
                        skill={skill}
                        source={source}
                        onToggle={() => handleToggleSkill(skill)}
                        onClick={() => setDetailSkill(skill)}
                      />
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Local skill detail modal */}
      {detailSkill && (
        <LocalSkillDetailModal
          skill={detailSkill}
          source={resolveSource(detailSkill)}
          onClose={() => setDetailSkill(null)}
          onToggle={() => handleToggleSkill(detailSkill)}
        />
      )}
    </div>
  );
}

// ===========================================================================
// LOCAL SKILL DETAIL MODAL
// ===========================================================================

function LocalSkillDetailModal({
  skill,
  source,
  onClose,
  onToggle,
}: {
  skill: SkillEntry;
  source: SourceGroup;
  onClose: () => void;
  onToggle: () => void;
}) {
  const codeTheme = useCodeTheme();
  const { t } = useTranslation();
  const [detailTab, setDetailTab] = useState<"info" | "files">("info");
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  // Files tab state
  const [fileList, setFileList] = useState<{ name: string; size: number }[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const filesLoaded = useRef(false);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [selectedFileContent, setSelectedFileContent] = useState<string | null>(null);
  const [fileContentLoading, setFileContentLoading] = useState(false);

  const style = SOURCE_STYLE[source];
  const SourceIcon = style.icon;
  const enabled = skill.enabled !== false;
  const hasMissing = (skill.missing_env?.length ?? 0) > 0 || (skill.missing_bins?.length ?? 0) > 0;

  // Determine skill directory from skill.path (e.g. /foo/bar/SKILL.md -> /foo/bar)
  const skillDir = skill.path?.replace(/\/[^/]+$/, "") ?? null;

  // Load SKILL.md content for Info tab
  useEffect(() => {
    if (skill.path) {
      fetchJSON<{ content: string }>(`/skills/content?path=${encodeURIComponent(skill.path)}`).then(data => {
        if (data?.content) setContent(data.content);
        setLoading(false);
      }).catch(() => setLoading(false));
    } else {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setLoading(false);
    }
  }, [skill.path]);

  // Lazy-load file list when Files tab is selected
  useEffect(() => {
    if (detailTab === "files" && !filesLoaded.current && skillDir) {
      filesLoaded.current = true;
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setFilesLoading(true);
      fetchJSON<{ files: { name: string; size: number }[] }>(`/skills/files?path=${encodeURIComponent(skillDir)}`).then(data => {
        if (data?.files) setFileList(data.files);
        setFilesLoading(false);
      }).catch(() => setFilesLoading(false));
    }
  }, [detailTab, skillDir]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4" onClick={onClose}>
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" />

      {/* Modal — same width as store modal */}
      <div
        className="relative w-full max-w-[1100px] h-[85vh] rounded-[var(--radius-lg)] bg-[var(--bg-elevated)] border border-[var(--border-subtle)] shadow-2xl overflow-hidden flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className="absolute top-3 right-3 z-10 p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
        >
          <X className="h-4 w-4" />
        </button>

        <div className="flex flex-col overflow-hidden flex-1">
          {/* Header — matches store modal structure exactly */}
          <div className="shrink-0 p-6 pb-4 border-b border-[var(--border-subtle)]">
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0 flex-1">
                {/* Name + slug — same as store: no icon box, just text */}
                <h2 className="text-[18px] font-bold text-[var(--text-primary)] mb-1">
                  {skill.emoji && <span className="mr-1.5">{skill.emoji}</span>}
                  {skill.name}
                </h2>
                <span className="text-[12px] font-mono text-[var(--text-tertiary)]">/{skill.name}</span>

                {/* Description */}
                {skill.description && (
                  <p className="text-[13px] text-[var(--text-secondary)] leading-[1.6] mt-3">
                    {skill.description}
                  </p>
                )}

                {/* Source badge — styled like store's license badge */}
                <div className="mt-3 inline-flex items-center gap-2 px-3 py-1.5 rounded-[var(--radius-md)] border" style={{ background: `color-mix(in srgb, ${style.color} 8%, transparent)`, borderColor: `color-mix(in srgb, ${style.color} 15%, transparent)` }}>
                  <SourceIcon className="h-3.5 w-3.5" style={{ color: style.color }} />
                  <span className="text-[11px] font-semibold" style={{ color: style.color }}>{t(style.labelKey)}</span>
                </div>

                {/* Stats row — like store's stars/downloads/installs row */}
                <div className="flex items-center gap-4 mt-3 text-[11px] text-[var(--text-secondary)]">
                  {skill.user_invocable && (
                    <span className="flex items-center gap-1">
                      <Zap className="h-3.5 w-3.5 text-[var(--accent)]" />
                      {t("dashboard.invocable", "invocable")}
                    </span>
                  )}
                  {skill.has_install_specs && (
                    <span className="flex items-center gap-1">
                      <Package className="h-3.5 w-3.5" />
                      {t("dashboard.hasInstallSpecs")}
                    </span>
                  )}
                  {skill.homepage && (
                    <a
                      href={skill.homepage}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1 text-[var(--accent)] hover:underline"
                    >
                      <ExternalLink className="h-3.5 w-3.5" />
                      {t("dashboard.skillHomepage")}
                    </a>
                  )}
                </div>

                {/* Missing requirements — like store's OS tags row */}
                {hasMissing && (
                  <div className="flex items-center gap-2 mt-3 flex-wrap">
                    {(skill.missing_env?.length ?? 0) > 0 && (
                      <span className="px-2 py-0.5 rounded-full bg-[var(--error)]/8 text-[10px] font-medium text-[var(--error)] border border-[var(--error)]/15">
                        ENV: {skill.missing_env!.join(", ")}
                      </span>
                    )}
                    {(skill.missing_bins?.length ?? 0) > 0 && (
                      <span className="px-2 py-0.5 rounded-full bg-[var(--error)]/8 text-[10px] font-medium text-[var(--error)] border border-[var(--error)]/15">
                        BIN: {skill.missing_bins!.join(", ")}
                      </span>
                    )}
                  </div>
                )}
              </div>

              {/* Right side: version + toggle — matches store's version card + install button */}
              <div className="flex flex-col items-end gap-2 shrink-0">
                {skill.version && (
                  <div className="text-center px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)]">
                    <div className="text-[9px] uppercase tracking-wider text-[var(--text-tertiary)] mb-0.5">
                      {t("dashboard.storeCurrentVersion", "Current Version")}
                    </div>
                    <div className="text-[14px] font-bold text-[var(--text-primary)]">v{skill.version}</div>
                  </div>
                )}

                {enabled ? (
                  <span
                    onClick={onToggle}
                    className="flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] bg-[var(--success)]/10 text-[var(--success)] text-[12px] font-medium border border-[var(--success)]/20 cursor-pointer hover:bg-[var(--success)]/15 transition-colors"
                  >
                    <CheckCircle2 className="h-4 w-4" />
                    {t("schedules.enabled", "Enabled")}
                  </span>
                ) : (
                  <button
                    onClick={onToggle}
                    className="flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] text-[var(--text-tertiary)] text-[12px] font-medium border border-[var(--border-subtle)] cursor-pointer hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    <EyeOff className="h-4 w-4" />
                    {t("schedules.disabled", "Disabled")}
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* Path bar — like install command bar in store modal */}
          {skill.path && (
            <div className="shrink-0 px-6 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-content)]/30">
              <div className="flex items-center gap-2">
                <code className="flex-1 text-[12px] font-mono text-[var(--text-secondary)] bg-[var(--bg-window)] px-3 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)] truncate">
                  {skill.path}
                </code>
                <button
                  onClick={() => {
                    navigator.clipboard.writeText(skill.path!);
                    setCopied(true);
                    setTimeout(() => setCopied(false), 2000);
                  }}
                  className="p-2 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  title={t("dashboard.storeCopy", "Copy")}
                >
                  {copied ? <Check className="h-4 w-4 text-[var(--success)]" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            </div>
          )}

          {/* Tabs — matches store modal: Info / Files */}
          <div className="shrink-0 px-6 pt-3 border-b border-[var(--border-subtle)] flex items-center gap-0">
            <button
              onClick={() => setDetailTab("info")}
              className={cn(
                "px-4 py-2 text-[12px] font-medium border-b-2 transition-colors cursor-pointer",
                detailTab === "info"
                  ? "border-[var(--accent)] text-[var(--accent)]"
                  : "border-transparent text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
              )}
            >
              {t("dashboard.storeTabInfo", "Info")}
            </button>
            <button
              onClick={() => setDetailTab("files")}
              className={cn(
                "px-4 py-2 text-[12px] font-medium border-b-2 transition-colors cursor-pointer",
                detailTab === "files"
                  ? "border-[var(--accent)] text-[var(--accent)]"
                  : "border-transparent text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
              )}
            >
              {t("dashboard.storeTabFiles", "Files")}
              {fileList.length > 0 && (
                <span className="ml-1.5 text-[10px] text-[var(--text-tertiary)]">({fileList.length})</span>
              )}
            </button>
          </div>

          {detailTab === "info" ? (
            /* Info tab — SKILL.md rendered content */
            <div className="flex-1 overflow-y-auto min-h-0">
              {loading ? (
                <div className="flex items-center justify-center py-12">
                  <Loader2 className="h-5 w-5 animate-spin text-[var(--accent)]" />
                </div>
              ) : content ? (
                <div className="p-6">
                  <div className="synapse-prose prose prose-sm max-w-none text-[13px] leading-[1.8] text-[var(--text-secondary)] [&_h1]:text-[20px] [&_h1]:font-bold [&_h1]:mt-6 [&_h1]:mb-3 [&_h2]:text-[17px] [&_h2]:font-semibold [&_h2]:mt-5 [&_h2]:mb-2 [&_h3]:text-[15px] [&_h3]:font-semibold [&_h3]:mt-4 [&_h3]:mb-2 [&_h1]:text-[var(--text-primary)] [&_h2]:text-[var(--text-primary)] [&_h3]:text-[var(--text-primary)] [&_code]:text-[12px] [&_code]:bg-[var(--bg-content)] [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:rounded [&_pre]:bg-[var(--bg-content)] [&_pre]:border [&_pre]:border-[var(--border-subtle)] [&_pre]:rounded-[var(--radius-md)] [&_pre]:p-4 [&_pre]:text-[12px] [&_table]:text-[12px] [&_table]:w-full [&_th]:bg-[var(--bg-content)] [&_th]:px-3 [&_th]:py-1.5 [&_th]:text-left [&_th]:font-semibold [&_td]:px-3 [&_td]:py-1.5 [&_td]:border-t [&_td]:border-[var(--border-subtle)] [&_a]:text-[var(--accent)] [&_blockquote]:border-l-2 [&_blockquote]:border-[var(--accent)]/30 [&_blockquote]:pl-4 [&_blockquote]:italic [&_li]:mb-1 [&_p]:mb-3 [&_hr]:border-[var(--border-subtle)] [&_hr]:my-6">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
                  </div>
                </div>
              ) : (
                <div className="flex items-center justify-center py-12 text-[12px] text-[var(--text-tertiary)]">
                  {t("dashboard.noSkillContent", "No SKILL.md content available")}
                </div>
              )}
            </div>
          ) : (
            /* Files tab — split pane: left file tree + right file content */
            <div className="flex flex-1 min-h-0">
              {filesLoading ? (
                <div className="flex-1 flex items-center justify-center py-8">
                  <Loader2 className="h-5 w-5 animate-spin text-[var(--accent)]" />
                </div>
              ) : fileList.length > 0 ? (
                <>
                  {/* Left: file tree */}
                  <div className="w-[240px] shrink-0 border-r border-[var(--border-subtle)] overflow-y-auto bg-[var(--bg-content)]/40">
                    {fileList.map((f) => (
                      <div
                        key={f.name}
                        onClick={() => {
                          setSelectedFile(f.name);
                          setSelectedFileContent(null);
                          setFileContentLoading(true);
                          const fullPath = `${skillDir}/${f.name}`;
                          fetchJSON<{ content: string }>(`/skills/content?path=${encodeURIComponent(fullPath)}`).then((d) => {
                            setSelectedFileContent(d?.content ?? null);
                            setFileContentLoading(false);
                          });
                        }}
                        className={cn(
                          "flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-mono cursor-pointer transition-colors border-l-2",
                          selectedFile === f.name
                            ? "bg-[var(--accent)]/8 border-l-[var(--accent)] text-[var(--text-primary)]"
                            : "border-l-transparent text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                        )}
                      >
                        <FileText className="h-3 w-3 shrink-0 text-[var(--text-tertiary)]" />
                        <span className="truncate flex-1">{f.name}</span>
                        <span className="text-[10px] text-[var(--text-tertiary)] tabular-nums shrink-0">
                          {f.size < 1024 ? `${f.size}B` : f.size < 1048576 ? `${(f.size / 1024).toFixed(1)}K` : `${(f.size / 1048576).toFixed(1)}M`}
                        </span>
                      </div>
                    ))}
                  </div>

                  {/* Right: file content */}
                  <div className="flex-1 overflow-y-auto">
                    {selectedFile ? (
                      <>
                        {/* File header */}
                        <div className="sticky top-0 z-10 px-4 py-2 border-b border-[var(--border-subtle)] bg-[var(--bg-elevated)] flex items-center gap-2">
                          <FileText className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
                          <span className="text-[12px] font-semibold font-mono text-[var(--text-primary)]">{selectedFile}</span>
                          <span className="text-[10px] text-[var(--text-tertiary)] flex-1">
                            {(() => { const f = fileList.find(x => x.name === selectedFile); if (!f) return ""; return f.size < 1024 ? `${f.size} bytes` : f.size < 1048576 ? `${(f.size / 1024).toFixed(1)} KB` : `${(f.size / 1048576).toFixed(1)} MB`; })()}
                          </span>
                          {selectedFileContent != null && (
                            <button
                              onClick={() => {
                                navigator.clipboard.writeText(selectedFileContent);
                                setCopied(true);
                                setTimeout(() => setCopied(false), 2000);
                              }}
                              className="p-1 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                              title={t("dashboard.storeCopy", "Copy")}
                            >
                              {copied ? <Check className="h-3.5 w-3.5 text-[var(--success)]" /> : <Copy className="h-3.5 w-3.5" />}
                            </button>
                          )}
                        </div>
                        {fileContentLoading ? (
                          <div className="flex items-center justify-center py-12">
                            <Loader2 className="h-5 w-5 animate-spin text-[var(--accent)]" />
                          </div>
                        ) : selectedFileContent != null ? (
                          <div className="p-5">
                            {selectedFile.endsWith(".md") ? (
                              <div className="synapse-prose prose prose-sm max-w-none text-[13px] leading-[1.8] text-[var(--text-secondary)] [&_h1]:text-[20px] [&_h1]:font-bold [&_h1]:mt-6 [&_h1]:mb-3 [&_h2]:text-[17px] [&_h2]:font-semibold [&_h2]:mt-5 [&_h2]:mb-2 [&_h3]:text-[15px] [&_h3]:font-semibold [&_h3]:mt-4 [&_h3]:mb-2 [&_h1]:text-[var(--text-primary)] [&_h2]:text-[var(--text-primary)] [&_h3]:text-[var(--text-primary)] [&_code]:text-[12px] [&_code]:bg-[var(--bg-content)] [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:rounded [&_pre]:bg-[var(--bg-content)] [&_pre]:border [&_pre]:border-[var(--border-subtle)] [&_pre]:rounded-[var(--radius-md)] [&_pre]:p-4 [&_pre]:text-[12px] [&_table]:text-[12px] [&_table]:w-full [&_th]:bg-[var(--bg-content)] [&_th]:px-3 [&_th]:py-1.5 [&_th]:text-left [&_th]:font-semibold [&_td]:px-3 [&_td]:py-1.5 [&_td]:border-t [&_td]:border-[var(--border-subtle)] [&_a]:text-[var(--accent)] [&_blockquote]:border-l-2 [&_blockquote]:border-[var(--accent)]/30 [&_blockquote]:pl-4 [&_blockquote]:italic [&_li]:mb-1 [&_p]:mb-3 [&_hr]:border-[var(--border-subtle)] [&_hr]:my-6">
                                <ReactMarkdown remarkPlugins={[remarkGfm]}>{selectedFileContent}</ReactMarkdown>
                              </div>
                            ) : (
                              <SyntaxHighlighter
                                language={extToLang(selectedFile)}
                                style={codeTheme}
                                customStyle={{ margin: 0, borderRadius: "var(--radius-md)", fontSize: "12px", lineHeight: "1.8" }}
                                wrapLongLines
                              >
                                {selectedFileContent}
                              </SyntaxHighlighter>
                            )}
                          </div>
                        ) : (
                          <div className="flex items-center justify-center py-12 text-[12px] text-[var(--text-tertiary)]">
                            {t("dashboard.storeNoFiles", "No files available")}
                          </div>
                        )}
                      </>
                    ) : (
                      <div className="flex items-center justify-center h-full text-[12px] text-[var(--text-tertiary)]">
                        {t("files.selectFile", "Select a file to view")}
                      </div>
                    )}
                  </div>
                </>
              ) : (
                <div className="flex-1 flex items-center justify-center py-8 text-[12px] text-[var(--text-tertiary)]">
                  {t("dashboard.storeNoFiles", "No files available")}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
