import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "../../lib/cn";

type ToastVariant = "success" | "warning" | "error" | "info" | "accent";

interface ToastOptions {
  variant?: ToastVariant;
  title: string;
  description?: string;
  duration?: number;
}

interface ToastItem extends ToastOptions {
  id: string;
  createdAt: number;
}

const VARIANT_BORDER: Record<ToastVariant, string> = {
  success: "border-l-[var(--success)]",
  warning: "border-l-[var(--warning)]",
  error: "border-l-[var(--error)]",
  info: "border-l-[var(--info)]",
  accent: "border-l-[var(--accent)]",
};

// Global state for toast
let toastListeners: Array<(toasts: ToastItem[]) => void> = [];
let toastItems: ToastItem[] = [];
let nextId = 0;

function notify(listeners: typeof toastListeners) {
  listeners.forEach((fn) => fn([...toastItems]));
}

export function useToast() {
  const [, setRenderTick] = useState(0);
  const listenerRef = useRef<((toasts: ToastItem[]) => void) | null>(null);

  useEffect(() => {
    const listener = () => setRenderTick((n) => n + 1);
    listenerRef.current = listener;
    toastListeners.push(listener);
    return () => {
      toastListeners = toastListeners.filter((l) => l !== listener);
    };
  }, []);

  const toast = useCallback((opts: ToastOptions) => {
    const id = `toast-${++nextId}`;
    const item: ToastItem = {
      ...opts,
      id,
      variant: opts.variant ?? "info",
      duration: opts.duration ?? 5000,
      createdAt: Date.now(),
    };
    toastItems = [...toastItems.slice(-2), item]; // max 3
    notify(toastListeners);

    setTimeout(() => {
      toastItems = toastItems.filter((t) => t.id !== id);
      notify(toastListeners);
    }, item.duration);

    return id;
  }, []);

  const dismiss = useCallback((id: string) => {
    toastItems = toastItems.filter((t) => t.id !== id);
    notify(toastListeners);
  }, []);

  return { toast, dismiss };
}

/** Place <Toaster /> once at the root of your app */
export function Toaster() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  useEffect(() => {
    const listener = (items: ToastItem[]) => setToasts(items);
    toastListeners.push(listener);
    return () => {
      toastListeners = toastListeners.filter((l) => l !== listener);
    };
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-[9999] flex flex-col gap-2 pointer-events-none">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            "pointer-events-auto w-[340px] rounded-[var(--radius-lg)] bg-[var(--bg-content)] shadow-[var(--shadow-md)]",
            "px-4 py-3 border-l-[3px]",
            VARIANT_BORDER[t.variant ?? "info"],
            "animate-message-in-right"
          )}
        >
          <div className="text-[13px] font-medium text-[var(--text-primary)]">{t.title}</div>
          {t.description && (
            <div className="text-[12px] text-[var(--text-secondary)] mt-0.5">{t.description}</div>
          )}
        </div>
      ))}
    </div>
  );
}
