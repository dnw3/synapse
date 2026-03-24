import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Cable, Plus, LayoutGrid, List, X, Loader2,
  Wrench, RefreshCw, Save, Trash2, Pencil, Zap,
} from "lucide-react";
import {
  useMcpServers, useCreateMcpServer, useUpdateMcpServer,
  useDeleteMcpServer, useTestMcpServer, usePersistMcpServer,
} from "../../hooks/queries/useMcpQueries";
import type { McpServerInfo, McpTestResult } from "../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  StatusDot,
  useInlineConfirm,
} from "./shared";
import { useToast } from "../ui/toast";
import McpServerModal from "./McpServerModal";
import ToolPlaygroundDrawer from "./ToolPlaygroundDrawer";
import { cn } from "../../lib/cn";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function mcpHealthToStatus(status: McpServerInfo["status"]): "online" | "offline" | "degraded" {
  if (status === "connected") return "online";
  if (status === "error") return "offline";
  if (status === "disconnected") return "offline";
  return "degraded"; // "unknown"
}

function transportBadgeStyle(transport: McpServerInfo["transport"]): React.CSSProperties {
  switch (transport) {
    case "stdio":
      return { background: "color-mix(in srgb, var(--accent) 12%, transparent)", color: "var(--accent)" };
    case "sse":
      return { background: "color-mix(in srgb, var(--warning) 12%, transparent)", color: "var(--warning)" };
    case "streamable-http":
      return { background: "color-mix(in srgb, var(--success) 12%, transparent)", color: "var(--success)" };
  }
}

const VIEW_KEY = "synapse:mcp-view";

// ---------------------------------------------------------------------------
// McpServersPage
// ---------------------------------------------------------------------------

export default function McpServersPage() {
  const { t } = useTranslation();
  const { toast } = useToast();
  const { confirming, requestConfirm, reset: resetConfirm } = useInlineConfirm();

  const serversQ = useMcpServers();
  const createMut = useCreateMcpServer();
  const updateMut = useUpdateMcpServer();
  const deleteMut = useDeleteMcpServer();
  const testMut = useTestMcpServer();
  const persistMut = usePersistMcpServer();

  const servers = serversQ.data ?? [];
  const loading = serversQ.isPending;

  // UI state
  const [viewMode, setViewMode] = useState<"grid" | "list">(() => {
    try { return (localStorage.getItem(VIEW_KEY) as "grid" | "list") || "grid"; } catch { return "grid"; }
  });
  const [selectedServer, setSelectedServer] = useState<McpServerInfo | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [editServer, setEditServer] = useState<McpServerInfo | null>(null);
  const [actionLoading, setActionLoading] = useState<Record<string, boolean>>({});
  const [playgroundTarget, setPlaygroundTarget] = useState<{ server: string; tool: string } | null>(null);

  // Persist view mode
  const changeView = (mode: "grid" | "list") => {
    setViewMode(mode);
    try { localStorage.setItem(VIEW_KEY, mode); } catch { /* noop */ }
  };

  // Action loading helper
  const withActionLoading = async (key: string, fn: () => Promise<void>) => {
    setActionLoading((prev) => ({ ...prev, [key]: true }));
    try { await fn(); } finally {
      setActionLoading((prev) => ({ ...prev, [key]: false }));
    }
  };

  // Test a server
  const handleTest = async (name: string) => {
    await withActionLoading(`test:${name}`, async () => {
      testMut.mutate(name, {
        onSuccess: (result) => {
          if (result?.success) {
            toast({ variant: "success", title: t("dashboard.mcpServers.testResult", { name, toolCount: String(result.toolCount), latency: String(result.latencyMs) }) });
          }
        },
      });
    });
  };

  // Reconnect
  const handleReconnect = async (server: McpServerInfo) => {
    await withActionLoading(`reconnect:${server.name}`, async () => {
      try {
        await updateMut.mutateAsync({ name: server.name, server: {} });
        toast({ variant: "success", title: t("dashboard.mcpServers.reconnected") });
      } catch (e) {
        toast({ variant: "error", title: t("dashboard.mcpServers.reconnectFailed") });
        console.error(e);
      }
    });
  };

  // Persist transient server
  const handlePersist = async (name: string) => {
    await withActionLoading(`persist:${name}`, async () => {
      persistMut.mutate(name);
    });
  };

  // Delete server
  const handleDelete = async (name: string) => {
    if (confirming !== `delete:${name}`) {
      requestConfirm(`delete:${name}`);
      return;
    }
    resetConfirm();
    await withActionLoading(`delete:${name}`, async () => {
      deleteMut.mutate(name);
    });
    if (selectedServer?.name === name) setSelectedServer(null);
  };

  // Modal save handler
  const handleModalSave = async (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">) => {
    if (editServer) {
      await updateMut.mutateAsync({ name: editServer.name, server });
    } else {
      await createMut.mutateAsync(server);
    }
    toast({ variant: "success", title: t("dashboard.mcpServers.saved") });
  };

  // Modal test handler
  const handleModalTest = async (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">): Promise<McpTestResult | null> => {
    try {
      if (editServer) {
        await updateMut.mutateAsync({ name: editServer.name, server });
      } else {
        await createMut.mutateAsync(server);
      }
      return await testMut.mutateAsync(server.name);
    } catch {
      return null;
    }
  };

  // Open edit modal
  const openEdit = (server: McpServerInfo) => {
    setEditServer(server);
    setShowModal(true);
  };

  // Open add modal
  const openAdd = () => {
    setEditServer(null);
    setShowModal(true);
  };

  // Escape key closes drawer
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (showModal) return;
        if (playgroundTarget) { setPlaygroundTarget(null); return; }
        if (selectedServer) setSelectedServer(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selectedServer, showModal, playgroundTarget]);

  // Keep selectedServer in sync with fetched data
  useEffect(() => {
    if (selectedServer) {
      const updated = servers.find((s) => s.name === selectedServer.name);
      if (updated) setSelectedServer(updated);
    }
  }, [servers]); // eslint-disable-line react-hooks/exhaustive-deps

  // Status summary
  const connectedCount = servers.filter((s) => s.status === "connected").length;

  return (
    <div className="space-y-4">
      <SectionCard>
        <SectionHeader
          icon={<Cable className="h-4 w-4" />}
          title={t("dashboard.mcpServers.title")}
          right={
            <div className="flex items-center gap-2">
              {/* Grid / List toggle */}
              <div className="flex items-center gap-0.5 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
                <button
                  onClick={() => changeView("grid")}
                  title={t("dashboard.mcpServers.viewGrid", "网格视图")}
                  className={cn(
                    "p-1.5 rounded-[var(--radius-sm)] transition-colors cursor-pointer",
                    viewMode === "grid"
                      ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                      : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
                  )}
                >
                  <LayoutGrid className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => changeView("list")}
                  title={t("dashboard.mcpServers.viewList", "列表视图")}
                  className={cn(
                    "p-1.5 rounded-[var(--radius-sm)] transition-colors cursor-pointer",
                    viewMode === "list"
                      ? "bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm"
                      : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
                  )}
                >
                  <List className="h-3.5 w-3.5" />
                </button>
              </div>

              {/* Status summary */}
              {!loading && servers.length > 0 && (
                <span className="text-xs text-[var(--text-tertiary)]">
                  {t("dashboard.mcpServers.statusSummary", {
                    connected: String(connectedCount),
                    total: String(servers.length),
                  })}
                </span>
              )}

              {/* Add button */}
              <button
                onClick={openAdd}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium bg-[var(--accent)] text-white hover:opacity-90 transition-opacity cursor-pointer"
              >
                <Plus className="h-3.5 w-3.5" />
                {t("dashboard.mcpServers.addServer")}
              </button>
            </div>
          }
        />

        {/* Loading */}
        {loading && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <LoadingSkeleton key={i} className="h-32" />
            ))}
          </div>
        )}

        {/* Empty */}
        {!loading && servers.length === 0 && (
          <EmptyState
            icon={<Cable className="h-8 w-8" />}
            message={t("dashboard.mcpServers.emptyTitle")}
            description={t("dashboard.mcpServers.emptyDescription")}
          />
        )}

        {/* Content */}
        {!loading && servers.length > 0 && (
          <div className="flex gap-4">
            {/* Main area */}
            <div className={cn(
              "flex-1 min-w-0",
              playgroundTarget && "max-w-[calc(100%-496px)]",
              !playgroundTarget && selectedServer && "max-w-[calc(100%-396px)]",
            )}>
              {viewMode === "grid" ? (
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
                  {servers.map((s) => (
                    <McpCard
                      key={s.name}
                      server={s}
                      selected={selectedServer?.name === s.name}
                      onClick={() => setSelectedServer(s)}
                      t={t}
                    />
                  ))}
                </div>
              ) : (
                <div className="flex flex-col gap-1.5">
                  {servers.map((s) => (
                    <McpRow
                      key={s.name}
                      server={s}
                      selected={selectedServer?.name === s.name}
                      onClick={() => setSelectedServer(s)}
                      t={t}
                    />
                  ))}
                </div>
              )}
            </div>

            {/* Drawer — show playground OR server detail, not both */}
            {playgroundTarget ? (
              <ToolPlaygroundDrawer
                servers={servers}
                initialServer={playgroundTarget.server}
                initialTool={playgroundTarget.tool}
                onClose={() => setPlaygroundTarget(null)}
              />
            ) : selectedServer ? (
              <McpDrawer
                server={selectedServer}
                onClose={() => setSelectedServer(null)}
                onTest={handleTest}
                onEdit={openEdit}
                onReconnect={handleReconnect}
                onPersist={handlePersist}
                onDelete={handleDelete}
                onTryTool={(serverName, toolName) => {
                  setSelectedServer(null);
                  setPlaygroundTarget({ server: serverName, tool: toolName });
                }}
                actionLoading={actionLoading}
                confirming={confirming}
                t={t}
              />
            ) : null}
          </div>
        )}
      </SectionCard>

      {/* Modal */}
      <McpServerModal
        isOpen={showModal}
        onClose={() => { setShowModal(false); setEditServer(null); }}
        onSave={handleModalSave}
        onTest={handleModalTest}
        editServer={editServer}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// McpCard (grid mode)
// ---------------------------------------------------------------------------

function McpCard({ server, selected, onClick, t }: {
  server: McpServerInfo;
  selected: boolean;
  onClick: () => void;
  t: (key: string, opts?: Record<string, string>) => string;
}) {
  return (
    <div
      onClick={onClick}
      className={cn(
        "bg-[var(--bg-elevated)] rounded-[var(--radius-lg)] p-4 cursor-pointer hover:bg-[var(--bg-hover)] transition-colors border",
        selected ? "border-[var(--accent)]" : "border-[var(--separator)]"
      )}
    >
      <div className="flex justify-between items-start mb-2">
        <h3 className="text-sm font-medium text-[var(--text-primary)] truncate">{server.name}</h3>
        <StatusDot status={mcpHealthToStatus(server.status)} />
      </div>
      <div className="flex items-center gap-2 mb-3">
        <span
          className="text-[10px] font-medium px-2 py-0.5 rounded-full"
          style={transportBadgeStyle(server.transport)}
        >
          {server.transport}
        </span>
        {server.transient && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-tertiary)]">
            {t("dashboard.mcpServers.transient")}
          </span>
        )}
      </div>
      {server.command && (
        <p className="text-xs text-[var(--text-secondary)] mb-1 truncate font-mono">{server.command}</p>
      )}
      {server.url && (
        <p className="text-xs text-[var(--text-secondary)] mb-1 truncate font-mono">{server.url}</p>
      )}
      <div className="flex items-center gap-2 mt-2">
        <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-secondary)]">
          {server.tools.length} {t("dashboard.mcpServers.tools")}
        </span>
      </div>
      {server.error && (
        <p className="text-[11px] text-[var(--error)] mt-2 truncate">{server.error}</p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// McpRow (list mode)
// ---------------------------------------------------------------------------

function McpRow({ server, selected, onClick, t }: {
  server: McpServerInfo;
  selected: boolean;
  onClick: () => void;
  t: (key: string, opts?: Record<string, string>) => string;
}) {
  return (
    <div
      onClick={onClick}
      className={cn(
        "flex items-center gap-4 px-4 py-3 rounded-[var(--radius-md)] cursor-pointer hover:bg-[var(--bg-hover)] transition-colors border",
        selected ? "border-[var(--accent)] bg-[var(--bg-elevated)]" : "border-transparent bg-[var(--bg-elevated)]"
      )}
    >
      <StatusDot status={mcpHealthToStatus(server.status)} />
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-[var(--text-primary)] truncate">{server.name}</span>
        </div>
        <p className="text-xs text-[var(--text-secondary)] truncate font-mono">
          {server.command || server.url || server.transport}
        </p>
      </div>
      <div className="flex items-center gap-2 flex-shrink-0">
        <span
          className="text-[10px] font-medium px-2 py-0.5 rounded-full"
          style={transportBadgeStyle(server.transport)}
        >
          {server.transport}
        </span>
        {server.transient && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-tertiary)]">
            {t("dashboard.mcpServers.transient")}
          </span>
        )}
        <span className="text-[10px] text-[var(--text-tertiary)]">
          {server.tools.length} {t("dashboard.mcpServers.tools")}
        </span>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// McpDrawer
// ---------------------------------------------------------------------------

function McpDrawer({ server, onClose, onTest, onEdit, onReconnect, onPersist, onDelete, onTryTool, actionLoading, confirming, t }: {
  server: McpServerInfo;
  onClose: () => void;
  onTest: (name: string) => void;
  onEdit: (server: McpServerInfo) => void;
  onReconnect: (server: McpServerInfo) => void;
  onPersist: (name: string) => void;
  onDelete: (name: string) => void;
  onTryTool: (serverName: string, toolName: string) => void;
  actionLoading: Record<string, boolean>;
  confirming: string | null;
  t: (key: string, opts?: Record<string, string>) => string;
}) {
  return (
    <div className="w-[380px] flex-shrink-0 bg-[var(--bg-elevated)] border border-[var(--separator)] rounded-[var(--radius-lg)] p-5 overflow-y-auto max-h-[calc(100vh-220px)] animate-fade-in">
      {/* Header */}
      <div className="flex items-start justify-between mb-4">
        <div>
          <h3 className="text-base font-semibold text-[var(--text-primary)]" style={{ fontFamily: "var(--font-heading)" }}>
            {server.name}
          </h3>
          <div className="flex items-center gap-2 mt-1">
            <StatusDot status={mcpHealthToStatus(server.status)} />
            <span className="text-xs text-[var(--text-secondary)]">{server.status}</span>
            <span className="text-[var(--separator)]">&middot;</span>
            <span
              className="text-[10px] font-medium px-2 py-0.5 rounded-full"
              style={transportBadgeStyle(server.transport)}
            >
              {server.transport}
            </span>
            {server.transient && (
              <>
                <span className="text-[var(--separator)]">&middot;</span>
                <span className="text-[10px] text-[var(--text-tertiary)]">{t("dashboard.mcpServers.transient")}</span>
              </>
            )}
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {/* Connection info */}
      <div className="mb-4">
        {server.command && (
          <div className="mb-2">
            <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)] block mb-1">Command</span>
            <p className="text-xs text-[var(--text-secondary)] font-mono bg-[var(--bg-window)] px-2 py-1.5 rounded-[var(--radius-sm)] break-all">
              {server.command} {server.args?.join(" ")}
            </p>
          </div>
        )}
        {server.url && (
          <div className="mb-2">
            <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)] block mb-1">URL</span>
            <p className="text-xs text-[var(--text-secondary)] font-mono bg-[var(--bg-window)] px-2 py-1.5 rounded-[var(--radius-sm)] break-all">
              {server.url}
            </p>
          </div>
        )}
      </div>

      {/* Error */}
      {server.error && (
        <div className="mb-4 px-3 py-2 rounded-[var(--radius-sm)] bg-[var(--error)]/10 border border-[var(--error)]/20">
          <span className="text-[11px] text-[var(--error)]">{server.error}</span>
        </div>
      )}

      {/* Actions bar */}
      <div className="flex items-center gap-2 mb-4 py-3 border-y border-[var(--separator)]">
        <button
          onClick={() => onTest(server.name)}
          disabled={!!actionLoading[`test:${server.name}`]}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50 border border-[var(--border-subtle)]"
        >
          {actionLoading[`test:${server.name}`]
            ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
            : <Zap className="h-3.5 w-3.5" />}
          {t("dashboard.mcpServers.test")}
        </button>
        <button
          onClick={() => onEdit(server)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer border border-[var(--border-subtle)]"
        >
          <Pencil className="h-3.5 w-3.5" />
          {t("dashboard.mcpServers.edit")}
        </button>
        <button
          onClick={() => onReconnect(server)}
          disabled={!!actionLoading[`reconnect:${server.name}`]}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50 border border-[var(--border-subtle)]"
        >
          {actionLoading[`reconnect:${server.name}`]
            ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
            : <RefreshCw className="h-3.5 w-3.5" />}
          {t("dashboard.mcpServers.reconnect")}
        </button>
      </div>

      {/* Tools */}
      <DrawerSection icon={<Wrench className="h-3.5 w-3.5" />} title={`${t("dashboard.mcpServers.tools")} (${server.tools.length})`}>
        {server.tools.length === 0 ? (
          <span className="text-xs text-[var(--text-tertiary)] italic">
            {t("dashboard.mcpServers.noTools")}
          </span>
        ) : (
          <div className="flex flex-col gap-1">
            {server.tools.map((tool) => (
              <div
                key={tool.name}
                className="py-1.5 px-2 rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] transition-colors flex items-start justify-between gap-1"
              >
                <div className="min-w-0 flex-1">
                  <span className="text-[11px] font-mono text-[var(--text-primary)] block">
                    {tool.name}
                  </span>
                  <span className="text-[11px] text-[var(--text-tertiary)] leading-relaxed line-clamp-2">
                    {tool.description}
                  </span>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onTryTool(server.name, tool.name);
                  }}
                  title={t("dashboard.mcpServers.playground.tryTool")}
                  className="flex-shrink-0 mt-0.5 p-1 rounded-[var(--radius-xs)] text-[var(--accent)] hover:bg-[var(--accent)]/10 transition-all cursor-pointer"
                >
                  <Zap className="h-3 w-3" />
                </button>
              </div>
            ))}
          </div>
        )}
      </DrawerSection>

      {/* Persist / Delete */}
      <div className="mt-4 pt-4 border-t border-[var(--separator)] flex items-center gap-2">
        {server.transient && (
          <button
            onClick={() => onPersist(server.name)}
            disabled={!!actionLoading[`persist:${server.name}`]}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50 border border-[var(--border-subtle)]"
          >
            {actionLoading[`persist:${server.name}`]
              ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
              : <Save className="h-3.5 w-3.5" />}
            {t("dashboard.mcpServers.persist")}
          </button>
        )}
        <button
          onClick={() => onDelete(server.name)}
          disabled={!!actionLoading[`delete:${server.name}`]}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium transition-colors cursor-pointer disabled:opacity-50",
            confirming === `delete:${server.name}`
              ? "bg-[var(--error)] text-white"
              : "text-[var(--error)] hover:bg-[var(--error)]/10"
          )}
        >
          {actionLoading[`delete:${server.name}`]
            ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
            : <Trash2 className="h-3.5 w-3.5" />}
          {confirming === `delete:${server.name}`
            ? t("dashboard.mcpServers.deleteConfirm")
            : t("dashboard.mcpServers.delete")}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// DrawerSection
// ---------------------------------------------------------------------------

function DrawerSection({ icon, title, children }: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-4">
      <div className="flex items-center gap-1.5 mb-2">
        <span className="text-[var(--text-tertiary)]">{icon}</span>
        <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)]">{title}</span>
      </div>
      {children}
    </div>
  );
}
