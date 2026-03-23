import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Download, Star, User,
  Loader2, CheckCircle2, GitBranch,
  X, Shield, Clock, FileText, Copy, Check,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { useCodeTheme } from "../../../hooks/useCodeTheme";
import { fetchJSON } from "../../../lib/api";
import type { StoreSkillDetail } from "../../../types/dashboard";
import { cn } from "../../../lib/cn";
import { formatNumber, extToLang } from "./skillsConstants";

// ===========================================================================
// STORE SKILL DETAIL MODAL
// ===========================================================================

export function StoreSkillDetailModal({
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
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    fetchJSON<StoreSkillDetail>(`/store/skills/${encodeURIComponent(slug)}`).then((d) => {
      setDetail(d);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [slug]);

  // Lazy-load files when Files tab is selected
  useEffect(() => {
    if (detailTab === "files" && !filesLoaded.current) {
      filesLoaded.current = true;
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setFilesLoading(true);
      fetchJSON<{ files: { name: string; size: number }[]; skillMd: string | null }>(`/store/skills/${encodeURIComponent(slug)}/files`).then((d) => {
        if (d) {
          setSkillMd(d.skillMd);
          setFileList(d.files);
        }
        setFilesLoading(false);
      }).catch(() => setFilesLoading(false));
    }
  }, [detailTab, slug]);

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
                              fetchJSON<{ content: string | null }>(`/store/skills/${encodeURIComponent(slug)}/files/${f.name.split("/").map(encodeURIComponent).join("/")}`).then((d) => {
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
