import { useCallback, useEffect, useRef, useState } from "react";
import type { WsEvent, WsCommand } from "../types";

const PING_INTERVAL_MS = 30_000;
const RPC_TIMEOUT_MS = 30_000;
const MAX_BACKOFF_MS = 30_000;
const BASE_BACKOFF_MS = 1_000;


interface PendingRpc {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

interface UseGatewayWSReturn {
  connected: boolean;
  status: string;
  events: WsEvent[];
  send: (cmd: WsCommand) => void;
  call: <T = unknown>(method: string, params?: Record<string, unknown>) => Promise<T>;
  clearEvents: () => void;
  reconnect: () => void;
}

/** Per-conversation cached state so switching away and back preserves context. */
interface ConversationCache {
  events: WsEvent[];
  status: string;
}

export function useGatewayWS(conversationId: string | null): UseGatewayWSReturn {
  const wsRef = useRef<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [status, setStatus] = useState("idle");
  const [events, setEvents] = useState<WsEvent[]>([]);

  const reconnectAttemptsRef = useRef(0);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pendingRpcsRef = useRef<Map<string, PendingRpc>>(new Map());
  const intentionalCloseRef = useRef(false);
  const conversationIdRef = useRef(conversationId);

  // Per-conversation event/status cache
  const cacheRef = useRef<Record<string, ConversationCache>>({});
  const prevConvIdRef = useRef<string | null>(null);

  // Keep conversationId ref in sync for use inside callbacks
  conversationIdRef.current = conversationId;

  const clearTimers = useCallback(() => {
    if (reconnectTimerRef.current !== null) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    if (pingTimerRef.current !== null) {
      clearInterval(pingTimerRef.current);
      pingTimerRef.current = null;
    }
  }, []);

  const rejectAllPendingRpcs = useCallback((reason: string) => {
    for (const [id, pending] of pendingRpcsRef.current) {
      clearTimeout(pending.timer);
      pending.reject(new Error(reason));
      pendingRpcsRef.current.delete(id);
    }
  }, []);

  const startPing = useCallback(() => {
    if (pingTimerRef.current !== null) {
      clearInterval(pingTimerRef.current);
    }
    pingTimerRef.current = setInterval(() => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: "ping" }));
      }
    }, PING_INTERVAL_MS);
  }, []);

  const connect = useCallback(() => {
    const convId = conversationIdRef.current;
    if (!convId) return;

    // Clean up any existing connection
    if (wsRef.current) {
      intentionalCloseRef.current = true;
      wsRef.current.close();
      wsRef.current = null;
    }
    intentionalCloseRef.current = false;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/${convId}`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      reconnectAttemptsRef.current = 0;
      startPing();
    };

    ws.onclose = () => {
      setConnected(false);
      clearTimers();
      rejectAllPendingRpcs("WebSocket connection closed");

      // Auto-reconnect unless intentionally closed or no conversation
      if (!intentionalCloseRef.current && conversationIdRef.current) {
        const attempt = reconnectAttemptsRef.current;
        const delay = Math.min(BASE_BACKOFF_MS * Math.pow(2, attempt), MAX_BACKOFF_MS);
        reconnectAttemptsRef.current = attempt + 1;
        reconnectTimerRef.current = setTimeout(() => {
          reconnectTimerRef.current = null;
          connect();
        }, delay);
      }
    };

    ws.onerror = () => {
      // onclose will fire after onerror, so reconnect logic is handled there
    };

    ws.onmessage = (event) => {
      try {
        const data: WsEvent = JSON.parse(event.data);

        // Handle RPC responses separately
        if (data.type === "rpc_response") {
          const pending = pendingRpcsRef.current.get(data.id);
          if (pending) {
            clearTimeout(pending.timer);
            pendingRpcsRef.current.delete(data.id);
            if (data.error) {
              pending.reject(new Error(data.error));
            } else {
              pending.resolve(data.result);
            }
          }
          // Still add to events for visibility
        }

        if (data.type === "status") {
          setStatus(data.state);
        }

        // Protocol v3 push event frames — pass through to events array
        // App.tsx handles: sessions.changed, session.compacted, update.available
        if (data.type === "event") {
          setEvents((prev) => [...prev, { type: "event", event: data.event, payload: data.payload }]);
          return;
        }

        setEvents((prev) => [...prev, data]);
      } catch {
        // ignore malformed messages
      }
    };
  }, [clearTimers, rejectAllPendingRpcs, startPing]);

  // Connect/disconnect when conversationId changes
  useEffect(() => {
    // Save current events/status to cache before switching away
    const prevId = prevConvIdRef.current;
    if (prevId) {
      // We read current state via a "snapshot" approach:
      // setEvents/setStatus are async, so use functional form to peek at current values.
      setEvents((currentEvents) => {
        setStatus((currentStatus) => {
          if (currentEvents.length > 0 || currentStatus !== "idle") {
            cacheRef.current[prevId] = {
              events: currentEvents,
              status: currentStatus,
            };
          }
          return currentStatus;
        });
        return currentEvents;
      });
    }
    prevConvIdRef.current = conversationId;

    clearTimers();
    rejectAllPendingRpcs("Conversation changed");

    if (!conversationId) {
      if (wsRef.current) {
        intentionalCloseRef.current = true;
        wsRef.current.close();
        wsRef.current = null;
      }
      setConnected(false);
      setStatus("idle");
      setEvents([]);
      return;
    }

    // Restore cached events/status for this conversation (if any)
    const cached = cacheRef.current[conversationId];
    if (cached) {
      setEvents(cached.events);
      setStatus(cached.status);
      delete cacheRef.current[conversationId];
    } else {
      setEvents([]);
      setStatus("idle");
    }

    // Fresh connection for new conversation
    reconnectAttemptsRef.current = 0;
    connect();

    return () => {
      clearTimers();
      rejectAllPendingRpcs("Component unmounting");
      if (wsRef.current) {
        intentionalCloseRef.current = true;
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [conversationId, connect, clearTimers, rejectAllPendingRpcs]);

  const send = useCallback((cmd: WsCommand) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(cmd));
    }
  }, []);

  const call = useCallback(<T = unknown>(method: string, params?: Record<string, unknown>): Promise<T> => {
    return new Promise<T>((resolve, reject) => {
      if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
        reject(new Error("WebSocket is not connected"));
        return;
      }

      const id = `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

      const timer = setTimeout(() => {
        pendingRpcsRef.current.delete(id);
        reject(new Error(`RPC call "${method}" timed out after ${RPC_TIMEOUT_MS}ms`));
      }, RPC_TIMEOUT_MS);

      pendingRpcsRef.current.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timer,
      });

      wsRef.current.send(JSON.stringify({
        type: "rpc_request",
        id,
        method,
        params,
      }));
    });
  }, []);

  const clearEvents = useCallback(() => {
    setEvents([]);
    setStatus("idle");
    // Also clear from cache
    const convId = conversationIdRef.current;
    if (convId) {
      delete cacheRef.current[convId];
    }
  }, []);

  const reconnect = useCallback(() => {
    clearTimers();
    reconnectAttemptsRef.current = 0;
    if (wsRef.current) {
      intentionalCloseRef.current = true;
      wsRef.current.close();
      wsRef.current = null;
    }
    intentionalCloseRef.current = false;
    connect();
  }, [connect, clearTimers]);

  return { connected, status, events, send, call, clearEvents, reconnect };
}
