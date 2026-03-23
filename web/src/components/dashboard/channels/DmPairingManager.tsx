import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { ShieldCheck, UserCheck, UserX, Check, Loader2 } from "lucide-react";
import { useDebugInvoke } from "../../../hooks/queries/useDebugQueries";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
} from "../shared";
import { useToast } from "../../ui/toast";
import { cn } from "../../../lib/cn";

interface PairingRequest {
  code: string;
  sender_id: string;
  channel: string;
  created_at: number;
  ttl_ms: number;
}

interface AllowlistEntry {
  channel: string;
  sender_id: string;
}

export function DmPairingSection() {
  const { t } = useTranslation();
  const { toast } = useToast();
  const debugInvoke = useDebugInvoke();
  const [pending, setPending] = useState<PairingRequest[]>([]);
  const [allowEntries, setAllowEntries] = useState<AllowlistEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [approvingCode, setApprovingCode] = useState<string | null>(null);
  const [removingKey, setRemovingKey] = useState<string | null>(null);
  const [fetchedAt, setFetchedAt] = useState(0);

  const fetchData = useCallback(async () => {
    // First discover which channels have pairing data
    const chResp = await debugInvoke.mutateAsync({ method: "dm.pairing.channels", params: {} }).catch(() => null);
    const channels: string[] = (chResp?.ok && chResp.result)
      ? (chResp.result as { channels: string[] }).channels ?? []
      : [];

    if (channels.length === 0) {
      setPending([]);
      setAllowEntries([]);
      setFetchedAt(Date.now());
      setLoading(false);
      return;
    }

    // Fetch pending + allowlist for all channels in parallel
    const results = await Promise.all(
      channels.flatMap((ch) => [
        debugInvoke.mutateAsync({ method: "dm.pairing.list", params: { channel: ch } }).catch(() => null),
        debugInvoke.mutateAsync({ method: "dm.pairing.allowlist", params: { channel: ch } }).catch(() => null),
      ])
    );

    const allPending: PairingRequest[] = [];
    const allAllow: AllowlistEntry[] = [];

    channels.forEach((ch, i) => {
      const pendingResp = results[i * 2];
      const allowResp = results[i * 2 + 1];
      if (pendingResp?.ok && pendingResp.result) {
        const items = (pendingResp.result as { pending: PairingRequest[] }).pending ?? [];
        allPending.push(...items.map((p) => ({ ...p, channel: ch })));
      }
      if (allowResp?.ok && allowResp.result) {
        const senders = (allowResp.result as { allowlist: string[] }).allowlist ?? [];
        allAllow.push(...senders.map((s) => ({ channel: ch, sender_id: s })));
      }
    });

    setPending(allPending);
    setAllowEntries(allAllow);
    setFetchedAt(Date.now());
    setLoading(false);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, 15_000);
    return () => clearInterval(iv);
  }, [fetchData]);

  const handleApprove = async (channel: string, code: string) => {
    setApprovingCode(code);
    const resp = await debugInvoke.mutateAsync({ method: "dm.pairing.approve", params: { channel, code } }).catch(() => null);
    setApprovingCode(null);
    if (resp?.ok && (resp.result as { approved?: boolean })?.approved) {
      toast({ variant: "success", title: t("dmPairing.approved") });
      fetchData();
    } else {
      const err = (resp?.result as { error?: string })?.error ?? "Unknown error";
      toast({ variant: "error", title: `${t("dmPairing.approveFailed")}: ${err}` });
    }
  };

  const handleRemove = async (channel: string, senderId: string) => {
    const key = `${channel}:${senderId}`;
    setRemovingKey(key);
    const resp = await debugInvoke.mutateAsync({ method: "dm.pairing.remove", params: { channel, sender_id: senderId } }).catch(() => null);
    setRemovingKey(null);
    if (resp?.ok && (resp.result as { removed?: boolean })?.removed) {
      toast({ variant: "success", title: t("dmPairing.removed") });
      fetchData();
    } else {
      toast({ variant: "error", title: t("dmPairing.removeFailed") });
    }
  };

  const isExpired = (req: PairingRequest) => fetchedAt > req.created_at + req.ttl_ms;

  const ChannelBadge = ({ channel }: { channel: string }) => (
    <span className="px-1.5 py-0.5 rounded bg-[var(--accent)]/10 text-[var(--accent)] text-[10px] font-medium border border-[var(--accent)]/20">
      {channel}
    </span>
  );

  return (
    <SectionCard>
      <SectionHeader
        icon={<ShieldCheck className="h-4 w-4" />}
        title={t("dmPairing.title")}
        right={
          <div className="flex items-center gap-2">
            {pending.length > 0 && (
              <span className="px-1.5 py-0.5 rounded-full bg-[var(--warning)]/15 text-[var(--warning)] text-[10px] font-mono tabular-nums border border-[var(--warning)]/25">
                {pending.length} {t("dmPairing.pendingCount")}
              </span>
            )}
            {allowEntries.length > 0 && (
              <span className="px-1.5 py-0.5 rounded-full bg-[var(--success)]/15 text-[var(--success)] text-[10px] font-mono tabular-nums border border-[var(--success)]/25">
                {allowEntries.length} {t("dmPairing.approvedCount")}
              </span>
            )}
          </div>
        }
      />

      {loading ? (
        <div className="flex gap-2 flex-wrap px-0.5">
          <LoadingSkeleton className="h-16 w-full" />
          <LoadingSkeleton className="h-16 w-full" />
        </div>
      ) : (
        <div className="space-y-4">
          {/* Pending requests */}
          {pending.length > 0 && (
            <div className="space-y-2">
              <div className="text-[11px] font-semibold text-[var(--text-secondary)] uppercase tracking-[0.05em]">
                {t("dmPairing.pendingRequests")}
              </div>
              {pending.map((req) => {
                const expired = isExpired(req);
                return (
                  <div
                    key={`${req.channel}:${req.code}`}
                    className={cn(
                      "px-3 py-2.5 rounded-[var(--radius-md)] border transition-all",
                      expired
                        ? "bg-[var(--bg-content)]/30 border-[var(--border-subtle)]/50 opacity-60"
                        : "bg-[var(--warning)]/5 border-[var(--warning)]/20"
                    )}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <ChannelBadge channel={req.channel} />
                          <span className="text-[13px] font-mono font-semibold text-[var(--text-primary)] tracking-wider">
                            {req.code}
                          </span>
                          {expired && (
                            <span className="px-1.5 py-0.5 rounded-full bg-[var(--error)]/10 text-[var(--error)] text-[9px] font-medium border border-[var(--error)]/20">
                              {t("dmPairing.expired")}
                            </span>
                          )}
                        </div>
                        <div className="text-[11px] text-[var(--text-tertiary)] mt-0.5 truncate">
                          {req.sender_id}
                        </div>
                      </div>
                      {!expired && (
                        <button
                          onClick={() => handleApprove(req.channel, req.code)}
                          disabled={approvingCode === req.code}
                          className={cn(
                            "flex items-center gap-1 px-2.5 py-1.5 rounded-[var(--radius-sm)] text-[11px] font-medium transition-all cursor-pointer",
                            "bg-[var(--success)] text-white hover:brightness-110 active:scale-[0.97] [text-shadow:0_1px_1px_rgba(0,0,0,0.2)]",
                            approvingCode === req.code && "opacity-60 cursor-not-allowed"
                          )}
                        >
                          {approvingCode === req.code ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <Check className="h-3 w-3" />
                          )}
                          {t("dmPairing.approve")}
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {/* Allowlist */}
          <div className="space-y-2">
            <div className="text-[11px] font-semibold text-[var(--text-secondary)] uppercase tracking-[0.05em]">
              {t("dmPairing.allowlist")}
            </div>
            {allowEntries.length === 0 ? (
              <EmptyState
                icon={<UserCheck className="h-5 w-5" />}
                message={t("dmPairing.noApproved")}
              />
            ) : (
              <div className="space-y-1.5">
                {allowEntries.map((entry) => {
                  const key = `${entry.channel}:${entry.sender_id}`;
                  return (
                    <div
                      key={key}
                      className="flex items-center justify-between px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-content)]/60 border border-[var(--border-subtle)] hover:border-[var(--separator)] transition-all group"
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        <UserCheck className="h-3.5 w-3.5 text-[var(--success)] flex-shrink-0" />
                        <ChannelBadge channel={entry.channel} />
                        <span className="text-[12px] font-mono text-[var(--text-primary)] truncate">
                          {entry.sender_id}
                        </span>
                      </div>
                      <button
                        onClick={() => handleRemove(entry.channel, entry.sender_id)}
                        disabled={removingKey === key}
                        className={cn(
                          "flex items-center gap-1 px-2 py-1 rounded-[var(--radius-sm)] text-[10px] font-medium transition-all cursor-pointer",
                          "text-[var(--error)] hover:bg-[var(--error)]/10 opacity-0 group-hover:opacity-100",
                          removingKey === key && "opacity-60 cursor-not-allowed"
                        )}
                      >
                        {removingKey === key ? (
                          <Loader2 className="h-3 w-3 animate-spin" />
                        ) : (
                          <UserX className="h-3 w-3" />
                        )}
                        {t("dmPairing.remove")}
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Empty state when no pending and no allowlist */}
          {pending.length === 0 && allowEntries.length === 0 && (
            <EmptyState
              icon={<ShieldCheck className="h-5 w-5" />}
              message={t("dmPairing.empty")}
            />
          )}
        </div>
      )}
    </SectionCard>
  );
}
