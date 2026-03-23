import { lazy, Suspense } from "react";
import { useParams, Navigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { cn } from "../lib/cn";
import { StatusDot } from "./dashboard/shared";
import { Badge } from "./ui/badge";
import { TABS } from "./Dashboard";
import ErrorFallback from "./ErrorFallback";
import PageSkeleton from "./PageSkeleton";
import { ErrorBoundary } from "./ErrorBoundary";

// Lazy page components
const pages: Record<string, React.LazyExoticComponent<React.ComponentType<any>>> = {
  overview: lazy(() => import("./dashboard/OverviewPage")),
  usage: lazy(() => import("./dashboard/UsagePage")),
  sessions: lazy(() => import("./dashboard/SessionsPage")),
  logs: lazy(() => import("./dashboard/logs")),
  schedules: lazy(() => import("./dashboard/SchedulesPage")),
  config: lazy(() => import("./dashboard/config")),
  skills: lazy(() => import("./dashboard/skills")),
  channels: lazy(() => import("./dashboard/ChannelsPage")),
  agents: lazy(() => import("./dashboard/AgentsPage")),
  debug: lazy(() => import("./dashboard/DebugPage")),
  instances: lazy(() => import("./dashboard/InstancesPage")),
  nodes: lazy(() => import("./dashboard/NodesPage")),
  plugins: lazy(() => import("./dashboard/PluginsPage")),
  "mcp-servers": lazy(() => import("./dashboard/McpServersPage")),
  sandbox: lazy(() => import("./dashboard/SandboxPanel")),
};

const CONFIG_SUB_TABS = new Set(["communications", "automation", "infrastructure", "ai-agents"]);
const SELF_SCROLL_TABS = new Set<string>(["config", "logs", "sessions", "communications", "automation", "infrastructure", "ai-agents"]);

export default function DashboardRouter() {
  const { tab = "overview" } = useParams<{ tab: string }>();
  const { t } = useTranslation();

  const isValidTab = tab in pages || CONFIG_SUB_TABS.has(tab);
  if (!isValidTab) {
    return <Navigate to="/dashboard/overview" replace />;
  }

  const isConfigSubTab = CONFIG_SUB_TABS.has(tab);
  const PageComponent = isConfigSubTab ? pages.config : pages[tab];
  const tabDef = TABS.find((tb) => tb.key === tab);

  return (
    <div className="flex-1 min-w-0 h-full flex flex-col overflow-hidden">
      {/* Sticky Header — replicated from old Dashboard.tsx */}
      <div className="flex-shrink-0 bg-[var(--bg-window)]/80 backdrop-blur-sm border-b border-[var(--border-subtle)] z-10">
        <div className="flex items-center justify-between px-6 h-14">
          <div className="flex flex-col gap-0.5">
            <h2
              className="text-[24px] font-bold text-[var(--text-primary)] leading-tight"
              style={{ fontFamily: "var(--font-heading)" }}
            >
              {tabDef ? t(tabDef.i18nKey) : ""}
            </h2>
            <span className="text-[14px] text-[var(--text-secondary)] hidden sm:inline">
              {new Date().toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" })}
            </span>
          </div>
          <DashboardStatusBadge />
        </div>
      </div>

      {/* Content Area */}
      <div className={cn(
        "flex-1 min-h-0 flex flex-col px-6 py-6",
        SELF_SCROLL_TABS.has(tab)
          ? ""
          : "overflow-y-auto [&>*:first-child]:flex-1 [&>*:first-child]:min-h-fit"
      )}>
        <ErrorBoundary FallbackComponent={ErrorFallback} resetKeys={[tab]}>
          <Suspense fallback={<PageSkeleton />}>
            {PageComponent && (
              isConfigSubTab
                ? <PageComponent filterSection={tab} />
                : <PageComponent />
            )}
          </Suspense>
        </ErrorBoundary>

        {/* Footer */}
        {!SELF_SCROLL_TABS.has(tab) && (
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

function DashboardStatusBadge() {
  const { t } = useTranslation();
  const health = useQuery({
    queryKey: ["health-check"],
    queryFn: async () => {
      const res = await fetch("/api/health");
      return res.ok;
    },
    staleTime: 15_000,
    refetchInterval: 15_000,
  });

  const online = health.data ?? false;

  return (
    <Badge variant={online ? "success" : "error"}>
      <span className="inline-flex items-center gap-1.5">
        <StatusDot status={online ? "online" : "offline"} />
        {online
          ? t("dashboard.allOperational", "All Systems Operational")
          : t("dashboard.gatewayDisconnected", "Gateway Disconnected")}
      </span>
    </Badge>
  );
}
