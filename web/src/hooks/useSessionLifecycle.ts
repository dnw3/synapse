import { useCallback, useEffect, useRef, useState } from "react";
import type { Session, Message, FileAttachment } from "../types";
import type { UseGatewayReturn } from "./useGateway";

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface UseSessionLifecycleReturn {
  sessions: Session[];
  activeKey: string | null;
  messages: Message[];
  loading: boolean;
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  setActiveKey: (key: string) => void;
  setSessions: React.Dispatch<React.SetStateAction<Session[]>>;
  refreshSessions: () => Promise<void>;
  refreshMessages: () => Promise<void>;
  activeKeyRef: React.RefObject<string | null>;
  /** Remove a key from the message cache. */
  evictCache: (key: string) => void;
  /** Update cache for a specific key. */
  updateCache: (key: string, msgs: Message[]) => void;
  /** Set the pending reconnect message. */
  setPendingMessage: (msg: { content: string; attachments?: FileAttachment[] } | null) => void;
  /** Get the current gateway ref (stable). */
  gwRef: React.RefObject<UseGatewayReturn>;
}

export function useSessionLifecycle(gw: UseGatewayReturn): UseSessionLifecycleReturn {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);

  // Per-session message cache (doesn't drive UI)
  const messageCacheRef = useRef<Record<string, Message[]>>({});
  const activeKeyRef = useRef<string | null>(null);

  // Pending message for when WS reconnects
  const pendingMessageRef = useRef<{ content: string; attachments?: FileAttachment[] } | null>(null);

  // Stable ref to gw — updated in effect to satisfy eslint refs rule
  const gwRef = useRef(gw);
  useEffect(() => {
    gwRef.current = gw;
  });

  // Keep activeKeyRef in sync via effect (not during render)
  useEffect(() => {
    activeKeyRef.current = activeKey;
  }, [activeKey]);

  // Cache helpers (avoid exposing refs for direct mutation)
  const evictCache = useCallback((key: string) => {
    delete messageCacheRef.current[key];
  }, []);

  const updateCache = useCallback((key: string, msgs: Message[]) => {
    messageCacheRef.current[key] = msgs;
  }, []);

  const setPendingMessage = useCallback((msg: { content: string; attachments?: FileAttachment[] } | null) => {
    pendingMessageRef.current = msg;
  }, []);

  // Load sessions
  const refreshSessions = useCallback(async () => {
    if (!gwRef.current.connected) return;
    try {
      const result = await gwRef.current.call<{ sessions: Session[] }>("sessions.list");
      const list = result.sessions ?? (result as unknown as Session[]);
      setSessions(Array.isArray(list) ? list : []);
    } catch {
      // Will retry on reconnect
    }
  }, []);

  // Load messages for active session
  const refreshMessages = useCallback(async () => {
    const key = activeKeyRef.current;
    if (!key || !gwRef.current.connected) return;
    try {
      const result = await gwRef.current.call<{ messages: Message[] }>("chat.history", { sessionKey: key });
      const msgs = result.messages ?? (result as unknown as Message[]);
      setMessages(msgs);
      messageCacheRef.current[key] = msgs;
    } catch {
      // ignore
    }
  }, []);

  // On connect: load sessions, send pending message
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    if (!gwRef.current.connected) return;
    refreshSessions().then(() => {
      if (pendingMessageRef.current && activeKeyRef.current) {
        const { content, attachments } = pendingMessageRef.current;
        pendingMessageRef.current = null;
        const id = `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        const params: Record<string, unknown> = {
          sessionKey: activeKeyRef.current,
          message: content,
          idempotencyKey: id,
        };
        if (attachments && attachments.length > 0) {
          params.attachments = attachments;
        }
        gwRef.current.send({
          type: "request",
          id,
          method: "chat.send",
          params,
        });
      }
    });
  }, [gw.connected, refreshSessions]);

  // Select default session when sessions load and no active key
  useEffect(() => {
    if (activeKey) return;
    if (sessions.length === 0) {
      setActiveKey("main");
      return;
    }
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

    // Fetch from server
    if (gwRef.current.connected) {
      setLoading(true);
      gwRef.current.call<{ messages: Message[] }>("chat.history", { sessionKey: activeKey })
        .then(result => {
          const msgs = result.messages ?? (result as unknown as Message[]);
          setMessages(current => (msgs.length >= current.length ? msgs : current));
          messageCacheRef.current[activeKey] = msgs;
        })
        .catch(() => {})
        .finally(() => setLoading(false));
    }
  }, [activeKey, gw.connected]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return {
    sessions,
    activeKey,
    messages,
    loading,
    setMessages,
    setActiveKey,
    setSessions,
    refreshSessions,
    refreshMessages,
    activeKeyRef,
    evictCache,
    updateCache,
    setPendingMessage,
    gwRef,
  };
}
