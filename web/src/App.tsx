import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Globe, PanelRightClose, PanelRightOpen,
  Sun, Moon, Monitor, Menu, X,
  MessageSquare as ChatIcon,
} from "lucide-react";
import { useConversation } from "./hooks/useConversation";
import { useFiles } from "./hooks/useFiles";
import { useGatewayWS } from "./hooks/useGatewayWS";
import { useTheme } from "./hooks/useTheme";
import { Button } from "./components/ui/button";
import Sidebar from "./components/Sidebar";
import ChatPanel from "./components/ChatPanel";
import FileTree from "./components/FileTree";
import FileViewer from "./components/FileViewer";
import Canvas from "./components/Canvas";
import StatusBar from "./components/StatusBar";
import Dashboard, { TABS, SIDEBAR_SECTIONS, type TabKey } from "./components/Dashboard";
import type { Message, FileAttachment } from "./types";
import type { CanvasBlock } from "./types/canvas";
import type { IdentityInfo } from "./types/dashboard";
import { CANVAS_OPEN_RE, CANVAS_CLOSE } from "./types/canvas";
import { cn } from "./lib/cn";

// Identity context for child components (e.g. MessageBubble)
export const IdentityContext = createContext<IdentityInfo | null>(null);
export const useIdentity = () => useContext(IdentityContext);

const MODE_ICONS = { light: Sun, dark: Moon, system: Monitor } as const;

// ---------------------------------------------------------------------------
// Canvas helpers
// ---------------------------------------------------------------------------

const CANVAS_STORAGE_KEY = "synapse-canvas-blocks";

function loadPersistedBlocks(convId: string | null | undefined): CanvasBlock[] {
  if (!convId) return [];
  try {
    const raw = localStorage.getItem(`${CANVAS_STORAGE_KEY}:${convId}`);
    return raw ? (JSON.parse(raw) as CanvasBlock[]) : [];
  } catch {
    return [];
  }
}

function persistBlocks(convId: string | null | undefined, blocks: CanvasBlock[]) {
  if (!convId) return;
  try {
    localStorage.setItem(`${CANVAS_STORAGE_KEY}:${convId}`, JSON.stringify(blocks));
  } catch {
    // ignore storage full
  }
}

function parseCanvasBlocks(messages: Message[]): CanvasBlock[] {
  const blocks: CanvasBlock[] = [];
  for (const msg of messages) {
    if (!msg.content) continue;
    CANVAS_OPEN_RE.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = CANVAS_OPEN_RE.exec(msg.content)) !== null) {
      const rawType = match[1] as CanvasBlock["type"];
      const attribs = match[2].trim();
      const openTag = match[0];
      const afterOpen = match.index + openTag.length;
      const closeIdx = msg.content.indexOf(CANVAS_CLOSE, afterOpen);
      if (closeIdx === -1) continue;
      const content = msg.content.slice(afterOpen, closeIdx);
      const attribMap: Record<string, string> = {};
      const attribRe = /(\w+)=(\S+)/g;
      let am: RegExpExecArray | null;
      while ((am = attribRe.exec(attribs)) !== null) {
        attribMap[am[1]] = am[2];
      }
      const validTypes: CanvasBlock["type"][] = ["code", "markdown", "chart", "form", "text"];
      const type: CanvasBlock["type"] = validTypes.includes(rawType) ? rawType : "text";
      blocks.push({
        id: `${msg.role}-${match.index}-${afterOpen}`,
        type,
        content,
        language: attribMap["lang"],
        metadata: Object.keys(attribMap).length > 0 ? attribMap : undefined,
        timestamp: Date.now(),
      });
    }
  }
  return blocks;
}

// ---------------------------------------------------------------------------
// Unified view type: "chat" or a dashboard tab key
// ---------------------------------------------------------------------------

type ViewKey = "chat" | TabKey;

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

type RightTab = "files" | "viewer" | "canvas";

export default function App() {
  const { t, i18n } = useTranslation();
  const isZh = i18n.language?.startsWith("zh");
  const conv = useConversation();
  const files = useFiles(".");
  const ws = useGatewayWS(conv.activeId);
  const theme = useTheme();
  const [rightPanel, setRightPanel] = useState<RightTab>("files");
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const [rightWidth, setRightWidth] = useState(360);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [activeView, setActiveView] = useState<ViewKey>("chat");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const draggingRef = useRef(false);

  // Canvas state
  const [extraBlocks, setExtraBlocks] = useState<CanvasBlock[]>(() => loadPersistedBlocks(conv.activeId));

  useEffect(() => {
    setExtraBlocks(loadPersistedBlocks(conv.activeId));
  }, [conv.activeId]);

  useEffect(() => {
    persistBlocks(conv.activeId, extraBlocks);
  }, [extraBlocks, conv.activeId]);

  const handleClearCanvas = () => {
    setExtraBlocks([]);
    if (conv.activeId) {
      localStorage.removeItem(`${CANVAS_STORAGE_KEY}:${conv.activeId}`);
    }
  };

  const onResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    const startX = e.clientX;
    const startW = rightWidth;
    const onMove = (ev: MouseEvent) => {
      if (!draggingRef.current) return;
      const delta = startX - ev.clientX;
      setRightWidth(Math.max(220, Math.min(700, startW + delta)));
    };
    const onUp = () => {
      draggingRef.current = false;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [rightWidth]);

  useEffect(() => {
    files.loadDirectory(".");
  }, []);

  // Fetch agent identity
  useEffect(() => {
    fetch("/api/dashboard/identity")
      .then(r => r.ok ? r.json() : null)
      .then((data: IdentityInfo | null) => {
        if (data) {
          setIdentity(data);
          // Apply theme color override
          if (data.theme_color) {
            document.documentElement.style.setProperty("--accent", data.theme_color);
          }
        }
      })
      .catch(() => {});
  }, []);

  // Track canvas blocks pushed from server via WS
  const [wsCanvasBlocks, setWsCanvasBlocks] = useState<
    Array<{ id: string; type: string; content: string; language?: string; timestamp: number }>
  >([]);

  const [notification, setNotification] = useState<string | null>(null);

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

    if (lastEvent.type === "subagent_complete") {
      const msg = `Task ${lastEvent.task_id}: ${lastEvent.summary}`;
      setNotification(msg);
      setTimeout(() => setNotification(null), 5000);
    }

    if (lastEvent.type === "canvas_update") {
      setWsCanvasBlocks((prev) => [
        ...prev,
        {
          id: `ws-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
          type: lastEvent.block_type,
          content: lastEvent.content,
          language: lastEvent.language,
          timestamp: Date.now(),
        },
      ]);
      // Auto-switch to canvas tab when new block arrives
      if (rightCollapsed) setRightCollapsed(false);
      setRightPanel("canvas");
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
  // The old WS received streaming tokens; this new one only knows status is "executing".
  // Use the check_execution RPC to detect when the server finishes, then load final messages.
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
  // This prevents rapid duplicate sends before the WS status event propagates.
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
      const created = await conv.createConversation([humanMsg]);
      conv.setTitles((prev) => ({ ...prev, [created.id]: content }));
      pendingMessageRef.current = content;
      pendingAttachmentsRef.current = attachments ?? null;
      setSendLock(true);
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
      conv.setTitles((prev) => prev[conv.activeId!] ? prev : { ...prev, [conv.activeId!]: content });
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

  const handleFormSubmit = (blockId: string, values: Record<string, string | boolean>) => {
    ws.send({ type: "form_submit", block_id: blockId, values });
  };

  const handleFileSelect = (path: string, isDir: boolean) => {
    if (isDir) {
      files.loadDirectory(path);
    } else {
      files.openFile(path);
      setRightPanel("viewer");
    }
  };

  // Build streaming messages from WS events and track approval requests
  const streamingMessages: Message[] = [];
  let currentAssistantContent = "";
  const streamingRequestId = currentRequestIdRef.current ?? undefined;
  let pendingApproval: { tool_name: string; args_preview: string; risk_level: string } | null = null;
  for (const evt of ws.events) {
    if (evt.type === "token") {
      currentAssistantContent += evt.content;
    } else if (evt.type === "tool_call") {
      if (currentAssistantContent) {
        streamingMessages.push({ role: "assistant", content: currentAssistantContent, tool_calls: [] });
        currentAssistantContent = "";
      }
      streamingMessages.push({ role: "assistant", content: "", tool_calls: [{ name: evt.name, arguments: evt.args }] });
    } else if (evt.type === "tool_result") {
      streamingMessages.push({ role: "tool", content: evt.content, tool_calls: [] });
    } else if (evt.type === "approval_request") {
      pendingApproval = { tool_name: evt.tool_name, args_preview: evt.args_preview, risk_level: evt.risk_level };
    }
  }
  if (currentAssistantContent) {
    streamingMessages.push({ role: "assistant", content: currentAssistantContent, tool_calls: [], request_id: streamingRequestId });
  }

  const handleApprovalRespond = useCallback((approved: boolean, allowAll?: boolean) => {
    ws.send({ type: "approval_response", approved, allow_all: allowAll });
  }, [ws]);

  const allMessages = [...conv.messages, ...streamingMessages];

  // Canvas blocks = parsed from messages + persisted extra
  const parsedBlocks = parseCanvasBlocks(allMessages);
  const allCanvasBlocks = [...parsedBlocks, ...extraBlocks];

  const ModeIcon = MODE_ICONS[theme.mode];

  const RIGHT_TABS: { key: RightTab; label: string }[] = [
    { key: "files", label: t("files.title") },
    { key: "viewer", label: t("files.viewer") },
    { key: "canvas", label: t("canvas.title") },
  ];

  const isChatView = activeView === "chat";

  return (
    <IdentityContext.Provider value={identity}>
    <div className="flex flex-col h-screen bg-[var(--bg-primary)] text-[var(--text-primary)]">
      {/* ── Header ─────────────────────────────────── */}
      <header className="flex items-center justify-between px-3 sm:px-5 h-12 border-b border-[var(--border-subtle)] bg-[var(--bg-elevated)]/80 backdrop-blur-md flex-shrink-0">
        <div className="flex items-center gap-2 sm:gap-3">
          {/* Mobile hamburger */}
          <button
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            className="md:hidden p-1.5 text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors rounded-[var(--radius-sm)]"
          >
            {mobileMenuOpen ? <X className="h-4 w-4" /> : <Menu className="h-4 w-4" />}
          </button>
          {identity?.avatar_url ? (
            <img src={identity.avatar_url} alt="" className="w-7 h-7 rounded-lg object-cover shadow-[0_2px_8px_var(--accent-glow)]" />
          ) : identity?.emoji ? (
            <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-[var(--accent)] to-[var(--accent-gradient-end)] flex items-center justify-center text-sm shadow-[0_2px_8px_var(--accent-glow)]">
              {identity.emoji}
            </div>
          ) : (
            <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-[var(--accent)] to-[var(--accent-gradient-end)] flex items-center justify-center text-white font-semibold text-xs shadow-[0_2px_8px_var(--accent-glow)]">
              S
            </div>
          )}
          <h1 className="text-[15px] font-semibold tracking-[-0.01em]">{identity?.name || t("app.title")}</h1>
          <span className="text-xs text-[var(--text-tertiary)] hidden sm:inline">{t("app.subtitle")}</span>
        </div>
        <div className="flex items-center gap-2">
          {/* Theme mode toggle */}
          <Button variant="ghost" size="icon" onClick={theme.cycleMode} title={t("settings.theme")} className="h-8 w-8">
            <ModeIcon className="h-3.5 w-3.5" />
          </Button>

          {/* Language toggle */}
          <Button variant="ghost" size="icon" onClick={toggleLanguage} title={t("settings.language")} className="h-8 w-8">
            <Globe className="h-3.5 w-3.5" />
          </Button>

          {/* Connection status badge — only in chat view (Dashboard has its own health check) */}
          {isChatView && (
            <div className={cn(
              "hidden sm:flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium border",
              ws.connected
                ? "bg-[var(--success)]/8 text-[var(--success)] border-[var(--success)]/20"
                : "bg-[var(--text-tertiary)]/8 text-[var(--text-tertiary)] border-[var(--text-tertiary)]/20"
            )}>
              <span className={cn(
                "w-1.5 h-1.5 rounded-full flex-shrink-0",
                ws.connected ? "bg-[var(--success)]" : "bg-[var(--text-tertiary)]"
              )} />
              {ws.connected ? t("app.connected") : t("app.disconnected")}
            </div>
          )}
          {isChatView && ws.status !== "idle" && (
            <span className="px-2 py-0.5 text-[10px] font-medium bg-[var(--accent-glow)] text-[var(--accent-light)] border border-[var(--accent)]/20 rounded-full animate-pulse-glow">
              {t(`status.${ws.status}`)}
            </span>
          )}
        </div>
      </header>

      {/* ── Body: Unified Sidebar + Main Content ─── */}
      <div className="flex flex-1 overflow-hidden relative">

        {/* Mobile sidebar overlay */}
        {mobileMenuOpen && (
          <div
            className="fixed inset-0 z-40 bg-black/50 md:hidden"
            onClick={() => setMobileMenuOpen(false)}
          />
        )}

        {/* ── Unified Left Sidebar (OpenClaw-style) ── */}
        <aside className={cn(
          "flex flex-col h-full w-[220px] bg-[var(--bg-elevated)] border-r border-[var(--border-subtle)] flex-shrink-0",
          // Mobile: fixed overlay
          "fixed inset-y-12 left-0 z-50 md:relative md:inset-auto",
          mobileMenuOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"
        )}>
          <nav className="flex-1 py-2 px-2 overflow-y-auto">
            {/* ── 聊天 Section ── */}
            <div>
              <div className="flex items-center justify-between px-3 mb-1">
                <span className="text-[11px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider">
                  {t("dashboard.chat")}
                </span>
                <span className="text-[var(--text-tertiary)] text-[10px]">–</span>
              </div>
              <button
                onClick={() => { setActiveView("chat"); setMobileMenuOpen(false); }}
                className={cn(
                  "relative flex items-center gap-2.5 w-full rounded-[var(--radius-md)] text-[13px] font-medium transition-all duration-150 cursor-pointer px-3 py-2",
                  isChatView
                    ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                )}
              >
                <ChatIcon className="h-4 w-4 flex-shrink-0" />
                <span className="truncate">{t("dashboard.chat")}</span>
              </button>
            </div>

            {/* ── Dashboard Sections ── */}
            {SIDEBAR_SECTIONS.map((section, si) => (
              <div key={si} className="mt-4">
                <div className="flex items-center justify-between px-3 mb-1">
                  <span className="text-[11px] font-medium text-[var(--text-tertiary)] uppercase tracking-wider">
                    {t(section.i18nKey)}
                  </span>
                  <span className="text-[var(--text-tertiary)] text-[10px]">–</span>
                </div>
                <div className="space-y-0.5">
                  {section.keys.map((key) => {
                    const tab = TABS.find((t) => t.key === key);
                    if (!tab) return null;
                    const isActive = activeView === key;
                    return (
                      <button
                        key={key}
                        onClick={() => { setActiveView(key); setMobileMenuOpen(false); }}
                        className={cn(
                          "relative flex items-center gap-2.5 w-full rounded-[var(--radius-md)] text-[13px] font-medium transition-all duration-150 cursor-pointer px-3 py-2",
                          isActive
                            ? "bg-[var(--accent)]/10 text-[var(--accent)]"
                            : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                        )}
                      >
                        <span className="flex-shrink-0">{tab.icon}</span>
                        <span className="truncate">{t(tab.i18nKey)}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </nav>
        </aside>

        {/* ── Main Content Area ─────────────────────── */}
        {isChatView ? (
          <>
            {/* Chat: Conversation sidebar + Chat panel + Right panel */}
            <div className="flex flex-1 min-w-0 overflow-hidden">
              {/* Conversation list sidebar */}
              <Sidebar
                conversations={conv.conversations}
                activeId={conv.activeId}
                titles={conv.titles}
                collapsed={false}
                onToggle={() => {}}
                onSelect={(id) => { conv.setActiveId(id); setMobileMenuOpen(false); }}
                onCreate={() => { conv.createConversation(); setMobileMenuOpen(false); }}
                onDelete={conv.deleteConversation}
              />

              {/* Chat panel */}
              <div className="flex-1 flex flex-col min-w-0 min-h-0">
                <ChatPanel
                  messages={allMessages}
                  loading={sendLock || conv.loading || ws.status === "executing" || ws.status === "thinking"}
                  streaming={currentAssistantContent.length > 0}
                  approvalRequest={pendingApproval}
                  onSend={handleSendMessage}
                  onCancel={handleCancel}
                  onApprovalRespond={handleApprovalRespond}
                  onNewChat={() => conv.createConversation()}
                  onReset={() => {
                    if (conv.activeId) {
                      conv.deleteConversation(conv.activeId);
                      conv.createConversation();
                    }
                  }}
                  onClearCanvas={handleClearCanvas}
                  queueSize={messageQueue.length}
                  chatError={chatError}
                  onDismissError={() => setChatError(null)}
                />
              </div>

              {/* Resize handle + collapse toggle */}
              <div className="hidden md:flex flex-col flex-shrink-0 border-l border-[var(--border-subtle)]">
                <button
                  onClick={() => setRightCollapsed(!rightCollapsed)}
                  className="flex items-center justify-center h-12 w-5 hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors"
                  title={rightCollapsed ? t("files.expand") : t("files.collapse")}
                >
                  {rightCollapsed ? <PanelRightOpen className="h-3.5 w-3.5" /> : <PanelRightClose className="h-3.5 w-3.5" />}
                </button>
                {!rightCollapsed && (
                  <div
                    onMouseDown={onResizeStart}
                    className="flex-1 w-5 cursor-col-resize group flex items-center justify-center"
                  >
                    <div className="w-[2px] h-8 rounded-full bg-[var(--border-subtle)] group-hover:bg-[var(--accent)]/40 transition-colors" />
                  </div>
                )}
              </div>

              {/* Right panel: Files / Viewer / Canvas */}
              {!rightCollapsed && (
                <div className="hidden md:flex flex-col bg-[var(--bg-elevated)]/50 flex-shrink-0" style={{ width: rightWidth }}>
                  <div className="flex h-10 border-b border-[var(--border-subtle)]">
                    {RIGHT_TABS.map((tab) => (
                      <button
                        key={tab.key}
                        onClick={() => setRightPanel(tab.key)}
                        className={cn(
                          "flex-1 text-xs font-medium transition-colors relative flex items-center justify-center gap-1",
                          rightPanel === tab.key
                            ? "text-[var(--text-primary)]"
                            : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
                        )}
                      >
                        {tab.label}
                        {tab.key === "canvas" && allCanvasBlocks.length > 0 && (
                          <span className="min-w-[16px] h-4 flex items-center justify-center text-[9px] font-semibold rounded-full bg-[var(--accent-glow)] text-[var(--accent-light)]">
                            {allCanvasBlocks.length}
                          </span>
                        )}
                        {rightPanel === tab.key && (
                          <span className="absolute bottom-0 left-1/2 -translate-x-1/2 w-8 h-[2px] rounded-full bg-[var(--accent)]" />
                        )}
                      </button>
                    ))}
                  </div>
                  <div className="flex-1 overflow-auto">
                    {rightPanel === "files" && (
                      <FileTree
                        currentPath={files.currentPath}
                        entries={files.entries}
                        onSelect={handleFileSelect}
                        onNavigateUp={files.navigateUp}
                      />
                    )}
                    {rightPanel === "viewer" && (
                      <FileViewer
                        path={files.selectedFile}
                        content={files.fileContent}
                        theme={theme.resolved}
                        onNavigate={(dir) => {
                          files.loadDirectory(dir);
                          setRightPanel("files");
                        }}
                      />
                    )}
                    {rightPanel === "canvas" && (
                      <Canvas
                        canvasBlocks={allCanvasBlocks}
                        onClear={handleClearCanvas}
                        onFormSubmit={handleFormSubmit}
                      />
                    )}
                  </div>
                </div>
              )}
            </div>
          </>
        ) : (
          /* Dashboard content for the active tab */
          <Dashboard
            connected={ws.connected}
            conversationCount={conv.conversations.length}
            messageCount={allMessages.length}
            activeTab={activeView as TabKey}
          />
        )}
      </div>

      {/* Status bar — only in chat view */}
      {isChatView && (
        <StatusBar
          conversationId={conv.activeId}
          messageCount={allMessages.length}
          status={ws.status}
          connected={ws.connected}
        />
      )}

      {/* Toast notifications */}
      {notification && (
        <div className="fixed bottom-6 right-6 z-50 animate-in slide-in-from-bottom-3">
          <div className="bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg px-4 py-3 text-sm text-[var(--text-primary)] max-w-sm">
            {notification}
          </div>
        </div>
      )}
    </div>
    </IdentityContext.Provider>
  );
}
