import { createContext, useContext, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useGateway } from "./hooks/useGateway";
import { useSession } from "./hooks/useSession";
import { useTheme } from "./hooks/useTheme";
import { Toaster, useToast } from "./components/ui/toast";
import UnifiedSidebar from "./components/UnifiedSidebar";
import Toolbar from "./components/Toolbar";
import ChatPanel, { FocusModeExitButton } from "./components/ChatPanel";
import Dashboard, { TABS, type TabKey } from "./components/Dashboard";
import CommandPalette, { type PaletteEntry } from "./components/CommandPalette";

import SetupWizard from "./components/SetupWizard";
import ToolOutputSidebar from "./components/ToolOutputSidebar";
import type { IdentityInfo } from "./types/dashboard";
import {
  LayoutDashboard, MessageSquare, Terminal,
} from "lucide-react";

// Identity context for child components (e.g. MessageBubble)
export const IdentityContext = createContext<IdentityInfo | null>(null);
export const useIdentity = () => useContext(IdentityContext);

// ---------------------------------------------------------------------------
// Unified view type: "chat" or a dashboard tab key
// ---------------------------------------------------------------------------

type ViewKey = "chat" | TabKey;

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export default function App() {
  const { t, i18n } = useTranslation();
  const gw = useGateway();
  const session = useSession(gw);
  const theme = useTheme();
  const { toast } = useToast();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [activeView, setActiveView] = useState<ViewKey>("overview");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [modelName, setModelName] = useState<string | null>(null);
  const [focusMode, setFocusMode] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [showWizard, setShowWizard] = useState(false);
  const [toolSidebar, setToolSidebar] = useState<{ open: boolean; content: string; toolName?: string }>({
    open: false,
    content: "",
  });

  // Chat agent selector
  const [, setChatAgent] = useState("default");
  const [agentList, setAgentList] = useState<{ id: string; name: string }[]>([]);
  useEffect(() => {
    fetch("/api/dashboard/agents")
      .then((r) => r.ok ? r.json() : [])
      .then((data: { name: string; id?: string }[]) => {
        if (Array.isArray(data)) {
          setAgentList(data.map((a) => ({ id: a.id ?? a.name, name: a.name })));
        }
      })
      .catch(() => {});
  }, []);

  // Suppress unused warnings for agent selector (kept for future use)
  void setChatAgent;
  void agentList;

  // Fetch agent identity + model name from health
  useEffect(() => {
    fetch("/api/dashboard/identity")
      .then(r => r.ok ? r.json() : null)
      .then((data: IdentityInfo | null) => {
        if (data) {
          setIdentity(data);
          if (data.theme_color) {
            document.documentElement.style.setProperty("--accent", data.theme_color);
          }
        }
      })
      .catch(() => {});
    fetch("/api/dashboard/health")
      .then(r => r.ok ? r.json() : null)
      .then((data: { config_summary?: { model?: string } } | null) => {
        if (data?.config_summary?.model) {
          setModelName(data.config_summary.model);
        }
      })
      .catch(() => {});
  }, []);

  // ── Cmd+K / Ctrl+K → Command Palette ─────────────────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setShowPalette((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // ── Handle toast events from gateway ──────────────────────────────────
  useEffect(() => {
    const unsubscribe = gw.subscribe((event, payload) => {
      if (event === "session.compacted") {
        toast({
          variant: "info",
          title: t("toast.sessionCompacted"),
          duration: 4000,
        });
      } else if (event === "update.available") {
        toast({
          variant: "info",
          title: t("toast.updateAvailable", { version: (payload.version as string) ?? "" }),
          duration: 8000,
        });
      }
    });
    return unsubscribe;
  }, [gw, toast, t]);

  // Poll for completion when reconnected mid-execution.
  useEffect(() => {
    if (gw.status !== "executing" && gw.status !== "thinking") return;
    if (!gw.connected) return;

    // Only poll if we have NO live streaming events
    const hasLiveTokens = session.streaming.messages.length > 0;
    if (hasLiveTokens) return;

    const timer = setInterval(async () => {
      try {
        const result = await gw.call<{ executing: boolean }>("check_execution");
        if (!result.executing) {
          await session.refreshMessages();
        }
      } catch {
        // WS not ready or RPC failed, will retry next interval
      }
    }, 3000);

    return () => clearInterval(timer);
  }, [gw.status, gw.connected, session.streaming.messages.length]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggleLanguage = () => {
    const next = i18n.language.startsWith("zh") ? "en" : "zh";
    i18n.changeLanguage(next);
  };

  // Combine persisted + streaming messages
  const allMessages = [...session.messages, ...session.streaming.messages];

  const isChatView = activeView === "chat";

  // Toolbar title
  const activeSession = session.sessions.find((s) => s.sessionKey === session.activeKey);
  const currentPageTitle = isChatView
    ? (activeSession?.displayName || t("chat.newChat"))
    : t(TABS.find((tb) => tb.key === activeView)?.i18nKey ?? "app.title");
  const toolbarSubtitle = isChatView ? t("sidebar.messages", { count: allMessages.length }) : undefined;
  const toolbarStatus = (!["idle", "pong"].includes(gw.status)) ? t(`status.${gw.status}`) : undefined;

  const chatPanel = (
    <div className="flex flex-1 min-w-0 min-h-0 overflow-hidden">
      <ChatPanel
        messages={allMessages}
        loading={session.sendLock || session.loading || gw.status === "executing" || gw.status === "thinking"}
        streaming={session.streaming.messages.some(m => m.role === "assistant" && m.content.length > 0)}
        approvalRequest={session.streaming.pendingApproval}
        onSend={(content, attachments) => session.sendMessage(content, attachments)}
        onCancel={() => session.cancelGeneration()}
        onApprovalRespond={(approved, allowAll) => session.respondApproval(approved, allowAll)}
        onNewChat={() => session.resetSession()}
        onReset={() => session.resetSession()}
        onResetSession={() => session.resetSession()}
        onToggleFocus={() => setFocusMode((f) => !f)}
        focusMode={focusMode}
        onClearMessages={() => session.setMessages(() => [])}
        onRefreshMessages={() => session.refreshMessages()}
        queueSize={session.messageQueue.length}
        chatError={session.chatError}
        onDismissError={() => session.dismissError()}
        onToolResultClick={(content, toolName) => setToolSidebar({ open: true, content, toolName })}
        sessions={session.sessions}
        activeSessionKey={session.activeKey}
        onSelectSession={(key) => { session.setActiveKey(key); }}
        modelName={modelName}
      />
      <ToolOutputSidebar
        open={toolSidebar.open}
        content={toolSidebar.content}
        toolName={toolSidebar.toolName}
        onClose={() => setToolSidebar((prev) => ({ ...prev, open: false }))}
      />
    </div>
  );

  // ── Command Palette entries ───────────────────────────────────────────
  const paletteEntries: PaletteEntry[] = [
    // Navigation — one entry per dashboard tab + chat
    {
      id: "nav:chat",
      label: "Chat",
      labelZh: "聊天",
      category: "navigation",
      icon: <MessageSquare className="h-3.5 w-3.5" />,
      action: () => setActiveView("chat"),
    },
    ...TABS.map((tab) => ({
      id: `nav:${tab.key}`,
      label: tab.labelEn,
      labelZh: tab.labelZh,
      category: "navigation" as const,
      icon: <LayoutDashboard className="h-3.5 w-3.5" />,
      action: () => setActiveView(tab.key as ViewKey),
    })),
    // Commands — slash commands
    {
      id: "cmd:new",
      label: "New chat",
      labelZh: "新建会话",
      category: "command",
      icon: <Terminal className="h-3.5 w-3.5" />,
      action: () => session.resetSession(),
    },
    {
      id: "cmd:focus",
      label: "Toggle focus mode",
      labelZh: "切换专注模式",
      category: "command",
      icon: <Terminal className="h-3.5 w-3.5" />,
      action: () => setFocusMode((f) => !f),
    },
    {
      id: "cmd:wizard",
      label: "Setup Wizard",
      labelZh: "设置向导",
      category: "command",
      icon: <Terminal className="h-3.5 w-3.5" />,
      action: () => setShowWizard(true),
    },
    // Sessions — recent sessions
    ...session.sessions.slice(0, 10).map((s) => ({
      id: `session:${s.sessionKey}`,
      label: s.displayName || s.sessionKey.slice(0, 8),
      labelZh: s.displayName || s.sessionKey.slice(0, 8),
      category: "session" as const,
      icon: <MessageSquare className="h-3.5 w-3.5" />,
      action: () => {
        session.setActiveKey(s.sessionKey);
        setActiveView("chat");
      },
    })),
  ];

  return (
    <IdentityContext.Provider value={identity}>
    <div className="flex h-screen bg-[var(--bg-window)] text-[var(--text-primary)]">
      {/* Focus mode: show only chat with exit button */}
      {focusMode ? (
        <>
          <main className="flex-1 flex min-w-0 min-h-0">
            {chatPanel}
          </main>
          <FocusModeExitButton onExit={() => setFocusMode(false)} />
        </>
      ) : (
        <>
          {/* Mobile backdrop */}
          {mobileMenuOpen && (
            <div
              className="fixed inset-0 z-40 bg-black/50 md:hidden"
              onClick={() => setMobileMenuOpen(false)}
            />
          )}

          {/* Unified Sidebar — single sidebar with all nav groups */}
          <UnifiedSidebar
            activeView={activeView}
            onViewChange={(v) => { setActiveView(v as ViewKey); setMobileMenuOpen(false); }}
            identity={identity}
            themeMode={theme.mode}
            onCycleTheme={theme.cycleMode}
            onToggleLanguage={toggleLanguage}
            connected={gw.connected}
            isOpen={mobileMenuOpen}
            onClose={() => setMobileMenuOpen(false)}
          />

          {/* Main content */}
          <main className="flex-1 flex flex-col min-w-0">
            {/* Chat view has its own top bar (session selector + model) — hide Toolbar */}
            {!isChatView && (
              <Toolbar
                title={currentPageTitle}
                subtitle={toolbarSubtitle}
                modelBadge={undefined}
                connected={gw.connected}
                status={toolbarStatus}
                onMenuClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                showMenu={true}
              />
            )}

            <div className="flex-1 flex overflow-hidden">
              {isChatView ? (
                chatPanel
              ) : (
                /* Dashboard content for the active tab */
                <Dashboard
                  connected={gw.connected}
                  sessionCount={session.sessions.length}
                  messageCount={allMessages.length}
                  activeTab={activeView as TabKey}
                  onNavigateToChat={(key) => {
                    session.setActiveKey(key);
                    setActiveView("chat");
                  }}
                />
              )}
            </div>
          </main>
        </>
      )}

      <Toaster />

      {/* Command Palette */}
      <CommandPalette
        open={showPalette}
        onClose={() => setShowPalette(false)}
        entries={paletteEntries}
      />

      {/* Setup Wizard */}
      <SetupWizard
        open={showWizard}
        onClose={() => setShowWizard(false)}
        onCall={gw.call}
      />
    </div>
    </IdentityContext.Provider>
  );
}
