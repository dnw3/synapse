import { useTranslation } from "react-i18next";
import { Clock, Bot } from "lucide-react";
import { PAGE_LIMIT } from "./sessionsHelpers";

interface SessionsFilterBarProps {
  activeWithinMin: string;
  setActiveWithinMin: (v: string) => void;
  includeGlobal: boolean;
  setIncludeGlobal: (v: boolean) => void;
  agentFilter: string;
  setAgentFilter: (v: string) => void;
  limitInput: string;
  setLimitInput: (v: string) => void;
  uniqueAgentIds: string[];
}

export default function SessionsFilterBar({
  activeWithinMin,
  setActiveWithinMin,
  includeGlobal,
  setIncludeGlobal,
  agentFilter,
  setAgentFilter,
  limitInput,
  setLimitInput,
  uniqueAgentIds,
}: SessionsFilterBarProps) {
  const { t } = useTranslation();

  return (
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
  );
}
