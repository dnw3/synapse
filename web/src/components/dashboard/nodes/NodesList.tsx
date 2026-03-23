import { useTranslation } from "react-i18next";
import {
  Network,
  RefreshCw,
  Clock,
  Laptop,
  KeyRound,
  Pencil,
  Trash2,
  RotateCw,
  Ban,
  Check,
  X,
} from "lucide-react";
import { SectionCard, SectionHeader, EmptyState, StatusDot } from "../shared";
import { cn } from "../../../lib/cn";

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

interface NodesListProps {
  nodes: PairedNode[];
  renamingId: string | null;
  renameValue: string;
  rotatedToken: { nodeId: string; token: string } | null;
  onRefresh: () => void;
  onStartRename: (nodeId: string, currentName: string) => void;
  onRenameChange: (value: string) => void;
  onRenameCommit: (nodeId: string) => void;
  onRenameCancel: () => void;
  onRotate: (nodeId: string) => void;
  onRevoke: (nodeId: string) => void;
  onRemove: (nodeId: string) => void;
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

export default function NodesList({
  nodes,
  renamingId,
  renameValue,
  onRefresh,
  onStartRename,
  onRenameChange,
  onRenameCommit,
  onRenameCancel,
  onRotate,
  onRevoke,
  onRemove,
}: NodesListProps) {
  const { t } = useTranslation();

  return (
    <SectionCard>
      <SectionHeader
        icon={<Network className="h-4 w-4" />}
        title={t("nodes.pairedNodes")}
        right={
          <button
            onClick={onRefresh}
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
                          onChange={(e) => onRenameChange(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") onRenameCommit(node.id);
                            if (e.key === "Escape") onRenameCancel();
                          }}
                          className="px-2 py-0.5 rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-[var(--bg-secondary)] text-[13px] text-[var(--text-primary)] w-32 focus:outline-none focus:border-[var(--accent)]"
                          autoFocus
                        />
                        <button
                          onClick={() => onRenameCommit(node.id)}
                          className="p-0.5 text-[var(--success)] hover:bg-[var(--success)]/10 rounded"
                        >
                          <Check className="h-3.5 w-3.5" />
                        </button>
                        <button
                          onClick={onRenameCancel}
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
                          onClick={() =>
                            onStartRename(node.id, node.name || "")
                          }
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
                    status={node.status === "online" ? "online" : "offline"}
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
                    onClick={() => onRotate(node.id)}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded-[var(--radius-sm)] text-[10px] font-medium text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
                    title={t("nodes.rotateToken")}
                  >
                    <RotateCw className="h-3 w-3" />
                    {t("nodes.rotate")}
                  </button>
                  {node.token_status !== "revoked" && (
                    <button
                      onClick={() => onRevoke(node.id)}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-[var(--radius-sm)] text-[10px] font-medium text-[var(--error)]/70 hover:text-[var(--error)] hover:bg-[var(--error)]/10 transition-colors"
                      title={t("nodes.revokeToken")}
                    >
                      <Ban className="h-3 w-3" />
                      {t("nodes.revoke")}
                    </button>
                  )}
                  <button
                    onClick={() => onRemove(node.id)}
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
  );
}
