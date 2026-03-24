import { lazy, Suspense } from "react";
import { useParams, Navigate } from "react-router-dom";
import { cn } from "../lib/cn";
import ErrorFallback from "./ErrorFallback";
import PageSkeleton from "./PageSkeleton";
import { ErrorBoundary } from "./ErrorBoundary";

// Lazy page components
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const pages: Record<string, React.LazyExoticComponent<React.ComponentType<any>>> = {
  overview: lazy(() => import("./dashboard/OverviewPage")),
  usage: lazy(() => import("./dashboard/UsagePage")),
  sessions: lazy(() => import("./dashboard/sessions")),
  logs: lazy(() => import("./dashboard/logs")),
  schedules: lazy(() => import("./dashboard/SchedulesPage")),
  config: lazy(() => import("./dashboard/config")),
  skills: lazy(() => import("./dashboard/skills")),
  channels: lazy(() => import("./dashboard/channels")),
  agents: lazy(() => import("./dashboard/agents")),
  debug: lazy(() => import("./dashboard/DebugPage")),
  instances: lazy(() => import("./dashboard/InstancesPage")),
  nodes: lazy(() => import("./dashboard/nodes")),
  plugins: lazy(() => import("./dashboard/PluginsPage")),
  "mcp-servers": lazy(() => import("./dashboard/McpServersPage")),
  sandbox: lazy(() => import("./dashboard/SandboxPanel")),
};

const CONFIG_SUB_TABS = new Set(["communications", "automation", "infrastructure", "ai-agents"]);
const SELF_SCROLL_TABS = new Set<string>(["config", "logs", "sessions", "communications", "automation", "infrastructure", "ai-agents"]);

export default function DashboardRouter() {
  const { tab = "overview" } = useParams<{ tab: string }>();

  const isValidTab = tab in pages || CONFIG_SUB_TABS.has(tab);
  if (!isValidTab) {
    return <Navigate to="/dashboard/overview" replace />;
  }

  const isConfigSubTab = CONFIG_SUB_TABS.has(tab);
  const PageComponent = isConfigSubTab ? pages.config : pages[tab];
  return (
    <div className="flex-1 min-w-0 h-full flex flex-col overflow-hidden">
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
      </div>
    </div>
  );
}
