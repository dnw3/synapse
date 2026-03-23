import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  FolderOpen,
  Eye, EyeOff, AlertTriangle, ExternalLink, Package,
  Zap, User, Box,
} from "lucide-react";
import type { SkillEntry } from "../../../types/dashboard";
import {
  Toggle,
} from "../shared";
import { cn } from "../../../lib/cn";

// ---------------------------------------------------------------------------
// Local tab types
// ---------------------------------------------------------------------------

export type SourceGroup = "project" | "personal" | "built-in";
export const SOURCE_ORDER: SourceGroup[] = ["project", "personal", "built-in"];

export function resolveSource(skill: SkillEntry): SourceGroup {
  if (skill.source === "personal") return "personal";
  if (skill.source === "built-in") return "built-in";
  return "project";
}

export const SOURCE_STYLE: Record<SourceGroup, { icon: typeof Zap; color: string; bg: string; border: string; labelKey: string }> = {
  project: { icon: FolderOpen, color: "var(--accent)", bg: "var(--accent)", border: "var(--accent)", labelKey: "dashboard.sourceProject" },
  personal: { icon: User, color: "#a78bfa", bg: "#a78bfa", border: "#a78bfa", labelKey: "dashboard.sourcePersonal" },
  "built-in": { icon: Box, color: "var(--success)", bg: "var(--success)", border: "var(--success)", labelKey: "dashboard.sourceBuiltIn" },
};

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

export function formatNumber(n: number): string {
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

export function extToLang(filename: string): string {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  return EXT_LANG[ext] ?? "text";
}

// ===========================================================================
// LOCAL SKILL CARD
// ===========================================================================

export function SkillCard({
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

export function SkillContentPreview({ path }: { path: string }) {
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
