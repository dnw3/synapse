import { useCallback, useEffect, useRef, useState } from "react";
import type { Session, Message, FileAttachment } from "../types";
import type { UseGatewayReturn } from "./useGateway";

/** Streaming event types that build real-time messages. */
export interface StreamingState {
  messages: Message[];
  pendingApproval: { tool_name: string; args_preview: string; risk_level: string } | null;
  requestId: string | null;
}

export interface UseSessionReturn {
  sessions: Session[];
  activeKey: string | null;
  setActiveKey: (key: string) => void;
  messages: Message[];
  loading: boolean;
  sendMessage: (content: string, attachments?: FileAttachment[]) => void;
  deleteSession: (key: string) => Promise<void>;
  resetSession: () => Promise<void>;
  refreshMessages: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  setMessages: (updater: (prev: Message[]) => Message[]) => void;
  streaming: StreamingState;
  sendLock: boolean;
  cancelGeneration: () => void;
  respondApproval: (approved: boolean, allowAll?: boolean) => void;
  messageQueue: Array<{ id: string; content: string; attachments?: FileAttachment[] }>;
  chatError: string | null;
  dismissError: () => void;
}

export function useSession(gw: UseGatewayReturn): UseSessionReturn {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [sendLock, setSendLock] = useState(false);
  const [messageQueue, setMessageQueue] = useState<Array<{ id: string; content: string; attachments?: FileAttachment[] }>>([]);
  const [chatError, setChatError] = useState<string | null>(null);

  // Streaming state
  const [streamingMessages, setStreamingMessages] = useState<Message[]>([]);
  const [pendingApproval, setPendingApproval] = useState<{ tool_name: string; args_preview: string; risk_level: string } | null>(null);
  const [currentRequestId, setCurrentRequestId] = useState<string | null>(null);

  // Accumulator refs for streaming content
  const assistantContentRef = useRef("");
  const reasoningContentRef = useRef("");

  // Per-session message cache
  const messageCacheRef = useRef<Record<string, Message[]>>({});
  const activeKeyRef = useRef<string | null>(null);

  // Pending message for when WS reconnects
  const pendingMessageRef = useRef<{ content: string; attachments?: FileAttachment[] } | null>(null);

  // Keep activeKeyRef in sync via effect (not during render)
  useEffect(() => {
    activeKeyRef.current = activeKey;
  }, [activeKey]);

  const clearStreaming = useCallback(() => {
    setStreamingMessages([]);
    setPendingApproval(null);
    setCurrentRequestId(null);
    assistantContentRef.current = "";
    reasoningContentRef.current = "";
  }, []);

  // Load sessions on mount + when connected
  const refreshSessions = useCallback(async () => {
    if (!gw.connected) return;
    try {
      const result = await gw.call<{ sessions: Session[] }>("sessions.list");
      const list = result.sessions ?? (result as unknown as Session[]);
      setSessions(Array.isArray(list) ? list : []);
    } catch {
      // Will retry on reconnect
    }
  }, [gw.connected]); // eslint-disable-line react-hooks/exhaustive-deps

  // Load messages for active session
  const refreshMessages = useCallback(async () => {
    const key = activeKeyRef.current;
    if (!key || !gw.connected) return;
    try {
      const result = await gw.call<{ messages: Message[] }>("chat.history", { sessionKey: key });
      const msgs = result.messages ?? (result as unknown as Message[]);
      setMessages(msgs);
      messageCacheRef.current[key] = msgs;
    } catch {
      // ignore
    }
  }, [gw.connected]); // eslint-disable-line react-hooks/exhaustive-deps

  // On connect: load sessions, select default
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    if (!gw.connected) return;
    refreshSessions().then(() => {
      // Send pending message if any
      if (pendingMessageRef.current && activeKeyRef.current) {
        const { content, attachments } = pendingMessageRef.current;
        pendingMessageRef.current = null;
        const id = rpcId();
        const params: Record<string, unknown> = {
          sessionKey: activeKeyRef.current,
          message: content,
          idempotencyKey: id,
        };
        if (attachments && attachments.length > 0) {
          params.attachments = attachments;
        }
        gw.send({
          type: "request",
          id,
          method: "chat.send",
          params,
        });
      }
    });
  }, [gw.connected]); // eslint-disable-line react-hooks/exhaustive-deps

  // Select default session when sessions load and no active key
  useEffect(() => {
    if (activeKey || sessions.length === 0) return;
    const webSession = sessions.find(s => s.channel === "web" || s.channel === "webchat");
    const defaultKey = webSession?.sessionKey ?? sessions[0]?.sessionKey ?? "main";
    setActiveKey(defaultKey);
  }, [sessions, activeKey]);

  // Load messages when active key changes
  useEffect(() => {
    if (!activeKey) {
      setMessages([]);
      return;
    }

    // Restore from cache
    const cached = messageCacheRef.current[activeKey];
    if (cached && cached.length > 0) {
      setMessages(cached);
    } else {
      setMessages([]);
    }

    // Clear streaming from previous session
    clearStreaming();
    setSendLock(false);
    setChatError(null);

    // Fetch from server
    if (gw.connected) {
      setLoading(true);
      gw.call<{ messages: Message[] }>("chat.history", { sessionKey: activeKey })
        .then(result => {
          const msgs = result.messages ?? (result as unknown as Message[]);
          setMessages(current => (msgs.length >= current.length ? msgs : current));
          messageCacheRef.current[activeKey] = msgs;
        })
        .catch(() => {})
        .finally(() => setLoading(false));
    }
  }, [activeKey, gw.connected]); // eslint-disable-line react-hooks/exhaustive-deps
  /* eslint-enable react-hooks/set-state-in-effect */

  // Subscribe to streaming events
  useEffect(() => {
    const unsubscribe = gw.subscribe((event, payload) => {
      // Filter by sessionKey if present
      const evtKey = payload.sessionKey as string | undefined;
      if (evtKey && evtKey !== activeKeyRef.current) return;

      switch (event) {
        case "agent.message.start":
          setCurrentRequestId((payload.request_id as string) ?? null);
          break;

        case "agent.message.delta":
          assistantContentRef.current += (payload.content as string) ?? "";
          // Trigger re-render with current accumulated content
          setStreamingMessages(prev => {
            const newMsg: Message = {
              role: "assistant",
              content: assistantContentRef.current,
              tool_calls: [],
              reasoning: reasoningContentRef.current || undefined,
            };
            // Replace last assistant message or add new one
            const lastIdx = prev.length - 1;
            if (lastIdx >= 0 && prev[lastIdx].role === "assistant" && prev[lastIdx].tool_calls.length === 0) {
              const updated = [...prev];
              updated[lastIdx] = newMsg;
              return updated;
            }
            return [...prev, newMsg];
          });
          break;

        case "agent.thinking.delta":
          reasoningContentRef.current += (payload.content as string) ?? "";
          setStreamingMessages(prev => {
            const newMsg: Message = {
              role: "assistant",
              content: assistantContentRef.current,
              tool_calls: [],
              reasoning: reasoningContentRef.current || undefined,
            };
            const lastIdx = prev.length - 1;
            if (lastIdx >= 0 && prev[lastIdx].role === "assistant" && prev[lastIdx].tool_calls.length === 0) {
              const updated = [...prev];
              updated[lastIdx] = newMsg;
              return updated;
            }
            return [...prev, newMsg];
          });
          break;

        case "agent.tool.start":
          // Flush any accumulated assistant content first
          if (assistantContentRef.current || reasoningContentRef.current) {
            setStreamingMessages(prev => {
              // Check if the last message is already the streaming assistant msg — keep it
              const last = prev[prev.length - 1];
              const needsFlush = !last || last.role !== "assistant" || last.tool_calls.length > 0;
              const base = needsFlush ? [...prev, {
                role: "assistant" as const,
                content: assistantContentRef.current,
                tool_calls: [],
                reasoning: reasoningContentRef.current || undefined,
              }] : prev;
              assistantContentRef.current = "";
              reasoningContentRef.current = "";
              return [
                ...base,
                {
                  role: "assistant" as const,
                  content: "",
                  tool_calls: [{ name: (payload.name as string) ?? "", arguments: (payload.args as Record<string, unknown>) ?? {} }],
                },
              ];
            });
          } else {
            setStreamingMessages(prev => [
              ...prev,
              {
                role: "assistant" as const,
                content: "",
                tool_calls: [{ name: (payload.name as string) ?? "", arguments: (payload.args as Record<string, unknown>) ?? {} }],
              },
            ]);
          }
          break;

        case "agent.tool.result":
          setStreamingMessages(prev => [
            ...prev,
            { role: "tool" as const, content: (payload.content as string) ?? "", tool_calls: [] },
          ]);
          break;

        case "approval.requested":
          setPendingApproval({
            tool_name: (payload.tool_name as string) ?? "",
            args_preview: (payload.args_preview as string) ?? "",
            risk_level: (payload.risk_level as string) ?? "",
          });
          break;

        case "agent.turn.complete": {
          setSendLock(false);
          // Refresh messages from server, then clear streaming
          const key = activeKeyRef.current;
          if (key && gw.connected) {
            gw.call<{ messages: Message[] }>("chat.history", { sessionKey: key })
              .then(result => {
                const msgs = result.messages ?? (result as unknown as Message[]);
                setMessages(msgs);
                messageCacheRef.current[key] = msgs;
              })
              .catch(() => {})
              .finally(() => {
                clearStreaming();
                // Flush queue
                setMessageQueue(prev => {
                  if (prev.length === 0) return prev;
                  const [next, ...rest] = prev;
                  setSendLock(true);
                  gw.send({
                    type: "request",
                    id: next.id,
                    method: "chat.send",
                    params: {
                      sessionKey: activeKeyRef.current,
                      message: next.content,
                      idempotencyKey: next.id,
                      ...(next.attachments && next.attachments.length > 0 ? { attachments: next.attachments } : {}),
                    },
                  });
                  return rest;
                });
              });
          } else {
            clearStreaming();
          }
          break;
        }

        case "agent.error": {
          const rid = (payload.request_id as string) ?? null;
          const msg = (payload.message as string) ?? "Unknown error";
          const errorMsg = rid ? `${msg}\n[LogID: ${rid}]` : msg;
          setSendLock(false);
          setChatError(errorMsg);
          break;
        }

        case "sessions.changed":
          refreshSessions();
          refreshMessages();
          break;

        case "session.compacted":
          // Handled by App.tsx toast — just refresh messages
          refreshMessages();
          break;

        default:
          break;
      }
    });

    return unsubscribe;
  }, [gw, clearStreaming, refreshSessions, refreshMessages]);

  const sendMessage = useCallback((content: string, attachments?: FileAttachment[]) => {
    const key = activeKeyRef.current;
    if (!key) return;

    const humanMsg: Message = { role: "human", content, tool_calls: [] };
    const idempotencyKey = crypto.randomUUID();

    setChatError(null);

    if (sendLock) {
      setMessageQueue(prev => [...prev, { id: idempotencyKey, content, attachments }]);
      setMessages(prev => [...prev, humanMsg]);
      return;
    }

    if (!gw.connected) {
      pendingMessageRef.current = { content, attachments };
      setMessages(prev => [...prev, humanMsg]);
      return;
    }

    setSendLock(true);
    setMessages(prev => [...prev, humanMsg]);

    const params: Record<string, unknown> = {
      sessionKey: key,
      message: content,
      idempotencyKey,
    };
    if (attachments && attachments.length > 0) {
      params.attachments = attachments;
    }
    gw.send({
      type: "request",
      id: rpcId(),
      method: "chat.send",
      params,
    });
  }, [gw, sendLock]);

  const deleteSession = useCallback(async (key: string) => {
    await gw.call("sessions.delete", { sessionKey: key });
    delete messageCacheRef.current[key];
    setSessions(prev => prev.filter(s => s.sessionKey !== key));
    if (activeKey === key) {
      setActiveKey(null);
      setMessages([]);
    }
  }, [gw, activeKey]);

  const resetSession = useCallback(async () => {
    const key = activeKeyRef.current;
    if (key) {
      try {
        await gw.call("sessions.delete", { sessionKey: key });
      } catch {
        // ignore
      }
      delete messageCacheRef.current[key];
    }
    setMessages([]);
    clearStreaming();
    setSendLock(false);
    setChatError(null);
    // Refresh sessions — backend may auto-recreate
    await refreshSessions();
  }, [gw, clearStreaming, refreshSessions]);

  const cancelGeneration = useCallback(() => {
    gw.send({
      type: "request",
      id: rpcId(),
      method: "chat.stop",
      params: {},
    });
  }, [gw.connected]); // eslint-disable-line react-hooks/exhaustive-deps

  const respondApproval = useCallback((approved: boolean, allowAll?: boolean) => {
    const method = approved ? "approval.approve" : "approval.deny";
    gw.send({
      type: "request",
      id: rpcId(),
      method,
      params: { allow_all: allowAll ?? false },
    });
  }, [gw.connected]); // eslint-disable-line react-hooks/exhaustive-deps

  const dismissError = useCallback(() => setChatError(null), []);

  const streaming: StreamingState = {
    messages: streamingMessages,
    pendingApproval,
    requestId: currentRequestId,
  };

  return {
    sessions,
    activeKey,
    setActiveKey,
    messages,
    loading,
    sendMessage,
    deleteSession,
    resetSession,
    refreshMessages,
    refreshSessions,
    setMessages,
    streaming,
    sendLock,
    cancelGeneration,
    respondApproval,
    messageQueue,
    chatError,
    dismissError,
  };
}

function rpcId(): string {
  return `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}
