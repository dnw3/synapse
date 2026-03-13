import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, X, Sun, Moon, Monitor, Globe } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Separator } from "./ui/separator";
import { ScrollArea } from "./ui/scroll-area";
import { cn } from "../lib/cn";
import { TABS, SIDEBAR_SECTIONS } from "./Dashboard";
import type { TabKey } from "./Dashboard";
import type { Conversation } from "../types";
import type { IdentityInfo } from "../types/dashboard";

const MODE_ICONS = { light: Sun, dark: Moon, system: Monitor } as const;

interface SidebarProps {
  // Conversations
  conversations: Conversation[];
  activeConversationId: string | null;
  titles: Record<string, string>;
  onSelectConversation: (id: string) => void;
  onNewConversation: () => void;
  onDeleteConversation: (id: string) => void;
  // Navigation
  activeView: string; // "chat" | TabKey
  onViewChange: (view: string) => void;
  // Dashboard tab definitions
  tabs: typeof TABS;
  sidebarSections: typeof SIDEBAR_SECTIONS;
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

function formatRelativeTime(dateStr: string): string {
  const now = Date.now();
  // Handle millisecond-timestamp strings from the API (e.g. "1773246544000")
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

export default function Sidebar({
  conversations,
  activeConversationId,
  titles,
  onSelectConversation,
  onNewConversation,
  onDeleteConversation,
  activeView,
  onViewChange,
  tabs,
  sidebarSections,
  identity,
  themeMode,
  onCycleTheme,
  onToggleLanguage,
  isOpen,
  onClose,
}: SidebarProps) {
  const { t } = useTranslation();
  const [search, setSearch] = useState("");

  const isChatView = activeView === "chat";

  const filteredConversations = search.trim()
    ? conversations.filter((c) => {
        const title = titles[c.id] ?? c.id;
        return title.toLowerCase().includes(search.toLowerCase());
      })
    : conversations;

  const ModeIcon = MODE_ICONS[themeMode as keyof typeof MODE_ICONS] ?? Monitor;

  return (
    <aside
      className={cn(
        "flex flex-col h-full w-[220px] flex-shrink-0 vibrancy-sidebar",
        "bg-[var(--bg-sidebar)]/80 border-r border-[var(--separator)]",
        // Mobile: fixed overlay
        "fixed inset-y-0 left-0 z-50 md:relative md:inset-auto",
        "transition-transform duration-200 ease-out",
        isOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"
      )}
    >
      {/* 1. Window controls area */}
      <div className="h-[52px] flex-shrink-0 flex items-end px-3 pb-2 md:block">
        {/* Mobile close button */}
        <button
          onClick={onClose}
          className="md:hidden p-1 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)]"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {/* 2. Search input */}
      <div className="px-3 pb-2">
        <Input
          variant="search"
          placeholder={t("sidebar.search")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {/* 3. New Chat button */}
      <div className="px-3 pb-2">
        <Button onClick={onNewConversation} className="w-full gap-1.5" size="sm">
          <Plus className="h-3.5 w-3.5" />
          {t("sidebar.newChat")}
        </Button>
      </div>

      {/* 4. Scrollable area with 3 groups */}
      <ScrollArea className="flex-1 min-h-0">
        <div className="px-2 pb-2">
          {/* ── Conversations ── */}
          <div>
            <div className="px-2 pt-2 pb-1">
              <span className="text-[10px] font-semibold text-[var(--text-tertiary)] uppercase tracking-[0.5px]">
                {t("sidebar.conversations")}
              </span>
            </div>
            <div className="space-y-px">
              {filteredConversations.map((conv) => {
                const isActive = activeConversationId === conv.id && isChatView;
                const title = titles[conv.id]
                  ? titles[conv.id].slice(0, 40)
                  : conv.message_count === 0
                    ? t("sidebar.newChat")
                    : conv.id.slice(0, 8);
                return (
                  <div
                    key={conv.id}
                    onClick={() => {
                      onViewChange("chat");
                      onSelectConversation(conv.id);
                    }}
                    className={cn(
                      "group/item relative flex items-center gap-2 px-2 py-1.5 rounded-[6px] cursor-pointer transition-all duration-150",
                      isActive
                        ? "bg-[var(--accent)]/12 font-medium"
                        : "hover:bg-[var(--bg-hover)]"
                    )}
                  >
                    <div className="min-w-0 flex-1">
                      <div className={cn(
                        "truncate text-[13px] leading-tight",
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
              {filteredConversations.length === 0 && (
                <div className="px-2 py-6 text-[11px] text-[var(--text-tertiary)] text-center">
                  {t("sidebar.noConversations")}
                </div>
              )}
            </div>
          </div>

          <Separator className="my-2" />

          {/* ── Dashboard sections ── */}
          {sidebarSections.map((section, si) => (
            <div key={si}>
              <div className="px-2 pt-2 pb-1">
                <span className="text-[10px] font-semibold text-[var(--text-tertiary)] uppercase tracking-[0.5px]">
                  {t(section.i18nKey)}
                </span>
              </div>
              <div className="space-y-px">
                {section.keys.map((key) => {
                  const tab = tabs.find((tb) => tb.key === key);
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
              {si < sidebarSections.length - 1 && <Separator className="my-2" />}
            </div>
          ))}
        </div>
      </ScrollArea>

      {/* 5. Bottom bar */}
      <div className="flex-shrink-0 border-t border-[var(--separator)] px-2.5 py-2.5 flex items-center justify-between">
        {/* Left: brand */}
        <div className="flex items-center gap-2 min-w-0">
          {identity?.avatar_url ? (
            <img src={identity.avatar_url} alt="" className="w-6 h-6 rounded-full object-cover" />
          ) : (
            <div className="w-6 h-6 rounded-full bg-gradient-to-br from-[var(--accent)] to-[var(--accent-gradient-end)] flex items-center justify-center text-white text-[10px] font-semibold flex-shrink-0">
              {identity?.emoji || "S"}
            </div>
          )}
          <span className="text-[12px] font-medium text-[var(--text-primary)] truncate">
            {identity?.name || t("sidebar.brand")}
          </span>
        </div>
        {/* Right: theme + language toggle */}
        <div className="flex items-center gap-0.5 flex-shrink-0">
          <button
            onClick={onCycleTheme}
            className="p-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
            title={t("settings.theme")}
          >
            <ModeIcon className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={onToggleLanguage}
            className="p-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
            title={t("settings.language")}
          >
            <Globe className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
    </aside>
  );
}
