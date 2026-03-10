import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Bot, Plus, Trash2, Save, Sparkles, Wrench, Radio, Settings2,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { AgentEntry } from "../../types/dashboard";
import {
  SectionCard, SectionHeader, EmptyState, LoadingSkeleton, useToast, ToastContainer,
} from "./shared";

type DetailTab = "overview" | "tools" | "skills" | "channels";

const DETAIL_TABS: { key: DetailTab; i18nKey: string; icon: React.ReactNode }[] = [
  { key: "overview", i18nKey: "agents.tabOverview", icon: <Settings2 className="h-3.5 w-3.5" /> },
  { key: "tools", i18nKey: "agents.tabTools", icon: <Wrench className="h-3.5 w-3.5" /> },
  { key: "skills", i18nKey: "agents.tabSkills", icon: <Sparkles className="h-3.5 w-3.5" /> },
  { key: "channels", i18nKey: "agents.tabChannels", icon: <Radio className="h-3.5 w-3.5" /> },
];

const EMOJI_PLACEHOLDERS = ["🤖", "🧠", "⚡", "🔮", "🎯", "🛠️", "💎", "🌟"];

function agentEmoji(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
  return EMOJI_PLACEHOLDERS[Math.abs(hash) % EMOJI_PLACEHOLDERS.length];
}

export default function AgentsPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();

  const [agents, setAgents] = useState<AgentEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [detailTab, setDetailTab] = useState<DetailTab>("overview");
  const [editing, setEditing] = useState(false);

  // Edit form state
  const [editName, setEditName] = useState("");
  const [editModel, setEditModel] = useState("");
  const [editPrompt, setEditPrompt] = useState("");
  const [saving, setSaving] = useState(false);

  const loadAgents = useCallback(async () => {
    const data = await api.fetchAgents();
    if (data) {
      setAgents(data);
      if (!selected && data.length > 0) {
        setSelected(data[0].name);
      }
    }
    setLoading(false);
  }, [api, selected]);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  const selectedAgent = agents.find((a) => a.name === selected) ?? null;

  const startEdit = (agent: AgentEntry) => {
    setEditName(agent.name);
    setEditModel(agent.model);
    setEditPrompt(agent.system_prompt ?? "");
    setEditing(true);
  };

  const startCreate = () => {
    setEditName("");
    setEditModel("");
    setEditPrompt("");
    setEditing(true);
    setSelected(null);
  };

  const handleSave = async () => {
    if (!editName.trim()) return;
    setSaving(true);
    const payload: Partial<AgentEntry> = {
      name: editName.trim(),
      model: editModel.trim(),
      system_prompt: editPrompt.trim() || undefined,
    };

    const existing = agents.find((a) => a.name === editName.trim());
    let result: AgentEntry | null;
    if (existing) {
      result = await api.updateAgent(editName.trim(), payload);
    } else {
      result = await api.createAgent(payload);
    }

    if (result) {
      addToast(t("agents.saved"), "success");
      setEditing(false);
      setSelected(editName.trim());
      await loadAgents();
    } else {
      addToast(t("agents.saveFailed"), "error");
    }
    setSaving(false);
  };

  const handleDelete = async (name: string) => {
    const ok = await api.deleteAgent(name);
    if (ok) {
      addToast(t("agents.deleted"), "success");
      if (selected === name) setSelected(null);
      await loadAgents();
    } else {
      addToast(t("agents.deleteFailed"), "error");
    }
  };

  if (loading) {
    return (
      <div className="animate-fade-in space-y-6">
        <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-4">
          <div className="space-y-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <LoadingSkeleton key={i} className="h-[72px]" />
            ))}
          </div>
          <LoadingSkeleton className="h-[400px]" />
        </div>
      </div>
    );
  }

  return (
    <div className="animate-fade-in space-y-6">
      <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-4">
        {/* Left: Agent List */}
        <div className="space-y-3">
          <button
            onClick={startCreate}
            className="w-full flex items-center justify-center gap-2 px-3 py-2.5 rounded-[var(--radius-lg)] border border-dashed border-[var(--border-default)] text-[12px] text-[var(--text-tertiary)] hover:text-[var(--accent)] hover:border-[var(--accent)] transition-colors cursor-pointer"
          >
            <Plus className="h-3.5 w-3.5" />
            {t("agents.createAgent")}
          </button>

          {agents.length === 0 ? (
            <EmptyState
              icon={<Bot className="h-8 w-8 opacity-40" />}
              message={t("agents.noAgents")}
            />
          ) : (
            agents.map((agent) => (
              <button
                key={agent.name}
                onClick={() => { setSelected(agent.name); setEditing(false); setDetailTab("overview"); }}
                className={cn(
                  "w-full flex items-center gap-3 p-3 rounded-[var(--radius-lg)] border transition-all cursor-pointer text-left",
                  selected === agent.name
                    ? "bg-[var(--bg-elevated)] border-[var(--accent)]/30 shadow-sm"
                    : "bg-[var(--bg-elevated)]/50 border-[var(--border-subtle)] hover:border-[var(--border-default)]"
                )}
              >
                <span className="text-xl flex-shrink-0">{agentEmoji(agent.name)}</span>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[13px] font-medium text-[var(--text-primary)] truncate">
                      {agent.name}
                    </span>
                    {agent.is_default && (
                      <span className="px-1.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-[var(--accent)]/10 text-[var(--accent)]">
                        DEFAULT
                      </span>
                    )}
                  </div>
                  <span className="text-[11px] text-[var(--text-tertiary)] font-mono truncate block">
                    {agent.model || "—"}
                  </span>
                </div>
              </button>
            ))
          )}
        </div>

        {/* Right: Agent Detail */}
        <SectionCard>
          {editing ? (
            /* Edit / Create Form */
            <div className="space-y-4">
              <SectionHeader
                icon={<Bot className="h-4 w-4" />}
                title={selected ? t("agents.editAgent") : t("agents.createAgent")}
              />
              <div className="space-y-3">
                <div>
                  <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
                    {t("agents.name")}
                  </label>
                  <input
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    disabled={!!selected}
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors disabled:opacity-50"
                    placeholder="my-agent"
                  />
                </div>
                <div>
                  <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
                    {t("agents.model")}
                  </label>
                  <input
                    value={editModel}
                    onChange={(e) => setEditModel(e.target.value)}
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] font-mono outline-none focus:border-[var(--accent)] transition-colors"
                    placeholder="gpt-4o"
                  />
                </div>
                <div>
                  <label className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)] block mb-1.5">
                    {t("agents.systemPrompt")}
                  </label>
                  <textarea
                    value={editPrompt}
                    onChange={(e) => setEditPrompt(e.target.value)}
                    rows={5}
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors resize-none"
                    placeholder={t("agents.promptPlaceholder")}
                  />
                </div>
              </div>
              <div className="flex items-center gap-2 pt-2">
                <button
                  onClick={handleSave}
                  disabled={saving || !editName.trim()}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[12px] font-medium hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-40"
                >
                  <Save className="h-3.5 w-3.5" />
                  {saving ? t("agents.saving") : t("agents.save")}
                </button>
                <button
                  onClick={() => setEditing(false)}
                  className="px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                >
                  {t("agents.cancel")}
                </button>
              </div>
            </div>
          ) : selectedAgent ? (
            /* Agent Detail View */
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <span className="text-2xl">{agentEmoji(selectedAgent.name)}</span>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-[15px] font-semibold text-[var(--text-primary)]">
                        {selectedAgent.name}
                      </span>
                      {selectedAgent.is_default && (
                        <span className="px-1.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-[var(--accent)]/10 text-[var(--accent)]">
                          DEFAULT
                        </span>
                      )}
                    </div>
                    <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
                      {selectedAgent.model}
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => startEdit(selectedAgent)}
                    className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    <Settings2 className="h-3.5 w-3.5" />
                  </button>
                  {!selectedAgent.is_default && (
                    <button
                      onClick={() => handleDelete(selectedAgent.name)}
                      className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/5 transition-colors cursor-pointer"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
              </div>

              {/* Sub-tabs */}
              <div className="flex items-center gap-0.5 border-b border-[var(--border-subtle)]">
                {DETAIL_TABS.map((tab) => (
                  <button
                    key={tab.key}
                    onClick={() => setDetailTab(tab.key)}
                    className={cn(
                      "flex items-center gap-1.5 px-3 py-2 text-[11px] font-medium border-b-2 -mb-[1px] transition-colors cursor-pointer",
                      detailTab === tab.key
                        ? "text-[var(--accent)] border-[var(--accent)]"
                        : "text-[var(--text-tertiary)] border-transparent hover:text-[var(--text-secondary)]"
                    )}
                  >
                    {tab.icon}
                    {t(tab.i18nKey)}
                  </button>
                ))}
              </div>

              {/* Tab Content */}
              <div className="pt-2">
                {detailTab === "overview" && (
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <InfoCell
                      label={t("agents.model")}
                      value={selectedAgent.model || "—"}
                      mono
                    />
                    <InfoCell
                      label={t("agents.default")}
                      value={selectedAgent.is_default ? t("agents.yes") : t("agents.no")}
                    />
                    <div className="sm:col-span-2">
                      <InfoCell
                        label={t("agents.systemPrompt")}
                        value={selectedAgent.system_prompt
                          ? (selectedAgent.system_prompt.length > 200
                            ? selectedAgent.system_prompt.slice(0, 200) + "..."
                            : selectedAgent.system_prompt)
                          : t("agents.notSet")
                        }
                      />
                    </div>
                  </div>
                )}

                {detailTab === "tools" && (
                  <div className="flex flex-col items-center justify-center py-10 gap-3 text-[var(--text-tertiary)]">
                    <Wrench className="h-8 w-8 opacity-40" />
                    <span className="text-[13px]">{t("agents.comingSoon")}</span>
                    <span className="text-[11px] max-w-[280px] text-center">
                      {t("agents.toolBindingWip")}
                    </span>
                  </div>
                )}

                {detailTab === "skills" && (
                  <div>
                    {(selectedAgent.skills ?? []).length === 0 ? (
                      <EmptyState
                        icon={<Sparkles className="h-8 w-8 opacity-40" />}
                        message={t("agents.noSkills")}
                      />
                    ) : (
                      <div className="space-y-1.5">
                        {(selectedAgent.skills ?? []).map((skill) => (
                          <div
                            key={skill}
                            className="flex items-center gap-2.5 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50 hover:bg-[var(--bg-surface)] transition-colors"
                          >
                            <Sparkles className="h-3.5 w-3.5 text-[var(--accent)]" />
                            <span className="text-[12px] text-[var(--text-secondary)] font-mono">
                              {skill}
                            </span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}

                {detailTab === "channels" && (
                  <div>
                    {(selectedAgent.channels ?? []).length === 0 ? (
                      <EmptyState
                        icon={<Radio className="h-8 w-8 opacity-40" />}
                        message={t("agents.noChannels")}
                      />
                    ) : (
                      <div className="space-y-1.5">
                        {(selectedAgent.channels ?? []).map((ch) => (
                          <div
                            key={ch}
                            className="flex items-center gap-2.5 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50 hover:bg-[var(--bg-surface)] transition-colors"
                          >
                            <Radio className="h-3.5 w-3.5 text-[var(--success)]" />
                            <span className="text-[12px] text-[var(--text-secondary)]">
                              {ch}
                            </span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
          ) : (
            <EmptyState
              icon={<Bot className="h-10 w-10 opacity-40" />}
              message={t("agents.selectAgent")}
            />
          )}
        </SectionCard>
      </div>

      <ToastContainer toasts={toasts} />
    </div>
  );
}

function InfoCell({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="p-3 rounded-[var(--radius-md)] bg-[var(--bg-surface)]/50">
      <div className="text-[10px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] mb-1">
        {label}
      </div>
      <div className={cn(
        "text-[13px] text-[var(--text-primary)] break-words",
        mono && "font-mono"
      )}>
        {value}
      </div>
    </div>
  );
}
