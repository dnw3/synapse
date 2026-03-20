import { useCallback, useEffect, useMemo, useRef, useState } from "react";

const PING_INTERVAL_MS = 30_000;
const RPC_TIMEOUT_MS = 30_000;
const MAX_BACKOFF_MS = 30_000;
const BASE_BACKOFF_MS = 1_000;

interface PendingRpc {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export interface UseGatewayReturn {
  connected: boolean;
  status: string;
  call: <T = unknown>(method: string, params?: Record<string, unknown>) => Promise<T>;
  send: (frame: Record<string, unknown>) => void;
  subscribe: (handler: (event: string, payload: Record<string, unknown>) => void) => () => void;
  reconnect: () => void;
}

/** Generate a short unique ID for v3 RPC frames. */
function rpcId(): string {
  return `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

type EventHandler = (event: string, payload: Record<string, unknown>) => void;

/**
 * Global WS singleton that connects to `/ws`.
 * No session ID in URL — session routing is done via RPC params.
 */
export function useGateway(): UseGatewayReturn {
  const wsRef = useRef<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [status, setStatus] = useState("idle");

  const reconnectAttemptsRef = useRef(0);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pendingRpcsRef = useRef<Map<string, PendingRpc>>(new Map());
  const intentionalCloseRef = useRef(false);
  const v3ReadyRef = useRef(false);
  const pendingFramesRef = useRef<Record<string, unknown>[]>([]);
  const subscribersRef = useRef<Set<EventHandler>>(new Set());
  /** Ref to hold the connect function for self-referencing in onclose. */
  const connectRef = useRef<() => void>(() => {});

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
        wsRef.current.send(JSON.stringify({
          type: "request",
          id: rpcId(),
          method: "ping",
          params: {},
        }));
      }
    }, PING_INTERVAL_MS);
  }, []);

  const sendRawFrame = useCallback((frame: Record<string, unknown>) => {
    if (!v3ReadyRef.current) {
      pendingFramesRef.current.push(frame);
      return;
    }
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(frame));
    }
  }, []);

  const notifySubscribers = useCallback((event: string, payload: Record<string, unknown>) => {
    for (const handler of subscribersRef.current) {
      try {
        handler(event, payload);
      } catch {
        // subscriber error should not break the loop
      }
    }
  }, []);

  // Set up the connect function and store in ref for self-referencing
  useEffect(() => {
    // Track whether this effect instance is still active (handles StrictMode double-mount)
    let mounted = true;

    const doConnect = () => {
      if (wsRef.current) {
        wsRef.current.onclose = null; // detach old handler to prevent stale reconnect
        wsRef.current.close();
        wsRef.current = null;
      }
      if (!mounted) return; // Don't connect if effect was already cleaned up
      v3ReadyRef.current = false;
      pendingFramesRef.current = [];

      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      // In dev mode (Vite on :5173), connect directly to backend on :3000
      // to avoid Vite proxy issues with bare /ws path
      const host = window.location.port === "5173"
        ? `${window.location.hostname}:3000`
        : window.location.host;
      const url = `${protocol}//${host}/ws`;
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!mounted) { ws.close(); return; }
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

        // Only auto-reconnect if this effect instance is still active
        if (mounted) {
          const attempt = reconnectAttemptsRef.current;
          const delay = Math.min(BASE_BACKOFF_MS * Math.pow(2, attempt), MAX_BACKOFF_MS);
          reconnectAttemptsRef.current = attempt + 1;
          reconnectTimerRef.current = setTimeout(() => {
            reconnectTimerRef.current = null;
            connectRef.current();
          }, delay);
        }
      };

      ws.onerror = () => {
        // onclose fires after onerror
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);

          // v3 response frame
          if (data.type === "response") {
            const frameId = data.id as string;

            // hello-ok handshake
            if (!v3ReadyRef.current && (data.ok || data.payload)) {
              v3ReadyRef.current = true;
              setConnected(true);
              reconnectAttemptsRef.current = 0;

              // Flush pending frames
              const pending = pendingFramesRef.current;
              pendingFramesRef.current = [];
              for (const f of pending) {
                if (wsRef.current?.readyState === WebSocket.OPEN) {
                  wsRef.current.send(JSON.stringify(f));
                }
              }
              return;
            }

            // Resolve pending RPC
            const pending = pendingRpcsRef.current.get(frameId);
            if (pending) {
              clearTimeout(pending.timer);
              pendingRpcsRef.current.delete(frameId);
              if (data.error || data.ok === false) {
                const errMsg = typeof data.error === "string" ? data.error : data.error?.message ?? "RPC error";
                pending.reject(new Error(errMsg));
              } else {
                pending.resolve(data.payload ?? data.result);
              }
              return;
            }

            // chat.send final response — notify as synthetic event
            const resultPayload = data.payload ?? data.result;
            if (resultPayload && typeof resultPayload === "object" && "request_id" in resultPayload) {
              setStatus("idle");
              notifySubscribers("agent.turn.complete", { cancelled: false });
            }
            return;
          }

          // v3 error frame
          if (data.type === "error") {
            const frameId = data.id as string;
            const pending = pendingRpcsRef.current.get(frameId);
            if (pending) {
              clearTimeout(pending.timer);
              pendingRpcsRef.current.delete(frameId);
              pending.reject(new Error(data.error?.message ?? "RPC error"));
              return;
            }
            notifySubscribers("agent.error", { message: data.error?.message ?? "Unknown server error" });
            return;
          }

          // v3 event frame
          if (data.type === "event" && data.event) {
            const eventName = data.event as string;
            const payload = (data.payload ?? {}) as Record<string, unknown>;

            // Update status for agent lifecycle events
            if (eventName === "agent.message.start") {
              setStatus("thinking");
            } else if (eventName === "agent.turn.complete" || eventName === "agent.error") {
              setStatus("idle");
            }

            notifySubscribers(eventName, payload);
            return;
          }
        } catch {
          // ignore malformed messages
        }
      };
    };

    connectRef.current = doConnect;
    doConnect();

    return () => {
      mounted = false;
      clearTimers();
      rejectAllPendingRpcs("Component unmounting");
      if (wsRef.current) {
        wsRef.current.onclose = null; // prevent reconnect from stale handler
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [clearTimers, rejectAllPendingRpcs, startPing, notifySubscribers]);

  const call = useCallback(<T = unknown>(method: string, params?: Record<string, unknown>): Promise<T> => {
    return new Promise<T>((resolve, reject) => {
      if (!v3ReadyRef.current || !wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
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

      wsRef.current!.send(JSON.stringify({
        type: "request",
        id,
        method,
        params: params ?? {},
      }));
    });
  }, []);

  const subscribe = useCallback((handler: EventHandler): (() => void) => {
    subscribersRef.current.add(handler);
    return () => {
      subscribersRef.current.delete(handler);
    };
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
    connectRef.current();
  }, [clearTimers]);

  return useMemo(() => ({ connected, status, call, send: sendRawFrame, subscribe, reconnect }), [connected, status, call, sendRawFrame, subscribe, reconnect]);
}
