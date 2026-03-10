import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles, Search, FolderOpen, ChevronDown, ChevronRight, Eye, EyeOff } from "lucide-react";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { SkillEntry } from "../../types/dashboard";
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

type SourceGroup = "project" | "personal" | "built-in";

const SOURCE_ORDER: SourceGroup[] = ["project", "personal", "built-in"];

function resolveSource(skill: SkillEntry): SourceGroup {
  if (skill.source === "personal") return "personal";
  if (skill.source === "built-in") return "built-in";
  return "project";
}

function sourceBadgeClass(source: SourceGroup): string {
  switch (source) {
    case "project":
      return "bg-[var(--accent)]/15 text-[var(--accent)] border-[var(--accent)]/30";
    case "personal":
      return "bg-[var(--warning)]/15 text-[var(--warning)] border-[var(--warning)]/30";
    case "built-in":
      return "bg-[var(--success)]/15 text-[var(--success)] border-[var(--success)]/30";
  }
}

export default function SkillsPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();

  const [skills, setSkills] = useState<SkillEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [collapsed, setCollapsed] = useState<Record<SourceGroup, boolean>>({
    project: false,
    personal: false,
    "built-in": false,
  });

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
    const map: Record<SourceGroup, SkillEntry[]> = {
      project: [],
      personal: [],
      "built-in": [],
    };
    for (const skill of filtered) {
      map[resolveSource(skill)].push(skill);
    }
    return map;
  }, [filtered]);

  const toggleGroup = (group: SourceGroup) => {
    setCollapsed((prev) => ({ ...prev, [group]: !prev[group] }));
  };

  const handleToggleSkill = async (skill: SkillEntry) => {
    // Optimistic update
    const prevEnabled = skill.enabled !== false;
    setSkills((prev) =>
      prev.map((s) =>
        s.name === skill.name ? { ...s, enabled: !prevEnabled } : s
      )
    );

    const result = await api.toggleSkill(skill.name);
    if (result === null) {
      // Rollback
      setSkills((prev) =>
        prev.map((s) =>
          s.name === skill.name ? { ...s, enabled: prevEnabled } : s
        )
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
        <LoadingSkeleton className="h-10 w-full" />
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-32" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <SectionCard>
        <SectionHeader
          icon={<Sparkles className="h-4 w-4" />}
          title={t("dashboard.skills", "Skills")}
          right={
            <span className="text-[11px] text-[var(--text-tertiary)] font-mono tabular-nums">
              {filtered.length}/{skills.length}
            </span>
          }
        />

        {/* Search bar */}
        <div className="relative mb-4 max-w-[360px]">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("dashboard.searchSkills", "Search skills...")}
            className="w-full pl-8 pr-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)]/50 transition-colors"
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

              return (
                <div key={source}>
                  {/* Group header */}
                  <button
                    onClick={() => toggleGroup(source)}
                    className="flex items-center gap-2 mb-3 cursor-pointer group"
                  >
                    {isCollapsed ? (
                      <ChevronRight className="h-3.5 w-3.5 text-[var(--text-tertiary)] group-hover:text-[var(--text-secondary)] transition-colors" />
                    ) : (
                      <ChevronDown className="h-3.5 w-3.5 text-[var(--text-tertiary)] group-hover:text-[var(--text-secondary)] transition-colors" />
                    )}
                    <span className="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-[0.06em]">
                      {source}
                    </span>
                    <span className="px-1.5 py-0.5 rounded-full bg-[var(--bg-surface)] text-[10px] font-mono text-[var(--text-tertiary)] tabular-nums">
                      {items.length}
                    </span>
                  </button>

                  {/* Skill cards grid */}
                  {!isCollapsed && (
                    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                      {items.map((skill) => (
                        <SkillCard
                          key={skill.name}
                          skill={skill}
                          source={source}
                          onToggle={() => handleToggleSkill(skill)}
                        />
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </SectionCard>

      <ToastContainer toasts={toasts} />
    </div>
  );
}

function SkillCard({
  skill,
  source,
  onToggle,
}: {
  skill: SkillEntry;
  source: SourceGroup;
  onToggle: () => void;
}) {
  const enabled = skill.enabled !== false;
  const [expanded, setExpanded] = useState(false);

  return (
    <div
      className={cn(
        "rounded-[var(--radius-md)] border p-3.5 transition-all",
        enabled
          ? "bg-[var(--bg-surface)]/60 border-[var(--border-subtle)] hover:border-[var(--border-default)]"
          : "bg-[var(--bg-surface)]/30 border-[var(--border-subtle)]/50 opacity-60"
      )}
    >
      {/* Top row: name + toggle */}
      <div className="flex items-start justify-between gap-2 mb-2">
        <span className="text-[13px] font-medium text-[var(--text-primary)] truncate">
          {skill.name}
        </span>
        <Toggle checked={enabled} onChange={onToggle} size="sm" />
      </div>

      {/* Description */}
      {skill.description && (
        <p className="text-[11px] text-[var(--text-secondary)] leading-relaxed mb-2.5 line-clamp-2">
          {skill.description}
        </p>
      )}

      {/* Path */}
      {skill.path && (
        <div className="text-[10px] font-mono text-[var(--text-tertiary)] truncate mb-2.5">
          {skill.path}
        </div>
      )}

      {/* Badges + View button */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5 flex-wrap">
          <span
            className={cn(
              "px-2 py-0.5 rounded-full text-[10px] font-medium border",
              sourceBadgeClass(source)
            )}
          >
            {source}
          </span>
          {skill.user_invocable && (
            <span className="px-2 py-0.5 rounded-full text-[10px] font-medium border bg-[var(--accent)]/15 text-[var(--accent)] border-[var(--accent)]/30">
              user-invocable
            </span>
          )}
        </div>
        {skill.path && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors cursor-pointer"
          >
            {expanded ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
            {expanded ? "Hide" : "View"}
          </button>
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
    // Fetch skill file content via a simple API call
    fetch(`/api/dashboard/skills/content?path=${encodeURIComponent(path)}`)
      .then(res => res.ok ? res.json() : null)
      .then(data => {
        if (data?.content) setContent(data.content);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [path]);

  if (loading) {
    return <div className="mt-2.5 p-2.5 rounded bg-[var(--bg-surface)] animate-pulse h-20" />;
  }

  if (!content) {
    return (
      <div className="mt-2.5 p-2.5 rounded bg-[var(--bg-surface)] text-[10px] text-[var(--text-tertiary)]">
        Unable to load skill content
      </div>
    );
  }

  // Show first 30 lines
  const lines = content.split("\n").slice(0, 30);
  const truncated = content.split("\n").length > 30;

  return (
    <div className="mt-2.5 p-2.5 rounded bg-[var(--bg-surface)] border border-[var(--border-subtle)] overflow-auto max-h-[200px]">
      <pre className="text-[10px] font-mono text-[var(--text-secondary)] leading-4 whitespace-pre-wrap">
        {lines.join("\n")}
        {truncated && "\n..."}
      </pre>
    </div>
  );
}
