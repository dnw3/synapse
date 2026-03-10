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
  RefreshCw,
  FolderOpen,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "../lib/cn";
import { StatusDot } from "./dashboard/shared";
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
import WorkspacePage from "./dashboard/WorkspacePage";
import DebugPage from "./dashboard/DebugPage";

// Tabs that need full-height flex layout (editors, log viewers, scrollable tables)
// Pages that manage their own scroll (have internal scroll areas, editors, etc.)
// These get overflow-hidden on the content wrapper so they don't double-scroll.
const SELF_SCROLL_TABS = new Set<TabKey>(["workspace", "config", "logs", "sessions"]);

// ---------------------------------------------------------------------------
// Tab & Sidebar Definitions (exported for App.tsx)
// ---------------------------------------------------------------------------

export type { TabKey };

// labelZh/labelEn kept for backward compat; i18nKey used when t() is available
export const TABS: (TabDef & { i18nKey: string })[] = [
  { key: "overview", labelZh: "概览", labelEn: "Overview", i18nKey: "dashboard.overview", icon: <BarChart3 className="h-4 w-4" /> },
  { key: "usage", labelZh: "用量", labelEn: "Usage", i18nKey: "dashboard.usage", icon: <TrendingUp className="h-4 w-4" /> },
  { key: "sessions", labelZh: "会话", labelEn: "Sessions", i18nKey: "dashboard.sessions", icon: <MessageSquare className="h-4 w-4" /> },
  { key: "logs", labelZh: "日志", labelEn: "Logs", i18nKey: "dashboard.logs", icon: <ScrollText className="h-4 w-4" /> },
  { key: "schedules", labelZh: "定时任务", labelEn: "Schedules", i18nKey: "dashboard.schedules", icon: <CalendarClock className="h-4 w-4" /> },
  { key: "config", labelZh: "配置", labelEn: "Config", i18nKey: "dashboard.config", icon: <Settings2 className="h-4 w-4" /> },
  { key: "skills", labelZh: "Skills", labelEn: "Skills", i18nKey: "dashboard.skills", icon: <Sparkles className="h-4 w-4" /> },
  { key: "channels", labelZh: "频道", labelEn: "Channels", i18nKey: "dashboard.channels", icon: <Radio className="h-4 w-4" /> },
  { key: "agents", labelZh: "代理", labelEn: "Agents", i18nKey: "dashboard.agents", icon: <Bot className="h-4 w-4" /> },
  { key: "workspace", labelZh: "工作区", labelEn: "Workspace", i18nKey: "dashboard.workspace", icon: <FolderOpen className="h-4 w-4" /> },
  { key: "debug", labelZh: "调试", labelEn: "Debug", i18nKey: "dashboard.debug", icon: <Terminal className="h-4 w-4" /> },
];

export const SIDEBAR_SECTIONS: (SidebarSection & { i18nKey: string })[] = [
  { labelZh: "监控", labelEn: "Monitor", i18nKey: "dashboard.monitor", keys: ["overview", "usage"] },
  { labelZh: "控制", labelEn: "Control", i18nKey: "dashboard.control", keys: ["channels", "sessions", "schedules"] },
  { labelZh: "代理", labelEn: "Agent", i18nKey: "dashboard.agent", keys: ["agents", "skills", "workspace"] },
  { labelZh: "设置", labelEn: "Settings", i18nKey: "dashboard.settings", keys: ["config", "logs", "debug"] },
];

// ---------------------------------------------------------------------------
// Dashboard — thin router
// ---------------------------------------------------------------------------

interface DashboardProps {
  connected: boolean;
  conversationCount: number;
  messageCount: number;
  activeTab: TabKey;
}

export default function Dashboard({ connected: wsConnected, conversationCount, messageCount, activeTab }: DashboardProps) {
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
      <div className="flex-shrink-0 bg-[var(--bg-primary)]/80 backdrop-blur-sm border-b border-[var(--border-subtle)] z-10">
        <div className="flex items-center justify-between px-6 h-12">
          <div className="flex items-center gap-3">
            <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
              {tab ? t(tab.i18nKey) : ""}
            </h2>
            <span className="text-[11px] text-[var(--text-tertiary)] font-mono hidden sm:inline">
              {new Date().toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" })}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className={cn(
              "inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[11px] font-medium border",
              gatewayOnline
                ? "bg-[var(--success)]/10 text-[var(--success)] border-[var(--success)]/20"
                : "bg-[var(--error)]/10 text-[var(--error)] border-[var(--error)]/20"
            )}>
              <StatusDot status={gatewayOnline ? "online" : "offline"} />
              {gatewayOnline ? t("dashboard.allOperational", "All Systems Operational") : t("dashboard.gatewayDisconnected", "Gateway Disconnected")}
            </span>
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
          <OverviewPage connected={gatewayOnline} conversationCount={conversationCount} messageCount={messageCount} />
        )}
        {activeTab === "usage" && <UsagePage />}
        {activeTab === "sessions" && <SessionsPage />}
        {activeTab === "logs" && <LogsPage />}
        {activeTab === "schedules" && <SchedulesPage />}
        {activeTab === "config" && <ConfigPage />}
        {activeTab === "skills" && <SkillsPage />}
        {activeTab === "channels" && <ChannelsPage />}
        {activeTab === "agents" && <AgentsPage />}
        {activeTab === "workspace" && <WorkspacePage />}
        {activeTab === "debug" && <DebugPage />}

        {/* Footer — pinned at bottom of scroll for normal pages, hidden for self-scroll */}
        {!SELF_SCROLL_TABS.has(activeTab) && (
          <div className="flex items-center justify-between pt-2 pb-4 mt-auto border-t border-[var(--border-subtle)] flex-shrink-0">
            <span className="text-[10px] text-[var(--text-tertiary)]/50 font-mono">
              Synapse · Powered by Synaptic Framework
            </span>
            <span className="text-[10px] text-[var(--text-tertiary)]/50 font-mono">
              {t("dashboard.lastRefresh")}: {new Date().toLocaleTimeString()}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
