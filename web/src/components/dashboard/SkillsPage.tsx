import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Sparkles, Search, FolderOpen, ChevronDown, ChevronRight,
  Eye, EyeOff, AlertTriangle, ExternalLink, Package,
  Zap, User, Box, Download, Star, ArrowDownWideNarrow,
  Store, Loader2, CheckCircle2, GitBranch,
  X, Shield, Clock, FileText, Copy, Check,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { useCodeTheme } from "../../hooks/useCodeTheme";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { SkillEntry, StoreSkillItem, StoreSkillDetail, StoreSearchResult } from "../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  Toggle,
  useToast,
  ToastContainer,
} from "./shared";
import { cn } from "../../lib/cn";

// ---------------------------------------------------------------------------
// Local tab types
// ---------------------------------------------------------------------------

type SourceGroup = "project" | "personal" | "built-in";
const SOURCE_ORDER: SourceGroup[] = ["project", "personal", "built-in"];

function resolveSource(skill: SkillEntry): SourceGroup {
  if (skill.source === "personal") return "personal";
  if (skill.source === "built-in") return "built-in";
  return "project";
}

const SOURCE_STYLE: Record<SourceGroup, { icon: typeof Zap; color: string; bg: string; border: string; labelKey: string }> = {
  project: { icon: FolderOpen, color: "var(--accent)", bg: "var(--accent)", border: "var(--accent)", labelKey: "dashboard.sourceProject" },
  personal: { icon: User, color: "#a78bfa", bg: "#a78bfa", border: "#a78bfa", labelKey: "dashboard.sourcePersonal" },
  "built-in": { icon: Box, color: "var(--success)", bg: "var(--success)", border: "var(--success)", labelKey: "dashboard.sourceBuiltIn" },
};

// ---------------------------------------------------------------------------
// Tab type
// ---------------------------------------------------------------------------

type Tab = "local" | "store";

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function SkillsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("local");
  const { toasts, addToast } = useToast();

  return (
    <div className="space-y-4">
      <SectionCard>
        <SectionHeader
          icon={<Sparkles className="h-4 w-4" />}
          title={t("dashboard.skills", "Skills")}
          right={
            <div className="flex items-center gap-1 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
              <TabButton active={tab === "local"} onClick={() => setTab("local")}>
                <FolderOpen className="h-3 w-3" />
                {t("dashboard.skillsLocal", "Local")}
              </TabButton>
              <TabButton active={tab === "store"} onClick={() => setTab("store")}>
                <Store className="h-3 w-3" />
                {t("dashboard.skillsStore", "Store")}
              </TabButton>
            </div>
          }
        />

        {tab === "local" ? <LocalTab addToast={addToast} /> : <StoreTab addToast={addToast} />}
      </SectionCard>

      <ToastContainer toasts={toasts} />
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-sm)] text-[11px] font-medium transition-all cursor-pointer",
        active
          ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
          : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
      )}
    >
      {children}
    </button>
  );
}

// ===========================================================================
// LOCAL TAB
// ===========================================================================

function LocalTab({ addToast }: { addToast: (msg: string, type: "success" | "error") => void }) {
  const { t } = useTranslation();
  const api = useDashboardAPI();

  const [skills, setSkills] = useState<SkillEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [collapsed, setCollapsed] = useState<Record<SourceGroup, boolean>>({
    project: false,
    personal: false,
    "built-in": false,
  });
  const [detailSkill, setDetailSkill] = useState<SkillEntry | null>(null);

  const loadSkills = useCallback(async () => {
    const data = await api.fetchSkills();
    if (data) setSkills(data);
    setLoading(false);
  }, [api]);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const filtered = useMemo(() => {
    if (!search.trim()) return skills;
    const q = search.toLowerCase();
    return skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q)
    );
  }, [skills, search]);

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
    const prevEnabled = skill.enabled !== false;
    setSkills((prev) =>
      prev.map((s) => (s.name === skill.name ? { ...s, enabled: !prevEnabled } : s))
    );
    const result = await api.toggleSkill(skill.name);
    if (result === null) {
      setSkills((prev) =>
        prev.map((s) => (s.name === skill.name ? { ...s, enabled: prevEnabled } : s))
      );
      addToast(t("dashboard.skillToggleFailed", "Failed to toggle skill"), "error");
    } else {
      addToast(
        result.enabled
          ? t("dashboard.skillEnabled", "Skill enabled")
          : t("dashboard.skillDisabled", "Skill disabled"),
        "success"
      );
    }
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
  const api = useDashboardAPI();
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

  // Determine skill directory from skill.path (e.g. /foo/bar/SKILL.md → /foo/bar)
  const skillDir = skill.path?.replace(/\/[^/]+$/, "") ?? null;

  // Load SKILL.md content for Info tab
  useEffect(() => {
    if (skill.path) {
      api.fetchSkillFileContent(skill.path).then(data => {
        if (data?.content) setContent(data.content);
        setLoading(false);
      });
    } else {
      setLoading(false);
    }
  }, [skill.path, api]);

  // Lazy-load file list when Files tab is selected
  useEffect(() => {
    if (detailTab === "files" && !filesLoaded.current && skillDir) {
      filesLoaded.current = true;
      setFilesLoading(true);
      api.fetchSkillFiles(skillDir).then(data => {
        if (data?.files) setFileList(data.files);
        setFilesLoading(false);
      });
    }
  }, [detailTab, api, skillDir]);

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
                          api.fetchSkillFileContent(fullPath).then((d) => {
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

// ===========================================================================
// STORE TAB
// ===========================================================================

type SortMode = "downloads" | "stars" | "recent";
type FilterMode = "all" | "installed" | "not-installed";

const PAGE_SIZE = 30;

function StoreTab({ addToast }: { addToast: (msg: string, type: "success" | "error") => void }) {
  const { t } = useTranslation();
  const api = useDashboardAPI();

  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortMode>("downloads");
  const [filter, setFilter] = useState<FilterMode>("all");
  const [items, setItems] = useState<StoreSkillItem[]>([]);
  const [searchResults, setSearchResults] = useState<StoreSearchResult[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState<Set<string>>(new Set());
  const [configured, setConfigured] = useState(true);
  const [detailSlug, setDetailSlug] = useState<string | null>(null);
  const searchTimeout = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Load status + initial list
  useEffect(() => {
    setLoading(true);
    setItems([]);
    setHasMore(true);
    (async () => {
      const [status, list] = await Promise.all([
        api.storeStatus(),
        api.storeList(PAGE_SIZE, sort),
      ]);
      if (status) {
        setConfigured(status.configured);
        setInstalled(new Set(status.installed));
      }
      if (list) {
        setItems(list.items);
        setHasMore(list.items.length >= PAGE_SIZE);
      }
      setLoading(false);
    })();
  }, [api, sort]);

  // Load more
  const loadMore = useCallback(async () => {
    if (loadingMore || !hasMore) return;
    setLoadingMore(true);
    const list = await api.storeList(PAGE_SIZE, sort, items.length);
    if (list) {
      setItems((prev) => [...prev, ...list.items]);
      setHasMore(list.items.length >= PAGE_SIZE);
    } else {
      setHasMore(false);
    }
    setLoadingMore(false);
  }, [api, sort, items.length, loadingMore, hasMore]);

  // Debounced search
  useEffect(() => {
    if (!search.trim()) {
      setSearchResults(null);
      return;
    }
    clearTimeout(searchTimeout.current);
    searchTimeout.current = setTimeout(async () => {
      const data = await api.storeSearch(search.trim(), 50);
      if (data) setSearchResults(data.results);
    }, 400);
    return () => clearTimeout(searchTimeout.current);
  }, [search, api]);

  const handleInstall = async (slug: string) => {
    setInstalling((prev) => new Set(prev).add(slug));
    const result = await api.storeInstall(slug);
    setInstalling((prev) => {
      const next = new Set(prev);
      next.delete(slug);
      return next;
    });
    if (result?.ok) {
      setInstalled((prev) => new Set(prev).add(slug));
      addToast(t("dashboard.storeInstallSuccess", "Skill installed successfully"), "success");
    } else {
      addToast(t("dashboard.storeInstallFailed", "Failed to install skill"), "error");
    }
  };

  if (!configured) {
    return (
      <EmptyState
        icon={<Store className="h-5 w-5" />}
        message={t("dashboard.storeNotConfigured", "Store not configured. Set CLAWHUB_API_KEY in .env")}
      />
    );
  }

  if (loading) {
    return (
      <div className="space-y-4">
        <LoadingSkeleton className="h-10 w-full" />
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
          {Array.from({ length: 9 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-40" />
          ))}
        </div>
      </div>
    );
  }

  let displayItems: StoreSkillItem[] = searchResults
    ? searchResults.map((r) => ({
        slug: r.slug,
        displayName: r.displayName,
        summary: r.summary,
        version: r.version,
      } as StoreSkillItem))
    : items;

  // Apply filter
  if (filter === "installed") {
    displayItems = displayItems.filter((i) => installed.has(i.slug));
  } else if (filter === "not-installed") {
    displayItems = displayItems.filter((i) => !installed.has(i.slug));
  }

  return (
    <div>
      {/* Search + sort + filter */}
      <div className="flex items-center gap-3 mb-5 flex-wrap">
        <div className="relative flex-1 max-w-[400px] min-w-[200px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("dashboard.storeSearch", "Search skills store...")}
            className="w-full pl-9 pr-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent)]/20 transition-colors"
          />
        </div>
        {!searchResults && (
          <div className="flex items-center gap-1 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
            <SortButton active={sort === "downloads"} onClick={() => setSort("downloads")}>
              <Download className="h-3 w-3" />
              {t("dashboard.storeSortDownloads", "Downloads")}
            </SortButton>
            <SortButton active={sort === "stars"} onClick={() => setSort("stars")}>
              <Star className="h-3 w-3" />
              {t("dashboard.storeSortStars", "Stars")}
            </SortButton>
            <SortButton active={sort === "recent"} onClick={() => setSort("recent")}>
              <ArrowDownWideNarrow className="h-3 w-3" />
              {t("dashboard.storeSortRecent", "Recent")}
            </SortButton>
          </div>
        )}
        {/* Filter */}
        <div className="flex items-center gap-1 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
          <SortButton active={filter === "all"} onClick={() => setFilter("all")}>
            {t("dashboard.storeFilterAll", "All")}
          </SortButton>
          <SortButton active={filter === "installed"} onClick={() => setFilter("installed")}>
            <CheckCircle2 className="h-3 w-3" />
            {t("dashboard.storeFilterInstalled", "Installed")}
          </SortButton>
          <SortButton active={filter === "not-installed"} onClick={() => setFilter("not-installed")}>
            {t("dashboard.storeFilterNew", "New")}
          </SortButton>
        </div>
      </div>

      {displayItems.length === 0 ? (
        <EmptyState
          icon={<Store className="h-5 w-5" />}
          message={t("dashboard.storeNoResults", "No skills found")}
        />
      ) : (
        <>
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {displayItems.map((item) => (
              <StoreSkillCard
                key={item.slug}
                item={item}
                isInstalled={installed.has(item.slug)}
                isInstalling={installing.has(item.slug)}
                onInstall={() => handleInstall(item.slug)}
                onDetail={() => setDetailSlug(item.slug)}
              />
            ))}
          </div>
          {/* Load more */}
          {!searchResults && hasMore && (
            <div className="flex justify-center mt-6">
              <button
                onClick={loadMore}
                disabled={loadingMore}
                className="flex items-center gap-2 px-6 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer disabled:opacity-50"
              >
                {loadingMore ? (
                  <>
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    {t("dashboard.storeLoading", "Loading...")}
                  </>
                ) : (
                  t("dashboard.storeLoadMore", "Load more")
                )}
              </button>
            </div>
          )}
        </>
      )}

      {/* Detail modal */}
      {detailSlug && (
        <StoreSkillDetailModal
          slug={detailSlug}
          isInstalled={installed.has(detailSlug)}
          isInstalling={installing.has(detailSlug)}
          onInstall={() => handleInstall(detailSlug)}
          onClose={() => setDetailSlug(null)}
        />
      )}
    </div>
  );
}

function SortButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-1 px-2.5 py-1 rounded-[var(--radius-sm)] text-[10px] font-medium transition-all cursor-pointer",
        active
          ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
          : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
      )}
    >
      {children}
    </button>
  );
}

function StoreSkillCard({
  item,
  isInstalled,
  isInstalling,
  onInstall,
  onDetail,
}: {
  item: StoreSkillItem;
  isInstalled: boolean;
  isInstalling: boolean;
  onInstall: () => void;
  onDetail: () => void;
}) {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const downloads = item.stats?.downloads ?? item.stats?.installsAllTime ?? 0;
  const stars = item.stats?.stars ?? 0;
  const versions = item.stats?.versions ?? 0;
  const version = item.latestVersion?.version || item.displayName;
  const osTags = item.metadata?.os?.filter(Boolean) ?? [];
  const [owner, setOwner] = useState<{ handle?: string; image?: string } | null>(null);

  // Lazy-load owner on hover
  const ownerLoaded = useRef(false);
  const onHover = useCallback(() => {
    if (ownerLoaded.current) return;
    ownerLoaded.current = true;
    api.storeDetail(item.slug).then((d) => {
      if (d?.owner) setOwner(d.owner);
    });
  }, [api, item.slug]);

  return (
    <div
      className="group relative rounded-[var(--radius-lg)] border bg-[var(--bg-elevated)]/70 border-[var(--border-subtle)] hover:border-[var(--separator)] hover:shadow-[var(--shadow-md)] overflow-hidden transition-all duration-200 cursor-pointer"
      onMouseEnter={onHover}
      onClick={onDetail}
    >
      {/* Accent bar */}
      <div className="h-[2px] bg-gradient-to-r from-[var(--accent)]/60 to-transparent" />

      <div className="p-4">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 mb-2">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
                {item.displayName || item.slug}
              </span>
              {version && (
                <span className="px-1.5 py-[1px] rounded-[var(--radius-sm)] bg-[var(--bg-content)] text-[9px] font-mono text-[var(--text-tertiary)] border border-[var(--border-subtle)] shrink-0">
                  v{version}
                </span>
              )}
              {osTags.length > 0 && osTags.map((os) => (
                <span key={os} className="px-1.5 py-[1px] rounded-full bg-[var(--accent)]/8 text-[9px] font-medium text-[var(--accent)] border border-[var(--accent)]/15 shrink-0">
                  {os === "darwin" ? "macOS" : os === "win32" ? "Windows" : os.charAt(0).toUpperCase() + os.slice(1)}
                </span>
              ))}
            </div>
            <span className="text-[10px] font-mono text-[var(--text-tertiary)] mt-0.5 block truncate">
              /{item.slug}
            </span>
          </div>

          {/* Install button */}
          {isInstalled ? (
            <span className="flex items-center gap-1 px-2.5 py-1.5 rounded-[var(--radius-md)] bg-[var(--success)]/10 text-[var(--success)] text-[10px] font-medium border border-[var(--success)]/20 shrink-0">
              <CheckCircle2 className="h-3 w-3" />
              {t("dashboard.storeInstalled", "Installed")}
            </span>
          ) : (
            <button
              onClick={(e) => { e.stopPropagation(); onInstall(); }}
              disabled={isInstalling}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[10px] font-medium transition-all shrink-0 cursor-pointer",
                isInstalling
                  ? "bg-[var(--bg-content)] text-[var(--text-tertiary)] border border-[var(--border-subtle)]"
                  : "bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]"
              )}
            >
              {isInstalling ? (
                <>
                  <Loader2 className="h-3 w-3 animate-spin" />
                  {t("dashboard.storeInstalling", "Installing...")}
                </>
              ) : (
                <>
                  <Download className="h-3 w-3" />
                  {t("dashboard.storeInstall", "Install")}
                </>
              )}
            </button>
          )}
        </div>

        {/* Summary */}
        {item.summary && (
          <p className="text-[11px] text-[var(--text-secondary)] leading-[1.6] mb-3 line-clamp-2">
            {item.summary}
          </p>
        )}

        {/* Footer: stats + author */}
        <div className="flex items-center justify-between pt-2 border-t border-[var(--border-subtle)]/50">
          <div className="flex items-center gap-3">
            {downloads > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                <Download className="h-3 w-3" />
                {formatNumber(downloads)}
              </span>
            )}
            {stars > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                <Star className="h-3 w-3" />
                {formatNumber(stars)}
              </span>
            )}
            {versions > 1 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                <GitBranch className="h-3 w-3" />
                {versions}v
              </span>
            )}
          </div>
          {owner && (
            <div className="flex items-center gap-1.5">
              {owner.image ? (
                <img src={owner.image} alt={owner.handle} className="h-4 w-4 rounded-full" />
              ) : (
                <User className="h-3 w-3 text-[var(--text-tertiary)]" />
              )}
              <span className="text-[10px] text-[var(--text-tertiary)]">@{owner.handle}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ===========================================================================
// STORE SKILL DETAIL MODAL
// ===========================================================================

function StoreSkillDetailModal({
  slug,
  isInstalled,
  isInstalling,
  onInstall,
  onClose,
}: {
  slug: string;
  isInstalled: boolean;
  isInstalling: boolean;
  onInstall: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const codeTheme = useCodeTheme();
  const api = useDashboardAPI();
  const [detail, setDetail] = useState<StoreSkillDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  const [detailTab, setDetailTab] = useState<"info" | "files">("info");
  const [skillMd, setSkillMd] = useState<string | null>(null);
  const [fileList, setFileList] = useState<{ name: string; size: number }[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const filesLoaded = useRef(false);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [selectedFileContent, setSelectedFileContent] = useState<string | null>(null);
  const [fileContentLoading, setFileContentLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    api.storeDetail(slug).then((d) => {
      setDetail(d);
      setLoading(false);
    });
  }, [api, slug]);

  // Lazy-load files when Files tab is selected
  useEffect(() => {
    if (detailTab === "files" && !filesLoaded.current) {
      filesLoaded.current = true;
      setFilesLoading(true);
      api.storeFiles(slug).then((d) => {
        if (d) {
          setSkillMd(d.skillMd);
          setFileList(d.files);
        }
        setFilesLoading(false);
      });
    }
  }, [detailTab, api, slug]);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  const copyInstallCmd = () => {
    const cmd = `clawhub install ${slug}`;
    navigator.clipboard.writeText(cmd);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const skill = detail?.skill;
  const downloads = skill?.stats?.downloads ?? skill?.stats?.installsAllTime ?? 0;
  const stars = skill?.stats?.stars ?? 0;
  const versions = skill?.stats?.versions ?? 0;
  const installsCurrent = skill?.stats?.installsCurrentVersion ?? 0;
  const installsAll = skill?.stats?.installsAllTime ?? 0;
  const license = detail?.latestVersion?.license;
  const version = detail?.latestVersion?.version;
  const osTags = detail?.metadata?.os?.filter(Boolean) ?? [];
  const changelog = detail?.latestVersion?.changelog;
  const owner = detail?.owner;
  const updatedAt = skill?.updatedAt;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4" onClick={onClose}>
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" />

      {/* Modal */}
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

        {loading ? (
          <div className="p-8 flex items-center justify-center">
            <Loader2 className="h-6 w-6 animate-spin text-[var(--accent)]" />
          </div>
        ) : detail ? (
          <div className="flex flex-col overflow-hidden" style={{ maxHeight: "calc(90vh - 2px)" }}>
            {/* Header — fixed */}
            <div className="shrink-0 p-6 pb-4 border-b border-[var(--border-subtle)]">
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                  <h2 className="text-[18px] font-bold text-[var(--text-primary)] mb-1">
                    {skill?.displayName || slug}
                  </h2>
                  <span className="text-[12px] font-mono text-[var(--text-tertiary)]">/{slug}</span>

                  {skill?.summary && (
                    <p className="text-[13px] text-[var(--text-secondary)] leading-[1.6] mt-3">
                      {skill.summary}
                    </p>
                  )}

                  {/* License */}
                  {license && (
                    <div className="mt-3 inline-flex items-center gap-2 px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--success)]/8 border border-[var(--success)]/15">
                      <Shield className="h-3.5 w-3.5 text-[var(--success)]" />
                      <span className="text-[11px] font-semibold text-[var(--success)]">{license}</span>
                    </div>
                  )}

                  {/* Stats row */}
                  <div className="flex items-center gap-4 mt-3 text-[11px] text-[var(--text-secondary)]">
                    {stars > 0 && (
                      <span className="flex items-center gap-1">
                        <Star className="h-3.5 w-3.5 text-amber-500" />
                        {formatNumber(stars)}
                      </span>
                    )}
                    {downloads > 0 && (
                      <span className="flex items-center gap-1">
                        <Download className="h-3.5 w-3.5" />
                        {formatNumber(downloads)}
                      </span>
                    )}
                    {installsCurrent > 0 && (
                      <span>{formatNumber(installsCurrent)} {t("dashboard.storeCurrentInstalls", "current installs")}</span>
                    )}
                    {installsAll > 0 && (
                      <span>{formatNumber(installsAll)} {t("dashboard.storeAllTimeInstalls", "all-time installs")}</span>
                    )}
                  </div>

                  {/* Author */}
                  {owner && (
                    <div className="flex items-center gap-2 mt-3">
                      <span className="text-[11px] text-[var(--text-tertiary)]">{t("dashboard.storeBy", "by")}</span>
                      {owner.image ? (
                        <img src={owner.image} alt={owner.handle} className="h-5 w-5 rounded-full" />
                      ) : (
                        <User className="h-4 w-4 text-[var(--text-tertiary)]" />
                      )}
                      <span className="text-[12px] font-medium text-[var(--text-secondary)]">
                        @{owner.handle || owner.displayName}
                      </span>
                    </div>
                  )}
                </div>

                {/* Right side: version + install */}
                <div className="flex flex-col items-end gap-2 shrink-0">
                  {version && (
                    <div className="text-center px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)]">
                      <div className="text-[9px] uppercase tracking-wider text-[var(--text-tertiary)] mb-0.5">
                        {t("dashboard.storeCurrentVersion", "Current Version")}
                      </div>
                      <div className="text-[14px] font-bold text-[var(--text-primary)]">v{version}</div>
                    </div>
                  )}

                  {isInstalled ? (
                    <span className="flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] bg-[var(--success)]/10 text-[var(--success)] text-[12px] font-medium border border-[var(--success)]/20">
                      <CheckCircle2 className="h-4 w-4" />
                      {t("dashboard.storeInstalled", "Installed")}
                    </span>
                  ) : (
                    <button
                      onClick={onInstall}
                      disabled={isInstalling}
                      className={cn(
                        "flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] text-[12px] font-medium transition-all cursor-pointer",
                        isInstalling
                          ? "bg-[var(--bg-content)] text-[var(--text-tertiary)] border border-[var(--border-subtle)]"
                          : "bg-[var(--accent)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]"
                      )}
                    >
                      {isInstalling ? (
                        <>
                          <Loader2 className="h-4 w-4 animate-spin" />
                          {t("dashboard.storeInstalling", "Installing...")}
                        </>
                      ) : (
                        <>
                          <Download className="h-4 w-4" />
                          {t("dashboard.storeInstall", "Install")}
                        </>
                      )}
                    </button>
                  )}
                </div>
              </div>

              {/* OS tags + versions */}
              <div className="flex items-center gap-2 mt-3 flex-wrap">
                {osTags.map((os) => (
                  <span key={os} className="px-2 py-0.5 rounded-full bg-[var(--accent)]/8 text-[10px] font-medium text-[var(--accent)] border border-[var(--accent)]/15">
                    {os === "darwin" ? "macOS" : os === "win32" ? "Windows" : os.charAt(0).toUpperCase() + os.slice(1)}
                  </span>
                ))}
                {versions > 0 && (
                  <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                    <GitBranch className="h-3 w-3" />
                    {versions} {t("dashboard.storeVersions", "versions")}
                  </span>
                )}
                {updatedAt && (
                  <span className="flex items-center gap-1 text-[10px] text-[var(--text-tertiary)]">
                    <Clock className="h-3 w-3" />
                    {t("dashboard.storeUpdated", "Updated")} {new Date(updatedAt).toLocaleDateString()}
                  </span>
                )}
              </div>
            </div>

            {/* Install command — fixed */}
            <div className="shrink-0 px-6 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-content)]/30">
              <div className="flex items-center gap-2">
                <code className="flex-1 text-[12px] font-mono text-[var(--text-secondary)] bg-[var(--bg-window)] px-3 py-2 rounded-[var(--radius-md)] border border-[var(--border-subtle)]">
                  clawhub install {slug}
                </code>
                <button
                  onClick={copyInstallCmd}
                  className="p-2 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  title={t("dashboard.storeCopy", "Copy")}
                >
                  {copied ? <Check className="h-4 w-4 text-[var(--success)]" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            </div>

            {/* Tabs: Info / Files — fixed */}
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
              <div className="flex-1 overflow-y-auto min-h-0">
                {/* Changelog */}
                {changelog && (
                  <div className="px-6 py-4 border-b border-[var(--border-subtle)]">
                    <h3 className="text-[12px] font-semibold text-[var(--text-primary)] uppercase tracking-wider mb-2 flex items-center gap-1.5">
                      <FileText className="h-3.5 w-3.5" />
                      {t("dashboard.storeChangelog", "Changelog")}
                    </h3>
                    <div className="text-[12px] text-[var(--text-secondary)] leading-[1.7] whitespace-pre-wrap bg-[var(--bg-window)] p-3 rounded-[var(--radius-md)] border border-[var(--border-subtle)] max-h-[200px] overflow-y-auto">
                      {changelog}
                    </div>
                  </div>
                )}

                {/* Comments count */}
                {(skill?.stats?.comments ?? 0) > 0 && (
                  <div className="px-6 py-3 text-[11px] text-[var(--text-tertiary)]">
                    {skill!.stats!.comments} {t("dashboard.storeComments", "comments")}
                  </div>
                )}
              </div>
            ) : (
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
                            if ((f.name === "SKILL.md" || f.name.endsWith("/SKILL.md")) && skillMd) {
                              setSelectedFileContent(skillMd);
                            } else {
                              setFileContentLoading(true);
                              api.storeFileContent(slug, f.name).then((d) => {
                                setSelectedFileContent(d?.content ?? null);
                                setFileContentLoading(false);
                              });
                            }
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
        ) : (
          <div className="p-8 text-center text-[var(--text-tertiary)] text-[13px]">
            {t("dashboard.storeDetailFailed", "Failed to load skill details")}
          </div>
        )}
      </div>
    </div>
  );
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

const EXT_LANG: Record<string, string> = {
  js: "javascript", ts: "typescript", jsx: "jsx", tsx: "tsx",
  sh: "bash", bash: "bash", zsh: "bash",
  py: "python", rb: "ruby", rs: "rust", go: "go",
  json: "json", yaml: "yaml", yml: "yaml", toml: "toml",
  html: "html", css: "css", scss: "scss",
  sql: "sql", lua: "lua", swift: "swift", kt: "kotlin",
  java: "java", c: "c", cpp: "cpp", h: "c", hpp: "cpp",
  xml: "xml", graphql: "graphql", dockerfile: "dockerfile",
};

function extToLang(filename: string): string {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  return EXT_LANG[ext] ?? "text";
}

// ===========================================================================
// LOCAL SKILL CARD (unchanged from before)
// ===========================================================================

function SkillCard({
  skill,
  source,
  onToggle,
  onClick,
}: {
  skill: SkillEntry;
  source: SourceGroup;
  onToggle: () => void;
  onClick?: () => void;
}) {
  const { t } = useTranslation();
  const enabled = skill.enabled !== false;
  const eligible = skill.eligible !== false;
  const hasMissing = (skill.missing_env?.length ?? 0) > 0 || (skill.missing_bins?.length ?? 0) > 0;
  const [expanded, setExpanded] = useState(false);
  const style = SOURCE_STYLE[source];
  const SourceIcon = style.icon;

  return (
    <div
      className={cn(
        "group relative rounded-[var(--radius-lg)] border overflow-hidden transition-all duration-200",
        enabled
          ? "bg-[var(--bg-elevated)]/70 border-[var(--border-subtle)] hover:border-[var(--separator)] hover:shadow-[var(--shadow-md)]"
          : "bg-[var(--bg-elevated)]/30 border-[var(--border-subtle)]/50 opacity-50",
        !eligible && enabled && "border-[var(--warning)]/40",
        onClick && "cursor-pointer"
      )}
      onClick={onClick}
    >
      {/* Top accent bar */}
      <div
        className="h-[2px]"
        style={{ background: enabled ? `color-mix(in srgb, ${style.color} 60%, transparent)` : 'transparent' }}
      />

      <div className="p-4">
        {/* Header: icon + name + version + toggle */}
        <div className="flex items-start justify-between gap-3 mb-3">
          <div className="flex items-center gap-2.5 min-w-0">
            <div
              className="w-9 h-9 rounded-[var(--radius-md)] flex items-center justify-center shrink-0 text-[18px]"
              style={{ background: `color-mix(in srgb, ${style.color} 12%, transparent)` }}
            >
              {skill.emoji || <SourceIcon className="h-4 w-4" style={{ color: style.color }} />}
            </div>
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
                  {skill.name}
                </span>
                {skill.version && (
                  <span className="px-1.5 py-[1px] rounded-[var(--radius-sm)] bg-[var(--bg-content)] text-[9px] font-mono text-[var(--text-tertiary)] border border-[var(--border-subtle)] shrink-0">
                    v{skill.version}
                  </span>
                )}
              </div>
              {/* Source + invocable inline under name */}
              <div className="flex items-center gap-1.5 mt-0.5">
                <span className="text-[10px] font-medium uppercase tracking-[0.06em]" style={{ color: style.color }}>
                  {t(style.labelKey)}
                </span>
                {skill.user_invocable && (
                  <>
                    <span className="text-[var(--text-tertiary)] text-[8px]">/</span>
                    <span className="text-[10px] text-[var(--accent)] font-medium flex items-center gap-0.5">
                      <Zap className="h-2.5 w-2.5" />
                      {t("dashboard.invocable", "invocable")}
                    </span>
                  </>
                )}
              </div>
            </div>
          </div>
          <div onClick={(e) => e.stopPropagation()}>
            <Toggle checked={enabled} onChange={onToggle} size="sm" />
          </div>
        </div>

        {/* Description */}
        {skill.description && (
          <p className="text-[11px] text-[var(--text-secondary)] leading-[1.6] mb-3 line-clamp-2">
            {skill.description}
          </p>
        )}

        {/* Missing requirements warning */}
        {hasMissing && enabled && (
          <div className="mb-3 p-2.5 rounded-[var(--radius-md)] bg-[var(--error)]/4 border border-[var(--error)]/12">
            <div className="flex items-center gap-1.5 mb-1.5">
              <AlertTriangle className="h-3 w-3 text-[var(--error)]/80 shrink-0" />
              <span className="text-[10px] font-semibold text-[var(--error)]/80 uppercase tracking-[0.04em]">
                {t("dashboard.ineligible", "Ineligible")}
              </span>
            </div>
            <div className="text-[10px] leading-[1.6] space-y-1 pl-[18px]">
              {(skill.missing_env?.length ?? 0) > 0 && (
                <div className="flex items-center gap-2">
                  <span className="px-1 py-[1px] rounded bg-[var(--bg-content)] text-[9px] font-mono text-[var(--text-tertiary)] border border-[var(--border-subtle)]">ENV</span>
                  <span className="font-mono text-[var(--text-secondary)]">{skill.missing_env!.join(", ")}</span>
                </div>
              )}
              {(skill.missing_bins?.length ?? 0) > 0 && (
                <div className="flex items-center gap-2">
                  <span className="px-1 py-[1px] rounded bg-[var(--bg-content)] text-[9px] font-mono text-[var(--text-tertiary)] border border-[var(--border-subtle)]">BIN</span>
                  <span className="font-mono text-[var(--text-secondary)]">{skill.missing_bins!.join(", ")}</span>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Footer: tags + actions — only render if there's something to show */}
        {(skill.has_install_specs || skill.homepage || skill.path) && (
          <div className="flex items-center justify-between pt-2 border-t border-[var(--border-subtle)]/50">
            <div className="flex items-center gap-1.5">
              {skill.has_install_specs && (
                <span
                  className="flex items-center gap-1 px-1.5 py-0.5 rounded-[var(--radius-sm)] bg-[var(--bg-content)] text-[10px] text-[var(--text-tertiary)] border border-[var(--border-subtle)] pointer-events-none select-none"
                  title={t("dashboard.hasInstallLabel", "Has install instructions")}
                >
                  <Package className="h-2.5 w-2.5" />
                  {t("dashboard.hasInstallSpecs", "install")}
                </span>
              )}
            </div>
            <div className="flex items-center gap-0.5">
              {skill.homepage && (
                <a
                  href={skill.homepage}
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={(e) => e.stopPropagation()}
                  className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--accent)] hover:bg-[var(--bg-hover)] transition-colors"
                  title={t("dashboard.skillHomepage", "Documentation")}
                >
                  <ExternalLink className="h-3.5 w-3.5" />
                </a>
              )}
              {skill.path && (
                <button
                  onClick={(e) => { e.stopPropagation(); setExpanded(!expanded); }}
                  className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  title={expanded ? t("dashboard.hide", "Hide") : t("dashboard.view", "View")}
                >
                  {expanded ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                </button>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Expanded content preview */}
      {expanded && skill.path && (
        <SkillContentPreview path={skill.path} />
      )}
    </div>
  );
}

function SkillContentPreview({ path }: { path: string }) {
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(`/api/dashboard/skills/content?path=${encodeURIComponent(path)}`)
      .then(res => res.ok ? res.json() : null)
      .then(data => {
        if (data?.content) setContent(data.content);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [path]);

  if (loading) {
    return <div className="mx-4 mb-4 p-3 rounded-[var(--radius-md)] bg-[var(--bg-window)] animate-pulse h-24" />;
  }

  if (!content) {
    return (
      <div className="mx-4 mb-4 p-3 rounded-[var(--radius-md)] bg-[var(--bg-window)] text-[10px] text-[var(--text-tertiary)] text-center">
        Unable to load skill content
      </div>
    );
  }

  const lines = content.split("\n").slice(0, 30);
  const truncated = content.split("\n").length > 30;

  return (
    <div className="mx-4 mb-4 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] overflow-hidden">
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-[var(--border-subtle)]/50 bg-[var(--bg-content)]/50">
        <span className="text-[9px] font-mono text-[var(--text-tertiary)] truncate">{path.split("/").pop()}</span>
        {truncated && <span className="text-[9px] text-[var(--text-tertiary)]">30/{content.split("\n").length} lines</span>}
      </div>
      <div className="overflow-auto max-h-[200px] p-3">
        <pre className="text-[10px] font-mono text-[var(--text-secondary)] leading-[1.6] whitespace-pre-wrap">
          {lines.join("\n")}
          {truncated && "\n..."}
        </pre>
      </div>
    </div>
  );
}
