import { useCallback, useEffect, useRef, useState } from "react";
import type { Conversation, Message } from "../types";
import { api } from "../api";

export function useConversation() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [titles, setTitles] = useState<Record<string, string>>({});

  // Per-conversation message cache: preserves local (optimistic) messages
  // so switching away and back doesn't lose unsaved state.
  const messageCacheRef = useRef<Record<string, Message[]>>({});
  const prevActiveIdRef = useRef<string | null>(null);

  // Load conversations on mount — titles come from the server response
  useEffect(() => {
    api.listConversations().then((convs) => {
      setConversations(convs);
      const newTitles: Record<string, string> = {};
      for (const conv of convs) {
        if (conv.title) {
          newTitles[conv.id] = conv.title;
        }
      }
      if (Object.keys(newTitles).length > 0) {
        setTitles((prev) => ({ ...prev, ...newTitles }));
      }
    }).catch(console.error);
  }, []);

  // Load messages when active conversation changes
  useEffect(() => {
    // Save current messages to cache before switching away
    const prevId = prevActiveIdRef.current;
    if (prevId && messages.length > 0) {
      messageCacheRef.current[prevId] = messages;
    }
    prevActiveIdRef.current = activeId;
    setLoading(false); // Reset loading when switching conversations

    if (!activeId) {
      setMessages([]);
      return;
    }
    // Skip loading for just-created conversations
    if (justCreatedRef.current) {
      justCreatedRef.current = false;
      // Use initial messages if provided (e.g. the first human message),
      // otherwise empty for "new chat" without a message.
      setMessages(pendingInitialMsgsRef.current ?? []);
      pendingInitialMsgsRef.current = null;
      return;
    }

    // Restore from cache immediately (so the user sees messages right away),
    // then fetch from server and merge (take whichever has more messages).
    const cached = messageCacheRef.current[activeId];
    if (cached && cached.length > 0) {
      setMessages(cached);
    }

    api.getMessages(activeId).then((serverMsgs) => {
      setMessages((current) => {
        // Use whichever source has more messages — the local/cached state
        // may include optimistic messages not yet persisted to the store.
        return serverMsgs.length >= current.length ? serverMsgs : current;
      });
    }).catch(console.error);
  }, [activeId]); // eslint-disable-line react-hooks/exhaustive-deps

  const justCreatedRef = useRef(false);
  const pendingInitialMsgsRef = useRef<Message[] | null>(null);

  const createConversation = useCallback(async (initialMessages?: Message[]) => {
    const conv = await api.createConversation();
    justCreatedRef.current = true;
    pendingInitialMsgsRef.current = initialMessages ?? null;
    setLoading(false); // Reset loading state for clean new session
    setConversations((prev) => [conv, ...prev]);
    setActiveId(conv.id);
    return conv;
  }, []);

  const deleteConversation = useCallback(
    async (id: string) => {
      await api.deleteConversation(id);
      delete messageCacheRef.current[id];
      setConversations((prev) => prev.filter((c) => c.id !== id));
      if (activeId === id) {
        setActiveId(null);
      }
    },
    [activeId]
  );

  const sendMessage = useCallback(
    async (content: string, taskMode = true) => {
      if (!activeId) return;
      setLoading(true);

      // Add human message immediately
      const humanMsg: Message = { role: "human", content, tool_calls: [] };
      setMessages((prev) => [...prev, humanMsg]);

      // Set title from the first human message
      if (activeId) {
        setTitles((prev) => prev[activeId] ? prev : { ...prev, [activeId]: content });
      }

      try {
        const response = await api.sendMessage(activeId, content, taskMode);
        setMessages((prev) => [...prev, ...response]);
      } catch (e) {
        console.error("Send message failed:", e);
      } finally {
        setLoading(false);
      }
    },
    [activeId]
  );

  const activeIdRef = useRef(activeId);
  activeIdRef.current = activeId;

  const refreshMessages = useCallback(async (): Promise<Message[]> => {
    const id = activeIdRef.current;
    if (!id) return [];
    const msgs = await api.getMessages(id);
    setMessages(msgs);
    // Update cache with server state
    messageCacheRef.current[id] = msgs;
    return msgs;
  }, []);

  /** Reset the current session: delete old conversation + create fresh one. */
  const resetSession = useCallback(async () => {
    const oldId = activeId;
    if (oldId) {
      await api.deleteConversation(oldId);
      delete messageCacheRef.current[oldId];
      setConversations((prev) => prev.filter((c) => c.id !== oldId));
    }
    const conv = await api.createConversation();
    justCreatedRef.current = true;
    pendingInitialMsgsRef.current = null;
    setLoading(false);
    setConversations((prev) => [conv, ...prev]);
    setActiveId(conv.id);
    return conv;
  }, [activeId]);

  /** Ensure a conversation with the given id exists in the local list. */
  const ensureConversation = useCallback((id: string) => {
    setConversations((prev) => {
      if (prev.some((c) => c.id === id)) return prev;
      return [{ id, created_at: new Date().toISOString(), message_count: 0 }, ...prev];
    });
  }, []);

  return {
    conversations,
    activeId,
    setActiveId,
    messages,
    setMessages,
    loading,
    titles,
    setTitles,
    createConversation,
    deleteConversation,
    sendMessage,
    refreshMessages,
    resetSession,
    ensureConversation,
  };
}
