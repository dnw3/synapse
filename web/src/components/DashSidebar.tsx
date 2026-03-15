import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import { ScrollArea } from "./ui/scroll-area";
import { cn } from "../lib/cn";
import { TABS, SIDEBAR_SECTIONS } from "./Dashboard";
import type { TabKey } from "./Dashboard";

const STORAGE_KEY = "synapse:nav-collapsed";

function loadCollapsed(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

interface DashSidebarProps {
  activeView: string;
  onViewChange: (view: string) => void;
}

export default function DashSidebar({ activeView, onViewChange }: DashSidebarProps) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(loadCollapsed);

  // Persist collapse state
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(collapsed));
  }, [collapsed]);

  const toggleGroup = (key: string) => {
    setCollapsed((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  return (
    <ScrollArea className="flex-1 min-h-0">
      <div className="px-2 pb-2 pt-1">
        {SIDEBAR_SECTIONS.map((section, si) => {
          const groupKey = section.i18nKey;
          const hasActiveTab = section.keys.includes(activeView as TabKey);
          // Auto-expand if group contains the active tab
          const isCollapsed = collapsed[groupKey] && !hasActiveTab;

          return (
            <div key={groupKey}>
              {/* Group header */}
              <button
                onClick={() => toggleGroup(groupKey)}
                className="w-full flex items-center justify-between px-2 pt-3 pb-1 group/header cursor-pointer"
              >
                <span className="text-[10px] font-semibold text-[var(--text-tertiary)] uppercase tracking-[0.5px] group-hover/header:text-[var(--text-secondary)] transition-colors">
                  {t(section.i18nKey)}
                </span>
                <ChevronDown
                  className={cn(
                    "h-3 w-3 text-[var(--text-tertiary)] transition-transform duration-200",
                    isCollapsed && "-rotate-90"
                  )}
                />
              </button>

              {/* Group items */}
              <div
                className={cn(
                  "space-y-px overflow-hidden transition-all duration-200",
                  isCollapsed ? "max-h-0 opacity-0" : "max-h-[500px] opacity-100"
                )}
              >
                {section.keys.map((key) => {
                  const tab = TABS.find((tb) => tb.key === key);
                  if (!tab) return null;
                  const isActive = activeView === key;
                  return (
                    <button
                      key={key}
                      onClick={() => onViewChange(key)}
                      className={cn(
                        "relative flex items-center gap-2 w-full rounded-[6px] text-[13px] transition-all duration-150 cursor-pointer px-2 py-1.5 h-[32px]",
                        isActive
                          ? "bg-[var(--accent)]/12 text-[var(--accent)] font-medium"
                          : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                      )}
                    >
                      <span className="flex-shrink-0 [&_svg]:h-3.5 [&_svg]:w-3.5">{tab.icon}</span>
                      <span className="truncate">{t(tab.i18nKey)}</span>
                    </button>
                  );
                })}
              </div>

              {si < SIDEBAR_SECTIONS.length - 1 && (
                <div className="my-1.5 mx-2 border-t border-[var(--separator)]" />
              )}
            </div>
          );
        })}
      </div>
    </ScrollArea>
  );
}
