import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  CalendarClock, Plus, Zap, Power, Trash2, Pencil, Play, Clock, Save, X,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { ScheduleEntry, ScheduleRunEntry } from "../../types/dashboard";
import {
  StatsCard, SectionCard, SectionHeader, EmptyState, LoadingSkeleton,
  Toggle, useInlineConfirm, useToast, ToastContainer,
} from "./shared";

type ScheduleType = "cron" | "interval";

interface FormState {
  name: string;
  prompt: string;
  scheduleType: ScheduleType;
  cron: string;
  interval_secs: number;
  description: string;
  enabled: boolean;
}

const emptyForm: FormState = {
  name: "",
  prompt: "",
  scheduleType: "cron",
  cron: "",
  interval_secs: 60,
  description: "",
  enabled: true,
};

export default function SchedulesPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();

  const [schedules, setSchedules] = useState<ScheduleEntry[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [triggeringName, setTriggeringName] = useState<string | null>(null);
  const [runs, setRuns] = useState<ScheduleRunEntry[]>([]);
  const [runsLoading, setRunsLoading] = useState(false);

  const { confirming, requestConfirm, reset: resetConfirm } = useInlineConfirm(3000);
  const { toasts, addToast } = useToast();

  // ── Load schedules ──
  const loadSchedules = useCallback(async () => {
    setLoading(true);
    const data = await api.fetchSchedules();
    if (data) setSchedules(data);
    setLoading(false);
  }, [api]);

  useEffect(() => {
    loadSchedules();
  }, [loadSchedules]);

  // ── Load runs when a schedule is selected ──
  const loadRuns = useCallback(async (name: string) => {
    setRunsLoading(true);
    const data = await api.fetchScheduleRuns(name);
    setRuns(data ?? []);
    setRunsLoading(false);
  }, [api]);

  useEffect(() => {
    if (selectedName) {
      loadRuns(selectedName);
    } else {
      setRuns([]);
    }
  }, [selectedName, loadRuns]);

  // ── Stats ──
  const enabledCount = schedules?.filter((s) => s.enabled).length ?? 0;
  const totalCount = schedules?.length ?? 0;

  // ── Form helpers ──
  const clearForm = () => {
    setForm(emptyForm);
    setEditing(false);
    setSelectedName(null);
  };

  const selectForEdit = (entry: ScheduleEntry) => {
    setSelectedName(entry.name);
    setEditing(true);
    setForm({
      name: entry.name,
      prompt: entry.prompt,
      scheduleType: entry.cron ? "cron" : "interval",
      cron: entry.cron ?? "",
      interval_secs: entry.interval_secs ?? 60,
      description: entry.description ?? "",
      enabled: entry.enabled,
    });
  };

  const handleSave = async () => {
    if (!form.name.trim() || !form.prompt.trim()) {
      addToast(t("schedules.nameAndPromptRequired"), "error");
      return;
    }
    setSaving(true);
    const payload: Partial<ScheduleEntry> = {
      name: form.name.trim(),
      prompt: form.prompt.trim(),
      enabled: form.enabled,
      description: form.description.trim() || undefined,
      cron: form.scheduleType === "cron" ? form.cron.trim() || undefined : undefined,
      interval_secs: form.scheduleType === "interval" ? form.interval_secs : undefined,
    };

    let result: ScheduleEntry | null;
    if (editing && selectedName) {
      result = await api.updateSchedule(selectedName, payload);
    } else {
      result = await api.createSchedule(payload);
    }

    if (result) {
      addToast(editing ? t("schedules.updated") : t("schedules.created"), "success");
      clearForm();
      await loadSchedules();
    } else {
      addToast(t("schedules.operationFailed"), "error");
    }
    setSaving(false);
  };

  const handleDelete = async (name: string) => {
    if (confirming !== name) {
      requestConfirm(name);
      return;
    }
    resetConfirm();
    const ok = await api.deleteSchedule(name);
    if (ok) {
      addToast(t("schedules.deleted"), "success");
      if (selectedName === name) clearForm();
      await loadSchedules();
    } else {
      addToast(t("schedules.deleteFailed"), "error");
    }
  };

  const handleToggle = async (entry: ScheduleEntry) => {
    const result = await api.toggleSchedule(entry.name);
    if (result) {
      setSchedules((prev) =>
        prev?.map((s) => (s.name === entry.name ? { ...s, enabled: result.enabled } : s)) ?? null
      );
      addToast(
        result.enabled
          ? t("schedules.enabled")
          : t("schedules.disabled"),
        "success"
      );
    }
  };

  const handleTrigger = async (name: string) => {
    setTriggeringName(name);
    const result = await api.triggerSchedule(name);
    if (result) {
      addToast(t("schedules.triggered"), "success");
      if (selectedName === name) loadRuns(name);
    } else {
      addToast(t("schedules.triggerFailed"), "error");
    }
    setTriggeringName(null);
  };

  const updateField = <K extends keyof FormState>(key: K, value: FormState[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  // ── Render ──
  if (loading) {
    return (
      <div className="space-y-4 animate-fade-in">
        <div className="grid grid-cols-3 gap-3">
          {[...Array(3)].map((_, i) => (
            <LoadingSkeleton key={i} className="h-24" />
          ))}
        </div>
        <LoadingSkeleton className="h-64" />
      </div>
    );
  }

  return (
    <div className="space-y-4 animate-fade-in">
      {/* ── Stats Cards ── */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <StatsCard
          icon={<Power className="h-4 w-4" />}
          label={t("schedules.enabledLabel")}
          value={enabledCount}
          sub={`/ ${totalCount} ${t("schedules.total")}`}
          accent="var(--success)"
        />
        <StatsCard
          icon={<CalendarClock className="h-4 w-4" />}
          label={t("schedules.totalTasks")}
          value={totalCount}
          accent="var(--accent)"
        />
        <StatsCard
          icon={<Clock className="h-4 w-4" />}
          label={t("schedules.nextWake")}
          value={"\u2014"}
          accent="var(--warning)"
        />
      </div>

      {/* ── Two-panel layout ── */}
      <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
        {/* ── Left: Task list ── */}
        <div className="lg:col-span-3">
          <SectionCard>
            <SectionHeader
              icon={<CalendarClock className="h-4 w-4" />}
              title={t("schedules.scheduledTasks")}
              right={
                <button
                  onClick={clearForm}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 text-[11px] font-medium rounded-[var(--radius-md)] bg-[var(--accent)]/10 text-[var(--accent-light)] hover:bg-[var(--accent)]/20 transition-colors cursor-pointer"
                >
                  <Plus className="h-3 w-3" />
                  {t("schedules.new")}
                </button>
              }
            />

            {!schedules?.length ? (
              <EmptyState
                icon={<CalendarClock className="h-8 w-8 opacity-40" />}
                message={t("schedules.noTasks")}
              />
            ) : (
              <div className="space-y-1">
                {schedules.map((entry) => (
                  <div
                    key={entry.name}
                    onClick={() => selectForEdit(entry)}
                    className={cn(
                      "group flex items-center gap-3 px-3 py-2.5 rounded-[var(--radius-md)] cursor-pointer transition-all",
                      selectedName === entry.name
                        ? "bg-[var(--accent)]/8 border border-[var(--accent)]/20"
                        : "hover:bg-[var(--bg-hover)] border border-transparent"
                    )}
                  >
                    {/* Info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] font-medium text-[var(--text-primary)] truncate">
                          {entry.name}
                        </span>
                        <span className="text-[10px] font-mono text-[var(--accent-light)] bg-[var(--accent)]/8 px-1.5 py-0.5 rounded-[var(--radius-sm)] flex-shrink-0">
                          {entry.cron || `${entry.interval_secs}s`}
                        </span>
                      </div>
                      <p className="text-[11px] text-[var(--text-tertiary)] truncate mt-0.5">
                        {entry.prompt.length > 80 ? entry.prompt.slice(0, 80) + "..." : entry.prompt}
                      </p>
                    </div>

                    {/* Actions */}
                    <div className="flex items-center gap-2 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
                      <Toggle
                        checked={entry.enabled}
                        onChange={() => handleToggle(entry)}
                        size="sm"
                      />
                      <button
                        onClick={() => handleTrigger(entry.name)}
                        disabled={triggeringName === entry.name}
                        className={cn(
                          "p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--warning)] hover:bg-[var(--warning)]/10 transition-colors cursor-pointer",
                          triggeringName === entry.name && "animate-pulse"
                        )}
                        title={t("schedules.triggerNow")}
                      >
                        <Zap className="h-3.5 w-3.5" />
                      </button>
                      <button
                        onClick={() => selectForEdit(entry)}
                        className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--accent-light)] hover:bg-[var(--accent)]/10 transition-colors cursor-pointer"
                        title={t("schedules.edit")}
                      >
                        <Pencil className="h-3.5 w-3.5" />
                      </button>
                      <button
                        onClick={() => handleDelete(entry.name)}
                        className={cn(
                          "p-1.5 rounded-[var(--radius-sm)] transition-colors cursor-pointer",
                          confirming === entry.name
                            ? "text-[var(--error)] bg-[var(--error)]/10"
                            : "text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/10"
                        )}
                        title={confirming === entry.name ? t("schedules.confirm") : t("schedules.delete")}
                      >
                        {confirming === entry.name ? (
                          <span className="text-[10px] font-medium px-0.5">{t("schedules.confirm")}</span>
                        ) : (
                          <Trash2 className="h-3.5 w-3.5" />
                        )}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </SectionCard>
        </div>

        {/* ── Right: Create / Edit form ── */}
        <div className="lg:col-span-2">
          <SectionCard>
            <SectionHeader
              icon={editing ? <Pencil className="h-4 w-4" /> : <Plus className="h-4 w-4" />}
              title={editing ? t("schedules.editTask") : t("schedules.newTask")}
              right={
                editing && (
                  <button
                    onClick={clearForm}
                    className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                )
              }
            />

            <div className="space-y-5">
              {/* ── Basic Info ── */}
              <div className="space-y-3">
                <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                  {t("schedules.basicInfo")}
                </span>

                <div>
                  <label className="text-[11px] font-medium text-[var(--text-secondary)] mb-1 block">
                    {t("schedules.name")}
                  </label>
                  <input
                    type="text"
                    value={form.name}
                    onChange={(e) => updateField("name", e.target.value)}
                    disabled={editing}
                    placeholder={t("schedules.taskNamePlaceholder")}
                    className={cn(
                      "w-full px-3 py-2 text-[12px] rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]",
                      "focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/50 focus:border-[var(--accent)]/50 transition-all",
                      editing && "opacity-60 cursor-not-allowed"
                    )}
                  />
                  {editing && (
                    <p className="text-[10px] text-[var(--text-tertiary)] mt-1">
                      {t("schedules.nameReadonly")}
                    </p>
                  )}
                </div>

                <div>
                  <label className="text-[11px] font-medium text-[var(--text-secondary)] mb-1 block">
                    {t("schedules.prompt")}
                  </label>
                  <textarea
                    value={form.prompt}
                    onChange={(e) => updateField("prompt", e.target.value)}
                    placeholder={t("schedules.promptPlaceholder")}
                    rows={3}
                    className="w-full px-3 py-2 text-[12px] rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/50 focus:border-[var(--accent)]/50 transition-all resize-none"
                  />
                </div>
              </div>

              {/* ── Schedule ── */}
              <div className="space-y-3">
                <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                  {t("schedules.schedule")}
                </span>

                <div className="flex items-center gap-4">
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="radio"
                      name="scheduleType"
                      checked={form.scheduleType === "cron"}
                      onChange={() => updateField("scheduleType", "cron")}
                      className="accent-[var(--accent)]"
                    />
                    <span className="text-[12px] text-[var(--text-secondary)]">Cron</span>
                  </label>
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="radio"
                      name="scheduleType"
                      checked={form.scheduleType === "interval"}
                      onChange={() => updateField("scheduleType", "interval")}
                      className="accent-[var(--accent)]"
                    />
                    <span className="text-[12px] text-[var(--text-secondary)]">Interval</span>
                  </label>
                </div>

                {form.scheduleType === "cron" ? (
                  <div>
                    <label className="text-[11px] font-medium text-[var(--text-secondary)] mb-1 block">
                      {t("schedules.cronExpression")}
                    </label>
                    <input
                      type="text"
                      value={form.cron}
                      onChange={(e) => updateField("cron", e.target.value)}
                      placeholder="0 */6 * * *"
                      className="w-full px-3 py-2 text-[12px] font-mono rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/50 focus:border-[var(--accent)]/50 transition-all"
                    />
                    <p className="text-[10px] text-[var(--text-tertiary)] mt-1">
                      {t("schedules.cronHint")}
                    </p>
                  </div>
                ) : (
                  <div>
                    <label className="text-[11px] font-medium text-[var(--text-secondary)] mb-1 block">
                      {t("schedules.intervalSeconds")}
                    </label>
                    <input
                      type="number"
                      value={form.interval_secs}
                      onChange={(e) => updateField("interval_secs", Math.max(1, parseInt(e.target.value) || 1))}
                      min={1}
                      className="w-full px-3 py-2 text-[12px] font-mono rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--text-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/50 focus:border-[var(--accent)]/50 transition-all"
                    />
                    <p className="text-[10px] text-[var(--text-tertiary)] mt-1">
                      {t("schedules.intervalHint")}
                    </p>
                  </div>
                )}
              </div>

              {/* ── Delivery ── */}
              <div className="space-y-3">
                <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                  {t("schedules.delivery")}
                </span>

                <div>
                  <label className="text-[11px] font-medium text-[var(--text-secondary)] mb-1 block">
                    {t("schedules.description")}
                  </label>
                  <input
                    type="text"
                    value={form.description}
                    onChange={(e) => updateField("description", e.target.value)}
                    placeholder={t("schedules.descriptionPlaceholder")}
                    className="w-full px-3 py-2 text-[12px] rounded-[var(--radius-md)] border border-[var(--border-subtle)] bg-[var(--bg-surface)] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/50 focus:border-[var(--accent)]/50 transition-all"
                  />
                </div>

                <div className="flex items-center justify-between">
                  <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                    {t("schedules.enabledLabel")}
                  </label>
                  <Toggle
                    checked={form.enabled}
                    onChange={(v) => updateField("enabled", v)}
                    size="sm"
                  />
                </div>
              </div>

              {/* ── Actions ── */}
              <div className="flex items-center gap-2 pt-2 border-t border-[var(--border-subtle)]">
                <button
                  onClick={handleSave}
                  disabled={saving}
                  className={cn(
                    "flex items-center gap-1.5 px-4 py-2 text-[12px] font-medium rounded-[var(--radius-md)] transition-all cursor-pointer",
                    "bg-[var(--accent)] text-white hover:brightness-110",
                    saving && "opacity-60 cursor-not-allowed"
                  )}
                >
                  <Save className="h-3.5 w-3.5" />
                  {saving
                    ? t("schedules.saving")
                    : editing
                      ? t("schedules.update")
                      : t("schedules.create")
                  }
                </button>
                <button
                  onClick={clearForm}
                  className="flex items-center gap-1.5 px-4 py-2 text-[12px] font-medium rounded-[var(--radius-md)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                >
                  <X className="h-3.5 w-3.5" />
                  {t("schedules.cancel")}
                </button>
              </div>
            </div>
          </SectionCard>
        </div>
      </div>

      {/* ── Run History ── */}
      {selectedName && (
        <SectionCard>
          <SectionHeader
            icon={<Play className="h-4 w-4" />}
            title={t("schedules.runHistory")}
            right={
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {selectedName}
              </span>
            }
          />

          {runsLoading ? (
            <LoadingSkeleton className="h-32" />
          ) : runs.length === 0 ? (
            <EmptyState
              icon={<Clock className="h-8 w-8 opacity-40" />}
              message={t("schedules.noRuns")}
            />
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-[12px]">
                <thead>
                  <tr className="border-b border-[var(--border-subtle)]">
                    <th className="text-left py-2 px-3 text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                      {t("schedules.time")}
                    </th>
                    <th className="text-left py-2 px-3 text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                      {t("schedules.statusLabel")}
                    </th>
                    <th className="text-left py-2 px-3 text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                      {t("schedules.duration")}
                    </th>
                    <th className="text-left py-2 px-3 text-[10px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                      {t("schedules.result")}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {runs.map((run) => {
                    const duration = run.started_at && run.finished_at
                      ? `${((new Date(run.finished_at).getTime() - new Date(run.started_at).getTime()) / 1000).toFixed(1)}s`
                      : "\u2014";
                    const statusColor =
                      run.status === "success"
                        ? "var(--success)"
                        : run.status === "error"
                          ? "var(--error)"
                          : "var(--accent)";
                    return (
                      <tr
                        key={run.id}
                        className="border-b border-[var(--border-subtle)] last:border-b-0 hover:bg-[var(--bg-hover)] transition-colors"
                      >
                        <td className="py-2 px-3 text-[var(--text-secondary)] font-mono text-[11px]">
                          {new Date(run.started_at).toLocaleString()}
                        </td>
                        <td className="py-2 px-3">
                          <span
                            className="inline-flex items-center px-1.5 py-0.5 rounded-[var(--radius-sm)] text-[10px] font-medium"
                            style={{
                              backgroundColor: `color-mix(in srgb, ${statusColor} 15%, transparent)`,
                              color: statusColor,
                            }}
                          >
                            {run.status}
                          </span>
                        </td>
                        <td className="py-2 px-3 text-[var(--text-tertiary)] font-mono text-[11px]">
                          {duration}
                        </td>
                        <td className="py-2 px-3 text-[var(--text-secondary)] text-[11px] max-w-[300px] truncate">
                          {run.error || run.result || "\u2014"}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </SectionCard>
      )}

      <ToastContainer toasts={toasts} />
    </div>
  );
}
