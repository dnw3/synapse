import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Puzzle, Plus, LayoutGrid, List, X, Loader2,
  Wrench, Radio, Shield, Bell, Play, Square, Trash2,
  RefreshCw, FolderOpen,
} from "lucide-react";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import type { PluginInfo, ServiceInfo } from "../../types/dashboard";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  Toggle,
  useToast,
  ToastContainer,
  useInlineConfirm,
} from "./shared";
import { StatusDot } from "./shared";
import { cn } from "../../lib/cn";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function healthToStatus(health: string, enabled: boolean): "online" | "offline" | "degraded" {
  if (!enabled) return "offline";
  if (health === "healthy") return "online";
  if (health === "error") return "offline";
  if (health === "unknown") return "online"; // No services to check — normal
  return "degraded";
}

function serviceStatusColor(status: ServiceInfo["status"]): string {
  switch (status) {
    case "running": return "var(--success)";
    case "stopped": return "var(--text-tertiary)";
    case "error": return "var(--error)";
    default: return "var(--warning)";
  }
}

const VIEW_KEY = "synapse:plugins-view";

// ---------------------------------------------------------------------------
// PluginsPage
// ---------------------------------------------------------------------------

export default function PluginsPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();
  const { confirming, requestConfirm, reset: resetConfirm } = useInlineConfirm();

  // Data state
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // UI state
  const [viewMode, setViewMode] = useState<"grid" | "list">(() => {
    try { return (localStorage.getItem(VIEW_KEY) as "grid" | "list") || "grid"; } catch { return "grid"; }
  });
  const [selectedPlugin, setSelectedPlugin] = useState<PluginInfo | null>(null);
  const [showInstallDialog, setShowInstallDialog] = useState(false);
  const [installPath, setInstallPath] = useState("");
  const [actionLoading, setActionLoading] = useState<Record<string, boolean>>({});

  // Fetch
  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.fetchPlugins();
      setPlugins(res?.plugins || []);
    } catch (e) {
      setError(t("dashboard.plugins.fetchError"));
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, [api, t]);

  useEffect(() => { fetchData(); }, [fetchData]);

  // Persist view mode
  const changeView = (mode: "grid" | "list") => {
    setViewMode(mode);
    try { localStorage.setItem(VIEW_KEY, mode); } catch { /* noop */ }
  };

  // Actions
  const withActionLoading = async (key: string, fn: () => Promise<void>) => {
    setActionLoading((prev) => ({ ...prev, [key]: true }));
    try { await fn(); } finally {
      setActionLoading((prev) => ({ ...prev, [key]: false }));
    }
  };

  const handleToggle = async (plugin: PluginInfo) => {
    await withActionLoading(`toggle:${plugin.name}`, async () => {
      try {
        await api.togglePlugin(plugin.name, !plugin.enabled);
        addToast(t("dashboard.plugins.toast.toggled", { action: plugin.enabled ? t("dashboard.plugins.detail.disable") : t("dashboard.plugins.detail.enable") }));
        addToast(t("dashboard.plugins.detail.restartRequired"), "success");
        await fetchData();
        // Update selected if same
        if (selectedPlugin?.name === plugin.name) {
          setSelectedPlugin((prev) => prev ? { ...prev, enabled: !prev.enabled } : null);
        }
      } catch (e) {
        addToast(t("dashboard.plugins.toast.error", { message: String(e) }), "error");
      }
    });
  };

  const handleServiceControl = async (plugin: PluginInfo, service: ServiceInfo, action: "start" | "stop") => {
    if (action === "stop" && confirming !== `svc:${service.id}`) {
      requestConfirm(`svc:${service.id}`);
      return;
    }
    resetConfirm();
    await withActionLoading(`svc:${service.id}`, async () => {
      try {
        await api.controlService(plugin.name, service.id, action);
        addToast(t("dashboard.plugins.toast.serviceControlled", { action }));
        await fetchData();
      } catch (e) {
        addToast(t("dashboard.plugins.toast.error", { message: String(e) }), "error");
      }
    });
  };

  const handleUninstall = async (plugin: PluginInfo) => {
    if (confirming !== `uninstall:${plugin.name}`) {
      requestConfirm(`uninstall:${plugin.name}`);
      return;
    }
    resetConfirm();
    await withActionLoading(`uninstall:${plugin.name}`, async () => {
      try {
        await api.removePlugin(plugin.name);
        addToast(t("dashboard.plugins.toast.removed"));
        setSelectedPlugin(null);
        await fetchData();
      } catch (e) {
        addToast(t("dashboard.plugins.toast.error", { message: String(e) }), "error");
      }
    });
  };

  const handleInstall = async () => {
    if (!installPath.trim()) return;
    await withActionLoading("install", async () => {
      try {
        await api.installPlugin(installPath.trim());
        addToast(t("dashboard.plugins.toast.installed"));
        setShowInstallDialog(false);
        setInstallPath("");
        await fetchData();
      } catch (e) {
        addToast(t("dashboard.plugins.toast.error", { message: String(e) }), "error");
      }
    });
  };

  // Escape key closes drawer
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (showInstallDialog) setShowInstallDialog(false);
        else if (selectedPlugin) setSelectedPlugin(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selectedPlugin, showInstallDialog]);

  // Keep selectedPlugin in sync with fetched data
  useEffect(() => {
    if (selectedPlugin) {
      const updated = plugins.find((p) => p.name === selectedPlugin.name);
      if (updated) setSelectedPlugin(updated);
    }
  }, [plugins]); // eslint-disable-line react-hooks/exhaustive-deps

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div className="space-y-4">
      <SectionCard>
        <SectionHeader
          icon={<Puzzle className="h-4 w-4" />}
          title={t("dashboard.plugins.title")}
          right={
            <div className="flex items-center gap-2">
              {/* Grid / List toggle */}
              <div className="flex items-center gap-0.5 bg-[var(--bg-window)] rounded-[var(--radius-md)] border border-[var(--border-subtle)] p-0.5">
                <button
                  onClick={() => changeView("grid")}
                  title={t("dashboard.plugins.viewGrid")}
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
                  title={t("dashboard.plugins.viewList")}
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

              {/* Install button */}
              <button
                onClick={() => setShowInstallDialog(true)}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium bg-[var(--accent)] text-white hover:opacity-90 transition-opacity cursor-pointer"
              >
                <Plus className="h-3.5 w-3.5" />
                {t("dashboard.plugins.install")}
              </button>
            </div>
          }
        />

        {/* Loading */}
        {loading && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <LoadingSkeleton key={i} className="h-32" />
            ))}
          </div>
        )}

        {/* Error */}
        {!loading && error && (
          <EmptyState
            icon={<Puzzle className="h-8 w-8" />}
            message={error}
            action={
              <button
                onClick={fetchData}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer border border-[var(--border-subtle)]"
              >
                <RefreshCw className="h-3.5 w-3.5" />
                {t("overview.refresh")}
              </button>
            }
          />
        )}

        {/* Empty */}
        {!loading && !error && plugins.length === 0 && (
          <EmptyState
            icon={<Puzzle className="h-8 w-8" />}
            message={t("dashboard.plugins.empty")}
            description={t("dashboard.plugins.builtinNote")}
          />
        )}

        {/* Content */}
        {!loading && !error && plugins.length > 0 && (
          <div className="flex gap-4">
            {/* Main area */}
            <div className={cn("flex-1 min-w-0", selectedPlugin && "max-w-[calc(100%-396px)]")}>
              {viewMode === "grid" ? (
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
                  {plugins.map((p) => (
                    <PluginCard
                      key={p.name}
                      plugin={p}
                      selected={selectedPlugin?.name === p.name}
                      onClick={() => setSelectedPlugin(p)}
                      t={t}
                    />
                  ))}
                </div>
              ) : (
                <div className="flex flex-col gap-1.5">
                  {plugins.map((p) => (
                    <PluginRow
                      key={p.name}
                      plugin={p}
                      selected={selectedPlugin?.name === p.name}
                      onClick={() => setSelectedPlugin(p)}
                      t={t}
                    />
                  ))}
                </div>
              )}
            </div>

            {/* Drawer */}
            {selectedPlugin && (
              <PluginDrawer
                plugin={selectedPlugin}
                onClose={() => setSelectedPlugin(null)}
                onToggle={handleToggle}
                onServiceControl={handleServiceControl}
                onUninstall={handleUninstall}
                actionLoading={actionLoading}
                confirming={confirming}
                t={t}
              />
            )}
          </div>
        )}
      </SectionCard>

      {/* Install Dialog */}
      {showInstallDialog && (
        <InstallDialog
          path={installPath}
          onPathChange={setInstallPath}
          onInstall={handleInstall}
          onClose={() => { setShowInstallDialog(false); setInstallPath(""); }}
          loading={!!actionLoading["install"]}
          t={t}
        />
      )}

      <ToastContainer toasts={toasts} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// PluginCard (grid mode)
// ---------------------------------------------------------------------------

function PluginCard({ plugin, selected, onClick, t }: {
  plugin: PluginInfo;
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
        <h3 className="text-sm font-medium text-[var(--text-primary)] truncate">{plugin.name}</h3>
        <StatusDot status={healthToStatus(plugin.health, plugin.enabled)} />
      </div>
      <p className="text-xs text-[var(--text-secondary)] mb-1 line-clamp-2">{plugin.description}</p>
      <p className="text-xs text-[var(--text-tertiary)] mb-3">v{plugin.version}</p>
      {plugin.capabilities.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-2">
          {plugin.capabilities.map((cap) => (
            <span
              key={cap}
              className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-secondary)]"
            >
              {cap}
            </span>
          ))}
        </div>
      )}
      <div className="text-[10px] text-[var(--text-tertiary)]">
        {t(`dashboard.plugins.source.${plugin.source}`)}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// PluginRow (list mode)
// ---------------------------------------------------------------------------

function PluginRow({ plugin, selected, onClick, t }: {
  plugin: PluginInfo;
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
      <StatusDot status={healthToStatus(plugin.health, plugin.enabled)} />
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-[var(--text-primary)] truncate">{plugin.name}</span>
          <span className="text-[10px] text-[var(--text-tertiary)] flex-shrink-0">v{plugin.version}</span>
        </div>
        <p className="text-xs text-[var(--text-secondary)] truncate">{plugin.description}</p>
      </div>
      <div className="flex items-center gap-2 flex-shrink-0">
        {plugin.capabilities.slice(0, 3).map((cap) => (
          <span
            key={cap}
            className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-secondary)]"
          >
            {cap}
          </span>
        ))}
        <span className="text-[10px] text-[var(--text-tertiary)]">
          {t(`dashboard.plugins.source.${plugin.source}`)}
        </span>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// PluginDrawer
// ---------------------------------------------------------------------------

function PluginDrawer({ plugin, onClose, onToggle, onServiceControl, onUninstall, actionLoading, confirming, t }: {
  plugin: PluginInfo;
  onClose: () => void;
  onToggle: (p: PluginInfo) => void;
  onServiceControl: (p: PluginInfo, s: ServiceInfo, action: "start" | "stop") => void;
  onUninstall: (p: PluginInfo) => void;
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
            {plugin.name}
          </h3>
          <div className="flex items-center gap-2 mt-1">
            <span className="text-xs text-[var(--text-tertiary)]">v{plugin.version}</span>
            {plugin.slot && (
              <>
                <span className="text-[var(--separator)]">&middot;</span>
                <span className="text-xs text-[var(--text-tertiary)]">{t("dashboard.plugins.detail.slot")}: {plugin.slot}</span>
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

      {/* Description */}
      <p className="text-xs text-[var(--text-secondary)] mb-4">{plugin.description}</p>

      {/* Health + Toggle */}
      <div className="flex items-center justify-between mb-4 py-2 border-y border-[var(--separator)]">
        <div className="flex items-center gap-2">
          <StatusDot status={healthToStatus(plugin.health, plugin.enabled)} />
          <span className="text-xs text-[var(--text-secondary)]">
            {!plugin.enabled
              ? t("dashboard.plugins.status.disabled")
              : plugin.health === "unknown"
                ? t("dashboard.plugins.status.active")
                : t(`dashboard.plugins.status.${plugin.health}`)}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-[var(--text-tertiary)]">
            {plugin.enabled ? t("dashboard.plugins.detail.enable") : t("dashboard.plugins.detail.disable")}
          </span>
          <Toggle
            checked={plugin.enabled}
            onChange={() => onToggle(plugin)}
            disabled={!!actionLoading[`toggle:${plugin.name}`]}
            size="sm"
            label={plugin.enabled ? t("dashboard.plugins.detail.disable") : t("dashboard.plugins.detail.enable")}
          />
        </div>
      </div>

      {/* Source + Capabilities */}
      <div className="mb-4">
        <div className="flex items-center gap-1.5 mb-2">
          <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)]">
            {t("dashboard.plugins.detail.source")}
          </span>
          <span className="text-xs text-[var(--text-secondary)]">
            {t(`dashboard.plugins.source.${plugin.source}`)}
          </span>
        </div>
        {plugin.capabilities.length > 0 && (
          <div>
            <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-tertiary)] block mb-1">
              {t("dashboard.plugins.detail.capabilities")}
            </span>
            <div className="flex flex-wrap gap-1">
              {plugin.capabilities.map((cap) => (
                <span key={cap} className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--text-secondary)]">
                  {cap}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Tools */}
      <DrawerSection icon={<Wrench className="h-3.5 w-3.5" />} title={t("dashboard.plugins.detail.tools")}>
        {plugin.tools.length === 0 ? (
          <span className="text-xs text-[var(--text-tertiary)]">{t("dashboard.plugins.detail.noTools")}</span>
        ) : (
          <div className="flex flex-wrap gap-1">
            {plugin.tools.map((tool) => (
              <span key={tool} className="text-[11px] px-2 py-0.5 rounded-[var(--radius-sm)] bg-[var(--bg-window)] text-[var(--text-secondary)] border border-[var(--border-subtle)] font-mono">
                {tool}
              </span>
            ))}
          </div>
        )}
      </DrawerSection>

      {/* Services */}
      <DrawerSection icon={<Radio className="h-3.5 w-3.5" />} title={t("dashboard.plugins.detail.services")}>
        {plugin.services.length === 0 ? (
          <span className="text-xs text-[var(--text-tertiary)]">{t("dashboard.plugins.detail.noServices")}</span>
        ) : (
          <div className="flex flex-col gap-1.5">
            {plugin.services.map((svc) => (
              <div key={svc.id} className="flex items-center justify-between py-1.5 px-2 rounded-[var(--radius-sm)] bg-[var(--bg-window)]">
                <div className="flex items-center gap-2">
                  <span
                    className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                    style={{ background: serviceStatusColor(svc.status) }}
                  />
                  <span className="text-xs text-[var(--text-primary)] font-mono">{svc.id}</span>
                  <span className="text-[10px] text-[var(--text-tertiary)]">{svc.status}</span>
                </div>
                <div className="flex items-center gap-1">
                  {svc.status === "stopped" && (
                    <button
                      onClick={() => onServiceControl(plugin, svc, "start")}
                      disabled={!!actionLoading[`svc:${svc.id}`]}
                      className="p-1 rounded text-[var(--success)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
                      title={t("dashboard.plugins.detail.start")}
                    >
                      {actionLoading[`svc:${svc.id}`] ? <Loader2 className="h-3 w-3 animate-spin" /> : <Play className="h-3 w-3" />}
                    </button>
                  )}
                  {svc.status === "running" && (
                    <button
                      onClick={() => onServiceControl(plugin, svc, "stop")}
                      disabled={!!actionLoading[`svc:${svc.id}`]}
                      className={cn(
                        "p-1 rounded transition-colors cursor-pointer disabled:opacity-50",
                        confirming === `svc:${svc.id}`
                          ? "text-[var(--error)] bg-[var(--error)]/10"
                          : "text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]"
                      )}
                      title={confirming === `svc:${svc.id}` ? t("dashboard.plugins.detail.stopConfirm", { name: svc.id }) : t("dashboard.plugins.detail.stop")}
                    >
                      {actionLoading[`svc:${svc.id}`] ? <Loader2 className="h-3 w-3 animate-spin" /> : <Square className="h-3 w-3" />}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </DrawerSection>

      {/* Interceptors */}
      {plugin.interceptors.length > 0 && (
        <DrawerSection icon={<Shield className="h-3.5 w-3.5" />} title={t("dashboard.plugins.detail.interceptors")}>
          <div className="flex flex-wrap gap-1">
            {plugin.interceptors.map((name) => (
              <span key={name} className="text-[11px] px-2 py-0.5 rounded-[var(--radius-sm)] bg-[var(--bg-window)] text-[var(--text-secondary)] border border-[var(--border-subtle)] font-mono">
                {name}
              </span>
            ))}
          </div>
        </DrawerSection>
      )}

      {/* Subscribers */}
      {plugin.subscribers.length > 0 && (
        <DrawerSection icon={<Bell className="h-3.5 w-3.5" />} title={t("dashboard.plugins.detail.subscribers")}>
          <div className="flex flex-wrap gap-1">
            {plugin.subscribers.map((name) => (
              <span key={name} className="text-[11px] px-2 py-0.5 rounded-[var(--radius-sm)] bg-[var(--bg-window)] text-[var(--text-secondary)] border border-[var(--border-subtle)] font-mono">
                {name}
              </span>
            ))}
          </div>
        </DrawerSection>
      )}

      {/* Uninstall */}
      {plugin.source !== "builtin" && (
        <div className="mt-6 pt-4 border-t border-[var(--separator)]">
          <button
            onClick={() => onUninstall(plugin)}
            disabled={!!actionLoading[`uninstall:${plugin.name}`]}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium transition-colors cursor-pointer disabled:opacity-50",
              confirming === `uninstall:${plugin.name}`
                ? "bg-[var(--error)] text-white"
                : "text-[var(--error)] hover:bg-[var(--error)]/10"
            )}
          >
            {actionLoading[`uninstall:${plugin.name}`] ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Trash2 className="h-3.5 w-3.5" />
            )}
            {confirming === `uninstall:${plugin.name}`
              ? t("dashboard.plugins.detail.uninstallConfirm", { name: plugin.name })
              : t("dashboard.plugins.detail.uninstall")}
          </button>
        </div>
      )}
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

// ---------------------------------------------------------------------------
// InstallDialog
// ---------------------------------------------------------------------------

function InstallDialog({ path, onPathChange, onInstall, onClose, loading, t }: {
  path: string;
  onPathChange: (v: string) => void;
  onInstall: () => void;
  onClose: () => void;
  loading: boolean;
  t: (key: string) => string;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" onClick={onClose} />

      {/* Dialog */}
      <div className="relative bg-[var(--bg-elevated)] rounded-[var(--radius-lg)] border border-[var(--separator)] shadow-[var(--shadow-lg)] p-6 w-full max-w-md animate-fade-in">
        <h3
          className="text-base font-semibold text-[var(--text-primary)] mb-4"
          style={{ fontFamily: "var(--font-heading)" }}
        >
          {t("dashboard.plugins.installDialog.title")}
        </h3>

        <label className="block text-xs text-[var(--text-secondary)] mb-1.5">
          {t("dashboard.plugins.installDialog.pathLabel")}
        </label>
        <div className="flex items-center gap-2 mb-6">
          <div className="flex-1 flex items-center gap-2 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-window)] border border-[var(--border-subtle)] focus-within:border-[var(--accent)] transition-colors">
            <FolderOpen className="h-3.5 w-3.5 text-[var(--text-tertiary)] flex-shrink-0" />
            <input
              type="text"
              value={path}
              onChange={(e) => onPathChange(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && onInstall()}
              placeholder={t("dashboard.plugins.installDialog.pathPlaceholder")}
              className="flex-1 text-sm text-[var(--text-primary)] bg-transparent outline-none placeholder:text-[var(--text-tertiary)]"
              autoFocus
            />
          </div>
        </div>

        <div className="flex items-center justify-end gap-2">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-[var(--radius-md)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          >
            {t("dashboard.plugins.installDialog.cancel")}
          </button>
          <button
            onClick={onInstall}
            disabled={loading || !path.trim()}
            className="flex items-center gap-1.5 px-4 py-2 rounded-[var(--radius-md)] text-[12px] font-medium bg-[var(--accent)] text-white hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
          >
            {loading && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            {t("dashboard.plugins.installDialog.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
