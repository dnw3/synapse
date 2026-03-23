import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import type { Message, FileAttachment } from "../types";
import type { UseGatewayReturn } from "./useGateway";
import { useSessionLifecycle } from "./useSessionLifecycle";
import { useStreamingHandler } from "./useStreamingHandler";
import type { ApprovalRequest } from "./useStreamingHandler";

// ---------------------------------------------------------------------------
// Public types (unchanged from original)
// ---------------------------------------------------------------------------

export interface StreamingState {
  messages: Message[];
  pendingApproval: { tool_name: string; args_preview: string; risk_level: string } | null;
  requestId: string | null;
}

export interface UseSessionReturn {
  sessions: import("../types").Session[];
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

// ---------------------------------------------------------------------------
// Chat state reducer
// ---------------------------------------------------------------------------

type ChatState = {
  sendLock: boolean;
  chatError: string | null;
  pendingApproval: ApprovalRequest | null;
};

type ChatAction =
  | { type: "LOCK" }
  | { type: "UNLOCK" }
  | { type: "SET_ERROR"; error: string }
  | { type: "CLEAR_ERROR" }
  | { type: "SET_APPROVAL"; request: ApprovalRequest }
  | { type: "CLEAR_APPROVAL" };

const initialChatState: ChatState = {
  sendLock: false,
  chatError: null,
  pendingApproval: null,
};

function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case "LOCK":
      return { ...state, sendLock: true };
    case "UNLOCK":
      return { ...state, sendLock: false };
    case "SET_ERROR":
      return { ...state, chatError: action.error };
    case "CLEAR_ERROR":
      return { ...state, chatError: null };
    case "SET_APPROVAL":
      return { ...state, pendingApproval: action.request };
    case "CLEAR_APPROVAL":
      return { ...state, pendingApproval: null };
    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Composition hook
// ---------------------------------------------------------------------------

export function useSession(gw: UseGatewayReturn): UseSessionReturn {
  const lifecycle = useSessionLifecycle(gw);
  const [chatState, dispatch] = useReducer(chatReducer, initialChatState);
  const [messageQueue, setMessageQueue] = useState<Array<{ id: string; content: string; attachments?: FileAttachment[] }>>([]);

  // Ref to clearStreaming so onTurnComplete can call it without a forward reference
  const clearStreamingRef = useRef<() => void>(() => {});

  // --- Callbacks for streaming handler ---

  const onTurnComplete = useCallback(() => {
    dispatch({ type: "UNLOCK" });
    // Refresh messages from server, then clear streaming & flush queue
    const key = lifecycle.activeKeyRef.current;
    if (key && lifecycle.gwRef.current.connected) {
      lifecycle.gwRef.current.call<{ messages: Message[] }>("chat.history", { sessionKey: key })
        .then(result => {
          const msgs = result.messages ?? (result as unknown as Message[]);
          lifecycle.setMessages(msgs);
          lifecycle.updateCache(key, msgs);
        })
        .catch(() => {})
        .finally(() => {
          clearStreamingRef.current();
          // Flush queue
          setMessageQueue(prev => {
            if (prev.length === 0) return prev;
            const [next, ...rest] = prev;
            dispatch({ type: "LOCK" });
            lifecycle.gwRef.current.send({
              type: "request",
              id: next.id,
              method: "chat.send",
              params: {
                sessionKey: lifecycle.activeKeyRef.current,
                message: next.content,
                idempotencyKey: next.id,
                ...(next.attachments && next.attachments.length > 0 ? { attachments: next.attachments } : {}),
              },
            });
            return rest;
          });
        });
    } else {
      clearStreamingRef.current();
    }
  }, [lifecycle]);

  const onApproval = useCallback((request: ApprovalRequest) => {
    dispatch({ type: "SET_APPROVAL", request });
  }, []);

  const onError = useCallback((error: string) => {
    dispatch({ type: "SET_ERROR", error });
    dispatch({ type: "UNLOCK" });
  }, []);

  const onSessionsChanged = useCallback(() => {
    lifecycle.refreshSessions();
    lifecycle.refreshMessages();
  }, [lifecycle]);

  const streamingHandler = useStreamingHandler(
    gw,
    lifecycle.activeKeyRef,
    onTurnComplete,
    onApproval,
    onError,
    onSessionsChanged,
  );

  // Keep the ref in sync
  useEffect(() => {
    clearStreamingRef.current = streamingHandler.clearStreaming;
  });

  // Clear streaming + chat state on session switch (matches original behavior)
  useEffect(() => {
    streamingHandler.clearStreaming();
    dispatch({ type: "UNLOCK" });
    dispatch({ type: "CLEAR_ERROR" });
  }, [lifecycle.activeKey]); // eslint-disable-line react-hooks/exhaustive-deps

  // --- Public actions ---

  const sendMessage = useCallback((content: string, attachments?: FileAttachment[]) => {
    const key = lifecycle.activeKeyRef.current;
    if (!key) return;

    const humanMsg: Message = { role: "human", content, tool_calls: [] };
    const idempotencyKey = crypto.randomUUID();

    dispatch({ type: "CLEAR_ERROR" });

    if (chatState.sendLock) {
      setMessageQueue(prev => [...prev, { id: idempotencyKey, content, attachments }]);
      lifecycle.setMessages(prev => [...prev, humanMsg]);
      return;
    }

    if (!lifecycle.gwRef.current.connected) {
      lifecycle.setPendingMessage({ content, attachments });
      lifecycle.setMessages(prev => [...prev, humanMsg]);
      return;
    }

    dispatch({ type: "LOCK" });
    lifecycle.setMessages(prev => [...prev, humanMsg]);

    const params: Record<string, unknown> = {
      sessionKey: key,
      message: content,
      idempotencyKey,
    };
    if (attachments && attachments.length > 0) {
      params.attachments = attachments;
    }
    lifecycle.gwRef.current.send({
      type: "request",
      id: rpcId(),
      method: "chat.send",
      params,
    });
  }, [chatState.sendLock, lifecycle]);

  const deleteSession = useCallback(async (key: string) => {
    await lifecycle.gwRef.current.call("sessions.delete", { sessionKey: key });
    lifecycle.evictCache(key);
    lifecycle.setSessions(prev => prev.filter(s => s.sessionKey !== key));
    if (lifecycle.activeKey === key) {
      lifecycle.setActiveKey("");
      lifecycle.setMessages([]);
    }
  }, [lifecycle]);

  const resetSession = useCallback(async () => {
    const key = lifecycle.activeKeyRef.current;
    if (key) {
      try {
        await lifecycle.gwRef.current.call("sessions.delete", { sessionKey: key });
      } catch {
        // ignore
      }
      lifecycle.evictCache(key);
    }
    lifecycle.setMessages([]);
    streamingHandler.clearStreaming();
    dispatch({ type: "UNLOCK" });
    dispatch({ type: "CLEAR_ERROR" });
    // Refresh sessions — backend may auto-recreate
    await lifecycle.refreshSessions();
  }, [lifecycle, streamingHandler]);

  const cancelGeneration = useCallback(() => {
    lifecycle.gwRef.current.send({
      type: "request",
      id: rpcId(),
      method: "chat.stop",
      params: {},
    });
  }, [lifecycle]);

  const respondApproval = useCallback((approved: boolean, allowAll?: boolean) => {
    const method = approved ? "approval.approve" : "approval.deny";
    lifecycle.gwRef.current.send({
      type: "request",
      id: rpcId(),
      method,
      params: { allow_all: allowAll ?? false },
    });
  }, [lifecycle]);

  const dismissError = useCallback(() => dispatch({ type: "CLEAR_ERROR" }), []);

  // --- Compose streaming state (add pendingApproval from chatState) ---

  const streaming: StreamingState = {
    messages: streamingHandler.streaming.messages,
    pendingApproval: chatState.pendingApproval,
    requestId: streamingHandler.streaming.requestId,
  };

  return {
    sessions: lifecycle.sessions,
    activeKey: lifecycle.activeKey,
    setActiveKey: lifecycle.setActiveKey,
    messages: lifecycle.messages,
    loading: lifecycle.loading,
    sendMessage,
    deleteSession,
    resetSession,
    refreshMessages: lifecycle.refreshMessages,
    refreshSessions: lifecycle.refreshSessions,
    setMessages: lifecycle.setMessages,
    streaming,
    sendLock: chatState.sendLock,
    cancelGeneration,
    respondApproval,
    messageQueue,
    chatError: chatState.chatError,
    dismissError,
  };
}

function rpcId(): string {
  return `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}
