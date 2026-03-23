import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Box, RefreshCw, Shield, Trash2, Info } from "lucide-react";
import { cn } from "../../lib/cn";
import { useSandboxAPI } from "../../hooks/useSandbox";
import type { SandboxInstanceInfo, SandboxExplanation } from "../../hooks/useSandbox";
import {
  StatsCard, SectionCard, SectionHeader,
  EmptyState, LoadingSpinner, useInlineConfirm,
  useToast, ToastContainer, formatDate,
} from "./shared";

export default function SandboxPanel() {
  const { t, i18n } = useTranslation();
  const api = useSandboxAPI();
  const { toasts, addToast } = useToast();
  const { confirming, requestConfirm, reset: resetConfirm } = useInlineConfirm();

  const [instances, setInstances] = useState<SandboxInstanceInfo[] | null>(null);
  const [providers, setProviders] = useState<string[] | null>(null);
  const [explanation, setExplanation] = useState<SandboxExplanation | null>(null);
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    const [inst, prov, expl] = await Promise.all([
      api.listInstances(),
      api.listProviders(),
      api.explain(),
    ]);
    if (inst) setInstances(inst);
    if (prov) setProviders(prov);
    if (expl) setExplanation(expl);
    setLoading(false);
  }, [api]);

  // Initial load — runs once, avoids setState-in-effect lint rule
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const [inst, prov, expl] = await Promise.all([
        api.listInstances(),
        api.listProviders(),
        api.explain(),
      ]);
      if (cancelled) return;
      if (inst) setInstances(inst);
      if (prov) setProviders(prov);
      if (expl) setExplanation(expl);
      setLoading(false);
    })();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Auto-refresh every 30s
  useEffect(() => {
    const timer = setInterval(loadData, 30_000);
    return () => clearInterval(timer);
  }, [loadData]);

  const handleDestroy = async (runtimeId: string) => {
    if (confirming !== runtimeId) {
      requestConfirm(runtimeId);
      return;
    }
    resetConfirm();
    const ok = await api.destroy(runtimeId);
    if (ok) {
      addToast(t("sandbox.destroy") + " OK", "success");
      loadData();
    } else {
      addToast(t("sandbox.destroy") + " failed", "error");
    }
  };

  const handleRecreateAll = async () => {
    const count = instances?.length ?? 0;
    if (count === 0) return;
    if (confirming !== "recreate-all") {
      requestConfirm("recreate-all");
      return;
    }
    resetConfirm();
    const result = await api.recreate({ all: true });
    if (result) {
      addToast(`${t("sandbox.recreate_all")}: ${result.count}`, "success");
      loadData();
    } else {
      addToast(t("sandbox.recreate_all") + " failed", "error");
    }
  };

  if (loading) return <LoadingSpinner />;

  const instanceCount = instances?.length ?? 0;
  const providerCount = providers?.length ?? 0;

  return (
    <div className="space-y-6 animate-fade-slide-in">
      {/* Stats row */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <StatsCard
          icon={<Shield className="h-5 w-5" />}
          label={t("sandbox.mode")}
          value={explanation?.mode ?? "—"}
          accent="var(--chart-1)"
        />
        <StatsCard
          icon={<Box className="h-5 w-5" />}
          label={t("sandbox.active_instances")}
          value={String(instanceCount)}
          accent="var(--chart-2)"
        />
        <StatsCard
          icon={<Info className="h-5 w-5" />}
          label={t("sandbox.providers")}
          value={String(providerCount)}
          sub={providers?.join(", ")}
          accent="var(--chart-3)"
        />
      </div>

      {/* Config explanation */}
      {explanation && (
        <SectionCard>
          <SectionHeader
            icon={<Shield className="h-4 w-4" />}
            title={t("sandbox.config_explain")}
          />
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-[13px]">
            <Field label="Backend" value={explanation.backend} />
            <Field label="Scope" value={explanation.scope} />
            <Field label="Workspace" value={explanation.workspace_access} />
            <Field label="Sandboxed" value={explanation.is_sandboxed ? "Yes" : "No"} />
            {explanation.security && (
              <>
                <Field label="Network" value={explanation.security.network_mode} />
                <Field label="Read-only root" value={explanation.security.read_only_root ? "Yes" : "No"} />
                <Field label="CAP_DROP" value={explanation.security.cap_drop?.join(", ") || "—"} />
              </>
            )}
          </div>
        </SectionCard>
      )}

      {/* Instances table */}
      <SectionCard>
        <SectionHeader
          icon={<Box className="h-4 w-4" />}
          title={t("sandbox.active_instances")}
          right={
            <div className="flex items-center gap-2">
              <button
                onClick={handleRecreateAll}
                disabled={instanceCount === 0}
                className={cn(
                  "px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium transition-colors cursor-pointer",
                  confirming === "recreate-all"
                    ? "bg-[var(--error)]/10 text-[var(--error)]"
                    : "text-[var(--text-secondary)] bg-[var(--bg-grouped)] hover:bg-[var(--bg-elevated)]",
                  instanceCount === 0 && "opacity-30 cursor-not-allowed"
                )}
              >
                {confirming === "recreate-all" ? t("sandbox.confirm_recreate", { count: instanceCount }) : t("sandbox.recreate_all")}
              </button>
              <button
                onClick={loadData}
                className="p-1.5 rounded-[var(--radius-sm)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-grouped)] transition-colors cursor-pointer"
              >
                <RefreshCw className="h-4 w-4" />
              </button>
            </div>
          }
        />

        {instanceCount === 0 ? (
          <EmptyState
            icon={<Box className="h-8 w-8" />}
            message={t("sandbox.no_instances")}
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="text-left text-[11px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
                  <th className="pb-2 pr-4 font-medium">Runtime ID</th>
                  <th className="pb-2 pr-4 font-medium">Provider</th>
                  <th className="pb-2 pr-4 font-medium">Scope</th>
                  <th className="pb-2 pr-4 font-medium">Image</th>
                  <th className="pb-2 pr-4 font-medium">Last Used</th>
                  <th className="pb-2 font-medium" />
                </tr>
              </thead>
              <tbody>
                {instances!.map((inst) => (
                  <tr
                    key={inst.runtime_id}
                    className="border-b border-[var(--border-subtle)] last:border-0 hover:bg-[var(--bg-grouped)]/50 transition-colors"
                  >
                    <td className="py-2.5 pr-4 font-mono text-[12px] text-[var(--text-primary)]">
                      {inst.runtime_id.slice(0, 12)}
                    </td>
                    <td className="py-2.5 pr-4 text-[var(--text-secondary)]">
                      {inst.provider_id}
                    </td>
                    <td className="py-2.5 pr-4 text-[var(--text-secondary)] font-mono text-[12px]">
                      {inst.scope_key}
                    </td>
                    <td className="py-2.5 pr-4 text-[var(--text-tertiary)] font-mono text-[11px]">
                      {inst.image ?? "—"}
                    </td>
                    <td className="py-2.5 pr-4 text-[var(--text-tertiary)] text-[12px] tabular-nums">
                      {formatDate(inst.last_used_at, i18n.language)}
                    </td>
                    <td className="py-2.5 text-right">
                      <button
                        onClick={() => handleDestroy(inst.runtime_id)}
                        className={cn(
                          "inline-flex items-center gap-1 px-2 py-1 rounded-[var(--radius-sm)] text-[11px] font-medium transition-colors cursor-pointer",
                          confirming === inst.runtime_id
                            ? "bg-[var(--error)]/10 text-[var(--error)]"
                            : "text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/5"
                        )}
                      >
                        <Trash2 className="h-3 w-3" />
                        {confirming === inst.runtime_id ? t("sessions.confirm") : t("sandbox.destroy")}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </SectionCard>

      <ToastContainer toasts={toasts} />
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[11px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] font-medium">
        {label}
      </span>
      <span className="text-[var(--text-primary)] font-mono text-[12px]">{value}</span>
    </div>
  );
}
