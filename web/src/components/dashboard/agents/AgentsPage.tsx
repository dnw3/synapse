import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Bot, Plus, Megaphone } from "lucide-react";
import { cn } from "../../../lib/cn";
import {
  useAgents, useCreateAgent, useUpdateAgent, useDeleteAgent,
  useToolsCatalog, useBindings, useBroadcasts,
} from "../../../hooks/queries/useAgentsQueries";
import { useSkills } from "../../../hooks/queries/useSkillsQueries";
import type { AgentEntry, BindingEntry, BroadcastGroupEntry, ToolCatalogGroup, SkillEntry } from "../../../types/dashboard";
import {
  SectionCard, SectionHeader, EmptyState, LoadingSkeleton,
} from "../shared";
import { useToast } from "../../ui/toast";
import AgentDetail from "./AgentDetail";
import AgentEditForm from "./AgentEditForm";

type DetailTab = "overview" | "tools" | "bindings" | "skills" | "cron" | "files";

const EMOJI_PLACEHOLDERS = ["🤖", "🧠", "⚡", "🔮", "🎯", "🛠️", "💎", "🌟"];

function agentEmoji(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
  return EMOJI_PLACEHOLDERS[Math.abs(hash) % EMOJI_PLACEHOLDERS.length];
}

export default function AgentsPage() {
  const { t } = useTranslation();
  const { toast } = useToast();

  const agentsQ = useAgents();
  const toolsQ = useToolsCatalog();
  const skillsQ = useSkills();
  const bindingsQ = useBindings();
  const broadcastsQ = useBroadcasts();
  const createMut = useCreateAgent();
  const updateMut = useUpdateAgent();
  const deleteMut = useDeleteAgent();

  const agents = agentsQ.data ?? [];
  const bindings: BindingEntry[] = bindingsQ.data ?? [];
  const broadcasts: BroadcastGroupEntry[] = broadcastsQ.data ?? [];
  const toolsCatalog: ToolCatalogGroup[] = toolsQ.data ?? [];
  const skills: SkillEntry[] = skillsQ.data ?? [];
  const loading = agentsQ.isPending;

  const [selected, setSelected] = useState<string | null>(null);
  const [detailTab, setDetailTab] = useState<DetailTab>("overview");
  const [editing, setEditing] = useState(false);

  // Edit form state
  const [editName, setEditName] = useState("");
  const [editModel, setEditModel] = useState("");
  const [editPrompt, setEditPrompt] = useState("");
  const [saving, setSaving] = useState(false);

  // Derive effective selection: fall back to first agent if nothing is explicitly selected
  const firstAgentName = agents[0]?.name ?? null;
  const effectiveSelected = selected ?? firstAgentName;

  const selectedAgent = agents.find((a) => a.name === effectiveSelected) ?? null;

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
    try {
      if (existing) {
        await updateMut.mutateAsync({ name: editName.trim(), agent: payload });
      } else {
        await createMut.mutateAsync(payload);
      }
      toast({ variant: "success", title: t("agents.saved") });
      setEditing(false);
      setSelected(editName.trim());
    } catch {
      toast({ variant: "error", title: t("agents.saveFailed") });
    }
    setSaving(false);
  };

  const handleDelete = async (name: string) => {
    try {
      await deleteMut.mutateAsync(name);
      if (selected === name) setSelected(null);
    } catch {
      // toast handled by mutation
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
            className="w-full flex items-center justify-center gap-2 px-3 py-2.5 rounded-[var(--radius-lg)] border border-dashed border-[var(--separator)] text-[12px] text-[var(--text-tertiary)] hover:text-[var(--accent)] hover:border-[var(--accent)] transition-colors cursor-pointer"
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
                  effectiveSelected === agent.name
                    ? "bg-[var(--bg-elevated)] border-[var(--accent)]/30 shadow-sm"
                    : "bg-[var(--bg-elevated)]/50 border-[var(--border-subtle)] hover:border-[var(--separator)]"
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
            <AgentEditForm
              isCreate={!selected}
              editName={editName}
              editModel={editModel}
              editPrompt={editPrompt}
              saving={saving}
              onChangeName={setEditName}
              onChangeModel={setEditModel}
              onChangePrompt={setEditPrompt}
              onSave={handleSave}
              onCancel={() => setEditing(false)}
            />
          ) : selectedAgent ? (
            <AgentDetail
              agent={selectedAgent}
              agentEmoji={agentEmoji}
              detailTab={detailTab}
              setDetailTab={setDetailTab}
              onEdit={startEdit}
              onDelete={handleDelete}
              toolsCatalog={toolsCatalog}
              bindings={bindings}
              skills={skills}
            />
          ) : (
            <EmptyState
              icon={<Bot className="h-10 w-10 opacity-40" />}
              message={t("agents.selectAgent")}
            />
          )}
        </SectionCard>
      </div>

      {/* Broadcast Groups */}
      {broadcasts.length > 0 && (
        <SectionCard>
          <SectionHeader
            icon={<Megaphone className="h-4 w-4" />}
            title={t("agents.broadcasts")}
            right={
              <span className="px-1.5 py-0.5 rounded-full bg-[var(--accent)]/15 text-[var(--accent)] text-[10px] font-mono tabular-nums border border-[var(--accent)]/25">
                {broadcasts.length}
              </span>
            }
          />
          <div className="space-y-2">
            {broadcasts.map((bg) => (
              <div
                key={bg.name}
                className="px-3 py-2.5 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50 border border-[var(--border-subtle)] hover:border-[var(--separator)] transition-all"
              >
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-[13px] font-semibold text-[var(--text-primary)]">{bg.name}</span>
                  <span className={cn(
                    "px-1.5 py-0.5 rounded text-[9px] font-medium border",
                    bg.strategy === "parallel" ? "bg-[var(--success)]/10 text-[var(--success)] border-[var(--success)]/20"
                    : bg.strategy === "aggregated" ? "bg-[var(--warning)]/10 text-[var(--warning)] border-[var(--warning)]/20"
                    : "bg-[var(--info)]/10 text-[var(--info)] border-[var(--info)]/20"
                  )}>
                    {bg.strategy === "parallel" ? t("agents.broadcastParallel")
                    : bg.strategy === "aggregated" ? t("agents.broadcastAggregated")
                    : t("agents.broadcastSequential")}
                  </span>
                  {bg.channel && (
                    <span className="px-1.5 py-0.5 rounded bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium border border-[var(--accent)]/20">
                      {bg.channel}
                    </span>
                  )}
                  {bg.peer_id && (
                    <span className="text-[10px] text-[var(--text-tertiary)] font-mono">
                      {t("agents.broadcastPeer")}: {bg.peer_id}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1.5 mt-1.5">
                  <span className="text-[10px] text-[var(--text-tertiary)]">{t("agents.broadcastAgents")}:</span>
                  {bg.agents.map((a) => (
                    <span key={a} className="px-1.5 py-0.5 rounded bg-[var(--bg-elevated)] text-[11px] font-mono text-[var(--text-secondary)] border border-[var(--border-subtle)]">
                      {a}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </SectionCard>
      )}

    </div>
  );
}
