import { useState, useEffect, useMemo, useRef, Fragment } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { MessageSquare, ChevronDown, ChevronRight, Trash2, Pencil, PackageMinus, Search, Filter, Clock, ExternalLink, Bot } from "lucide-react";
import { cn } from "../../lib/cn";
import {
  useSessions, useDeleteSession, useRenameSession,
  usePatchSessionOverrides, useCompactSession,
} from "../../hooks/queries/useSessionsQueries";
import { useSessionCtx } from "../../contexts";
import type { SessionEntry } from "../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  Pagination,
  StatsCard,
  useInlineConfirm,
} from "./shared";
import { formatTokens, formatCost, formatDate } from "../../lib/format";

type SortField = "id" | "created_at" | "message_count" | "token_count";
type SortOrder = "asc" | "desc";

const PAGE_LIMIT = 50;

function extractAgentId(sessionKey: string): string {
  const match = sessionKey.match(/^agent:([^:]+):/);
  return match?.[1] ?? "default";
}

const THINKING_OPTIONS = ["off", "low", "medium", "high", "adaptive"] as const;

/** Normalize a raw API session object: ensure `id` mirrors `key`. */
function normalizeSession(raw: Record<string, unknown>): SessionEntry {
  const key = (raw.key as string) ?? (raw.id as string) ?? "";
  return {
    ...(raw as unknown as SessionEntry),
    key,
    id: key,
  };
}

/** Badge component for channel / kind labels. */
function Badge({ value, colorClass }: { value: string; colorClass?: string }) {
  return (
    <span
      className={cn(
        "px-1.5 py-0.5 rounded text-[10px] font-medium border flex-shrink-0",
        colorClass ??
          "bg-[var(--accent)]/10 text-[var(--accent)] border-[var(--accent)]/20"
      )}
    >
      {value}
    </span>
  );
}

const CHANNEL_COLORS: Record<string, string> = {
  web: "bg-blue-500/10 text-blue-400 border-blue-500/20",
  lark: "bg-teal-500/10 text-teal-400 border-teal-500/20",
  telegram: "bg-sky-500/10 text-sky-400 border-sky-500/20",
  discord: "bg-indigo-500/10 text-indigo-400 border-indigo-500/20",
  slack: "bg-purple-500/10 text-purple-400 border-purple-500/20",
};

const KIND_COLORS: Record<string, string> = {
  direct: "bg-green-500/10 text-green-400 border-green-500/20",
  group: "bg-orange-500/10 text-orange-400 border-orange-500/20",
  main: "bg-[var(--accent)]/10 text-[var(--accent)] border-[var(--accent)]/20",
};

/** Sort column header button. */
function SortHeader({
  field,
  label,
  align,
  sortField,
  sortOrder,
  onSort,
}: {
  field: SortField;
  label: string;
  align?: "right";
  sortField: SortField;
  sortOrder: SortOrder;
  onSort: (f: SortField) => void;
}) {
  return (
    <th
      className={cn(
        "px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)] cursor-pointer select-none hover:text-[var(--text-secondary)] transition-colors",
        align === "right" && "text-right"
      )}
      onClick={() => onSort(field)}
    >
      <span className="inline-flex items-center gap-1">
        {label}
        <ChevronDown
          className={cn(
            "h-3 w-3 transition-transform",
            sortField === field ? "opacity-100" : "opacity-0",
            sortField === field && sortOrder === "asc" && "rotate-180"
          )}
        />
      </span>
    </th>
  );
}

/** Inline dropdown selector. */
function InlineSelect({
  value,
  options,
  onChange,
}: {
  value: string;
  options: readonly string[];
  onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="px-1.5 py-0.5 text-[11px] rounded-[var(--radius-sm)] bg-[var(--bg-grouped)] border border-[var(--separator)] text-[var(--text-secondary)] focus:outline-none focus:border-[var(--accent)] transition-colors cursor-pointer appearance-none pr-5"
      style={{
        backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E")`,
        backgroundRepeat: "no-repeat",
        backgroundPosition: "right 4px center",
      }}
    >
      {options.map((o) => (
        <option key={o} value={o}>
          {o}
        </option>
      ))}
    </select>
  );
}

export default function SessionsPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const sessionCtx = useSessionCtx();

  const [offset, setOffset] = useState(0);
  const [sortField, setSortField] = useState<SortField>("created_at");
  const [sortOrder, setSortOrder] = useState<SortOrder>("desc");
  const [search, setSearch] = useState("");

  // Filter bar state
  const [showFilters, setShowFilters] = useState(false);
  const [activeWithinMin, setActiveWithinMin] = useState<string>("");
  const [includeGlobal, setIncludeGlobal] = useState(false);
  const [limitInput, setLimitInput] = useState<string>("");
  const [agentFilter, setAgentFilter] = useState<string>("");


  // Inline label editing
  const [editingLabelId, setEditingLabelId] = useState<string | null>(null);
  const [editLabelValue, setEditLabelValue] = useState("");
  const labelEditRef = useRef<HTMLInputElement>(null);

  // Inline ID editing (rename)
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const editRef = useRef<HTMLInputElement>(null);

  // Expanded row
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const { confirming, requestConfirm, reset: resetConfirm } = useInlineConfirm(3000);

  const deleteMut = useDeleteSession();
  const renameMut = useRenameSession();
  const patchMut = usePatchSessionOverrides();
  const compactMut = useCompactSession();

  const sessionsQ = useSessions({
    limit: limitInput ? parseInt(limitInput, 10) || PAGE_LIMIT : PAGE_LIMIT,
    offset,
    sort: sortField,
    order: sortOrder,
  });

  const rawData = sessionsQ.data;
  const sessions = useMemo(() => {
    if (!rawData) return [];
    const rawArr = rawData.sessions ?? [];
    return rawArr.map((r) => normalizeSession(r as unknown as Record<string, unknown>));
  }, [rawData]);
  const total = rawData?.total ?? sessions.length;
  const loading = sessionsQ.isPending;

  // Helper for navigation to chat
  const onNavigateToChat = (key: string) => {
    sessionCtx.setActiveKey(key);
    navigate("/chat");
  };

  // Focus edit inputs
  useEffect(() => {
    if (editingId && editRef.current) {
      editRef.current.focus();
      editRef.current.select();
    }
  }, [editingId]);

  useEffect(() => {
    if (editingLabelId && labelEditRef.current) {
      labelEditRef.current.focus();
      labelEditRef.current.select();
    }
  }, [editingLabelId]);

  // Sort handler
  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortOrder((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortOrder("desc");
    }
    setOffset(0);
  };

  // Actions
  const handleDelete = async (id: string) => {
    if (confirming !== id) {
      requestConfirm(id);
      return;
    }
    resetConfirm();
    deleteMut.mutate(id);
  };

  const handleRename = async (id: string) => {
    if (!editValue.trim()) {
      setEditingId(null);
      return;
    }
    renameMut.mutate({ id, displayName: editValue.trim() });
    setEditingId(null);
  };

  const handleLabelSave = async (id: string) => {
    patchMut.mutate({ id, overrides: { label: editLabelValue.trim() || undefined } });
    setEditingLabelId(null);
  };

  const handleThinkingChange = async (id: string, value: string) => {
    patchMut.mutate({ id, overrides: { thinking: value } });
  };

  const handleCompact = async (id: string) => {
    compactMut.mutate(id);
  };

  const startEditing = (id: string) => {
    setEditingId(id);
    setEditValue(id);
  };

  const _startLabelEdit = (session: SessionEntry) => {
    setEditingLabelId(session.id);
    setEditLabelValue(session.label ?? "");
  };

  const toggleExpand = (id: string) => {
    setExpandedId((prev) => (prev === id ? null : id));
  };

  // Extract unique agent IDs from sessions
  const uniqueAgentIds = useMemo(() => {
    const ids = new Set(sessions.map((s) => extractAgentId(s.id)));
    return Array.from(ids).sort();
  }, [sessions]);

  // Filter by search, active-within, global, and agent
  const filtered = useMemo(() => {
    let result = search
      ? sessions.filter(
          (s) =>
            s.id.toLowerCase().includes(search.toLowerCase()) ||
            (s.label && s.label.toLowerCase().includes(search.toLowerCase()))
        )
      : sessions;

    // Active within filter
    if (activeWithinMin) {
      const mins = parseInt(activeWithinMin, 10);
      if (mins > 0) {
        const cutoff = Date.now() - mins * 60 * 1000;
        result = result.filter((s) => {
          const ts = s.updated_at || s.created_at;
          if (!ts) return true;
          const d = /^\d+$/.test(ts) ? new Date(Number(ts)) : new Date(ts);
          return d.getTime() >= cutoff;
        });
      }
    }

    // Include global filter (currently a placeholder - filters sessions with "global" in id)
    if (!includeGlobal) {
      result = result.filter((s) => !s.id.startsWith("global:"));
    }

    // Agent filter
    if (agentFilter) {
      result = result.filter((s) => extractAgentId(s.id) === agentFilter);
    }

    return result;
  }, [sessions, search, activeWithinMin, includeGlobal, agentFilter]);

  // Stats
  const totalMessages = sessions.reduce((sum, s) => sum + (s.message_count ?? 0), 0);
  const totalTokens = sessions.reduce((sum, s) => sum + (s.token_count ?? 0), 0);

  return (
    <div className="flex flex-col h-full min-h-0 gap-5">
      {/* Stats cards */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <StatsCard
          icon={<MessageSquare className="h-5 w-5" />}
          label={t("sessions.totalSessions")}
          value={total.toLocaleString()}
          accent="var(--accent)"
        />
        <StatsCard
          icon={<MessageSquare className="h-5 w-5" />}
          label={t("sessions.totalMessages")}
          value={totalMessages.toLocaleString()}
          accent="var(--chart-2)"
        />
        <StatsCard
          icon={<MessageSquare className="h-5 w-5" />}
          label={t("sessions.totalTokens")}
          value={formatTokens(totalTokens)}
          accent="var(--chart-3)"
        />
      </div>

      {/* Sessions table */}
      <SectionCard className="flex-1 min-h-0 flex flex-col">
        <SectionHeader
          icon={<MessageSquare className="h-4 w-4" />}
          title={t("sessions.sessionList")}
          right={
            <div className="flex items-center gap-2">
              {/* Filter toggle */}
              <button
                onClick={() => setShowFilters((v) => !v)}
                className={cn(
                  "p-1.5 rounded-[var(--radius-sm)] transition-colors cursor-pointer",
                  showFilters
                    ? "text-[var(--accent)] bg-[var(--accent)]/10"
                    : "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-content)]"
                )}
                title={t("sessions.filters")}
              >
                <Filter className="h-3.5 w-3.5" />
              </button>
              {/* Search */}
              <div className="relative">
                <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-[var(--text-tertiary)]" />
                <input
                  type="text"
                  placeholder={t("sessions.searchPlaceholder")}
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="pl-8 pr-3 py-1.5 text-[12px] rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--border-subtle)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:border-[var(--accent)] focus:ring-1 focus:ring-[var(--accent-glow)] transition-all w-52"
                />
              </div>
            </div>
          }
        />

        {/* Filter bar */}
        {showFilters && (
          <div className="flex flex-wrap items-center gap-4 px-3 py-2.5 mb-2 rounded-[var(--radius-md)] bg-[var(--bg-grouped)] border border-[var(--separator)]">
            {/* Active within */}
            <label className="flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
              <Clock className="h-3 w-3 text-[var(--text-tertiary)]" />
              {t("sessions.activeWithin")}
              <input
                type="number"
                min="0"
                value={activeWithinMin}
                onChange={(e) => setActiveWithinMin(e.target.value)}
                placeholder="--"
                className="w-14 px-1.5 py-0.5 text-[11px] rounded-[var(--radius-sm)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] text-center"
              />
              <span className="text-[var(--text-tertiary)]">{t("sessions.minutes")}</span>
            </label>

            {/* Include global */}
            <label className="flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)] cursor-pointer select-none">
              <input
                type="checkbox"
                checked={includeGlobal}
                onChange={(e) => setIncludeGlobal(e.target.checked)}
                className="rounded border-[var(--border-subtle)] text-[var(--accent)] focus:ring-[var(--accent)] h-3 w-3"
              />
              {t("sessions.includeGlobal")}
            </label>

            {/* Agent filter */}
            <label className="flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
              <Bot className="h-3 w-3 text-[var(--text-tertiary)]" />
              {t("sessions.agentFilter")}
              <select
                value={agentFilter}
                onChange={(e) => setAgentFilter(e.target.value)}
                className="px-1.5 py-0.5 text-[11px] rounded-[var(--radius-sm)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] cursor-pointer"
              >
                <option value="">{t("sessions.allAgents")}</option>
                {uniqueAgentIds.map((id) => (
                  <option key={id} value={id}>{id}</option>
                ))}
              </select>
            </label>

            {/* Limit */}
            <label className="flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
              {t("sessions.limit")}
              <input
                type="number"
                min="1"
                max="500"
                value={limitInput}
                onChange={(e) => setLimitInput(e.target.value)}
                placeholder={String(PAGE_LIMIT)}
                className="w-16 px-1.5 py-0.5 text-[11px] rounded-[var(--radius-sm)] bg-[var(--bg-window)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] text-center"
              />
            </label>
          </div>
        )}

        {loading ? (
          <div className="space-y-2">
            {Array.from({ length: 8 }).map((_, i) => (
              <LoadingSkeleton key={i} className="h-10 w-full" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={<MessageSquare className="h-8 w-8" />}
            message={t("sessions.noSessions")}
          />
        ) : (
          <>
            <div className="overflow-x-auto flex-1 min-h-0 overflow-y-auto">
              <table className="w-full text-[12px]">
                <thead>
                  <tr>
                    <th className="w-6 px-1 py-2.5 border-b border-[var(--border-subtle)]" />
                    {/* KEY column */}
                    <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
                      {t("sessions.colKey")}
                    </th>
                    {/* LABEL column */}
                    <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
                      {t("sessions.colLabel")}
                    </th>
                    {/* CHANNEL column */}
                    <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
                      {t("sessions.colChannel")}
                    </th>
                    {/* KIND column */}
                    <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
                      {t("sessions.colKind")}
                    </th>
                    {/* UPDATED column */}
                    <SortHeader field="created_at" label={t("sessions.colUpdated")} sortField={sortField} sortOrder={sortOrder} onSort={handleSort} />
                    <SortHeader field="message_count" label={t("sessions.messages")} align="right" sortField={sortField} sortOrder={sortOrder} onSort={handleSort} />
                    <SortHeader field="token_count" label="Tokens" align="right" sortField={sortField} sortOrder={sortOrder} onSort={handleSort} />
                    <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
                      Thinking
                    </th>
                    <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)] text-right">
                      {t("sessions.actions")}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.map((session) => (
                    <Fragment key={session.id}>
                      <tr
                        className={cn(
                          "hover:bg-[var(--bg-hover)] transition-colors",
                          expandedId === session.id && "bg-[var(--bg-hover)]"
                        )}
                      >
                        {/* Expand toggle */}
                        <td className="px-1 py-2.5 border-b border-[var(--border-subtle)]">
                          <button
                            onClick={() => toggleExpand(session.id)}
                            className="p-0.5 rounded text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                          >
                            {expandedId === session.id ? (
                              <ChevronDown className="h-3 w-3" />
                            ) : (
                              <ChevronRight className="h-3 w-3" />
                            )}
                          </button>
                        </td>

                        {/* KEY */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)]">
                          {editingId === session.id ? (
                            <input
                              ref={editRef}
                              type="text"
                              value={editValue}
                              onChange={(e) => setEditValue(e.target.value)}
                              onBlur={() => handleRename(session.id)}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") handleRename(session.id);
                                if (e.key === "Escape") setEditingId(null);
                              }}
                              className="px-1.5 py-0.5 text-[12px] font-mono bg-[var(--bg-grouped)] border border-[var(--accent)] rounded-[var(--radius-sm)] text-[var(--text-primary)] focus:outline-none w-36"
                            />
                          ) : (
                            <span
                              className="font-mono text-[var(--accent-light)] cursor-default"
                              title={session.key}
                            >
                              {session.key.length > 14 ? `${session.key.slice(0, 14)}…` : session.key}
                            </span>
                          )}
                        </td>

                        {/* LABEL — click to navigate to chat */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)] max-w-[220px] overflow-hidden">
                          {editingLabelId === session.id ? (
                            <input
                              ref={labelEditRef}
                              type="text"
                              value={editLabelValue}
                              onChange={(e) => setEditLabelValue(e.target.value)}
                              onBlur={() => handleLabelSave(session.id)}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") handleLabelSave(session.id);
                                if (e.key === "Escape") setEditingLabelId(null);
                              }}
                              placeholder={t("sessions.enterLabel")}
                              className="px-1.5 py-0.5 text-[12px] bg-[var(--bg-grouped)] border border-[var(--accent)] rounded-[var(--radius-sm)] text-[var(--text-primary)] focus:outline-none w-40"
                            />
                          ) : (
                            <span
                              onClick={() => onNavigateToChat(session.id)}
                              className="flex items-center gap-1 group truncate cursor-pointer hover:text-[var(--accent)] transition-colors"
                              title={session.label || session.display_name || session.id}
                            >
                              {session.label ? (
                                <span className="font-medium text-[var(--text-secondary)]">{session.label}</span>
                              ) : session.display_name ? (
                                <span className="truncate text-[var(--text-secondary)]">{session.display_name}</span>
                              ) : (
                                <span className="text-[var(--text-tertiary)] italic text-[11px]">
                                  {t("sessions.noLabel")}
                                </span>
                              )}
                              <ExternalLink className="h-2.5 w-2.5 opacity-0 group-hover:opacity-50 transition-opacity flex-shrink-0" />
                            </span>
                          )}
                        </td>

                        {/* CHANNEL badge */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)]">
                          {session.channel ? (
                            <Badge
                              value={session.channel}
                              colorClass={CHANNEL_COLORS[session.channel.toLowerCase()]}
                            />
                          ) : (
                            <span className="text-[var(--text-tertiary)] text-[11px]">—</span>
                          )}
                        </td>

                        {/* KIND badge */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)]">
                          {session.kind ? (
                            <Badge
                              value={session.kind}
                              colorClass={KIND_COLORS[session.kind.toLowerCase()]}
                            />
                          ) : (
                            <span className="text-[var(--text-tertiary)] text-[11px]">—</span>
                          )}
                        </td>

                        {/* UPDATED */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)] text-[var(--text-secondary)] whitespace-nowrap">
                          {session.updated_at
                            ? formatDate(session.updated_at, i18n.language)
                            : formatDate(session.created_at, i18n.language)}
                        </td>

                        {/* Messages */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)] text-right tabular-nums text-[var(--text-secondary)]">
                          {(session.message_count ?? 0).toLocaleString()}
                        </td>

                        {/* Tokens */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)] text-right tabular-nums text-[var(--text-secondary)]">
                          {formatTokens(session.token_count ?? 0)}
                        </td>

                        {/* Thinking level */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)]">
                          <InlineSelect
                            value={session.thinking_level ?? "off"}
                            options={THINKING_OPTIONS}
                            onChange={(v) => handleThinkingChange(session.id, v)}
                          />
                        </td>

                        {/* Actions */}
                        <td className="px-3 py-2.5 border-b border-[var(--border-subtle)] text-right">
                          <div className="flex items-center justify-end gap-1">
                            {/* Rename */}
                            <button
                              onClick={() => startEditing(session.id)}
                              className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-content)] transition-colors cursor-pointer"
                              title={t("sessions.rename")}
                            >
                              <Pencil className="h-3.5 w-3.5" />
                            </button>

                            {/* Compact */}
                            <button
                              onClick={() => handleCompact(session.id)}
                              className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-content)] transition-colors cursor-pointer"
                              title={t("sessions.compact")}
                            >
                              <PackageMinus className="h-3.5 w-3.5" />
                            </button>

                            {/* Delete */}
                            <button
                              onClick={() => handleDelete(session.id)}
                              className={cn(
                                "p-1.5 rounded-[var(--radius-sm)] transition-colors cursor-pointer",
                                confirming === session.id
                                  ? "text-[var(--error)] bg-[var(--error)]/10"
                                  : "text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--bg-content)]"
                              )}
                              title={confirming === session.id ? t("sessions.clickToConfirm") : t("sessions.delete")}
                            >
                              {confirming === session.id ? (
                                <span className="text-[11px] font-medium px-0.5">
                                  {t("sessions.confirm")}
                                </span>
                              ) : (
                                <Trash2 className="h-3.5 w-3.5" />
                              )}
                            </button>
                          </div>
                        </td>
                      </tr>

                      {/* Expanded detail row */}
                      {expandedId === session.id && (
                        <tr key={`${session.id}-detail`}>
                          <td
                            colSpan={10}
                            className="px-6 py-3 border-b border-[var(--separator)] bg-[var(--bg-grouped)]"
                          >
                            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-[11px]">
                              {/* Token breakdown */}
                              <div>
                                <div className="text-[var(--text-tertiary)] uppercase tracking-wider mb-1">
                                  {t("sessions.tokenBreakdown")}
                                </div>
                                <div className="space-y-0.5 text-[var(--text-secondary)]">
                                  <div className="flex justify-between">
                                    <span>Input</span>
                                    <span className="tabular-nums">{formatTokens(session.input_tokens ?? 0)}</span>
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Output</span>
                                    <span className="tabular-nums">{formatTokens(session.output_tokens ?? 0)}</span>
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Cache</span>
                                    <span className="tabular-nums">{formatTokens(session.cache_tokens ?? 0)}</span>
                                  </div>
                                </div>
                              </div>

                              {/* Cost */}
                              <div>
                                <div className="text-[var(--text-tertiary)] uppercase tracking-wider mb-1">
                                  {t("sessions.cost")}
                                </div>
                                <div className="text-[var(--text-primary)] font-medium text-[13px]">
                                  {formatCost(session.cost ?? 0)}
                                </div>
                              </div>

                              {/* Timestamps */}
                              <div>
                                <div className="text-[var(--text-tertiary)] uppercase tracking-wider mb-1">
                                  {t("sessions.timestamps")}
                                </div>
                                <div className="space-y-0.5 text-[var(--text-secondary)]">
                                  <div>
                                    <span className="text-[var(--text-tertiary)]">{t("sessions.createdLabel")}</span>
                                    {formatDate(session.created_at, i18n.language)}
                                  </div>
                                  {session.updated_at && (
                                    <div>
                                      <span className="text-[var(--text-tertiary)]">{t("sessions.updatedLabel")}</span>
                                      {formatDate(session.updated_at, i18n.language)}
                                    </div>
                                  )}
                                </div>
                              </div>

                              {/* Model */}
                              <div>
                                <div className="text-[var(--text-tertiary)] uppercase tracking-wider mb-1">
                                  {t("sessions.model")}
                                </div>
                                <div className="text-[var(--text-secondary)] font-mono text-[11px]">
                                  {session.model || (
                                    <span className="text-[var(--text-tertiary)] italic">{t("sessions.notSet")}</span>
                                  )}
                                </div>
                              </div>
                            </div>
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  ))}
                </tbody>
              </table>
            </div>

            <Pagination
              total={search || activeWithinMin ? filtered.length : total}
              limit={limitInput ? parseInt(limitInput, 10) || PAGE_LIMIT : PAGE_LIMIT}
              offset={offset}
              onChange={setOffset}
            />
          </>
        )}
      </SectionCard>

    </div>
  );
}
