import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { X, ChevronDown, MessageSquare } from "lucide-react";
import { ScrollArea } from "./ui/scroll-area";
import { cn } from "../lib/cn";
import { TABS, SIDEBAR_SECTIONS } from "./Dashboard";
import type { TabKey } from "./Dashboard";
import type { IdentityInfo } from "../types/dashboard";
import SidebarFooter from "./SidebarFooter";

const STORAGE_KEY = "synapse:nav-collapsed";

function loadCollapsed(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

interface UnifiedSidebarProps {
  // Identity
  identity: IdentityInfo | null;
  // Theme
  themeMode: string;
  onCycleTheme: () => void;
  onToggleLanguage: () => void;
  // Connection
  connected?: boolean;
  // Mobile
  isOpen: boolean;
  onClose: () => void;
}

export default function UnifiedSidebar({
  identity,
  themeMode,
  onCycleTheme,
  onToggleLanguage,
  connected,
  isOpen,
  onClose,
}: UnifiedSidebarProps) {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const activeView = location.pathname === "/chat" ? "chat" : location.pathname.replace("/dashboard/", "");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(loadCollapsed);

  // Persist collapse state
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(collapsed));
  }, [collapsed]);

  const toggleGroup = (key: string) => {
    setCollapsed((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  const isChatActive = activeView === "chat";

  return (
    <aside
      className={cn(
        "flex flex-col h-full w-[220px] flex-shrink-0 vibrancy-sidebar",
        "bg-[var(--bg-sidebar)]/80 border-r border-[var(--separator)]",
        "fixed inset-y-0 left-0 z-50 md:relative md:inset-auto",
        "transition-transform duration-200 ease-out",
        isOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"
      )}
    >
      {/* Brand header + mobile close */}
      <div className="flex-shrink-0 relative px-3 pt-3 pb-2">
        {/* Mobile close */}
        <button
          onClick={onClose}
          className="md:hidden absolute top-2.5 right-2.5 p-1 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)]"
        >
          <X className="h-4 w-4" />
        </button>
        {/* Brand identity */}
        <div className="flex items-center gap-2 min-w-0 px-1">
          {identity?.avatar_url ? (
            <img src={identity.avatar_url} alt="" className="w-6 h-6 rounded-full object-cover flex-shrink-0" />
          ) : (
            <div className="w-6 h-6 rounded-full bg-gradient-to-br from-[var(--accent)] to-[var(--accent-gradient-end)] flex items-center justify-center text-white text-[10px] font-semibold flex-shrink-0">
              {identity?.emoji || "S"}
            </div>
          )}
          <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
            {identity?.name || t("sidebar.brand")}
          </span>
          {connected !== undefined && (
            <span
              className={`w-2 h-2 rounded-full flex-shrink-0 transition-colors ${
                connected
                  ? "bg-[var(--status-ok,#34c759)]"
                  : "bg-[var(--status-danger,#ff3b30)]"
              }`}
              style={{
                boxShadow: connected
                  ? "0 0 0 3px color-mix(in srgb, var(--status-ok, #34c759) 14%, transparent)"
                  : "0 0 0 3px color-mix(in srgb, var(--status-danger, #ff3b30) 14%, transparent)",
              }}
              role="img"
              aria-live="polite"
              aria-label={connected ? t("status.connected") : t("status.disconnected")}
              title={connected ? t("status.connected") : t("status.disconnected")}
            />
          )}
        </div>
      </div>

      {/* Scrollable navigation */}
      <ScrollArea className="flex-1 min-h-0">
        <div className="px-2 pb-2 pt-1">
          {/* ── Chat Group ── */}
          <div>
            <button
              onClick={() => toggleGroup("sidebar.chat")}
              className="w-full flex items-center justify-between px-2 pt-2 pb-1 group/header cursor-pointer"
            >
              <span className="text-[10px] font-semibold text-[var(--text-tertiary)] uppercase tracking-[0.5px] group-hover/header:text-[var(--text-secondary)] transition-colors">
                {t("sidebar.chat")}
              </span>
              <ChevronDown
                className={cn(
                  "h-3 w-3 text-[var(--text-tertiary)] transition-transform duration-200",
                  collapsed["sidebar.chat"] && !isChatActive && "-rotate-90"
                )}
              />
            </button>

            <div
              className={cn(
                "overflow-hidden transition-all duration-200",
                collapsed["sidebar.chat"] && !isChatActive ? "max-h-0 opacity-0" : "max-h-[9999px] opacity-100"
              )}
            >
              {/* Chat tab item */}
              <button
                onClick={() => navigate("/chat")}
                className={cn(
                  "relative flex items-center gap-2 w-full rounded-[6px] text-[13px] transition-all duration-150 cursor-pointer px-2 py-1.5 h-[32px]",
                  isChatActive
                    ? "bg-[var(--accent)]/12 text-[var(--accent)] font-medium"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                )}
              >
                <MessageSquare className="h-3.5 w-3.5 flex-shrink-0" />
                <span className="truncate">{t("sidebar.chatTab")}</span>
              </button>

            </div>

            <div className="my-1.5 mx-2 border-t border-[var(--separator)]" />
          </div>

          {/* ── Dashboard Groups (Control, Agent, Settings) ── */}
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
                        onClick={() => navigate(`/dashboard/${key}`)}
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

      {/* Footer — no mode switch needed */}
      <SidebarFooter
        identity={identity}
        themeMode={themeMode}
        onCycleTheme={onCycleTheme}
        onToggleLanguage={onToggleLanguage}
        connected={connected}
      />
    </aside>
  );
}
