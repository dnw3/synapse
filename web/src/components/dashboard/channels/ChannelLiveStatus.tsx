import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Wifi, WifiOff } from "lucide-react";
import { useDebugInvoke } from "../../../hooks/queries/useDebugQueries";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
} from "../shared";
import { cn } from "../../../lib/cn";

interface ChannelAccountStatus {
  account_id: string;
  state: "connected" | "connecting" | "disconnected";
  running: boolean;
  busy: boolean;
  active_runs: number;
  connected_at: number | null;
  last_event_at: number | null;
  last_inbound_at: number | null;
  last_outbound_at: number | null;
  last_error: string | null;
  reconnect_count: number;
  mode: string | null;
  last_disconnect: { at: number; error: string | null } | null;
}

function formatRelativeTime(unixSecs: number): string {
  const diff = Math.floor(Date.now() / 1000) - unixSecs;
  if (diff < 60) return `${diff}s`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
  return `${Math.floor(diff / 86400)}d`;
}

export function LiveStatusSection() {
  const { t } = useTranslation();
  const debugInvoke = useDebugInvoke();
  const [channelMap, setChannelMap] = useState<Record<string, ChannelAccountStatus[]> | null>(null);
  const [loadingLive, setLoadingLive] = useState(true);
  const [expandedErrors, setExpandedErrors] = useState<Set<string>>(new Set());
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchLiveStatus = useCallback(async () => {
    const resp = await debugInvoke.mutateAsync({ method: "channels.status", params: {} }).catch(() => null);
    if (resp?.ok && resp.result && typeof resp.result === "object" && "channels" in resp.result) {
      const raw = (resp.result as { channels: Record<string, { accounts: ChannelAccountStatus[]; configured: number }> }).channels;
      // Extract accounts arrays, filtering to only channels with configured > 0
      const mapped: Record<string, ChannelAccountStatus[]> = {};
      for (const [name, entry] of Object.entries(raw)) {
        if (entry.configured > 0 || (entry.accounts && entry.accounts.length > 0)) {
          mapped[name] = entry.accounts ?? [];
        }
      }
      setChannelMap(mapped);
    } else {
      setChannelMap({});
    }
    setLoadingLive(false);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    fetchLiveStatus();
    intervalRef.current = setInterval(fetchLiveStatus, 30_000);
    return () => {
      if (intervalRef.current !== null) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [fetchLiveStatus]);

  const toggleError = (key: string) => {
    setExpandedErrors((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const channelNames = channelMap ? Object.keys(channelMap).sort() : [];
  const onlineCount = channelMap
    ? Object.values(channelMap)
        .flat()
        .filter((a) => a.state === "connected").length
    : 0;

  return (
    <SectionCard className="lg:col-span-2">
      <SectionHeader
        icon={<Wifi className="h-4 w-4" />}
        title={t("channels.liveStatus")}
        right={
          onlineCount > 0 && (
            <span className="px-1.5 py-0.5 rounded-full bg-[var(--success)]/15 text-[var(--success)] text-[10px] font-mono tabular-nums border border-[var(--success)]/25">
              {onlineCount} {t("channels.online")}
            </span>
          )
        }
      />
      {loadingLive ? (
        <div className="flex gap-2 flex-wrap px-0.5">
          {Array.from({ length: 4 }).map((_, i) => (
            <LoadingSkeleton key={i} className="h-20 w-full" />
          ))}
        </div>
      ) : channelNames.length === 0 ? (
        <EmptyState
          icon={<WifiOff className="h-5 w-5" />}
          message={t("channels.noChannels")}
        />
      ) : (
        <div className="space-y-3">
          {channelNames.map((channelName) => {
            const accounts = channelMap![channelName];
            const connectedCount = accounts.filter((a) => a.state === "connected").length;
            return (
              <div key={channelName} className="space-y-2">
                {/* Channel group header */}
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-semibold text-[var(--text-primary)]">{channelName}</span>
                  <span className="px-1.5 py-0.5 rounded-full bg-[var(--bg-content)] text-[10px] font-mono text-[var(--text-tertiary)] tabular-nums">
                    {connectedCount}/{accounts.length} {t("channels.accounts")}
                  </span>
                </div>

                {accounts.length === 0 ? (
                  <div className="text-[12px] text-[var(--text-tertiary)] italic px-1">
                    {t("channels.no_accounts")}
                  </div>
                ) : (
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                    {accounts.map((acct) => {
                      const isStale =
                        acct.state === "connected" &&
                        acct.last_event_at != null &&
                        Date.now() / 1000 - acct.last_event_at > 1800;
                      const errorKey = `${channelName}:${acct.account_id}`;

                      return (
                        <div
                          key={acct.account_id}
                          className={cn(
                            "px-3 py-2.5 rounded-[var(--radius-md)] border transition-all space-y-1.5",
                            acct.state === "connected"
                              ? "bg-[var(--success)]/5 border-[var(--success)]/20"
                              : acct.state === "connecting"
                                ? "bg-[var(--warning)]/5 border-[var(--warning)]/20"
                                : "bg-[var(--bg-content)]/50 border-[var(--border-subtle)]"
                          )}
                        >
                          {/* Row 1: status dot + account_id + mode chip */}
                          <div className="flex items-center gap-2">
                            <span
                              className={cn(
                                "w-2 h-2 rounded-full flex-shrink-0",
                                acct.state === "connected"
                                  ? "bg-green-500"
                                  : acct.state === "connecting"
                                    ? "bg-yellow-500 animate-pulse"
                                    : "bg-gray-400"
                              )}
                            />
                            <span className="text-[12px] font-medium text-[var(--text-primary)] truncate flex-1">
                              {acct.account_id}
                            </span>
                            {acct.mode && (
                              <span className="px-1.5 py-0.5 rounded-full bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium border border-[var(--accent)]/20 flex-shrink-0">
                                {acct.mode}
                              </span>
                            )}
                          </div>

                          {/* Row 2: metadata line */}
                          <div className="flex items-center gap-3 text-[10px] text-[var(--text-tertiary)] flex-wrap">
                            {/* Connected duration */}
                            {acct.state === "connected" && acct.connected_at != null && (
                              <span>
                                {t("channels.connected_since")} {formatRelativeTime(acct.connected_at)}
                              </span>
                            )}
                            {/* State label for non-connected */}
                            {acct.state !== "connected" && (
                              <span>{t(`channels.${acct.state}`)}</span>
                            )}
                            {/* Last event */}
                            {acct.last_event_at != null && (
                              <span
                                className={cn(isStale && "text-[var(--warning)] font-medium")}
                                title={isStale ? t("channels.stale_warning") : undefined}
                              >
                                {t("channels.last_event")} {formatRelativeTime(acct.last_event_at)}
                                {isStale && " !"}
                              </span>
                            )}
                            {/* Reconnect count */}
                            {acct.reconnect_count > 0 && (
                              <span className="px-1 py-0 rounded bg-[var(--warning)]/15 text-[var(--warning)] font-medium">
                                {t("channels.reconnect_count")}: {acct.reconnect_count}
                              </span>
                            )}
                          </div>

                          {/* Row 3: last error (collapsible) */}
                          {acct.last_error && (
                            <div>
                              <button
                                onClick={() => toggleError(errorKey)}
                                className="text-[10px] text-[var(--error)] hover:underline cursor-pointer"
                              >
                                {t("channels.last_error")} {expandedErrors.has(errorKey) ? "▾" : "▸"}
                              </button>
                              {expandedErrors.has(errorKey) && (
                                <div className="mt-1 px-2 py-1.5 rounded-[var(--radius-sm)] bg-[var(--error)]/5 border border-[var(--error)]/15 text-[10px] text-[var(--error)] font-mono break-all">
                                  {acct.last_error}
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </SectionCard>
  );
}
