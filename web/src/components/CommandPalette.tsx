import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, LayoutDashboard, Terminal, MessageSquare } from "lucide-react";
import { cn } from "../lib/cn";

export interface PaletteEntry {
  id: string;
  label: string;
  labelZh: string;
  category: "navigation" | "command" | "session";
  icon?: React.ReactNode;
  action: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  entries: PaletteEntry[];
}

const CATEGORY_ORDER: PaletteEntry["category"][] = ["navigation", "command", "session"];

function categoryIcon(cat: PaletteEntry["category"]) {
  const cls = "h-3.5 w-3.5 flex-shrink-0";
  switch (cat) {
    case "navigation": return <LayoutDashboard className={cls} />;
    case "command": return <Terminal className={cls} />;
    case "session": return <MessageSquare className={cls} />;
  }
}

export default function CommandPalette({ open, onClose, entries }: CommandPaletteProps) {
  const { t, i18n } = useTranslation();
  const isZh = i18n.language.startsWith("zh");
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Reset on open
  useEffect(() => {
    if (open) {
      setQuery("");
      setCursor(0);
      setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [open]);

  // Fuzzy filter
  const lower = query.toLowerCase();
  const filtered = entries.filter((e) => {
    if (!query) return true;
    const label = (isZh ? e.labelZh : e.label).toLowerCase();
    return label.includes(lower) || e.id.toLowerCase().includes(lower);
  });

  // Group
  const groups: { cat: PaletteEntry["category"]; items: PaletteEntry[] }[] = CATEGORY_ORDER
    .map((cat) => ({ cat, items: filtered.filter((e) => e.category === cat) }))
    .filter((g) => g.items.length > 0);

  // Flat ordered list for keyboard navigation
  const flatItems = groups.flatMap((g) => g.items);

  const safeCursor = Math.min(cursor, Math.max(0, flatItems.length - 1));

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") { onClose(); return; }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setCursor((c) => Math.min(c + 1, flatItems.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setCursor((c) => Math.max(c - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = flatItems[safeCursor];
      if (item) { item.action(); onClose(); }
    }
  };

  // Scroll selected item into view
  useEffect(() => {
    if (!listRef.current) return;
    const el = listRef.current.querySelector("[data-selected='true']");
    if (el) (el as HTMLElement).scrollIntoView({ block: "nearest" });
  }, [safeCursor]);

  if (!open) return null;

  const categoryLabel = (cat: PaletteEntry["category"]) => {
    switch (cat) {
      case "navigation": return t("palette.navigation");
      case "command": return t("palette.commands");
      case "session": return t("palette.sessions");
    }
  };

  let flatIndex = 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]"
      style={{ background: "rgba(0,0,0,0.45)" }}
      onClick={onClose}
    >
      {/* backdrop blur */}
      <div className="absolute inset-0 backdrop-blur-sm" />

      <div
        className="relative w-full max-w-lg mx-4 rounded-[var(--radius-xl)] shadow-2xl overflow-hidden border border-[var(--border-subtle)]"
        style={{ background: "var(--bg-elevated)" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3.5 border-b border-[var(--border-subtle)]">
          <Search className="h-4 w-4 text-[var(--text-tertiary)] flex-shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => { setQuery(e.target.value); setCursor(0); }}
            onKeyDown={handleKeyDown}
            placeholder={t("palette.placeholder")}
            className="flex-1 bg-transparent outline-none text-[14px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]"
          />
          <kbd className="hidden sm:flex items-center px-1.5 py-0.5 rounded text-[10px] text-[var(--text-tertiary)] border border-[var(--border-subtle)] font-mono">
            ESC
          </kbd>
        </div>

        {/* Results */}
        <div
          ref={listRef}
          className="max-h-[360px] overflow-y-auto py-2"
        >
          {groups.length === 0 ? (
            <div className="px-4 py-8 text-center text-[13px] text-[var(--text-tertiary)]">
              {t("palette.empty")}
            </div>
          ) : (
            groups.map(({ cat, items }) => (
              <div key={cat}>
                {/* Category header */}
                <div className="flex items-center gap-2 px-4 py-1.5">
                  <span className="text-[var(--text-tertiary)]">{categoryIcon(cat)}</span>
                  <span className="text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--text-tertiary)]">
                    {categoryLabel(cat)}
                  </span>
                </div>

                {items.map((entry) => {
                  const idx = flatIndex++;
                  const isSelected = idx === safeCursor;
                  return (
                    <button
                      key={entry.id}
                      data-selected={isSelected}
                      onClick={() => { entry.action(); onClose(); }}
                      onMouseEnter={() => setCursor(idx)}
                      className={cn(
                        "w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors cursor-pointer",
                        isSelected
                          ? "bg-[var(--accent)] text-white"
                          : "hover:bg-[var(--bg-hover)]"
                      )}
                    >
                      {entry.icon && (
                        <span className={cn(
                          "flex-shrink-0",
                          isSelected ? "text-white/80" : "text-[var(--text-tertiary)]"
                        )}>
                          {entry.icon}
                        </span>
                      )}
                      <span className={cn(
                        "text-[13px]",
                        isSelected ? "text-white font-medium" : "text-[var(--text-primary)]"
                      )}>
                        {isZh ? entry.labelZh : entry.label}
                      </span>
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
