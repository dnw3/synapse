import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Network, Shield, CheckCircle, XCircle, RefreshCw, Clock, Laptop, AlertTriangle } from "lucide-react";
import { useDashboardAPI } from "../../hooks/useDashboardAPI";
import {
  SectionCard,
  SectionHeader,
  EmptyState,
  LoadingSkeleton,
  StatusDot,
  useToast,
  ToastContainer,
} from "./shared";
import { cn } from "../../lib/cn";

interface PairedNode {
  id: string;
  name?: string;
  platform?: string;
  status: "online" | "offline";
  paired_at?: string;
  last_seen?: string;
}

interface PendingRequest {
  id: string;
  node_name?: string;
  platform?: string;
  requested_at?: string;
}

interface ExecApprovalConfig {
  security_mode: string;
  ask_policy: string;
  allowlist: string[];
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

export default function NodesPage() {
  const { t } = useTranslation();
  const api = useDashboardAPI();
  const { toasts, addToast } = useToast();
  const [nodes, setNodes] = useState<PairedNode[]>([]);
  const [pending, setPending] = useState<PendingRequest[]>([]);
  const [approvalConfig, setApprovalConfig] = useState<ExecApprovalConfig>({
    security_mode: "strict",
    ask_policy: "always",
    allowlist: [],
  });
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      // Fetch nodes and approval config via REST fallback
      const [nodesRes, configRes] = await Promise.allSettled([
        fetch("/api/dashboard/nodes").then((r) => r.ok ? r.json() : null),
        fetch("/api/dashboard/exec-approvals").then((r) => r.ok ? r.json() : null),
      ]);

      if (nodesRes.status === "fulfilled" && nodesRes.value) {
        setNodes(nodesRes.value.nodes ?? []);
        setPending(nodesRes.value.pending ?? []);
      }
      if (configRes.status === "fulfilled" && configRes.value) {
        setApprovalConfig({
          security_mode: configRes.value.security_mode ?? "strict",
          ask_policy: configRes.value.ask_policy ?? "always",
          allowlist: configRes.value.allowlist ?? [],
        });
      }
    } catch {
      // silently fail — pages may not have backend support yet
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleApprove = async (requestId: string) => {
    try {
      const res = await fetch(`/api/dashboard/nodes/approve`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ request_id: requestId }),
      });
      if (res.ok) {
        addToast(t("nodes.approve"), "success");
        loadData();
      } else {
        addToast(t("nodes.approve") + " failed", "error");
      }
    } catch {
      addToast(t("nodes.approve") + " failed", "error");
    }
  };

  const handleReject = async (requestId: string) => {
    try {
      const res = await fetch(`/api/dashboard/nodes/reject`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ request_id: requestId }),
      });
      if (res.ok) {
        addToast(t("nodes.reject"), "success");
        loadData();
      } else {
        addToast(t("nodes.reject") + " failed", "error");
      }
    } catch {
      addToast(t("nodes.reject") + " failed", "error");
    }
  };

  if (loading) {
    return <LoadingSkeleton />;
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Left Panel: Exec Approvals */}
        <SectionCard>
          <SectionHeader
            icon={<Shield className="h-4 w-4" />}
            title={t("nodes.execApprovals")}
          />

          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between py-2 border-b border-[var(--border-subtle)]">
              <span className="text-[13px] text-[var(--text-secondary)]">{t("nodes.securityMode")}</span>
              <span className={cn(
                "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium border",
                approvalConfig.security_mode === "strict"
                  ? "bg-[var(--error)]/10 text-[var(--error)] border-[var(--error)]/30"
                  : "bg-[var(--success)]/10 text-[var(--success)] border-[var(--success)]/30"
              )}>
                <Shield className="h-3 w-3" />
                {approvalConfig.security_mode}
              </span>
            </div>

            <div className="flex items-center justify-between py-2 border-b border-[var(--border-subtle)]">
              <span className="text-[13px] text-[var(--text-secondary)]">{t("nodes.askPolicy")}</span>
              <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[var(--accent)]/10 text-[var(--accent)] border border-[var(--accent)]/30">
                {approvalConfig.ask_policy}
              </span>
            </div>

            <div className="py-2">
              <span className="text-[13px] text-[var(--text-secondary)] block mb-2">{t("nodes.allowlist")}</span>
              {approvalConfig.allowlist.length === 0 ? (
                <span className="text-[11px] text-[var(--text-tertiary)] italic">*</span>
              ) : (
                <div className="flex flex-wrap gap-1.5">
                  {approvalConfig.allowlist.map((pattern, i) => (
                    <span
                      key={i}
                      className="px-2 py-0.5 rounded-[var(--radius-sm)] text-[11px] font-mono bg-[var(--bg-secondary)] text-[var(--text-secondary)] border border-[var(--border-subtle)]"
                    >
                      {pattern}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>
        </SectionCard>

        {/* Right Panel: Paired Nodes & Pending Requests */}
        <div className="flex flex-col gap-4">
          {/* Paired Nodes */}
          <SectionCard>
            <SectionHeader
              icon={<Network className="h-4 w-4" />}
              title={t("nodes.pairedNodes")}
              right={
                <button
                  onClick={loadData}
                  className="p-1.5 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <RefreshCw className="h-3.5 w-3.5" />
                </button>
              }
            />

            {nodes.length === 0 ? (
              <EmptyState
                icon={<Network className="h-8 w-8" />}
                message={t("nodes.empty")}
              />
            ) : (
              <div className="mt-3 space-y-2">
                {nodes.map((node) => (
                  <div
                    key={node.id}
                    className="flex items-center justify-between p-3 rounded-[var(--radius-md)] bg-[var(--bg-primary)] border border-[var(--border-subtle)] hover:border-[var(--separator)] transition-colors"
                  >
                    <div className="flex items-center gap-3">
                      <div
                        className="flex items-center justify-center w-8 h-8 rounded-[var(--radius-md)]"
                        style={{ background: "color-mix(in srgb, var(--accent) 12%, transparent)" }}
                      >
                        <Laptop className="h-4 w-4" style={{ color: "var(--accent-light)" }} />
                      </div>
                      <div className="flex flex-col">
                        <span className="text-[13px] font-semibold text-[var(--text-primary)]">
                          {node.name || node.id.slice(0, 12)}
                        </span>
                        {node.platform && (
                          <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
                            {node.platform}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {node.last_seen && (
                        <span className="text-[11px] text-[var(--text-tertiary)] inline-flex items-center gap-1">
                          <Clock className="h-3 w-3" />
                          {relativeTime(node.last_seen)}
                        </span>
                      )}
                      <StatusDot status={node.status === "online" ? "online" : "offline"} />
                    </div>
                  </div>
                ))}
              </div>
            )}
          </SectionCard>

          {/* Pending Requests */}
          {pending.length > 0 && (
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
                        {req.requested_at && (
                          <span className="text-[11px] text-[var(--text-tertiary)]">
                            {relativeTime(req.requested_at)}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => handleApprove(req.id)}
                        className="inline-flex items-center gap-1 px-2.5 py-1 rounded-[var(--radius-md)] text-[11px] font-medium bg-[var(--success)]/10 text-[var(--success)] border border-[var(--success)]/30 hover:bg-[var(--success)]/20 transition-colors"
                      >
                        <CheckCircle className="h-3 w-3" />
                        {t("nodes.approve")}
                      </button>
                      <button
                        onClick={() => handleReject(req.id)}
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
          )}
        </div>
      </div>

      <ToastContainer toasts={toasts} />
    </div>
  );
}
