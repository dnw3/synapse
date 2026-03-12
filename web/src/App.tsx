import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  PanelRightClose, PanelRightOpen,
} from "lucide-react";
import { useConversation } from "./hooks/useConversation";
import { useFiles } from "./hooks/useFiles";
import { useGatewayWS } from "./hooks/useGatewayWS";
import { useTheme } from "./hooks/useTheme";
import { Button } from "./components/ui/button";
import { SegmentedControl } from "./components/ui/segmented-control";
import { Toaster, useToast } from "./components/ui/toast";
import Sidebar from "./components/Sidebar";
import Toolbar from "./components/Toolbar";
import ChatPanel from "./components/ChatPanel";
import FileTree from "./components/FileTree";
import FileViewer from "./components/FileViewer";
import Canvas from "./components/Canvas";
import Dashboard, { TABS, SIDEBAR_SECTIONS, type TabKey } from "./components/Dashboard";
import type { Message, FileAttachment } from "./types";
import type { CanvasBlock } from "./types/canvas";
import type { IdentityInfo } from "./types/dashboard";
import { CANVAS_OPEN_RE, CANVAS_CLOSE } from "./types/canvas";

// Identity context for child components (e.g. MessageBubble)
export const IdentityContext = createContext<IdentityInfo | null>(null);
export const useIdentity = () => useContext(IdentityContext);

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
  const conv = useConversation();
  const files = useFiles(".");
  const ws = useGatewayWS(conv.activeId);
  const theme = useTheme();
  const { toast } = useToast();
  const [rightPanel, setRightPanel] = useState<RightTab>("files");
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const [rightWidth, setRightWidth] = useState(260);
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
      setRightWidth(Math.max(220, Math.min(500, startW + delta)));
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
      toast({
        variant: "info",
        title: `Task ${lastEvent.task_id}`,
        description: lastEvent.summary,
        duration: 5000,
      });
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

  const handleApprovalRespond = useCallback((approved: boolean, allowAll?: boolean) => {
    ws.send({ type: "approval_response", approved, allow_all: allowAll });
  }, [ws]);

  const allMessages = [...conv.messages, ...streamingMessages];

  // Canvas blocks = parsed from messages + persisted extra
  const parsedBlocks = parseCanvasBlocks(allMessages);
  const allCanvasBlocks = [...parsedBlocks, ...extraBlocks];

  const RIGHT_TABS: { key: RightTab; label: string }[] = [
    { key: "files", label: t("files.title") },
    { key: "viewer", label: t("files.viewer") },
    { key: "canvas", label: t("canvas.title") },
  ];

  const isChatView = activeView === "chat";

  // Toolbar title/subtitle computation
  const toolbarTitle = isChatView
    ? (conv.titles[conv.activeId ?? ""] || t("chat.newChat"))
    : t(TABS.find((tb) => tb.key === activeView)?.i18nKey ?? "app.title");
  const toolbarSubtitle = isChatView ? `${allMessages.length} msgs` : undefined;
  const toolbarModel = isChatView ? "claude-3.5-sonnet" : undefined;
  const toolbarStatus = (!["idle", "pong"].includes(ws.status)) ? t(`status.${ws.status}`) : undefined;

  return (
    <IdentityContext.Provider value={identity}>
    <div className="flex h-screen bg-[var(--bg-window)] text-[var(--text-primary)]">
      {/* Mobile backdrop */}
      {mobileMenuOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/50 md:hidden"
          onClick={() => setMobileMenuOpen(false)}
        />
      )}

      {/* Unified Sidebar */}
      <Sidebar
        conversations={conv.conversations}
        activeConversationId={conv.activeId}
        titles={conv.titles}
        onSelectConversation={(id) => { conv.setActiveId(id); setMobileMenuOpen(false); }}
        onNewConversation={() => { conv.createConversation(); setMobileMenuOpen(false); }}
        onDeleteConversation={conv.deleteConversation}
        activeView={activeView}
        onViewChange={(v) => { setActiveView(v as ViewKey); setMobileMenuOpen(false); }}
        tabs={TABS}
        sidebarSections={SIDEBAR_SECTIONS}
        identity={identity}
        themeMode={theme.mode}
        onCycleTheme={theme.cycleMode}
        onToggleLanguage={toggleLanguage}
        isOpen={mobileMenuOpen}
        onClose={() => setMobileMenuOpen(false)}
      />

      {/* Main content */}
      <main className="flex-1 flex flex-col min-w-0">
        <Toolbar
          title={toolbarTitle}
          subtitle={toolbarSubtitle}
          modelBadge={toolbarModel}
          connected={ws.connected}
          status={toolbarStatus}
          onMenuClick={() => setMobileMenuOpen(!mobileMenuOpen)}
          showMenu={true}
          actions={isChatView ? (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setRightCollapsed(!rightCollapsed)}
              className="hidden md:flex"
            >
              {rightCollapsed ? <PanelRightOpen className="h-4 w-4" /> : <PanelRightClose className="h-4 w-4" />}
            </Button>
          ) : undefined}
        />

        <div className="flex-1 flex overflow-hidden">
          {isChatView ? (
            <>
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

              {/* Resize handle */}
              {!rightCollapsed && (
                <div className="hidden md:flex flex-shrink-0 items-center">
                  <div
                    onMouseDown={onResizeStart}
                    className="w-[5px] h-full cursor-col-resize group flex items-center justify-center"
                  >
                    <div className="w-[1px] h-8 rounded-full bg-[var(--separator)] group-hover:bg-[var(--accent)]/40 transition-colors" />
                  </div>
                </div>
              )}

              {/* Right panel */}
              {!rightCollapsed && (
                <div className="hidden md:flex flex-col bg-[var(--bg-content)] flex-shrink-0 border-l border-[var(--separator)]" style={{ width: rightWidth }}>
                  <div className="flex items-center justify-center px-2 py-2 border-b border-[var(--separator)]">
                    <SegmentedControl
                      items={RIGHT_TABS.map((tab) => ({
                        label: tab.key === "canvas" && allCanvasBlocks.length > 0
                          ? `${tab.label} (${allCanvasBlocks.length})`
                          : tab.label,
                        value: tab.key,
                      }))}
                      value={rightPanel}
                      onChange={(v) => setRightPanel(v as RightTab)}
                      className="w-full"
                    />
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
            </>
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

      <Toaster />
    </div>
    </IdentityContext.Provider>
  );
}
