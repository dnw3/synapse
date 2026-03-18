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

/** Generate a short unique ID for v3 RPC frames. */
function rpcId(): string {
  return `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
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
  /** Whether v3 connect handshake has completed (hello-ok received). */
  const v3ReadyRef = useRef(false);
  /** Queue of commands to send once v3 handshake completes. */
  const pendingCommandsRef = useRef<WsCommand[]>([]);

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
        // v3 ping: send as RPC request
        wsRef.current.send(JSON.stringify({
          type: "request",
          id: rpcId(),
          method: "ping",
          params: {},
        }));
      }
    }, PING_INTERVAL_MS);
  }, []);

  /** Send a raw v3 ClientFrame over the WebSocket. */
  const sendV3Frame = useCallback((frame: Record<string, unknown>) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(frame));
    }
  }, []);

  /** Translate a legacy WsCommand into v3 ClientFrame(s) and send. */
  const sendCommand = useCallback((cmd: WsCommand) => {
    if (!v3ReadyRef.current) {
      // Queue until handshake completes
      pendingCommandsRef.current.push(cmd);
      return;
    }

    switch (cmd.type) {
      case "message": {
        const id = rpcId();
        const params: Record<string, unknown> = { content: cmd.content };
        if (cmd.attachments && cmd.attachments.length > 0) {
          params.attachments = cmd.attachments;
        }
        if (cmd.idempotency_key) {
          params.idempotency_key = cmd.idempotency_key;
        }
        if (cmd.delivery) {
          params.delivery = cmd.delivery;
        }
        sendV3Frame({ type: "request", id, method: "chat.send", params });
        break;
      }
      case "cancel": {
        const id = rpcId();
        sendV3Frame({ type: "request", id, method: "chat.stop", params: {} });
        break;
      }
      case "approval_response": {
        const id = rpcId();
        const method = cmd.approved ? "approval.approve" : "approval.deny";
        sendV3Frame({
          type: "request",
          id,
          method,
          params: { allow_all: cmd.allow_all ?? false },
        });
        break;
      }
      case "form_submit": {
        const id = rpcId();
        sendV3Frame({
          type: "request",
          id,
          method: "form.submit",
          params: { block_id: cmd.block_id, values: cmd.values },
        });
        break;
      }
      case "ping": {
        const id = rpcId();
        sendV3Frame({ type: "request", id, method: "ping", params: {} });
        break;
      }
      case "rpc_request": {
        sendV3Frame({
          type: "request",
          id: cmd.id,
          method: cmd.method,
          params: cmd.params ?? {},
        });
        break;
      }
    }
  }, [sendV3Frame]);

  /** Translate a v3 ServerFrame event into legacy WsEvent(s). */
  const translateV3Event = useCallback((eventName: string, payload: Record<string, unknown>): WsEvent | null => {
    switch (eventName) {
      case "agent.message.delta":
        return { type: "token", content: (payload.content as string) ?? "" };
      case "agent.thinking.delta":
        return { type: "reasoning", content: (payload.content as string) ?? "" };
      case "agent.tool.start":
        return {
          type: "tool_call",
          name: (payload.name as string) ?? "",
          args: (payload.args as Record<string, unknown>) ?? {},
        };
      case "agent.tool.result":
        return {
          type: "tool_result",
          name: (payload.name as string) ?? "",
          content: (payload.content as string) ?? "",
        };
      case "agent.message.start":
        return {
          type: "status",
          state: "thinking",
          request_id: payload.request_id as string | undefined,
        };
      case "agent.turn.complete":
        return {
          type: "done",
          usage: undefined,
          model: undefined,
          stop_reason: (payload.cancelled as boolean) ? "cancelled" : "end_turn",
        };
      case "agent.error":
        return {
          type: "error",
          message: (payload.message as string) ?? "Unknown error",
          request_id: payload.request_id as string | undefined,
        };
      case "agent.message.complete":
        if (payload.cancelled) {
          return {
            type: "status",
            state: "cancelled",
            request_id: payload.request_id as string | undefined,
          };
        }
        return null;
      case "approval.requested":
        return {
          type: "approval_request",
          tool_name: (payload.tool_name as string) ?? "",
          args_preview: (payload.args_preview as string) ?? "",
          risk_level: (payload.risk_level as string) ?? "",
        };
      case "tick":
      case "connect.challenge":
        return null; // internal, don't surface
      default:
        // Pass through as generic event for App.tsx to handle
        return { type: "event", event: eventName, payload };
    }
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
    v3ReadyRef.current = false;
    pendingCommandsRef.current = [];

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/${convId}`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      // v3 protocol: send connect handshake after receiving connect.challenge
      // The server sends connect.challenge immediately, we respond with connect request
      // in onmessage when we see it. For now, send it proactively.
      const connectFrame = JSON.stringify({
        type: "request",
        id: rpcId(),
        method: "connect",
        params: {
          min_protocol: 3,
          max_protocol: 3,
          client: { name: "synapse-web", version: "1.0.0" },
          caps: [],
          commands: [],
          scopes: [],
          permissions: [],
        },
      });
      ws.send(connectFrame);
      startPing();
    };

    ws.onclose = () => {
      setConnected(false);
      v3ReadyRef.current = false;
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
        const data = JSON.parse(event.data);

        // --- v3 ServerFrame handling ---

        // v3 response frame (type: "response")
        if (data.type === "response") {
          const frameId = data.id as string;

          // Check if this is the connect handshake response (hello-ok)
          if (!v3ReadyRef.current && data.result) {
            // This is likely the hello-ok response to our connect request
            v3ReadyRef.current = true;
            setConnected(true);
            reconnectAttemptsRef.current = 0;

            // Flush pending commands
            const pending = pendingCommandsRef.current;
            pendingCommandsRef.current = [];
            for (const cmd of pending) {
              sendCommand(cmd);
            }
            return;
          }

          // Handle RPC responses
          const pending = pendingRpcsRef.current.get(frameId);
          if (pending) {
            clearTimeout(pending.timer);
            pendingRpcsRef.current.delete(frameId);
            if (data.error) {
              pending.reject(new Error(typeof data.error === "string" ? data.error : data.error.message ?? "RPC error"));
            } else {
              pending.resolve(data.result);
            }
            return;
          }

          // chat.send final response — treat as "done" event
          if (data.result && typeof data.result === "object" && "request_id" in data.result) {
            setEvents((prev) => [
              ...prev,
              {
                type: "done",
                usage: undefined,
                model: undefined,
                stop_reason: "end_turn",
              } as WsEvent,
            ]);
          }
          return;
        }

        // v3 error frame (type: "error")
        if (data.type === "error") {
          const frameId = data.id as string;
          const pending = pendingRpcsRef.current.get(frameId);
          if (pending) {
            clearTimeout(pending.timer);
            pendingRpcsRef.current.delete(frameId);
            pending.reject(new Error(data.error?.message ?? "RPC error"));
            return;
          }

          setEvents((prev) => [
            ...prev,
            {
              type: "error",
              message: data.error?.message ?? "Unknown server error",
            } as WsEvent,
          ]);
          return;
        }

        // v3 event frame (type: "event")
        if (data.type === "event" && data.event) {
          const translated = translateV3Event(data.event, data.payload ?? {});
          if (translated) {
            if (translated.type === "status") {
              setStatus(translated.state);
            }
            setEvents((prev) => [...prev, translated]);
          }

          // Also pass through raw event for App.tsx handlers
          // (sessions.changed, session.compacted, update.available)
          if (
            data.event === "sessions.changed" ||
            data.event === "session.compacted" ||
            data.event === "update.available"
          ) {
            setEvents((prev) => [
              ...prev,
              { type: "event", event: data.event, payload: data.payload },
            ]);
          }
          return;
        }

        // --- Legacy format fallback (for backwards compatibility during transition) ---
        const legacyData: WsEvent = data;

        if (legacyData.type === "rpc_response") {
          const pending = pendingRpcsRef.current.get((legacyData as { id: string }).id);
          if (pending) {
            clearTimeout(pending.timer);
            pendingRpcsRef.current.delete((legacyData as { id: string }).id);
            if ((legacyData as { error?: string }).error) {
              pending.reject(new Error((legacyData as { error: string }).error));
            } else {
              pending.resolve((legacyData as { result?: unknown }).result);
            }
          }
        }

        if (legacyData.type === "status") {
          setStatus((legacyData as { state: string }).state);
        }

        if (legacyData.type === "event") {
          setEvents((prev) => [...prev, legacyData]);
          return;
        }

        setEvents((prev) => [...prev, legacyData]);
      } catch {
        // ignore malformed messages
      }
    };
  }, [clearTimers, rejectAllPendingRpcs, startPing, sendCommand, translateV3Event]);

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

    // Restore cached events for this conversation (if any).
    // Always reset status to "idle" — the server will send the real status
    // via the WS connection once established. This prevents stale "thinking"
    // status from a previous visit when the agent has since completed.
    const cached = cacheRef.current[conversationId];
    if (cached) {
      setEvents(cached.events);
      delete cacheRef.current[conversationId];
    } else {
      setEvents([]);
    }
    setStatus("idle");

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
    sendCommand(cmd);
  }, [sendCommand]);

  const call = useCallback(<T = unknown>(method: string, params?: Record<string, unknown>): Promise<T> => {
    return new Promise<T>((resolve, reject) => {
      if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
        reject(new Error("WebSocket is not connected"));
        return;
      }

      const id = rpcId();

      const timer = setTimeout(() => {
        pendingRpcsRef.current.delete(id);
        reject(new Error(`RPC call "${method}" timed out after ${RPC_TIMEOUT_MS}ms`));
      }, RPC_TIMEOUT_MS);

      pendingRpcsRef.current.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timer,
      });

      // Send as v3 ClientFrame
      wsRef.current.send(JSON.stringify({
        type: "request",
        id,
        method,
        params: params ?? {},
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
