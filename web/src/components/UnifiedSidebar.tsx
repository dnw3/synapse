import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X, Plus, Trash2, ChevronDown, MessageSquare } from "lucide-react";
import { ScrollArea } from "./ui/scroll-area";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { cn } from "../lib/cn";
import { TABS, SIDEBAR_SECTIONS } from "./Dashboard";
import type { TabKey } from "./Dashboard";
import type { Conversation } from "../types";
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

function formatRelativeTime(dateStr: string): string {
  const now = Date.now();
  const parsed = /^\d+$/.test(dateStr) ? parseInt(dateStr, 10) : new Date(dateStr).getTime();
  if (isNaN(parsed)) return "";
  const diffMs = now - parsed;
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "now";
  if (diffMin < 60) return `${diffMin}m`;
  const diffH = Math.floor(diffMin / 60);
  if (diffH < 24) return `${diffH}h`;
  const diffD = Math.floor(diffH / 24);
  return `${diffD}d`;
}

interface UnifiedSidebarProps {
  // Conversations
  conversations: Conversation[];
  activeConversationId: string | null;
  titles: Record<string, string>;
  onSelectConversation: (id: string) => void;
  onNewConversation: () => void;
  onDeleteConversation: (id: string) => void;
  // Navigation
  activeView: string;
  onViewChange: (view: string) => void;
  // Identity
  identity: IdentityInfo | null;
  // Theme
  themeMode: string;
  onCycleTheme: () => void;
  onToggleLanguage: () => void;
  // Mobile
  isOpen: boolean;
  onClose: () => void;
}

export default function UnifiedSidebar({
  conversations,
  activeConversationId,
  titles,
  onSelectConversation,
  onNewConversation,
  onDeleteConversation,
  activeView,
  onViewChange,
  identity,
  themeMode,
  onCycleTheme,
  onToggleLanguage,
  isOpen,
  onClose,
}: UnifiedSidebarProps) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(loadCollapsed);
  const [search, setSearch] = useState("");

  // Persist collapse state
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(collapsed));
  }, [collapsed]);

  const toggleGroup = (key: string) => {
    setCollapsed((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  const isChatActive = activeView === "chat";

  // Filter conversations for search
  const filtered = search.trim()
    ? conversations.filter((c) => {
        const title = titles[c.id] ?? c.id;
        return title.toLowerCase().includes(search.toLowerCase());
      })
    : conversations;

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
      {/* Window controls / mobile close */}
      <div className="flex-shrink-0 flex items-end px-3 md:block h-[38px] pb-1">
        <button
          onClick={onClose}
          className="md:hidden p-1 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)]"
        >
          <X className="h-4 w-4" />
        </button>
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
                onClick={() => onViewChange("chat")}
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

              {/* Inline conversation list (when chat is active) */}
              {isChatActive && (
                <div className="mt-1">
                  {/* Search */}
                  <div className="px-1 pb-1.5">
                    <Input
                      variant="search"
                      placeholder={t("sidebar.search")}
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                  </div>

                  {/* New Chat button */}
                  <div className="px-1 pb-1.5">
                    <Button onClick={onNewConversation} className="w-full gap-1.5" size="sm">
                      <Plus className="h-3.5 w-3.5" />
                      {t("sidebar.newChat")}
                    </Button>
                  </div>

                  {/* Conversation items */}
                  <div className="px-0 space-y-px">
                    <div className="px-2 pt-1 pb-0.5">
                      <span className="text-[10px] font-semibold text-[var(--text-tertiary)] uppercase tracking-[0.5px]">
                        {t("sidebar.conversations")}
                      </span>
                    </div>
                    {filtered.map((conv) => {
                      const isActive = activeConversationId === conv.id;
                      const title = titles[conv.id]
                        ? titles[conv.id].slice(0, 40)
                        : conv.message_count === 0
                          ? t("sidebar.newChat")
                          : conv.id.slice(0, 8);
                      return (
                        <div
                          key={conv.id}
                          onClick={() => onSelectConversation(conv.id)}
                          className={cn(
                            "group/item relative flex items-center gap-2 px-2 py-1.5 rounded-[6px] cursor-pointer transition-all duration-150 ml-4",
                            isActive
                              ? "bg-[var(--accent)]/12 font-medium"
                              : "hover:bg-[var(--bg-hover)]"
                          )}
                        >
                          <div className="min-w-0 flex-1">
                            <div className={cn(
                              "truncate text-[12px] leading-tight",
                              isActive ? "text-[var(--text-primary)] font-medium" : "text-[var(--text-secondary)]"
                            )}>
                              {title}
                            </div>
                          </div>
                          <span className="text-[10px] text-[var(--text-tertiary)] flex-shrink-0 group-hover/item:hidden">
                            {formatRelativeTime(conv.created_at)}
                          </span>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              onDeleteConversation(conv.id);
                            }}
                            className="hidden group-hover/item:flex items-center justify-center w-5 h-5 rounded text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/10 transition-all duration-150 flex-shrink-0 absolute right-1.5"
                            title={t("sidebar.deleteConfirm")}
                          >
                            <Trash2 className="h-3 w-3" />
                          </button>
                        </div>
                      );
                    })}
                    {filtered.length === 0 && (
                      <div className="px-2 py-4 text-[11px] text-[var(--text-tertiary)] text-center">
                        {t("sidebar.noConversations")}
                      </div>
                    )}
                  </div>
                </div>
              )}
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

      {/* Footer — no mode switch needed */}
      <SidebarFooter
        identity={identity}
        themeMode={themeMode}
        onCycleTheme={onCycleTheme}
        onToggleLanguage={onToggleLanguage}
      />
    </aside>
  );
}
