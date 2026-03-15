import { X } from "lucide-react";
import { cn } from "../lib/cn";
import ChatSidebar from "./ChatSidebar";
import DashSidebar from "./DashSidebar";
import SidebarFooter from "./SidebarFooter";
import type { Conversation } from "../types";
import type { IdentityInfo } from "../types/dashboard";

interface SidebarProps {
  // Conversations (passed to ChatSidebar)
  conversations: Conversation[];
  activeConversationId: string | null;
  titles: Record<string, string>;
  onSelectConversation: (id: string) => void;
  onNewConversation: () => void;
  onDeleteConversation: (id: string) => void;
  // Navigation
  activeView: string;
  onViewChange: (view: string) => void;
  sidebarMode: "chat" | "dashboard";
  onSwitchMode: () => void;
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

export default function Sidebar({
  conversations,
  activeConversationId,
  titles,
  onSelectConversation,
  onNewConversation,
  onDeleteConversation,
  activeView,
  onViewChange,
  sidebarMode,
  onSwitchMode,
  identity,
  themeMode,
  onCycleTheme,
  onToggleLanguage,
  isOpen,
  onClose,
}: SidebarProps) {
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
      {/* Window controls / mobile close — compact in dashboard mode */}
      <div className={cn(
        "flex-shrink-0 flex items-end px-3 md:block",
        sidebarMode === "chat" ? "h-[52px] pb-2" : "h-[38px] pb-1"
      )}>
        <button
          onClick={onClose}
          className="md:hidden p-1 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)]"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {/* Mode content */}
      {sidebarMode === "chat" ? (
        <ChatSidebar
          conversations={conversations}
          activeConversationId={activeConversationId}
          titles={titles}
          onSelectConversation={onSelectConversation}
          onNewConversation={onNewConversation}
          onDeleteConversation={onDeleteConversation}
        />
      ) : (
        <DashSidebar
          activeView={activeView}
          onViewChange={onViewChange}
        />
      )}

      {/* Footer */}
      <SidebarFooter
        identity={identity}
        themeMode={themeMode}
        onCycleTheme={onCycleTheme}
        onToggleLanguage={onToggleLanguage}
        sidebarMode={sidebarMode}
        onSwitchMode={onSwitchMode}
      />
    </aside>
  );
}
