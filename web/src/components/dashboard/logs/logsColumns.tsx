import { useCallback } from "react";

// ─── Resizable Columns ──────────────────────────────────────────────────────

export type ColumnId = "time" | "level" | "traceId" | "target";

export const COLUMN_DEFAULTS: Record<ColumnId, { min: number; max: number; default: number }> = {
  time:    { min: 60, max: 180, default: 86 },
  level:   { min: 44, max: 100, default: 52 },
  traceId: { min: 60, max: 200, default: 90 },
  target:  { min: 60, max: 300, default: 100 },
};

export const STORAGE_KEY = "synapse-log-col-widths";

export function loadColumnWidths(): Record<ColumnId, number> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      const result = {} as Record<ColumnId, number>;
      for (const [k, def] of Object.entries(COLUMN_DEFAULTS)) {
        const v = parsed[k];
        result[k as ColumnId] = typeof v === "number" ? Math.max(def.min, Math.min(def.max, v)) : def.default;
      }
      return result;
    }
  } catch { /* ignore */ }
  const result = {} as Record<ColumnId, number>;
  for (const [k, def] of Object.entries(COLUMN_DEFAULTS)) {
    result[k as ColumnId] = def.default;
  }
  return result;
}

export function saveColumnWidths(widths: Record<ColumnId, number>) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(widths)); } catch { /* ignore */ }
}

/** Drag handle for column resize */
export function ResizeHandle({ columnId, widths, setWidths }: {
  columnId: ColumnId;
  widths: Record<ColumnId, number>;
  setWidths: React.Dispatch<React.SetStateAction<Record<ColumnId, number>>>;
}) {
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = widths[columnId];
    const { min, max } = COLUMN_DEFAULTS[columnId];

    const onMove = (ev: MouseEvent) => {
      const delta = ev.clientX - startX;
      const newW = Math.max(min, Math.min(max, startW + delta));
      setWidths(prev => {
        const next = { ...prev, [columnId]: newW };
        saveColumnWidths(next);
        return next;
      });
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [columnId, widths, setWidths]);

  return (
    <div
      onMouseDown={handleMouseDown}
      className="absolute right-0 top-0 bottom-0 w-[5px] cursor-col-resize z-10 group/handle hover:bg-[var(--accent)]/20 active:bg-[var(--accent)]/30 transition-colors"
    >
      <div className="absolute right-[2px] top-[4px] bottom-[4px] w-[1px] bg-[var(--border-subtle)]/0 group-hover/handle:bg-[var(--accent)]/40 transition-colors" />
    </div>
  );
}
