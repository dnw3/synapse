import { useCallback, useEffect, useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Cable, Plus, MoreVertical, RefreshCw, Save, Trash2,
  Loader2, Pencil, Zap, ChevronDown, ChevronRight,
} from "lucide-react";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { McpServerInfo, McpTestResult } from "../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  StatusDot,
  useToast,
  ToastContainer,
  useInlineConfirm,
} from "./shared";
import McpServerModal from "./McpServerModal";
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

// ---------------------------------------------------------------------------
// McpServersPage
// ---------------------------------------------------------------------------

export default function McpServersPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();
  const { confirming, requestConfirm, reset: resetConfirm } = useInlineConfirm();

  // Data state
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedServer, setExpandedServer] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [editServer, setEditServer] = useState<McpServerInfo | null>(null);
  const [actionLoading, setActionLoading] = useState<Record<string, boolean>>({});
  const [menuOpen, setMenuOpen] = useState<string | null>(null);

  // Fetch
  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const res = await api.fetchMcpServers();
      setServers(res ?? []);
    } catch (e) {
      addToast(t("dashboard.mcpServers.fetchError"), "error");
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, [api, t, addToast]);

  useEffect(() => { fetchData(); }, [fetchData]);

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
      try {
        const result = await api.testMcpServer(name);
        if (result?.success) {
          addToast(t("dashboard.mcpServers.testResult", {
            name,
            toolCount: String(result.toolCount),
            latency: String(result.latencyMs),
          }));
        } else {
          addToast(result?.error ?? t("dashboard.mcpServers.testFailed"), "error");
        }
      } catch (e) {
        addToast(t("dashboard.mcpServers.testFailed"), "error");
        console.error(e);
      }
    });
  };

  // Reconnect (update triggers reconnect on backend)
  const handleReconnect = async (server: McpServerInfo) => {
    setMenuOpen(null);
    await withActionLoading(`reconnect:${server.name}`, async () => {
      try {
        await api.updateMcpServer(server.name, {});
        addToast(t("dashboard.mcpServers.reconnected"));
        await fetchData();
      } catch (e) {
        addToast(t("dashboard.mcpServers.reconnectFailed"), "error");
        console.error(e);
      }
    });
  };

  // Persist transient server
  const handlePersist = async (name: string) => {
    setMenuOpen(null);
    await withActionLoading(`persist:${name}`, async () => {
      try {
        await api.persistMcpServer(name);
        addToast(t("dashboard.mcpServers.persisted"));
        await fetchData();
      } catch (e) {
        addToast(String(e), "error");
        console.error(e);
      }
    });
  };

  // Delete server
  const handleDelete = async (name: string) => {
    if (confirming !== `delete:${name}`) {
      requestConfirm(`delete:${name}`);
      return;
    }
    resetConfirm();
    setMenuOpen(null);
    await withActionLoading(`delete:${name}`, async () => {
      try {
        await api.deleteMcpServer(name);
        addToast(t("dashboard.mcpServers.deleted"));
        await fetchData();
      } catch (e) {
        addToast(String(e), "error");
        console.error(e);
      }
    });
  };

  // Modal save handler
  const handleModalSave = async (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">) => {
    if (editServer) {
      await api.updateMcpServer(editServer.name, server);
    } else {
      await api.createMcpServer(server);
    }
    addToast(t("dashboard.mcpServers.saved"));
    await fetchData();
  };

  // Modal test handler — creates/updates first then tests
  const handleModalTest = async (server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">): Promise<McpTestResult | null> => {
    try {
      if (editServer) {
        await api.updateMcpServer(editServer.name, server);
      } else {
        await api.createMcpServer(server);
      }
      const result = await api.testMcpServer(server.name);
      await fetchData();
      return result;
    } catch {
      await fetchData();
      return null;
    }
  };

  // Open edit modal
  const openEdit = (server: McpServerInfo) => {
    setMenuOpen(null);
    setEditServer(server);
    setShowModal(true);
  };

  // Open add modal
  const openAdd = () => {
    setEditServer(null);
    setShowModal(true);
  };

  // Close menu when clicking outside
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(null);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [menuOpen]);

  // Status summary
  const connectedCount = servers.filter((s) => s.status === "connected").length;

  return (
    <div className="space-y-4">
      <SectionCard>
        <SectionHeader
          icon={<Cable className="h-4 w-4" />}
          title={t("dashboard.mcpServers.title")}
          right={
            <div className="flex items-center gap-3">
              {!loading && servers.length > 0 && (
                <span className="text-xs text-[var(--text-tertiary)]">
                  {t("dashboard.mcpServers.statusSummary", {
                    connected: String(connectedCount),
                    total: String(servers.length),
                  })}
                </span>
              )}
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
          <div className="flex flex-col gap-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <LoadingSkeleton key={i} className="h-16" />
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

        {/* Server list */}
        {!loading && servers.length > 0 && (
          <div className="flex flex-col gap-2">
            {servers.map((server) => {
              const isExpanded = expandedServer === server.name;
              const isLoading = (key: string) => !!actionLoading[key];

              return (
                <div
                  key={server.name}
                  className="bg-[var(--bg-elevated)] rounded-[var(--radius-lg)] border border-[var(--separator)] overflow-hidden transition-colors"
                >
                  {/* Collapsed row */}
                  <div className="flex items-center gap-3 px-4 py-3">
                    {/* Expand toggle */}
                    <button
                      onClick={() => setExpandedServer(isExpanded ? null : server.name)}
                      className="p-0.5 rounded text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors cursor-pointer"
                    >
                      {isExpanded
                        ? <ChevronDown className="h-4 w-4" />
                        : <ChevronRight className="h-4 w-4" />}
                    </button>

                    {/* Status */}
                    <StatusDot status={mcpHealthToStatus(server.status)} />

                    {/* Name */}
                    <button
                      onClick={() => setExpandedServer(isExpanded ? null : server.name)}
                      className="text-sm font-medium text-[var(--text-primary)] truncate cursor-pointer bg-transparent border-none p-0 text-left"
                    >
                      {server.name}
                    </button>

                    {/* Transport badge */}
                    <span
                      className="text-[10px] font-medium px-2 py-0.5 rounded-full flex-shrink-0"
                      style={transportBadgeStyle(server.transport)}
                    >
                      {server.transport}
                    </span>

                    {/* Transient badge */}
                    {server.transient && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-tertiary)] flex-shrink-0">
                        {t("dashboard.mcpServers.transient")}
                      </span>
                    )}

                    {/* Tool count */}
                    <span className="text-xs text-[var(--text-tertiary)] flex-shrink-0 ml-auto mr-2">
                      {server.tools.length} {t("dashboard.mcpServers.tools")}
                    </span>

                    {/* Actions */}
                    <div className="flex items-center gap-1 flex-shrink-0">
                      {/* Test */}
                      <button
                        onClick={(e) => { e.stopPropagation(); handleTest(server.name); }}
                        disabled={isLoading(`test:${server.name}`)}
                        className="flex items-center gap-1 px-2 py-1 rounded-[var(--radius-sm)] text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
                        title={t("dashboard.mcpServers.test")}
                      >
                        {isLoading(`test:${server.name}`)
                          ? <Loader2 className="h-3 w-3 animate-spin" />
                          : <Zap className="h-3 w-3" />}
                        {t("dashboard.mcpServers.test")}
                      </button>

                      {/* Edit */}
                      <button
                        onClick={(e) => { e.stopPropagation(); openEdit(server); }}
                        className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                        title={t("dashboard.mcpServers.edit")}
                      >
                        <Pencil className="h-3.5 w-3.5" />
                      </button>

                      {/* More menu */}
                      <div className="relative" ref={menuOpen === server.name ? menuRef : undefined}>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setMenuOpen(menuOpen === server.name ? null : server.name);
                          }}
                          className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                        >
                          <MoreVertical className="h-3.5 w-3.5" />
                        </button>

                        {menuOpen === server.name && (
                          <div className="absolute right-0 top-full mt-1 w-40 bg-[var(--bg-elevated)] border border-[var(--separator)] rounded-[var(--radius-md)] shadow-[var(--shadow-lg)] py-1 z-30 animate-fade-in">
                            {/* Reconnect */}
                            <button
                              onClick={() => handleReconnect(server)}
                              disabled={isLoading(`reconnect:${server.name}`)}
                              className="flex items-center gap-2 w-full px-3 py-1.5 text-[12px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
                            >
                              {isLoading(`reconnect:${server.name}`)
                                ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                : <RefreshCw className="h-3.5 w-3.5" />}
                              {t("dashboard.mcpServers.reconnect")}
                            </button>

                            {/* Persist (transient only) */}
                            {server.transient && (
                              <button
                                onClick={() => handlePersist(server.name)}
                                disabled={isLoading(`persist:${server.name}`)}
                                className="flex items-center gap-2 w-full px-3 py-1.5 text-[12px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
                              >
                                {isLoading(`persist:${server.name}`)
                                  ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                  : <Save className="h-3.5 w-3.5" />}
                                {t("dashboard.mcpServers.persist")}
                              </button>
                            )}

                            {/* Delete */}
                            <button
                              onClick={() => handleDelete(server.name)}
                              disabled={isLoading(`delete:${server.name}`)}
                              className={cn(
                                "flex items-center gap-2 w-full px-3 py-1.5 text-[12px] transition-colors cursor-pointer disabled:opacity-50",
                                confirming === `delete:${server.name}`
                                  ? "text-[var(--error)] bg-[var(--error)]/10"
                                  : "text-[var(--error)] hover:bg-[var(--bg-hover)]"
                              )}
                            >
                              {isLoading(`delete:${server.name}`)
                                ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                : <Trash2 className="h-3.5 w-3.5" />}
                              {confirming === `delete:${server.name}`
                                ? t("dashboard.mcpServers.deleteConfirm")
                                : t("dashboard.mcpServers.delete")}
                            </button>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* Error message */}
                  {server.error && (
                    <div className="px-4 pb-2 -mt-1">
                      <span className="text-[11px] text-[var(--error)]">{server.error}</span>
                    </div>
                  )}

                  {/* Expanded: tools table */}
                  {isExpanded && (
                    <div className="px-4 pb-3 pt-1 border-t border-[var(--separator)]">
                      {server.tools.length === 0 ? (
                        <span className="text-xs text-[var(--text-tertiary)] italic">
                          {t("dashboard.mcpServers.noTools")}
                        </span>
                      ) : (
                        <div className="flex flex-col gap-1">
                          {server.tools.map((tool) => (
                            <div
                              key={tool.name}
                              className="flex items-start gap-4 py-1.5 px-2 rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] transition-colors"
                            >
                              <span className="text-[12px] font-mono text-[var(--text-primary)] flex-shrink-0 min-w-[180px]">
                                {tool.name}
                              </span>
                              <span className="text-[12px] text-[var(--text-secondary)] leading-relaxed">
                                {tool.description}
                              </span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
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

      <ToastContainer toasts={toasts} />
    </div>
  );
}
