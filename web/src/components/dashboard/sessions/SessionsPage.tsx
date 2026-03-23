import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { MessageSquare, Filter, Search } from "lucide-react";
import { cn } from "../../../lib/cn";
import {
  useSessions, useDeleteSession, useRenameSession,
  usePatchSessionOverrides, useCompactSession,
} from "../../../hooks/queries/useSessionsQueries";
import { useSessionCtx } from "../../../contexts";
import type { NormalizedSession } from "./sessionsHelpers";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  Pagination,
  StatsCard,
  useInlineConfirm,
} from "../shared";
import { formatTokens } from "../../../lib/format";
import {
  type SortField,
  type SortOrder,
  PAGE_LIMIT,
  extractAgentId,
  normalizeSession,
} from "./sessionsHelpers";
import SessionsFilterBar from "./SessionsFilterBar";
import SessionsTable from "./SessionsTable";

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

  // Inline ID editing (rename)
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

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
    const rawArr = Array.isArray(rawData) ? rawData : [];
    return rawArr.map((r) => normalizeSession(r as unknown as Record<string, unknown>));
  }, [rawData]);
  const total = sessions.length;
  const loading = sessionsQ.isPending;

  // Helper for navigation to chat
  const onNavigateToChat = (key: string) => {
    sessionCtx.setActiveKey(key);
    navigate("/chat");
  };

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

  const startLabelEdit = (session: NormalizedSession) => {
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
        // eslint-disable-next-line react-hooks/purity
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
          <SessionsFilterBar
            activeWithinMin={activeWithinMin}
            setActiveWithinMin={setActiveWithinMin}
            includeGlobal={includeGlobal}
            setIncludeGlobal={setIncludeGlobal}
            agentFilter={agentFilter}
            setAgentFilter={setAgentFilter}
            limitInput={limitInput}
            setLimitInput={setLimitInput}
            uniqueAgentIds={uniqueAgentIds}
          />
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
            <SessionsTable
              sessions={filtered}
              sortField={sortField}
              sortOrder={sortOrder}
              onSort={handleSort}
              expandedId={expandedId}
              onToggleExpand={toggleExpand}
              editingId={editingId}
              editValue={editValue}
              setEditValue={setEditValue}
              onStartEditing={startEditing}
              onRename={handleRename}
              onCancelEdit={() => setEditingId(null)}
              editingLabelId={editingLabelId}
              editLabelValue={editLabelValue}
              setEditLabelValue={setEditLabelValue}
              onStartLabelEdit={startLabelEdit}
              onLabelSave={handleLabelSave}
              onCancelLabelEdit={() => setEditingLabelId(null)}
              confirming={confirming}
              onDelete={handleDelete}
              onCompact={handleCompact}
              onThinkingChange={handleThinkingChange}
              onNavigateToChat={onNavigateToChat}
              language={i18n.language}
            />

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
