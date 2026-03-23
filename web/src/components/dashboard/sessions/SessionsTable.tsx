import { useRef, Fragment } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight, Trash2, Pencil, PackageMinus, ExternalLink } from "lucide-react";
import { cn } from "../../../lib/cn";
import type { NormalizedSession } from "./sessionsHelpers";
import { formatTokens, formatCost, formatDate } from "../../../lib/format";
import {
  type SortField,
  type SortOrder,
  THINKING_OPTIONS,
  CHANNEL_COLORS,
  KIND_COLORS,
} from "./sessionsHelpers";

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

interface SessionsTableProps {
  sessions: NormalizedSession[];
  sortField: SortField;
  sortOrder: SortOrder;
  onSort: (f: SortField) => void;
  expandedId: string | null;
  onToggleExpand: (id: string) => void;
  editingId: string | null;
  editValue: string;
  setEditValue: (v: string) => void;
  onStartEditing: (id: string) => void;
  onRename: (id: string) => void;
  onCancelEdit: () => void;
  editingLabelId: string | null;
  editLabelValue: string;
  setEditLabelValue: (v: string) => void;
  onStartLabelEdit: (session: NormalizedSession) => void;
  onLabelSave: (id: string) => void;
  onCancelLabelEdit: () => void;
  confirming: string | null;
  onDelete: (id: string) => void;
  onCompact: (id: string) => void;
  onThinkingChange: (id: string, value: string) => void;
  onNavigateToChat: (key: string) => void;
  language: string;
}

export default function SessionsTable({
  sessions,
  sortField,
  sortOrder,
  onSort,
  expandedId,
  onToggleExpand,
  editingId,
  editValue,
  setEditValue,
  onStartEditing,
  onRename,
  onCancelEdit,
  editingLabelId,
  editLabelValue,
  setEditLabelValue,
  onLabelSave,
  onCancelLabelEdit,
  confirming,
  onDelete,
  onCompact,
  onThinkingChange,
  onNavigateToChat,
  language,
}: SessionsTableProps) {
  const { t } = useTranslation();
  const editRef = useRef<HTMLInputElement>(null);
  const labelEditRef = useRef<HTMLInputElement>(null);

  return (
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
            <SortHeader field="created_at" label={t("sessions.colUpdated")} sortField={sortField} sortOrder={sortOrder} onSort={onSort} />
            <SortHeader field="message_count" label={t("sessions.messages")} align="right" sortField={sortField} sortOrder={sortOrder} onSort={onSort} />
            <SortHeader field="token_count" label="Tokens" align="right" sortField={sortField} sortOrder={sortOrder} onSort={onSort} />
            <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
              Thinking
            </th>
            <th className="px-3 py-2.5 text-xs uppercase tracking-wider text-[var(--text-tertiary)] border-b border-[var(--border-subtle)] text-right">
              {t("sessions.actions")}
            </th>
          </tr>
        </thead>
        <tbody>
          {sessions.map((session) => (
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
                    onClick={() => onToggleExpand(session.id)}
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
                      onBlur={() => onRename(session.id)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") onRename(session.id);
                        if (e.key === "Escape") onCancelEdit();
                      }}
                      className="px-1.5 py-0.5 text-[12px] font-mono bg-[var(--bg-grouped)] border border-[var(--accent)] rounded-[var(--radius-sm)] text-[var(--text-primary)] focus:outline-none w-36"
                      autoFocus
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
                      onBlur={() => onLabelSave(session.id)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") onLabelSave(session.id);
                        if (e.key === "Escape") onCancelLabelEdit();
                      }}
                      placeholder={t("sessions.enterLabel")}
                      className="px-1.5 py-0.5 text-[12px] bg-[var(--bg-grouped)] border border-[var(--accent)] rounded-[var(--radius-sm)] text-[var(--text-primary)] focus:outline-none w-40"
                      autoFocus
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
                    ? formatDate(session.updated_at, language)
                    : formatDate(session.created_at, language)}
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
                    onChange={(v) => onThinkingChange(session.id, v)}
                  />
                </td>

                {/* Actions */}
                <td className="px-3 py-2.5 border-b border-[var(--border-subtle)] text-right">
                  <div className="flex items-center justify-end gap-1">
                    {/* Rename */}
                    <button
                      onClick={() => onStartEditing(session.id)}
                      className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-content)] transition-colors cursor-pointer"
                      title={t("sessions.rename")}
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>

                    {/* Compact */}
                    <button
                      onClick={() => onCompact(session.id)}
                      className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-content)] transition-colors cursor-pointer"
                      title={t("sessions.compact")}
                    >
                      <PackageMinus className="h-3.5 w-3.5" />
                    </button>

                    {/* Delete */}
                    <button
                      onClick={() => onDelete(session.id)}
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
                            {formatDate(session.created_at, language)}
                          </div>
                          {session.updated_at && (
                            <div>
                              <span className="text-[var(--text-tertiary)]">{t("sessions.updatedLabel")}</span>
                              {formatDate(session.updated_at, language)}
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
  );
}
