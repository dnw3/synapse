import { useTranslation } from "react-i18next";
import {
  Bot, Trash2, Settings2, Wrench, Sparkles, Link2,
  CalendarClock, FileText, Terminal, Brain, MessageSquare, Puzzle, FolderOpen,
} from "lucide-react";
import { cn } from "../../../lib/cn";
import type { AgentEntry, BindingEntry, ToolCatalogGroup, SkillEntry } from "../../../types/dashboard";
import { EmptyState } from "../shared";
import AgentFilesTab from "./AgentFilesTab";

type DetailTab = "overview" | "tools" | "bindings" | "skills" | "cron" | "files";

const DETAIL_TABS: { key: DetailTab; i18nKey: string; icon: React.ReactNode }[] = [
  { key: "overview", i18nKey: "agents.tabOverview", icon: <Settings2 className="h-3.5 w-3.5" /> },
  { key: "tools", i18nKey: "agents.tabTools", icon: <Wrench className="h-3.5 w-3.5" /> },
  { key: "bindings", i18nKey: "agents.tabBindings", icon: <Link2 className="h-3.5 w-3.5" /> },
  { key: "skills", i18nKey: "agents.tabSkills", icon: <Sparkles className="h-3.5 w-3.5" /> },
  { key: "cron", i18nKey: "dashboard.schedules", icon: <CalendarClock className="h-3.5 w-3.5" /> },
  { key: "files", i18nKey: "agentFiles.title", icon: <FileText className="h-3.5 w-3.5" /> },
];

export function ToolGroupIcon({ id }: { id: string }) {
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

export function InfoCell({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
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

interface AgentDetailProps {
  agent: AgentEntry;
  agentEmoji: (name: string) => string;
  detailTab: DetailTab;
  setDetailTab: (tab: DetailTab) => void;
  onEdit: (agent: AgentEntry) => void;
  onDelete: (name: string) => void;
  toolsCatalog: ToolCatalogGroup[];
  bindings: BindingEntry[];
  skills: SkillEntry[];
}

export default function AgentDetail({
  agent,
  agentEmoji,
  detailTab,
  setDetailTab,
  onEdit,
  onDelete,
  toolsCatalog,
  bindings,
  skills,
}: AgentDetailProps) {
  const { t } = useTranslation();

  const agentBindings = bindings.filter((b) => b.agent === agent.name);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="text-2xl">{agentEmoji(agent.name)}</span>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-[15px] font-semibold text-[var(--text-primary)]">
                {agent.name}
              </span>
              {agent.is_default && (
                <span className="px-1.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-[var(--accent)]/10 text-[var(--accent)]">
                  DEFAULT
                </span>
              )}
            </div>
            <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
              {agent.model}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => onEdit(agent)}
            className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          >
            <Settings2 className="h-3.5 w-3.5" />
          </button>
          {!agent.is_default && (
            <button
              onClick={() => onDelete(agent.name)}
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
              value={agent.model || "—"}
              mono
            />
            <InfoCell
              label={t("agents.default")}
              value={agent.is_default ? t("agents.yes") : t("agents.no")}
            />
            <InfoCell
              label={t("agents.dmScope")}
              value={
                agent.dm_scope === "perpeer" ? t("agents.dmScopePerPeer")
                : agent.dm_scope === "perchannelpeer" ? t("agents.dmScopePerChannelPeer")
                : agent.dm_scope === "peraccountchannelpeer" ? t("agents.dmScopePerAccountChannelPeer")
                : agent.dm_scope === "main" ? t("agents.dmScopeMain")
                : t("agents.dmScopePerChannelPeer")
              }
            />
            <InfoCell
              label={t("agents.workspacePath")}
              value={agent.workspace || "—"}
              mono
            />
            {(agent.tool_allow?.length ?? 0) > 0 && (
              <InfoCell
                label={t("agents.toolAllow")}
                value={agent.tool_allow?.join(", ") ?? "—"}
                mono
              />
            )}
            {(agent.tool_deny?.length ?? 0) > 0 && (
              <InfoCell
                label={t("agents.toolDeny")}
                value={agent.tool_deny?.join(", ") ?? "—"}
                mono
              />
            )}
            <div className="sm:col-span-2">
              <InfoCell
                label={t("agents.systemPrompt")}
                value={agent.system_prompt
                  ? (agent.system_prompt.length > 200
                    ? agent.system_prompt.slice(0, 200) + "..."
                    : agent.system_prompt)
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

        {detailTab === "bindings" && (
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
        )}

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
          <AgentFilesTab agentName={agent.name} />
        )}
      </div>
    </div>
  );
}
