import { useTranslation } from "react-i18next";
import { Plus, Trash2, MessageSquare, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";
import { cn } from "../lib/cn";
import type { Conversation } from "../types";

interface Props {
  conversations: Conversation[];
  activeId: string | null;
  titles: Record<string, string>;
  collapsed: boolean;
  onToggle: () => void;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
}

export default function Sidebar({
  conversations,
  activeId,
  titles,
  collapsed,
  onToggle,
  onSelect,
  onCreate,
  onDelete,
}: Props) {
  const { t } = useTranslation();

  // Collapsed: narrow icon strip
  if (collapsed) {
    return (
      <div className="w-12 flex flex-col items-center bg-[var(--bg-elevated)]/60 border-r border-[var(--border-subtle)] py-2 gap-2">
        <button
          onClick={onToggle}
          className="p-2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
          title={t("sidebar.expand")}
        >
          <PanelLeftOpen className="h-4 w-4" />
        </button>
        <button
          onClick={onCreate}
          className="p-2 text-[var(--text-tertiary)] hover:text-[var(--accent-light)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
          title={t("sidebar.newChat")}
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>
    );
  }

  return (
    <div className="w-56 h-full flex flex-col bg-[var(--bg-elevated)] md:bg-[var(--bg-elevated)]/60 border-r border-[var(--border-subtle)]">
      {/* Header with collapse button */}
      <div className="flex items-center gap-1.5 p-2.5">
        <Button onClick={onCreate} className="flex-1 gap-1.5" size="sm" variant="secondary">
          <Plus className="h-3.5 w-3.5" />
          {t("sidebar.newChat")}
        </Button>
        <button
          onClick={onToggle}
          className="p-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]"
          title={t("sidebar.collapse")}
        >
          <PanelLeftClose className="h-3.5 w-3.5" />
        </button>
      </div>

      <ScrollArea className="flex-1">
        <div className="px-2 pb-2 space-y-px">
          {conversations.map((conv) => {
            const isActive = activeId === conv.id;
            return (
              <div
                key={conv.id}
                onClick={() => onSelect(conv.id)}
                className={cn(
                  "group/item relative flex items-center gap-2.5 px-2.5 py-2 rounded-[var(--radius-sm)] cursor-pointer transition-all duration-150",
                  isActive
                    ? "bg-[var(--bg-surface)] text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                )}
              >
                {isActive && (
                  <span className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 rounded-full bg-[var(--accent)]" />
                )}
                <MessageSquare className={cn("h-3.5 w-3.5 flex-shrink-0", isActive ? "text-[var(--accent-light)]" : "text-[var(--text-tertiary)]")} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] leading-tight">
                    {titles[conv.id]
                      ? titles[conv.id].slice(0, 30)
                      : conv.message_count === 0
                        ? t("sidebar.newChat")
                        : conv.id.slice(0, 8)}
                  </div>
                  <div className="text-[10px] text-[var(--text-tertiary)] mt-0.5">
                    {conv.message_count > 0
                      ? t("sidebar.messages", { count: conv.message_count })
                      : t("sidebar.noMessages")}
                  </div>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(conv.id);
                  }}
                  className="opacity-0 pointer-events-none group-hover/item:opacity-100 group-hover/item:pointer-events-auto flex items-center justify-center w-5 h-5 rounded text-[var(--text-tertiary)] hover:text-[var(--error)] hover:bg-[var(--error)]/10 transition-all duration-150 flex-shrink-0"
                  title="Delete"
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </div>
            );
          })}

          {conversations.length === 0 && (
            <div className="px-3 py-10 text-xs text-[var(--text-tertiary)] text-center">
              {t("sidebar.noConversations")}
            </div>
          )}
        </div>
      </ScrollArea>
    </div>
  );
}
