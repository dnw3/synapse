import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Monitor, Laptop, Server, Globe, RefreshCw, Clock } from "lucide-react";
import { useGatewayClient } from "../../hooks/useGatewayClient";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  StatusDot,
} from "./shared";
import { cn } from "../../lib/cn";

interface RawPresenceEntry {
  instance_id?: string;
  device_id?: string;
  key?: string;
  reason?: string;
  roles?: string[];
  platform?: string;
  version?: string;
  mode?: string;
  ts?: number;
  text?: string;
  host?: string;
}

interface PresenceEntry {
  instance_id: string;
  type: string;
  platform?: string;
  version?: string;
  roles?: string[];
  mode?: string;
  connected_at?: string;
  display_name?: string;
}

function typeIcon(type: string) {
  switch (type) {
    case "gateway": return <Server className="h-4 w-4" />;
    case "webchat": return <Monitor className="h-4 w-4" />;
    case "node": return <Laptop className="h-4 w-4" />;
    default: return <Globe className="h-4 w-4" />;
  }
}

function typeBadgeClass(type: string): string {
  switch (type) {
    case "gateway":
      return "bg-[var(--accent)]/15 text-[var(--accent)] border-[var(--accent)]/30";
    case "webchat":
      return "bg-[var(--success)]/15 text-[var(--success)] border-[var(--success)]/30";
    case "node":
      return "bg-[var(--warning)]/15 text-[var(--warning)] border-[var(--warning)]/30";
    default:
      return "bg-[var(--bg-content)] text-[var(--text-secondary)] border-[var(--separator)]";
  }
}

function relativeTime(isoDate?: string): string {
  if (!isoDate) return "";
  const diff = Date.now() - new Date(isoDate).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

export default function InstancesPage() {
  const { t } = useTranslation();
  const { client, connected, helloOk } = useGatewayClient();
  const [instances, setInstances] = useState<PresenceEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const mapPresence = (raw: RawPresenceEntry[]): PresenceEntry[] =>
    raw.map((e) => ({
      instance_id: e.instance_id || e.device_id || e.key || "unknown",
      type: e.reason || (e.roles?.includes("gateway") ? "gateway" : "webchat"),
      platform: e.platform,
      version: e.version,
      roles: e.roles,
      mode: e.mode,
      connected_at: e.ts ? new Date(e.ts).toISOString() : undefined,
      display_name: e.text || e.host || undefined,
    }));

  const loadInstances = useCallback(() => {
    // Try snapshot first
    if (helloOk?.snapshot?.presence) {
      const presence = helloOk.snapshot.presence;
      const arr = (Array.isArray(presence) ? presence : typeof presence === "object" ? Object.values(presence) : []) as RawPresenceEntry[];
      setInstances(mapPresence(arr));
      setLoading(false);
      return;
    }

    // Fallback: query via presence.list RPC
    if (client?.isConnected) {
      client.request<RawPresenceEntry[]>("presence.list").then((data) => {
        const arr = Array.isArray(data) ? data : [];
        setInstances(mapPresence(arr));
        setLoading(false);
      }).catch(() => {
        setLoading(false);
      });
    } else {
      setLoading(false);
    }
  }, [client, helloOk]);

  useEffect(() => {
    loadInstances();
  }, [loadInstances]);

  // Subscribe to presence events for real-time updates
  useEffect(() => {
    if (!client?.isConnected) return;
    const unsub = client.onEvent("presence", (payload: unknown) => {
      const p = payload as { instances?: RawPresenceEntry[] } | RawPresenceEntry[] | null;
      const arr = Array.isArray(p) ? p : (p && typeof p === "object" && "instances" in p && Array.isArray(p.instances)) ? p.instances : [];
      setInstances(mapPresence(arr));
    });
    return unsub;
  }, [client, connected]);

  if (loading) {
    return <LoadingSkeleton />;
  }

  return (
    <div className="flex flex-col gap-6">
      <SectionCard>
        <SectionHeader
          icon={<Monitor className="h-4 w-4" />}
          title={t("instances.title")}
          right={
            <button
              onClick={loadInstances}
              className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
            >
              <RefreshCw className="h-3.5 w-3.5" />
            </button>
          }
        />

        {instances.length === 0 ? (
          <EmptyState
            icon={<Monitor className="h-8 w-8" />}
            message={t("instances.empty")}
          />
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 mt-4">
            {instances.map((inst) => (
              <div
                key={inst.instance_id}
                className="group relative overflow-hidden rounded-[var(--radius-lg)] bg-[var(--bg-primary)] border border-[var(--border-subtle)] p-4 transition-all duration-200 hover:border-[var(--separator)] hover:shadow-[var(--shadow-sm)]"
              >
                {/* Top accent line */}
                <div
                  className="absolute top-0 left-0 right-0 h-[2px] opacity-60 group-hover:opacity-100 transition-opacity"
                  style={{ background: "var(--accent)" }}
                />

                <div className="flex items-start justify-between gap-2 mb-3">
                  <div className="flex items-center gap-2">
                    <div
                      className="flex items-center justify-center w-8 h-8 rounded-[var(--radius-md)]"
                      style={{ background: "color-mix(in srgb, var(--accent) 12%, transparent)" }}
                    >
                      <span style={{ color: "var(--accent-light)" }}>{typeIcon(inst.type)}</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate max-w-[140px]">
                        {inst.display_name || inst.instance_id.slice(0, 12)}
                      </span>
                      <span className="text-[11px] font-mono text-[var(--text-tertiary)] truncate max-w-[140px]">
                        {inst.instance_id.slice(0, 16)}
                      </span>
                    </div>
                  </div>
                  <StatusDot status="online" />
                </div>

                <div className="space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] text-[var(--text-tertiary)]">{t("instances.type")}</span>
                    <span className={cn(
                      "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium border",
                      typeBadgeClass(inst.type)
                    )}>
                      {inst.type}
                    </span>
                  </div>

                  {inst.platform && (
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] text-[var(--text-tertiary)]">{t("instances.platform")}</span>
                      <span className="text-[11px] text-[var(--text-secondary)] font-mono">{inst.platform}</span>
                    </div>
                  )}

                  {inst.version && (
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] text-[var(--text-tertiary)]">{t("instances.version")}</span>
                      <span className="text-[11px] text-[var(--text-secondary)] font-mono">{inst.version}</span>
                    </div>
                  )}

                  {inst.mode && (
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] text-[var(--text-tertiary)]">{t("instances.mode")}</span>
                      <span className="text-[11px] text-[var(--text-secondary)]">{inst.mode}</span>
                    </div>
                  )}

                  {inst.connected_at && (
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] text-[var(--text-tertiary)]">{t("instances.connected")}</span>
                      <span className="inline-flex items-center gap-1 text-[11px] text-[var(--text-secondary)]">
                        <Clock className="h-3 w-3" />
                        {relativeTime(inst.connected_at)}
                      </span>
                    </div>
                  )}

                  {inst.roles && inst.roles.length > 0 && (
                    <div className="flex flex-wrap gap-1 pt-1">
                      {inst.roles.map((role) => (
                        <span
                          key={role}
                          className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-[var(--bg-secondary)] text-[var(--text-tertiary)] border border-[var(--border-subtle)]"
                        >
                          {role}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </SectionCard>
    </div>
  );
}
