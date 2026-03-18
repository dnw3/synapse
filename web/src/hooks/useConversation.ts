import { useCallback, useEffect, useRef, useState } from "react";
import type { Conversation, Message } from "../types";
import { api } from "../api";

export function useConversation() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);

  // Per-conversation message cache: preserves local (optimistic) messages
  // so switching away and back doesn't lose unsaved state.
  const messageCacheRef = useRef<Record<string, Message[]>>({});
  const prevActiveIdRef = useRef<string | null>(null);

  // Load conversations on mount — select the main web session
  useEffect(() => {
    api.listConversations().then((convs) => {
      setConversations(convs);
      if (!activeId) {
        // Find the main web session (or most recent)
        const main = convs.find((c) => c.channel === "web") || convs[convs.length - 1];
        if (main) {
          setActiveId(main.id);
        } else {
          // No sessions at all — create the main session
          api.createConversation().then((conv) => {
            setConversations([conv]);
            setActiveId(conv.id);
          });
        }
      }
    }).catch(console.error);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

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

  /** Reset the current session: delete + re-fetch (backend preserves session key). */
  const resetSession = useCallback(async () => {
    const oldId = activeId;
    if (oldId) {
      await api.deleteConversation(oldId);
      delete messageCacheRef.current[oldId];
    }
    // Backend auto-creates new session with same key on next list
    const convs = await api.listConversations();
    setConversations(convs);
    const main = convs.find((c) => c.channel === "web") || convs[0];
    if (main) {
      setActiveId(main.id);
      setMessages([]);
    }
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
    deleteConversation,
    refreshMessages,
    resetSession,
    ensureConversation,
  };
}
