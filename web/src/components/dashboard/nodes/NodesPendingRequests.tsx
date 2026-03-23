import { useTranslation } from "react-i18next";
import { AlertTriangle, Laptop, CheckCircle, XCircle } from "lucide-react";
import { SectionCard, SectionHeader } from "../shared";

interface PendingRequest {
  id: string;
  node_name?: string;
  platform?: string;
  ip?: string;
  requested_at?: string;
}

interface NodesPendingRequestsProps {
  pending: PendingRequest[];
  onApprove: (requestId: string) => void;
  onReject: (requestId: string) => void;
}

function relativeTime(isoOrMs?: string | number): string {
  if (!isoOrMs) return "";
  const ts =
    typeof isoOrMs === "number"
      ? isoOrMs
      : Number(isoOrMs) || new Date(isoOrMs).getTime();
  const diff = Date.now() - ts;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

export default function NodesPendingRequests({
  pending,
  onApprove,
  onReject,
}: NodesPendingRequestsProps) {
  const { t } = useTranslation();

  if (pending.length === 0) return null;

  return (
    <SectionCard>
      <SectionHeader
        icon={<AlertTriangle className="h-4 w-4" />}
        title={t("nodes.pendingRequests")}
      />
      <div className="mt-3 space-y-2">
        {pending.map((req) => (
          <div
            key={req.id}
            className="flex items-center justify-between p-3 rounded-[var(--radius-md)] bg-[var(--bg-primary)] border border-[var(--warning)]/30 hover:border-[var(--warning)]/50 transition-colors"
          >
            <div className="flex items-center gap-3">
              <div className="flex items-center justify-center w-8 h-8 rounded-[var(--radius-md)] bg-[var(--warning)]/10">
                <Laptop className="h-4 w-4 text-[var(--warning)]" />
              </div>
              <div className="flex flex-col">
                <span className="text-[13px] font-semibold text-[var(--text-primary)]">
                  {req.node_name || req.id.slice(0, 12)}
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
                  {req.id.slice(0, 12)}
                  {req.ip && ` · ${req.ip}`}
                </span>
                {req.requested_at && (
                  <span className="text-[11px] text-[var(--text-tertiary)]">
                    {t("nodes.requestedAgo", {
                      time: relativeTime(req.requested_at),
                    })}
                  </span>
                )}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => onApprove(req.id)}
                className="inline-flex items-center gap-1 px-2.5 py-1 rounded-[var(--radius-md)] text-[11px] font-medium bg-[var(--success)]/10 text-[var(--success)] border border-[var(--success)]/30 hover:bg-[var(--success)]/20 transition-colors"
              >
                <CheckCircle className="h-3 w-3" />
                {t("nodes.approve")}
              </button>
              <button
                onClick={() => onReject(req.id)}
                className="inline-flex items-center gap-1 px-2.5 py-1 rounded-[var(--radius-md)] text-[11px] font-medium bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 hover:bg-[var(--error)]/20 transition-colors"
              >
                <XCircle className="h-3 w-3" />
                {t("nodes.reject")}
              </button>
            </div>
          </div>
        ))}
      </div>
    </SectionCard>
  );
}
