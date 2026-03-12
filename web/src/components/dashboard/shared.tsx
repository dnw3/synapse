import { ArrowUpRight, ArrowDownRight, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "../../lib/cn";
import type { StatsCardProps } from "../../types/dashboard";

export function StatsCard({ icon, label, value, sub, trend, accent, pulse }: StatsCardProps) {
  return (
    <div className="group relative overflow-hidden rounded-[var(--radius-xl)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-4 sm:p-5 transition-all duration-300 hover:border-[var(--separator)] hover:shadow-[var(--shadow-md)] animate-fade-slide-in">
      <div
        className="absolute top-0 left-0 right-0 h-[2px] opacity-60 group-hover:opacity-100 transition-opacity"
        style={{ background: accent || "var(--accent)" }}
      />
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1.5">
          <span className="text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
            {label}
          </span>
          <div className="flex items-baseline gap-2">
            <span
              className="text-[26px] font-bold tracking-[-0.02em] text-[var(--text-primary)] tabular-nums"
              style={{ fontFamily: "var(--font-heading)" }}
            >
              {value}
            </span>
            {trend && (
              <span className={cn(
                "inline-flex items-center gap-0.5 px-2 py-[2px] rounded-full text-[11px] font-medium",
                trend.up
                  ? "bg-[var(--success)]/10 text-[var(--success)]"
                  : "bg-[var(--error)]/10 text-[var(--error)]"
              )}>
                {trend.up ? <ArrowUpRight className="h-3 w-3" /> : <ArrowDownRight className="h-3 w-3" />}
                {trend.value}%
              </span>
            )}
          </div>
          {sub && (
            <span className="text-[11px] text-[var(--text-tertiary)] font-mono">{sub}</span>
          )}
        </div>
        <div className={cn(
          "flex items-center justify-center w-10 h-10 rounded-[var(--radius-md)] transition-colors",
          pulse && "animate-pulse-glow"
        )} style={{ background: `color-mix(in srgb, ${accent || "var(--accent)"} 12%, transparent)` }}>
          <div style={{ color: accent || "var(--accent-light)" }}>{icon}</div>
        </div>
      </div>
    </div>
  );
}

export function StatusDot({ status }: { status: "online" | "degraded" | "offline" }) {
  const colors = {
    online: "bg-[var(--success)]",
    degraded: "bg-[var(--warning)]",
    offline: "bg-[var(--error)]",
  };
  return (
    <span className="relative flex h-2 w-2">
      {status === "online" && (
        <span className={cn("absolute inset-0 rounded-full animate-ping opacity-40", colors[status])} />
      )}
      <span className={cn("relative inline-flex h-2 w-2 rounded-full", colors[status])} />
    </span>
  );
}

export function SectionCard({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn(
      "rounded-[var(--radius-xl)] bg-[var(--bg-content)] border border-[var(--border-subtle)] p-4 sm:p-5",
      className
    )}>
      {children}
    </div>
  );
}

export function SectionHeader({ icon, title, right }: { icon: React.ReactNode; title: string; right?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between mb-4">
      <div className="flex items-center gap-2">
        <span className="text-[var(--text-tertiary)]">{icon}</span>
        <span
          className="text-[18px] font-semibold text-[var(--text-primary)]"
          style={{ fontFamily: "var(--font-heading)" }}
        >
          {title}
        </span>
      </div>
      {right}
    </div>
  );
}

export function EmptyState({ icon, message, description, action }: {
  icon: React.ReactNode;
  message: string;
  description?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center py-16 gap-3 text-[var(--text-tertiary)]">
      <div className="text-[var(--text-tertiary)] opacity-40">{icon}</div>
      <span
        className="text-[16px] font-semibold text-[var(--text-secondary)]"
        style={{ fontFamily: "var(--font-heading)" }}
      >
        {message}
      </span>
      {description && (
        <span className="text-[13px] text-[var(--text-tertiary)] text-center max-w-[280px]">{description}</span>
      )}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

export function LoadingSkeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn("rounded-[var(--radius-md)]", className)}
      style={{
        background: "linear-gradient(90deg, var(--bg-content) 25%, var(--bg-elevated) 50%, var(--bg-content) 75%)",
        backgroundSize: "200% 100%",
        animation: "skeleton-shimmer 1.5s ease-in-out infinite",
      }}
    />
  );
}

export function LoadingSpinner() {
  return (
    <div className="flex items-center justify-center py-12">
      <RefreshCw className="h-5 w-5 text-[var(--text-tertiary)] animate-spin" />
    </div>
  );
}

export function ChartTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ name: string; value: number; color: string }>; label?: string }) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-[var(--radius-md)] bg-[var(--bg-elevated)] border border-[var(--separator)] shadow-[var(--shadow-md)] px-3 py-2 text-[11px]">
      <div className="font-semibold text-[var(--text-primary)] mb-1" style={{ fontFamily: "var(--font-heading)" }}>{label}</div>
      {payload.map((p, i) => (
        <div key={i} className="flex items-center gap-2 text-[var(--text-secondary)]">
          <span className="w-2 h-2 rounded-full" style={{ background: p.color }} />
          <span>{p.name}:</span>
          <span className="font-mono tabular-nums text-[var(--text-primary)]">{p.value.toLocaleString()}</span>
        </div>
      ))}
    </div>
  );
}

// Inline confirm for destructive actions
export function useInlineConfirm(timeoutMs = 3000) {
  const [confirming, setConfirming] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(null);

  const requestConfirm = useCallback((id: string) => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setConfirming(id);
    timerRef.current = setTimeout(() => setConfirming(null), timeoutMs);
  }, [timeoutMs]);

  const reset = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setConfirming(null);
  }, []);

  return { confirming, requestConfirm, reset };
}

// Toast notification system
export function useToast() {
  const [toasts, setToasts] = useState<Array<{ id: number; message: string; type: "success" | "error" }>>([]);
  const nextId = useRef(0);

  const addToast = useCallback((message: string, type: "success" | "error" = "success") => {
    const id = nextId.current++;
    setToasts((prev) => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3000);
  }, []);

  return { toasts, addToast };
}

export function ToastContainer({ toasts }: { toasts: Array<{ id: number; message: string; type: "success" | "error" }> }) {
  if (!toasts.length) return null;
  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            "px-4 py-2.5 rounded-[var(--radius-md)] text-[12px] font-medium shadow-[var(--shadow-md)] border animate-fade-in",
            t.type === "success"
              ? "bg-[var(--bg-elevated)] text-[var(--success)] border-[var(--success)]/20"
              : "bg-[var(--bg-elevated)] text-[var(--error)] border-[var(--error)]/20"
          )}
        >
          {t.message}
        </div>
      ))}
    </div>
  );
}

// Pagination component
export function Pagination({
  total, limit, offset, onChange,
}: {
  total: number;
  limit: number;
  offset: number;
  onChange: (offset: number) => void;
}) {
  const { t } = useTranslation();
  const totalPages = Math.ceil(total / limit);
  const currentPage = Math.floor(offset / limit) + 1;
  const from = offset + 1;
  const to = Math.min(offset + limit, total);

  if (total <= limit) return null;

  return (
    <div className="flex items-center justify-between pt-3 text-[11px]">
      <span className="text-[var(--text-tertiary)] font-mono tabular-nums">
        {t("pagination.ofTotal", { from, to, total })}
      </span>
      <div className="flex items-center gap-1">
        <button
          onClick={() => onChange(Math.max(0, offset - limit))}
          disabled={currentPage <= 1}
          className="px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium text-[var(--text-secondary)] bg-[var(--bg-grouped)] hover:bg-[var(--bg-elevated)] disabled:opacity-30 disabled:cursor-not-allowed transition-colors cursor-pointer"
        >
          {t("pagination.prev")}
        </button>
        <span className="px-2 py-1 text-[var(--text-tertiary)] font-mono tabular-nums text-[11px]">
          {currentPage}/{totalPages}
        </span>
        <button
          onClick={() => onChange(Math.min((totalPages - 1) * limit, offset + limit))}
          disabled={currentPage >= totalPages}
          className="px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium text-[var(--text-secondary)] bg-[var(--bg-grouped)] hover:bg-[var(--bg-elevated)] disabled:opacity-30 disabled:cursor-not-allowed transition-colors cursor-pointer"
        >
          {t("pagination.next")}
        </button>
      </div>
    </div>
  );
}

// Toggle switch component
export function Toggle({
  checked, onChange, disabled, size = "md", label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  size?: "sm" | "md";
  label?: string;
}) {
  const w = size === "sm" ? "w-8 h-4" : "w-9 h-5";
  const dot = size === "sm" ? "w-3 h-3" : "w-3.5 h-3.5";
  const translate = size === "sm" ? "translate-x-3.5" : "translate-x-4";

  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => !disabled && onChange(!checked)}
      className={cn(
        "relative inline-flex flex-shrink-0 rounded-full border-2 border-transparent transition-colors duration-200 cursor-pointer",
        w,
        checked ? "bg-[var(--accent)]" : "bg-[var(--bg-content)] border-[var(--separator)]",
        disabled && "opacity-50 cursor-not-allowed"
      )}
    >
      <span
        className={cn(
          "pointer-events-none inline-block rounded-full bg-white shadow-sm transition-transform duration-200",
          dot,
          checked ? translate : "translate-x-0.5"
        )}
        style={{ marginTop: "1px" }}
      />
    </button>
  );
}

// Helpers
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function formatCost(c: number): string {
  return `$${c.toFixed(2)}`;
}

export function formatDate(v: string, locale?: string): string {
  try {
    const d = /^\d+$/.test(v) ? new Date(Number(v)) : new Date(v);
    if (isNaN(d.getTime())) return v;
    return d.toLocaleString(locale?.startsWith("zh") ? "zh-CN" : "en-US", {
      month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
    });
  } catch {
    return v;
  }
}

export function formatUptime(s: number): string {
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d > 0) return `${d}d ${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

// Need to import these for hooks
import { useState, useRef, useCallback } from "react";
