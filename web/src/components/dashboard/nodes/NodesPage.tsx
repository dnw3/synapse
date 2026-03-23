import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Shield, Copy, KeyRound, X } from "lucide-react";
import { SectionCard, SectionHeader, LoadingSkeleton } from "../shared";
import { useToast } from "../../ui/toast";
import { cn } from "../../../lib/cn";
import NodesQrPairing from "./NodesQrPairing";
import NodesPendingRequests from "./NodesPendingRequests";
import NodesList from "./NodesList";

interface PairedNode {
  id: string;
  name?: string;
  platform?: string;
  status: "online" | "offline";
  paired_at?: string;
  last_seen?: string;
  device_id?: string;
  token_status?: "active" | "revoked" | "none";
  connected_at?: number;
  capabilities?: string[];
}

interface PendingRequest {
  id: string;
  node_name?: string;
  platform?: string;
  ip?: string;
  requested_at?: string;
}

interface ExecApprovalConfig {
  security_mode: string;
  ask_policy: string;
  allowlist: string[];
}

interface QrData {
  qr_svg: string;
  setup_code: string;
  gateway_url: string;
  bootstrap_token: string;
  ttl_ms: number;
}

export default function NodesPage() {
  const { t } = useTranslation();
  const { toast } = useToast();
  const [nodes, setNodes] = useState<PairedNode[]>([]);
  const [pending, setPending] = useState<PendingRequest[]>([]);
  const [approvalConfig, setApprovalConfig] = useState<ExecApprovalConfig>({
    security_mode: "strict",
    ask_policy: "always",
    allowlist: [],
  });
  const [loading, setLoading] = useState(true);
  const [qrData, setQrData] = useState<QrData | null>(null);
  const [qrLoading, setQrLoading] = useState(false);
  const [qrExpiry, setQrExpiry] = useState(0);
  const qrTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Rename state
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  // Token rotate result
  const [rotatedToken, setRotatedToken] = useState<{
    nodeId: string;
    token: string;
  } | null>(null);

  const loadData = useCallback(
    async (quiet = false) => {
      if (!quiet) setLoading(true);
      try {
        const [nodesRes, configRes] = await Promise.allSettled([
          fetch("/api/dashboard/nodes").then((r) => (r.ok ? r.json() : null)),
          fetch("/api/dashboard/exec-approvals").then((r) =>
            r.ok ? r.json() : null,
          ),
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
        // silently fail
      }
      if (!quiet) setLoading(false);
    },
    [],
  );

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadData();
  }, [loadData]);

  // Auto-refresh every 15s (like OpenClaw event-based refresh)
  useEffect(() => {
    const interval = setInterval(() => loadData(true), 15000);
    return () => clearInterval(interval);
  }, [loadData]);

  // QR expiry countdown — only (re)start the interval when expiry transitions to active
  const qrActive = qrExpiry > 0;
  useEffect(() => {
    if (!qrActive) {
      if (qrTimerRef.current) clearInterval(qrTimerRef.current);
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setQrData(null);
      return;
    }
    qrTimerRef.current = setInterval(() => {
      setQrExpiry((prev) => (prev <= 1 ? 0 : prev - 1));
    }, 1000);
    return () => {
      if (qrTimerRef.current) clearInterval(qrTimerRef.current);
    };
  }, [qrActive]);

  const handleGenerateQr = async () => {
    setQrLoading(true);
    try {
      const res = await fetch("/api/dashboard/nodes/qr", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      if (res.ok) {
        const data = await res.json();
        setQrData(data);
        setQrExpiry(Math.floor((data.ttl_ms ?? 600000) / 1000));
        toast({ variant: "success", title: t("nodes.qrGenerated") });
      } else {
        toast({ variant: "error", title: t("nodes.qrFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.qrFailed") });
    }
    setQrLoading(false);
  };

  const handleCopySetupCode = () => {
    if (qrData?.setup_code) {
      navigator.clipboard.writeText(qrData.setup_code);
      toast({ variant: "success", title: t("nodes.setupCodeCopied") });
    }
  };

  const handleApprove = async (requestId: string) => {
    try {
      const res = await fetch(`/api/dashboard/nodes/approve`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ request_id: requestId }),
      });
      if (res.ok) {
        toast({ variant: "success", title: t("nodes.approved") });
        loadData(true);
      } else {
        toast({ variant: "error", title: t("nodes.approveFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.approveFailed") });
    }
  };

  const handleReject = async (requestId: string) => {
    if (!confirm(t("nodes.confirmReject"))) return;
    try {
      const res = await fetch(`/api/dashboard/nodes/reject`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ request_id: requestId }),
      });
      if (res.ok) {
        toast({ variant: "success", title: t("nodes.rejected") });
        loadData(true);
      } else {
        toast({ variant: "error", title: t("nodes.rejectFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.rejectFailed") });
    }
  };

  const handleRemove = async (nodeId: string) => {
    if (!confirm(t("nodes.confirmRemove"))) return;
    try {
      const res = await fetch(`/api/dashboard/nodes/remove`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ node_id: nodeId }),
      });
      if (res.ok) {
        toast({ variant: "success", title: t("nodes.removed") });
        loadData(true);
      } else {
        toast({ variant: "error", title: t("nodes.removeFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.removeFailed") });
    }
  };

  const handleRename = async (nodeId: string) => {
    if (!renameValue.trim()) return;
    try {
      const res = await fetch(`/api/dashboard/nodes/rename`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ node_id: nodeId, name: renameValue.trim() }),
      });
      if (res.ok) {
        toast({ variant: "success", title: t("nodes.renamed") });
        setRenamingId(null);
        loadData(true);
      } else {
        toast({ variant: "error", title: t("nodes.renameFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.renameFailed") });
    }
  };

  const handleRotate = async (nodeId: string) => {
    if (!confirm(t("nodes.confirmRotate"))) return;
    try {
      const res = await fetch(`/api/dashboard/nodes/rotate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ node_id: nodeId }),
      });
      if (res.ok) {
        const data = await res.json();
        setRotatedToken({ nodeId, token: data.token });
        toast({ variant: "success", title: t("nodes.tokenRotated") });
        loadData(true);
      } else {
        toast({ variant: "error", title: t("nodes.rotateFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.rotateFailed") });
    }
  };

  const handleRevoke = async (nodeId: string) => {
    if (!confirm(t("nodes.confirmRevoke"))) return;
    try {
      const res = await fetch(`/api/dashboard/nodes/revoke`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ node_id: nodeId }),
      });
      if (res.ok) {
        toast({ variant: "success", title: t("nodes.tokenRevoked") });
        loadData(true);
      } else {
        toast({ variant: "error", title: t("nodes.revokeFailed") });
      }
    } catch {
      toast({ variant: "error", title: t("nodes.revokeFailed") });
    }
  };

  const handleCopyToken = (token: string) => {
    navigator.clipboard.writeText(token);
    toast({ variant: "success", title: t("nodes.tokenCopied") });
  };

  if (loading) {
    return <LoadingSkeleton />;
  }

  return (
    <div className="flex flex-col gap-6">
      {/* Rotated token banner */}
      {rotatedToken && (
        <div className="flex items-center gap-3 p-3 rounded-[var(--radius-md)] bg-[var(--warning)]/10 border border-[var(--warning)]/30">
          <KeyRound className="h-4 w-4 text-[var(--warning)] flex-shrink-0" />
          <div className="flex-1 min-w-0">
            <span className="text-[12px] font-medium text-[var(--text-primary)]">
              {t("nodes.newTokenFor", {
                node:
                  nodes.find((n) => n.id === rotatedToken.nodeId)?.name ||
                  rotatedToken.nodeId.slice(0, 12),
              })}
            </span>
            <code className="block mt-1 px-2 py-1 rounded-[var(--radius-sm)] bg-[var(--bg-secondary)] text-[11px] font-mono text-[var(--text-secondary)] truncate select-all">
              {rotatedToken.token}
            </code>
          </div>
          <button
            onClick={() => handleCopyToken(rotatedToken.token)}
            className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
          >
            <Copy className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={() => setRotatedToken(null)}
            className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      {/* QR Code Pairing */}
      <NodesQrPairing
        qrData={qrData}
        qrLoading={qrLoading}
        qrExpiry={qrExpiry}
        onGenerateQr={handleGenerateQr}
        onCopySetupCode={handleCopySetupCode}
      />

      {/* Pending Requests (shown prominently when present) */}
      <NodesPendingRequests
        pending={pending}
        onApprove={handleApprove}
        onReject={handleReject}
      />

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Left Panel: Paired Nodes */}
        <NodesList
          nodes={nodes}
          renamingId={renamingId}
          renameValue={renameValue}
          rotatedToken={rotatedToken}
          onRefresh={() => loadData()}
          onStartRename={(nodeId, currentName) => {
            setRenamingId(nodeId);
            setRenameValue(currentName);
          }}
          onRenameChange={setRenameValue}
          onRenameCommit={handleRename}
          onRenameCancel={() => setRenamingId(null)}
          onRotate={handleRotate}
          onRevoke={handleRevoke}
          onRemove={handleRemove}
        />

        {/* Right Panel: Exec Approvals */}
        <SectionCard>
          <SectionHeader
            icon={<Shield className="h-4 w-4" />}
            title={t("nodes.execApprovals")}
          />

          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between py-2 border-b border-[var(--border-subtle)]">
              <span className="text-[13px] text-[var(--text-secondary)]">
                {t("nodes.securityMode")}
              </span>
              <span
                className={cn(
                  "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium border",
                  approvalConfig.security_mode === "strict"
                    ? "bg-[var(--error)]/10 text-[var(--error)] border-[var(--error)]/30"
                    : "bg-[var(--success)]/10 text-[var(--success)] border-[var(--success)]/30",
                )}
              >
                <Shield className="h-3 w-3" />
                {approvalConfig.security_mode}
              </span>
            </div>

            <div className="flex items-center justify-between py-2 border-b border-[var(--border-subtle)]">
              <span className="text-[13px] text-[var(--text-secondary)]">
                {t("nodes.askPolicy")}
              </span>
              <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium bg-[var(--accent)]/10 text-[var(--accent)] border border-[var(--accent)]/30">
                {approvalConfig.ask_policy}
              </span>
            </div>

            <div className="py-2">
              <span className="text-[13px] text-[var(--text-secondary)] block mb-2">
                {t("nodes.allowlist")}
              </span>
              {approvalConfig.allowlist.length === 0 ? (
                <span className="text-[11px] text-[var(--text-tertiary)] italic">
                  *
                </span>
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
      </div>
    </div>
  );
}
