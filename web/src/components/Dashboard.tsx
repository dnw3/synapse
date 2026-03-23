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
  Cable,
} from "lucide-react";
import type { TabKey, TabDef, SidebarSection } from "../types/dashboard";

// ---------------------------------------------------------------------------
// Tab & Sidebar Definitions (exported for App.tsx, DashboardRouter, Sidebar)
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
  { key: "mcp-servers", labelZh: "MCP 服务器", labelEn: "MCP Servers", i18nKey: "dashboard.mcpServers.title", icon: <Cable className="h-4 w-4" /> },
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
  { labelZh: "控制", labelEn: "Control", i18nKey: "dashboard.control", keys: ["overview", "channels", "instances", "sessions", "usage", "schedules", "nodes", "plugins", "mcp-servers", "sandbox"] },
  { labelZh: "代理", labelEn: "Agent", i18nKey: "dashboard.agent", keys: ["agents", "skills"] },
  { labelZh: "设置", labelEn: "Settings", i18nKey: "dashboard.settings", keys: ["config", "communications", "automation", "infrastructure", "ai-agents", "logs", "debug"] },
];

// ---------------------------------------------------------------------------
// Stub default export — kept so App.tsx does not break before Task 6 replaces it
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export default function Dashboard(_props: any) { return null; }
