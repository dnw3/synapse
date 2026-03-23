import {
  BarChart3,
  MessageSquare,
  ScrollText,
  CalendarClock,
  Settings2,
  Sparkles,
  Radio,
  TrendingUp,
  Bot,
  Terminal,
  Monitor,
  Network,
  Send,
  Workflow,
  Server,
  Brain,
  Puzzle,
  Box,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "../lib/cn";
import { StatusDot } from "./dashboard/shared";
import { Badge } from "./ui/badge";
import type { TabKey, TabDef, SidebarSection } from "../types/dashboard";

// Lazy-loaded page components
import OverviewPage from "./dashboard/OverviewPage";
import UsagePage from "./dashboard/UsagePage";
import SessionsPage from "./dashboard/SessionsPage";
import LogsPage from "./dashboard/LogsPage";
import SchedulesPage from "./dashboard/SchedulesPage";
import ConfigPage from "./dashboard/ConfigPage";
import SkillsPage from "./dashboard/SkillsPage";
import ChannelsPage from "./dashboard/ChannelsPage";
import AgentsPage from "./dashboard/AgentsPage";
import DebugPage from "./dashboard/DebugPage";
import InstancesPage from "./dashboard/InstancesPage";
import NodesPage from "./dashboard/NodesPage";
import PluginsPage from "./dashboard/PluginsPage";
import SandboxPanel from "./dashboard/SandboxPanel";

// Tabs that need full-height flex layout (editors, log viewers, scrollable tables)
// Pages that manage their own scroll (have internal scroll areas, editors, etc.)
// These get overflow-hidden on the content wrapper so they don't double-scroll.
const SELF_SCROLL_TABS = new Set<TabKey>(["config", "logs", "sessions", "communications", "automation", "infrastructure", "ai-agents"]);

// ---------------------------------------------------------------------------
// Tab & Sidebar Definitions (exported for App.tsx)
// ---------------------------------------------------------------------------

export type { TabKey };

// labelZh/labelEn kept for backward compat; i18nKey used when t() is available
export const TABS: (TabDef & { i18nKey: string })[] = [
  // Control
  { key: "overview", labelZh: "概览", labelEn: "Overview", i18nKey: "dashboard.overview", icon: <BarChart3 className="h-4 w-4" /> },
  { key: "channels", labelZh: "频道", labelEn: "Channels", i18nKey: "dashboard.channels", icon: <Radio className="h-4 w-4" /> },
  { key: "instances", labelZh: "实例", labelEn: "Instances", i18nKey: "dashboard.instances", icon: <Monitor className="h-4 w-4" /> },
  { key: "sessions", labelZh: "会话", labelEn: "Sessions", i18nKey: "dashboard.sessions", icon: <MessageSquare className="h-4 w-4" /> },
  { key: "usage", labelZh: "用量", labelEn: "Usage", i18nKey: "dashboard.usage", icon: <TrendingUp className="h-4 w-4" /> },
  { key: "schedules", labelZh: "定时任务", labelEn: "Schedules", i18nKey: "dashboard.schedules", icon: <CalendarClock className="h-4 w-4" /> },
  { key: "nodes", labelZh: "节点", labelEn: "Nodes", i18nKey: "dashboard.nodes", icon: <Network className="h-4 w-4" /> },
  { key: "plugins", labelZh: "插件", labelEn: "Plugins", i18nKey: "dashboard.plugins.title", icon: <Puzzle className="h-4 w-4" /> },
  { key: "sandbox", labelZh: "沙箱", labelEn: "Sandbox", i18nKey: "sandbox.title", icon: <Box className="h-4 w-4" /> },
  // Agent
  { key: "agents", labelZh: "代理", labelEn: "Agents", i18nKey: "dashboard.agents", icon: <Bot className="h-4 w-4" /> },
  { key: "skills", labelZh: "Skills", labelEn: "Skills", i18nKey: "dashboard.skills", icon: <Sparkles className="h-4 w-4" /> },
  // Settings
  { key: "config", labelZh: "通用配置", labelEn: "General", i18nKey: "dashboard.generalConfig", icon: <Settings2 className="h-4 w-4" /> },
  { key: "communications", labelZh: "通信", labelEn: "Communications", i18nKey: "dashboard.communications", icon: <Send className="h-4 w-4" /> },
  { key: "automation", labelZh: "自动化", labelEn: "Automation", i18nKey: "dashboard.automation", icon: <Workflow className="h-4 w-4" /> },
  { key: "infrastructure", labelZh: "基础设施", labelEn: "Infrastructure", i18nKey: "dashboard.infrastructure", icon: <Server className="h-4 w-4" /> },
  { key: "ai-agents", labelZh: "AI 与代理", labelEn: "AI & Agents", i18nKey: "dashboard.aiAgents", icon: <Brain className="h-4 w-4" /> },
  { key: "logs", labelZh: "日志", labelEn: "Logs", i18nKey: "dashboard.logs", icon: <ScrollText className="h-4 w-4" /> },
  { key: "debug", labelZh: "调试", labelEn: "Debug", i18nKey: "dashboard.debug", icon: <Terminal className="h-4 w-4" /> },
];

export const SIDEBAR_SECTIONS: (SidebarSection & { i18nKey: string })[] = [
  { labelZh: "控制", labelEn: "Control", i18nKey: "dashboard.control", keys: ["overview", "channels", "instances", "sessions", "usage", "schedules", "nodes", "plugins", "sandbox"] },
  { labelZh: "代理", labelEn: "Agent", i18nKey: "dashboard.agent", keys: ["agents", "skills"] },
  { labelZh: "设置", labelEn: "Settings", i18nKey: "dashboard.settings", keys: ["config", "communications", "automation", "infrastructure", "ai-agents", "logs", "debug"] },
];

// ---------------------------------------------------------------------------
// Dashboard — thin router
// ---------------------------------------------------------------------------

interface DashboardProps {
  connected: boolean;
  sessionCount: number;
  messageCount: number;
  activeTab: TabKey;
  onNavigateToChat?: (sessionKey: string) => void;
}

export default function Dashboard({ connected: wsConnected, sessionCount, messageCount, activeTab, onNavigateToChat }: DashboardProps) {
  const { t } = useTranslation();

  // Use HTTP health check for gateway status (WS is only connected during chat)
  const [gatewayOnline, setGatewayOnline] = useState(wsConnected);
  useEffect(() => {
    let cancelled = false;
    const check = () => {
      fetch("/api/health").then((r) => {
        if (!cancelled) setGatewayOnline(r.ok);
      }).catch(() => {
        if (!cancelled) setGatewayOnline(false);
      });
    };
    check();
    const timer = setInterval(check, 15_000);
    return () => { cancelled = true; clearInterval(timer); };
  }, []);
  const tab = TABS.find(t => t.key === activeTab);

  return (
    <div className="flex-1 min-w-0 h-full flex flex-col overflow-hidden">
      {/* Sticky Header */}
      <div className="flex-shrink-0 bg-[var(--bg-window)]/80 backdrop-blur-sm border-b border-[var(--border-subtle)] z-10">
        <div className="flex items-center justify-between px-6 h-14">
          <div className="flex flex-col gap-0.5">
            <h2
              className="text-[24px] font-bold text-[var(--text-primary)] leading-tight"
              style={{ fontFamily: "var(--font-heading)" }}
            >
              {tab ? t(tab.i18nKey) : ""}
            </h2>
            <span className="text-[14px] text-[var(--text-secondary)] hidden sm:inline">
              {new Date().toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" })}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant={gatewayOnline ? "success" : "error"}>
              <span className="inline-flex items-center gap-1.5">
                <StatusDot status={gatewayOnline ? "online" : "offline"} />
                {gatewayOnline ? t("dashboard.allOperational", "All Systems Operational") : t("dashboard.gatewayDisconnected", "Gateway Disconnected")}
              </span>
            </Badge>
          </div>
        </div>
      </div>

      {/* Content Area — always flex-1, pages fill available height */}
      <div className={cn(
        "flex-1 min-h-0 flex flex-col px-6 py-6",
        SELF_SCROLL_TABS.has(activeTab)
          ? ""
          : "overflow-y-auto [&>*:first-child]:flex-1 [&>*:first-child]:min-h-fit"
      )}>
        {activeTab === "overview" && (
          <OverviewPage connected={gatewayOnline} sessionCount={sessionCount} messageCount={messageCount} />
        )}
        {activeTab === "usage" && <UsagePage />}
        {activeTab === "sessions" && <SessionsPage onNavigateToChat={onNavigateToChat} />}
        {activeTab === "logs" && <LogsPage />}
        {activeTab === "schedules" && <SchedulesPage />}
        {activeTab === "config" && <ConfigPage />}
        {activeTab === "skills" && <SkillsPage />}
        {activeTab === "channels" && <ChannelsPage />}
        {activeTab === "agents" && <AgentsPage />}
        {activeTab === "instances" && <InstancesPage />}
        {activeTab === "nodes" && <NodesPage />}
        {activeTab === "plugins" && <PluginsPage />}
        {activeTab === "sandbox" && <SandboxPanel />}
        {activeTab === "communications" && <ConfigPage filterSection="communications" />}
        {activeTab === "automation" && <ConfigPage filterSection="automation" />}
        {activeTab === "infrastructure" && <ConfigPage filterSection="infrastructure" />}
        {activeTab === "ai-agents" && <ConfigPage filterSection="ai-agents" />}
        {activeTab === "debug" && <DebugPage />}

        {/* Footer — pinned at bottom of scroll for normal pages, hidden for self-scroll */}
        {!SELF_SCROLL_TABS.has(activeTab) && (
          <div className="flex items-center justify-between pt-3 pb-4 mt-auto border-t border-[var(--border-subtle)] flex-shrink-0">
            <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
              {t("dashboard.poweredBy")}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
              {t("dashboard.lastRefresh")}: {new Date().toLocaleTimeString()}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
