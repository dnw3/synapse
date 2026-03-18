import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useConversation } from "./hooks/useConversation";
import { useGatewayWS } from "./hooks/useGatewayWS";
import { useTheme } from "./hooks/useTheme";
import { Toaster, useToast } from "./components/ui/toast";
import UnifiedSidebar from "./components/UnifiedSidebar";
import Toolbar from "./components/Toolbar";
import ChatPanel, { FocusModeExitButton } from "./components/ChatPanel";
import Dashboard, { TABS, type TabKey } from "./components/Dashboard";
import CommandPalette, { type PaletteEntry } from "./components/CommandPalette";

import SetupWizard from "./components/SetupWizard";
import ToolOutputSidebar from "./components/ToolOutputSidebar";
import type { Message, FileAttachment } from "./types";
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
  const conv = useConversation();
  const ws = useGatewayWS(conv.activeId);
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
  const [chatAgent, setChatAgent] = useState("default");
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

  // (Mode toggle removed — unified sidebar handles all navigation)

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

  // ── Request ID (LogID) tracking ──────────────────────────────────────
  // Captured from WS status events for streaming display.
  // Historical messages get request_id from the backend (stored in additional_kwargs).
  const currentRequestIdRef = useRef<string | null>(null);

  useEffect(() => {
    if (ws.events.length === 0) return;
    const lastEvent = ws.events[ws.events.length - 1];

    // Capture request_id from status events
    if (lastEvent.type === "status" && lastEvent.request_id) {
      currentRequestIdRef.current = lastEvent.request_id;
    }

    // Handle Protocol v3 push events
    if (lastEvent.type === "event") {
      const evtFrame = lastEvent as { type: "event"; event: string; payload: unknown };
      if (evtFrame.event === "sessions.changed") {
        conv.refreshMessages();
      } else if (evtFrame.event === "session.compacted") {
        toast({
          variant: "info",
          title: t("toast.sessionCompacted"),
          duration: 4000,
        });
      } else if (evtFrame.event === "update.available") {
        const payload = evtFrame.payload as { version?: string } | null;
        toast({
          variant: "info",
          title: t("toast.updateAvailable", { version: payload?.version ?? "" }),
          duration: 8000,
        });
      }
    }

    if (lastEvent.type === "subagent_complete") {
      toast({
        variant: "info",
        title: `Task ${lastEvent.task_id}`,
        description: lastEvent.summary,
        duration: 5000,
      });
    }

    if (lastEvent.type === "error") {
      const errEvt = lastEvent as { type: "error"; message: string; request_id?: string };
      const rid = errEvt.request_id || currentRequestIdRef.current;
      const errorMsg = rid
        ? `${errEvt.message}\n[LogID: ${rid}]`
        : errEvt.message;
      setSendLock(false);
      setChatError(errorMsg);
      // Don't flush queue on error — let user decide
    }

    if (lastEvent.type === "done") {
      setSendLock(false);
      draftRef.current = null;
      conv.refreshMessages().then(() => {
        currentRequestIdRef.current = null;
        ws.clearEvents();
        // Flush queue: send next queued message (OpenClaw pattern)
        setMessageQueue((prev) => {
          if (prev.length === 0) return prev;
          const [next, ...rest] = prev;
          setSendLock(true);
          ws.send({
            type: "message",
            content: next.content,
            attachments: next.attachments && next.attachments.length > 0 ? next.attachments : undefined,
            idempotency_key: next.id,
          });
          return rest;
        });
      });
    }
  }, [ws.events]);

  // Poll for completion when reconnected mid-execution.
  useEffect(() => {
    if (ws.status !== "executing" && ws.status !== "thinking") return;
    if (!ws.connected) return;

    // Only poll if we have NO live streaming events (i.e. this is a reconnected session)
    const hasLiveTokens = ws.events.some(
      (e) => e.type === "token" || e.type === "tool_call" || e.type === "tool_result"
    );
    if (hasLiveTokens) return;

    const timer = setInterval(async () => {
      try {
        const result = await ws.call<{ executing: boolean }>("check_execution");
        if (!result.executing) {
          // Execution completed on the server — load final messages and clear status
          await conv.refreshMessages();
          ws.clearEvents();
        }
      } catch {
        // WS not ready or RPC failed, will retry next interval
      }
    }, 3000);

    return () => clearInterval(timer);
  }, [ws.status, ws.connected, ws.events.length]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggleLanguage = () => {
    const next = i18n.language.startsWith("zh") ? "en" : "zh";
    i18n.changeLanguage(next);
  };

  const pendingMessageRef = useRef<string | null>(null);
  const pendingAttachmentsRef = useRef<FileAttachment[] | null>(null);

  useEffect(() => {
    if (ws.connected && pendingMessageRef.current !== null) {
      const content = pendingMessageRef.current;
      const attachments = pendingAttachmentsRef.current;
      pendingMessageRef.current = null;
      pendingAttachmentsRef.current = null;
      // Human message was already added in handleSendMessage — just send via WS
      ws.send({
        type: "message",
        content,
        attachments: attachments && attachments.length > 0 ? attachments : undefined,
        idempotency_key: crypto.randomUUID(),
      });
    }
  }, [ws.connected]);

  // Local send lock: set immediately on send, cleared when "done" or "error" arrives.
  const [sendLock, setSendLock] = useState(false);
  const [messageQueue, setMessageQueue] = useState<Array<{ id: string; content: string; attachments?: FileAttachment[] }>>([]);
  const [chatError, setChatError] = useState<string | null>(null);
  const draftRef = useRef<{ content: string; attachments?: FileAttachment[] } | null>(null);

  const handleSendMessage = async (content: string, attachments?: FileAttachment[]) => {
    const humanMsg: Message = { role: "human", content, tool_calls: [] };
    const idempotencyKey = crypto.randomUUID();

    // Clear error on new send attempt
    setChatError(null);

    if (!conv.activeId) {
      // No active session — main session auto-creates on mount; skip.
      return;
    }

    // If busy, queue the message (OpenClaw pattern)
    if (sendLock) {
      setMessageQueue((prev) => [...prev, { id: idempotencyKey, content, attachments }]);
      conv.setMessages((prev) => [...prev, humanMsg]);
      return;
    }

    if (ws.connected) {
      setSendLock(true);
      draftRef.current = { content, attachments };
      conv.setMessages((prev) => [...prev, humanMsg]);
      ws.send({
        type: "message",
        content,
        attachments: attachments && attachments.length > 0 ? attachments : undefined,
        idempotency_key: idempotencyKey,
      });
    } else {
      await conv.sendMessage(content);
    }
  };

  const handleCancel = () => {
    ws.send({ type: "cancel" });
  };

  const handleApprovalRespond = useCallback((approved: boolean, allowAll?: boolean) => {
    ws.send({ type: "approval_response", approved, allow_all: allowAll });
  }, [ws]);

  // Build streaming messages from WS events and track approval requests
  const streamingMessages: Message[] = [];
  let currentAssistantContent = "";
  let currentReasoning = "";
  const streamingRequestId = currentRequestIdRef.current ?? undefined;
  let pendingApproval: { tool_name: string; args_preview: string; risk_level: string } | null = null;
  for (const evt of ws.events) {
    if (evt.type === "token") {
      currentAssistantContent += evt.content;
    } else if (evt.type === "reasoning") {
      currentReasoning += evt.content;
    } else if (evt.type === "tool_call") {
      if (currentAssistantContent || currentReasoning) {
        streamingMessages.push({ role: "assistant", content: currentAssistantContent, tool_calls: [], reasoning: currentReasoning || undefined });
        currentAssistantContent = "";
        currentReasoning = "";
      }
      streamingMessages.push({ role: "assistant", content: "", tool_calls: [{ name: evt.name, arguments: evt.args }] });
    } else if (evt.type === "tool_result") {
      streamingMessages.push({ role: "tool", content: evt.content, tool_calls: [] });
    } else if (evt.type === "approval_request") {
      pendingApproval = { tool_name: evt.tool_name, args_preview: evt.args_preview, risk_level: evt.risk_level };
    }
  }
  if (currentAssistantContent || currentReasoning) {
    streamingMessages.push({
      role: "assistant",
      content: currentAssistantContent,
      tool_calls: [],
      request_id: streamingRequestId,
      reasoning: currentReasoning || undefined,
    });
  }

  const allMessages = [...conv.messages, ...streamingMessages];

  const isChatView = activeView === "chat";

  // Toolbar title: breadcrumb style — "Synapse > Page"
  const activeConv = conv.conversations.find((c) => c.id === conv.activeId);
  const currentPageTitle = isChatView
    ? (activeConv?.display_name || activeConv?.title || t("chat.newChat"))
    : t(TABS.find((tb) => tb.key === activeView)?.i18nKey ?? "app.title");
  const toolbarSubtitle = isChatView ? t("sidebar.messages", { count: allMessages.length }) : undefined;
  const toolbarModel = isChatView ? (modelName ?? undefined) : undefined;
  const toolbarStatus = (!["idle", "pong"].includes(ws.status)) ? t(`status.${ws.status}`) : undefined;

  const chatPanel = (
    <div className="flex flex-1 min-w-0 min-h-0 overflow-hidden">
      <ChatPanel
        messages={allMessages}
        loading={sendLock || conv.loading || ws.status === "executing" || ws.status === "thinking"}
        streaming={currentAssistantContent.length > 0}
        approvalRequest={pendingApproval}
        onSend={handleSendMessage}
        onCancel={handleCancel}
        onApprovalRespond={handleApprovalRespond}
        onNewChat={() => conv.resetSession()}
        onReset={() => conv.resetSession()}
        onResetSession={() => conv.resetSession()}
        onToggleFocus={() => setFocusMode((f) => !f)}
        focusMode={focusMode}
        onClearMessages={() => conv.setMessages([])}
        onRefreshMessages={() => conv.refreshMessages()}
        queueSize={messageQueue.length}
        chatError={chatError}
        onDismissError={() => setChatError(null)}
        onToolResultClick={(content, toolName) => setToolSidebar({ open: true, content, toolName })}
        conversations={conv.conversations}
        activeSessionId={conv.activeId}
        onSelectSession={(id) => { conv.setActiveId(id); }}
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
      action: () => conv.resetSession(),
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
    // Sessions — recent conversations
    ...conv.conversations.slice(0, 10).map((c) => ({
      id: `session:${c.id}`,
      label: c.display_name || c.title || c.id.slice(0, 8),
      labelZh: c.display_name || c.title || c.id.slice(0, 8),
      category: "session" as const,
      icon: <MessageSquare className="h-3.5 w-3.5" />,
      action: () => {
        conv.setActiveId(c.id);
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
                connected={ws.connected}
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
                  connected={ws.connected}
                  conversationCount={conv.conversations.length}
                  messageCount={allMessages.length}
                  activeTab={activeView as TabKey}
                  onNavigateToChat={(id) => {
                    conv.ensureConversation(id);
                    conv.setActiveId(id);
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
        onCall={ws.call}
      />
    </div>
    </IdentityContext.Provider>
  );
}
