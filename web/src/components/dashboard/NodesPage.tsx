import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Network,
  Shield,
  CheckCircle,
  XCircle,
  RefreshCw,
  Clock,
  Laptop,
  AlertTriangle,
  QrCode,
  Copy,
  Trash2,
  RotateCw,
  KeyRound,
  Pencil,
  Ban,
  Check,
  X,
} from "lucide-react";
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

export default function NodesPage() {
  const { t } = useTranslation();
  const { toasts, addToast } = useToast();
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
    loadData();
  }, [loadData]);

  // Auto-refresh every 15s (like OpenClaw event-based refresh)
  useEffect(() => {
    const interval = setInterval(() => loadData(true), 15000);
    return () => clearInterval(interval);
  }, [loadData]);

  // QR expiry countdown
  useEffect(() => {
    if (qrExpiry <= 0) {
      if (qrTimerRef.current) clearInterval(qrTimerRef.current);
      if (qrData) setQrData(null);
      return;
    }
    qrTimerRef.current = setInterval(() => {
      setQrExpiry((prev) => {
        if (prev <= 1) {
          setQrData(null);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => {
      if (qrTimerRef.current) clearInterval(qrTimerRef.current);
    };
  }, [qrExpiry > 0]);

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
        addToast(t("nodes.qrGenerated"), "success");
      } else {
        addToast(t("nodes.qrFailed"), "error");
      }
    } catch {
      addToast(t("nodes.qrFailed"), "error");
    }
    setQrLoading(false);
  };

  const handleCopySetupCode = () => {
    if (qrData?.setup_code) {
      navigator.clipboard.writeText(qrData.setup_code);
      addToast(t("nodes.setupCodeCopied"), "success");
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
        addToast(t("nodes.approved"), "success");
        loadData(true);
      } else {
        addToast(t("nodes.approveFailed"), "error");
      }
    } catch {
      addToast(t("nodes.approveFailed"), "error");
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
        addToast(t("nodes.rejected"), "success");
        loadData(true);
      } else {
        addToast(t("nodes.rejectFailed"), "error");
      }
    } catch {
      addToast(t("nodes.rejectFailed"), "error");
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
        addToast(t("nodes.removed"), "success");
        loadData(true);
      } else {
        addToast(t("nodes.removeFailed"), "error");
      }
    } catch {
      addToast(t("nodes.removeFailed"), "error");
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
        addToast(t("nodes.renamed"), "success");
        setRenamingId(null);
        loadData(true);
      } else {
        addToast(t("nodes.renameFailed"), "error");
      }
    } catch {
      addToast(t("nodes.renameFailed"), "error");
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
        addToast(t("nodes.tokenRotated"), "success");
        loadData(true);
      } else {
        addToast(t("nodes.rotateFailed"), "error");
      }
    } catch {
      addToast(t("nodes.rotateFailed"), "error");
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
        addToast(t("nodes.tokenRevoked"), "success");
        loadData(true);
      } else {
        addToast(t("nodes.revokeFailed"), "error");
      }
    } catch {
      addToast(t("nodes.revokeFailed"), "error");
    }
  };

  const handleCopyToken = (token: string) => {
    navigator.clipboard.writeText(token);
    addToast(t("nodes.tokenCopied"), "success");
  };

  if (loading) {
    return <LoadingSkeleton />;
  }

  const formatExpiry = (secs: number) => {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Rotated token banner */}
      {rotatedToken && (
        <div className="flex items-center gap-3 p-3 rounded-[var(--radius-md)] bg-[var(--warning)]/10 border border-[var(--warning)]/30">
          <KeyRound className="h-4 w-4 text-[var(--warning)] flex-shrink-0" />
          <div className="flex-1 min-w-0">
            <span className="text-[12px] font-medium text-[var(--text-primary)]">
              {t("nodes.newTokenFor", {
                node: nodes.find((n) => n.id === rotatedToken.nodeId)?.name || rotatedToken.nodeId.slice(0, 12),
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
      <SectionCard>
        <SectionHeader
          icon={<QrCode className="h-4 w-4" />}
          title={t("nodes.devicePairing")}
          right={
            <button
              onClick={handleGenerateQr}
              disabled={qrLoading}
              className={cn(
                "inline-flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] text-[12px] font-medium transition-colors",
                "bg-[var(--accent)] text-white hover:opacity-90",
                qrLoading && "opacity-50 cursor-not-allowed",
              )}
            >
              {qrLoading ? (
                <RotateCw className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <QrCode className="h-3.5 w-3.5" />
              )}
              {qrData ? t("nodes.regenerateQr") : t("nodes.generateQr")}
            </button>
          }
        />

        {qrData ? (
          <div className="mt-4 flex flex-col sm:flex-row items-start gap-6">
            {/* QR Code SVG */}
            <div className="flex-shrink-0 p-4 rounded-[var(--radius-lg)] bg-white border border-[var(--border-subtle)]">
              <div
                className="w-[200px] h-[200px]"
                dangerouslySetInnerHTML={{ __html: qrData.qr_svg }}
              />
            </div>

            {/* Pairing Info */}
            <div className="flex flex-col gap-3 flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <Clock className="h-3.5 w-3.5 text-[var(--text-tertiary)]" />
                <span
                  className={cn(
                    "text-[12px] font-mono font-medium",
                    qrExpiry < 60
                      ? "text-[var(--error)]"
                      : "text-[var(--text-secondary)]",
                  )}
                >
                  {t("nodes.expiresIn", { time: formatExpiry(qrExpiry) })}
                </span>
              </div>

              <div className="space-y-2">
                <label className="text-[11px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider">
                  {t("nodes.setupCode")}
                </label>
                <div className="flex items-center gap-2">
                  <code className="flex-1 px-3 py-2 rounded-[var(--radius-md)] bg-[var(--bg-secondary)] border border-[var(--border-subtle)] text-[11px] font-mono text-[var(--text-secondary)] truncate select-all">
                    {qrData.setup_code}
                  </code>
                  <button
                    onClick={handleCopySetupCode}
                    className="p-2 rounded-[var(--radius-md)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    <Copy className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-[11px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider">
                  {t("nodes.gatewayUrl")}
                </label>
                <span className="text-[12px] font-mono text-[var(--text-secondary)]">
                  {qrData.gateway_url}
                </span>
              </div>

              <p className="text-[11px] text-[var(--text-tertiary)] leading-relaxed mt-1">
                {t("nodes.qrHint")}
              </p>
            </div>
          </div>
        ) : (
          <div className="mt-4 flex flex-col items-center justify-center py-8 text-center">
            <QrCode className="h-10 w-10 text-[var(--text-tertiary)] opacity-40 mb-3" />
            <p className="text-[13px] text-[var(--text-secondary)]">
              {t("nodes.qrDescription")}
            </p>
          </div>
        )}
      </SectionCard>

      {/* Pending Requests (shown prominently when present) */}
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

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Left Panel: Paired Nodes */}
        <SectionCard>
          <SectionHeader
            icon={<Network className="h-4 w-4" />}
            title={t("nodes.pairedNodes")}
            right={
              <button
                onClick={() => loadData()}
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
                  className="p-3 rounded-[var(--radius-md)] bg-[var(--bg-primary)] border border-[var(--border-subtle)] hover:border-[var(--separator)] transition-colors"
                >
                  {/* Header row */}
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div
                        className="flex items-center justify-center w-8 h-8 rounded-[var(--radius-md)]"
                        style={{
                          background:
                            "color-mix(in srgb, var(--accent) 12%, transparent)",
                        }}
                      >
                        <Laptop
                          className="h-4 w-4"
                          style={{ color: "var(--accent-light)" }}
                        />
                      </div>
                      <div className="flex flex-col">
                        {renamingId === node.id ? (
                          <div className="flex items-center gap-1.5">
                            <input
                              type="text"
                              value={renameValue}
                              onChange={(e) => setRenameValue(e.target.value)}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") handleRename(node.id);
                                if (e.key === "Escape") setRenamingId(null);
                              }}
                              className="px-2 py-0.5 rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-[var(--bg-secondary)] text-[13px] text-[var(--text-primary)] w-32 focus:outline-none focus:border-[var(--accent)]"
                              autoFocus
                            />
                            <button
                              onClick={() => handleRename(node.id)}
                              className="p-0.5 text-[var(--success)] hover:bg-[var(--success)]/10 rounded"
                            >
                              <Check className="h-3.5 w-3.5" />
                            </button>
                            <button
                              onClick={() => setRenamingId(null)}
                              className="p-0.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] rounded"
                            >
                              <X className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        ) : (
                          <div className="flex items-center gap-1.5">
                            <span className="text-[13px] font-semibold text-[var(--text-primary)]">
                              {node.name || node.id.slice(0, 12)}
                            </span>
                            <button
                              onClick={() => {
                                setRenamingId(node.id);
                                setRenameValue(node.name || "");
                              }}
                              className="p-0.5 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] opacity-0 group-hover:opacity-100 transition-opacity"
                              title={t("nodes.rename")}
                            >
                              <Pencil className="h-3 w-3" />
                            </button>
                          </div>
                        )}
                        <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
                          {node.platform && `${node.platform} · `}
                          {node.id.slice(0, 16)}
                        </span>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {node.connected_at && (
                        <span className="text-[11px] text-[var(--text-tertiary)] inline-flex items-center gap-1">
                          <Clock className="h-3 w-3" />
                          {relativeTime(node.connected_at)}
                        </span>
                      )}
                      <StatusDot
                        status={
                          node.status === "online" ? "online" : "offline"
                        }
                      />
                    </div>
                  </div>

                  {/* Capabilities chips */}
                  {node.capabilities && node.capabilities.length > 0 && (
                    <div className="flex flex-wrap gap-1 mt-2 ml-11">
                      {node.capabilities.slice(0, 8).map((cap) => (
                        <span
                          key={cap}
                          className="px-1.5 py-0.5 rounded-[var(--radius-sm)] text-[10px] font-mono bg-[var(--bg-secondary)] text-[var(--text-tertiary)] border border-[var(--border-subtle)]"
                        >
                          {cap}
                        </span>
                      ))}
                    </div>
                  )}

                  {/* Token management row */}
                  <div className="flex items-center justify-between mt-2 ml-11">
                    <div className="flex items-center gap-2">
                      <KeyRound className="h-3 w-3 text-[var(--text-tertiary)]" />
                      <span
                        className={cn(
                          "text-[11px] font-medium",
                          node.token_status === "active"
                            ? "text-[var(--success)]"
                            : node.token_status === "revoked"
                              ? "text-[var(--error)]"
                              : "text-[var(--text-tertiary)]",
                        )}
                      >
                        {t(`nodes.token_${node.token_status || "none"}`)}
                      </span>
                    </div>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => handleRotate(node.id)}
                        className="inline-flex items-center gap-1 px-2 py-0.5 rounded-[var(--radius-sm)] text-[10px] font-medium text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
                        title={t("nodes.rotateToken")}
                      >
                        <RotateCw className="h-3 w-3" />
                        {t("nodes.rotate")}
                      </button>
                      {node.token_status !== "revoked" && (
                        <button
                          onClick={() => handleRevoke(node.id)}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-[var(--radius-sm)] text-[10px] font-medium text-[var(--error)]/70 hover:text-[var(--error)] hover:bg-[var(--error)]/10 transition-colors"
                          title={t("nodes.revokeToken")}
                        >
                          <Ban className="h-3 w-3" />
                          {t("nodes.revoke")}
                        </button>
                      )}
                      <button
                        onClick={() => handleRemove(node.id)}
                        className="p-1 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/10 transition-colors"
                        title={t("nodes.remove")}
                      >
                        <Trash2 className="h-3 w-3" />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </SectionCard>

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

      <ToastContainer toasts={toasts} />
    </div>
  );
}
