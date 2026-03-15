import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { ScrollArea } from "./ui/scroll-area";
import { cn } from "../lib/cn";
import type { Conversation } from "../types";

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

interface ChatSidebarProps {
  conversations: Conversation[];
  activeConversationId: string | null;
  titles: Record<string, string>;
  onSelectConversation: (id: string) => void;
  onNewConversation: () => void;
  onDeleteConversation: (id: string) => void;
}

export default function ChatSidebar({
  conversations,
  activeConversationId,
  titles,
  onSelectConversation,
  onNewConversation,
  onDeleteConversation,
}: ChatSidebarProps) {
  const { t } = useTranslation();
  const [search, setSearch] = useState("");

  const filtered = search.trim()
    ? conversations.filter((c) => {
        const title = titles[c.id] ?? c.id;
        return title.toLowerCase().includes(search.toLowerCase());
      })
    : conversations;

  return (
    <>
      {/* Search */}
      <div className="px-3 pb-2">
        <Input
          variant="search"
          placeholder={t("sidebar.search")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {/* New Chat button */}
      <div className="px-3 pb-2">
        <Button onClick={onNewConversation} className="w-full gap-1.5" size="sm">
          <Plus className="h-3.5 w-3.5" />
          {t("sidebar.newChat")}
        </Button>
      </div>

      {/* Conversation list */}
      <ScrollArea className="flex-1 min-h-0">
        <div className="px-2 pb-2">
          <div className="px-2 pt-2 pb-1">
            <span className="text-[10px] font-semibold text-[var(--text-tertiary)] uppercase tracking-[0.5px]">
              {t("sidebar.conversations")}
            </span>
          </div>
          <div className="space-y-px">
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
            {filtered.length === 0 && (
              <div className="px-2 py-6 text-[11px] text-[var(--text-tertiary)] text-center">
                {t("sidebar.noConversations")}
              </div>
            )}
          </div>
        </div>
      </ScrollArea>
    </>
  );
}
