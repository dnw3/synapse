import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Bot, Plus, Trash2, Save, Sparkles, Wrench, Settings2,
  FileText, Terminal, Brain, MessageSquare, Puzzle, FolderOpen, Files,
  Link2, Megaphone, CalendarClock, Eye, Pencil,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { AgentEntry, BindingEntry, BroadcastGroupEntry, ToolCatalogGroup, SkillEntry } from "../../types/dashboard";
import {
  SectionCard, SectionHeader, EmptyState, LoadingSkeleton, useToast, ToastContainer,
} from "./shared";

type DetailTab = "overview" | "tools" | "bindings" | "skills" | "cron" | "files";

const DETAIL_TABS: { key: DetailTab; i18nKey: string; icon: React.ReactNode }[] = [
  { key: "overview", i18nKey: "agents.tabOverview", icon: <Settings2 className="h-3.5 w-3.5" /> },
  { key: "tools", i18nKey: "agents.tabTools", icon: <Wrench className="h-3.5 w-3.5" /> },
  { key: "bindings", i18nKey: "agents.tabBindings", icon: <Link2 className="h-3.5 w-3.5" /> },
  { key: "skills", i18nKey: "agents.tabSkills", icon: <Sparkles className="h-3.5 w-3.5" /> },
  { key: "cron", i18nKey: "dashboard.schedules", icon: <CalendarClock className="h-3.5 w-3.5" /> },
  { key: "files", i18nKey: "agentFiles.title", icon: <Files className="h-3.5 w-3.5" /> },
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
  const [bindings, setBindings] = useState<BindingEntry[]>([]);
  const [broadcasts, setBroadcasts] = useState<BroadcastGroupEntry[]>([]);
  const [toolsCatalog, setToolsCatalog] = useState<ToolCatalogGroup[]>([]);
  const [skills, setSkills] = useState<SkillEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [detailTab, setDetailTab] = useState<DetailTab>("overview");
  const [editing, setEditing] = useState(false);

  // Edit form state
  const [editName, setEditName] = useState("");
  const [editModel, setEditModel] = useState("");
  const [editPrompt, setEditPrompt] = useState("");
  const [saving, setSaving] = useState(false);

  // Agent files tab state — uses workspace API with ?agent= param
  const [workspaceFiles, setWorkspaceFiles] = useState<{ name: string; size?: number }[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState("");
  const [fileLoading, setFileLoading] = useState(false);
  const [fileSaving, setFileSaving] = useState(false);
  const [fileSaved, setFileSaved] = useState(false);
  const [filePreview, setFilePreview] = useState(false);

  const agentParam = selected && selected !== "default" ? selected : undefined;

  const loadWorkspaceFiles = useCallback(async () => {
    const data = await api.fetchWorkspaceFiles(agentParam);
    if (data) setWorkspaceFiles(data.map((f) => ({ name: f.filename, size: f.size_bytes ?? 0 })));
  }, [api, agentParam]);

  // Reload workspace files when selected agent changes
  useEffect(() => {
    if (selected) {
      loadWorkspaceFiles();
      setSelectedFile(null);
      setFileContent("");
    }
  }, [selected, loadWorkspaceFiles]);

  const loadFile = useCallback(async (filename: string) => {
    setSelectedFile(filename);
    setFileContent("");
    setFileSaved(false);
    setFileLoading(true);
    try {
      const data = await api.fetchWorkspaceFile(filename, agentParam);
      setFileContent(data?.content ?? "");
    } catch {
      setFileContent("");
    } finally {
      setFileLoading(false);
    }
  }, [api, agentParam]);

  const saveFile = useCallback(async () => {
    if (!selectedFile) return;
    setFileSaving(true);
    try {
      await api.saveWorkspaceFile(selectedFile, fileContent, agentParam);
      setFileSaved(true);
      setTimeout(() => setFileSaved(false), 2000);
    } catch { /* ignore */ }
    finally {
      setFileSaving(false);
    }
  }, [api, selectedFile, fileContent, agentParam]);

  const loadAgents = useCallback(async () => {
    const [data, tools, sk, bd, bc] = await Promise.all([
      api.fetchAgents(), api.fetchToolsCatalog(), api.fetchSkills(),
      api.fetchBindings(), api.fetchBroadcasts(),
    ]);
    if (data) {
      setAgents(data);
      if (!selected && data.length > 0) {
        setSelected(data[0].name);
      }
    }
    if (tools) setToolsCatalog(tools);
    if (sk) setSkills(sk);
    setBindings(bd ?? []);
    setBroadcasts(bc ?? []);
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
                  selected === agent.name
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
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors disabled:opacity-50"
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
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] font-mono outline-none focus:border-[var(--accent)] transition-colors"
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
                    className="w-full px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors resize-none"
                    placeholder={t("agents.promptPlaceholder")}
                  />
                </div>
              </div>
              <div className="flex items-center gap-2 pt-2">
                <button
                  onClick={handleSave}
                  disabled={saving || !editName.trim()}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[12px] font-medium hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)] transition-all cursor-pointer disabled:opacity-40"
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
                    <InfoCell
                      label={t("agents.dmScope")}
                      value={
                        selectedAgent.dm_scope === "perpeer" ? t("agents.dmScopePerPeer")
                        : selectedAgent.dm_scope === "perchannelpeer" ? t("agents.dmScopePerChannelPeer")
                        : selectedAgent.dm_scope === "peraccountchannelpeer" ? t("agents.dmScopePerAccountChannelPeer")
                        : selectedAgent.dm_scope === "main" ? t("agents.dmScopeMain")
                        : t("agents.dmScopePerChannelPeer")
                      }
                    />
                    <InfoCell
                      label={t("agents.workspacePath")}
                      value={selectedAgent.workspace || "—"}
                      mono
                    />
                    {(selectedAgent.tool_allow?.length ?? 0) > 0 && (
                      <InfoCell
                        label={t("agents.toolAllow")}
                        value={selectedAgent.tool_allow?.join(", ") ?? "—"}
                        mono
                      />
                    )}
                    {(selectedAgent.tool_deny?.length ?? 0) > 0 && (
                      <InfoCell
                        label={t("agents.toolDeny")}
                        value={selectedAgent.tool_deny?.join(", ") ?? "—"}
                        mono
                      />
                    )}
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
                  <div>
                    {toolsCatalog.length === 0 ? (
                      <EmptyState
                        icon={<Wrench className="h-8 w-8 opacity-40" />}
                        message={t("agents.noTools")}
                      />
                    ) : (
                      <div className="space-y-4">
                        {toolsCatalog.map((group) => (
                          <div key={group.id}>
                            <div className="flex items-center gap-2 mb-2">
                              <ToolGroupIcon id={group.id} />
                              <span className="text-[11px] font-medium uppercase tracking-[0.06em] text-[var(--text-tertiary)]">
                                {group.label}
                              </span>
                              <span className="text-[10px] text-[var(--text-tertiary)]">
                                {group.tools.length}
                              </span>
                            </div>
                            <div className="space-y-1">
                              {group.tools.map((tool) => (
                                <div
                                  key={tool.name}
                                  className="flex items-start gap-2.5 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50 hover:bg-[var(--bg-content)] transition-colors"
                                >
                                  <Wrench className="h-3.5 w-3.5 text-[var(--accent)] mt-0.5 flex-shrink-0" />
                                  <div className="min-w-0 flex-1">
                                    <span className="text-[12px] font-medium text-[var(--text-primary)] font-mono">
                                      {tool.name}
                                    </span>
                                    <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 leading-relaxed">
                                      {tool.description}
                                    </p>
                                  </div>
                                </div>
                              ))}
                            </div>
                          </div>
                        ))}
                        <div className="text-[10px] text-[var(--text-tertiary)] pt-2">
                          {t("agents.toolsTotal", { count: toolsCatalog.reduce((s, g) => s + g.tools.length, 0) })}
                        </div>
                      </div>
                    )}
                  </div>
                )}

                {detailTab === "skills" && (
                  <div>
                    {skills.length === 0 ? (
                      <EmptyState
                        icon={<Sparkles className="h-8 w-8 opacity-40" />}
                        message={t("agents.noSkills")}
                      />
                    ) : (
                      <div className="space-y-1">
                        {skills.map((skill) => (
                          <div
                            key={skill.name}
                            className="flex items-start gap-2.5 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50 hover:bg-[var(--bg-content)] transition-colors"
                          >
                            <Sparkles className="h-3.5 w-3.5 text-[var(--accent)] mt-0.5 flex-shrink-0" />
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-2">
                                <span className="text-[12px] font-medium text-[var(--text-primary)] font-mono">
                                  {skill.name}
                                </span>
                                {skill.user_invocable && (
                                  <span className="px-1.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-[var(--accent)]/10 text-[var(--accent)]">
                                    /{skill.name}
                                  </span>
                                )}
                                {skill.enabled === false && (
                                  <span className="px-1.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-[var(--error)]/10 text-[var(--error)]">
                                    {t("agents.disabled")}
                                  </span>
                                )}
                              </div>
                              {skill.description && (
                                <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 leading-relaxed">
                                  {skill.description}
                                </p>
                              )}
                              <span className="text-[10px] text-[var(--text-tertiary)]">
                                {skill.source}
                              </span>
                            </div>
                          </div>
                        ))}
                        <div className="text-[10px] text-[var(--text-tertiary)] pt-2">
                          {t("agents.skillsTotal", { count: skills.length })}
                        </div>
                      </div>
                    )}
                  </div>
                )}

                {detailTab === "bindings" && (() => {
                  const agentBindings = bindings.filter((b) => b.agent === selectedAgent.name);
                  return (
                    <div>
                      {agentBindings.length === 0 ? (
                        <EmptyState
                          icon={<Link2 className="h-8 w-8 opacity-40" />}
                          message={t("agents.noBindings")}
                        />
                      ) : (
                        <div className="space-y-1.5">
                          {agentBindings.map((b, i) => (
                            <div
                              key={i}
                              className="flex items-center gap-2 p-2.5 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50 hover:bg-[var(--bg-content)] transition-colors flex-wrap"
                            >
                              {b.channel && (
                                <span className="px-1.5 py-0.5 rounded bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium border border-[var(--accent)]/20">
                                  {b.channel}
                                </span>
                              )}
                              {b.account_id && (
                                <span className="text-[11px] text-[var(--text-tertiary)]">
                                  {t("agents.bindingAccount")}: <span className="font-mono">{b.account_id}</span>
                                </span>
                              )}
                              {b.peer && (
                                <span className="text-[11px] text-[var(--text-tertiary)]">
                                  {b.peer.kind}: <span className="font-mono">{b.peer.id}</span>
                                </span>
                              )}
                              {b.guild_id && (
                                <span className="text-[11px] text-[var(--text-tertiary)]">
                                  {t("agents.bindingGuild")}: <span className="font-mono">{b.guild_id}</span>
                                </span>
                              )}
                              {b.team_id && (
                                <span className="text-[11px] text-[var(--text-tertiary)]">
                                  {t("agents.bindingTeam")}: <span className="font-mono">{b.team_id}</span>
                                </span>
                              )}
                              {(b.roles?.length ?? 0) > 0 && (
                                <span className="text-[11px] text-[var(--text-tertiary)]">
                                  {t("agents.bindingRoles")}: {b.roles?.join(", ")}
                                </span>
                              )}
                              {b.comment && (
                                <span className="text-[11px] text-[var(--text-quaternary)] italic ml-auto">
                                  {b.comment}
                                </span>
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })()}

                {detailTab === "cron" && (
                  <div>
                    <EmptyState
                      icon={<CalendarClock className="h-8 w-8 opacity-40" />}
                      message={t("schedules.noTasks")}
                    />
                    <div className="text-center mt-2">
                      <span className="text-[11px] text-[var(--text-tertiary)]">
                        {t("dashboard.schedules")}
                      </span>
                    </div>
                  </div>
                )}

                {detailTab === "files" && (
                  <div className="space-y-3">
                    {/* File buttons — dynamic from workspace API */}
                    <div className="flex flex-wrap gap-2">
                      {workspaceFiles.map((f) => (
                        <button
                          key={f.name}
                          onClick={() => loadFile(f.name)}
                          className={cn(
                            "flex items-center gap-1.5 px-2.5 py-1.5 rounded-[var(--radius-md)] text-[12px] font-mono border transition-colors cursor-pointer",
                            selectedFile === f.name
                              ? "bg-[var(--accent)]/10 border-[var(--accent)]/40 text-[var(--accent)]"
                              : "bg-[var(--bg-content)]/50 border-[var(--border-subtle)] text-[var(--text-secondary)] hover:border-[var(--separator)]"
                          )}
                        >
                          <FileText className="h-3 w-3" />
                          {f.name}
                        </button>
                      ))}
                      {workspaceFiles.length === 0 && (
                        <span className="text-[11px] text-[var(--text-tertiary)]">
                          {t("agents.noChannels")}
                        </span>
                      )}
                    </div>

                    {/* Editor / Preview */}
                    {fileLoading ? (
                      <div className="text-[12px] text-[var(--text-tertiary)] py-4">
                        {t("agentFiles.loading")}
                      </div>
                    ) : selectedFile ? (
                      <div className="space-y-2">
                        {/* Toolbar: Edit/Preview toggle + Save */}
                        <div className="flex items-center justify-between">
                          <div className="flex items-center rounded-[var(--radius-md)] border border-[var(--border-subtle)] overflow-hidden">
                            <button
                              onClick={() => setFilePreview(false)}
                              className={cn(
                                "flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium transition-colors cursor-pointer",
                                !filePreview
                                  ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                                  : "text-[var(--text-secondary)] hover:bg-[var(--bg-content)]"
                              )}
                            >
                              <Pencil className="h-3 w-3" />
                              Edit
                            </button>
                            <button
                              onClick={() => setFilePreview(true)}
                              className={cn(
                                "flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium transition-colors cursor-pointer border-l border-[var(--border-subtle)]",
                                filePreview
                                  ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                                  : "text-[var(--text-secondary)] hover:bg-[var(--bg-content)]"
                              )}
                            >
                              <Eye className="h-3 w-3" />
                              Preview
                            </button>
                          </div>
                          <div className="flex items-center gap-2">
                            {fileSaved && (
                              <span className="text-[11px] text-[var(--success)]">
                                {t("agentFiles.saved")}
                              </span>
                            )}
                            <button
                              onClick={saveFile}
                              disabled={fileSaving || filePreview}
                              className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--accent)] text-white text-[11px] font-medium hover:brightness-110 active:scale-[0.97] transition-all cursor-pointer disabled:opacity-40"
                            >
                              <Save className="h-3 w-3" />
                              {fileSaving ? t("agentFiles.loading") : t("agentFiles.save")}
                            </button>
                          </div>
                        </div>

                        {/* Content area */}
                        {filePreview ? (
                          <div className="px-4 py-3 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] min-h-[300px] max-h-[500px] overflow-y-auto prose prose-sm max-w-none text-[13px]">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                              {fileContent || "*Empty file*"}
                            </ReactMarkdown>
                          </div>
                        ) : (
                          <textarea
                            value={fileContent}
                            onChange={(e) => { setFileContent(e.target.value); setFileSaved(false); }}
                            rows={16}
                            className="w-full px-4 py-3 rounded-[var(--radius-md)] bg-[var(--bg-content)] border border-[var(--border-subtle)] text-[13px] font-mono text-[var(--text-primary)] outline-none focus:border-[var(--accent)] transition-colors resize-y leading-relaxed"
                            spellCheck={false}
                          />
                        )}
                      </div>
                    ) : (
                      <div className="text-[12px] text-[var(--text-tertiary)] py-4">
                        {t("agentFiles.noAgent")}
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

      <ToastContainer toasts={toasts} />
    </div>
  );
}

function ToolGroupIcon({ id }: { id: string }) {
  const cls = "h-3.5 w-3.5 text-[var(--text-tertiary)]";
  switch (id) {
    case "filesystem": return <FolderOpen className={cls} />;
    case "core": return <FileText className={cls} />;
    case "agent": return <Bot className={cls} />;
    case "memory": return <Brain className={cls} />;
    case "session": return <MessageSquare className={cls} />;
    case "mcp": return <Puzzle className={cls} />;
    default: return <Terminal className={cls} />;
  }
}

function InfoCell({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="p-3 rounded-[var(--radius-md)] bg-[var(--bg-content)]/50">
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
