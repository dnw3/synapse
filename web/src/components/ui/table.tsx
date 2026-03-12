import { ChevronDown, ChevronUp } from "lucide-react";
import { useState, type ReactNode } from "react";
import { cn } from "../../lib/cn";

interface TableColumn<T> {
  key: string;
  header: string;
  render?: (item: T) => ReactNode;
  sortable?: boolean;
  width?: string;
}

interface TableProps<T> {
  columns: TableColumn<T>[];
  data: T[];
  onSort?: (key: string, dir: "asc" | "desc") => void;
  onRowClick?: (item: T) => void;
  className?: string;
}

function Table<T extends Record<string, unknown>>({
  columns,
  data,
  onSort,
  onRowClick,
  className,
}: TableProps<T>) {
  const [sortKey, setSortKey] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  const handleSort = (key: string) => {
    const newDir = sortKey === key && sortDir === "asc" ? "desc" : "asc";
    setSortKey(key);
    setSortDir(newDir);
    onSort?.(key, newDir);
  };

  return (
    <div className={cn("overflow-auto rounded-[var(--radius-lg)] border border-[var(--border-subtle)]", className)}>
      <table className="w-full border-collapse">
        <thead>
          <tr className="border-b border-[var(--separator)]">
            {columns.map((col) => (
              <th
                key={col.key}
                style={col.width ? { width: col.width } : undefined}
                className={cn(
                  "h-10 px-3 text-left text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]",
                  "bg-[var(--bg-grouped)]",
                  col.sortable && "cursor-pointer select-none hover:text-[var(--text-secondary)]"
                )}
                onClick={() => col.sortable && handleSort(col.key)}
              >
                <span className="inline-flex items-center gap-1">
                  {col.header}
                  {col.sortable && sortKey === col.key && (
                    sortDir === "asc" ? (
                      <ChevronUp className="h-3 w-3" />
                    ) : (
                      <ChevronDown className="h-3 w-3" />
                    )
                  )}
                </span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.map((item, i) => (
            <tr
              key={i}
              onClick={() => onRowClick?.(item)}
              className={cn(
                "h-10 border-b border-[var(--border-subtle)] last:border-b-0 transition-colors",
                i % 2 === 1 && "bg-[var(--bg-hover)]",
                "hover:bg-[var(--bg-hover)]",
                onRowClick && "cursor-pointer"
              )}
            >
              {columns.map((col) => (
                <td
                  key={col.key}
                  className="px-3 py-2 text-[13px] text-[var(--text-primary)]"
                >
                  {col.render ? col.render(item) : String(item[col.key] ?? "")}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export { Table };
export type { TableColumn, TableProps };
