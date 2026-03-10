import { useCallback, useEffect, useRef, useState } from "react";
import type { WsEvent, WsCommand } from "../types";

interface UseWebSocketReturn {
  connected: boolean;
  status: string;
  events: WsEvent[];
  send: (cmd: WsCommand) => void;
  clearEvents: () => void;
}

export function useWebSocket(conversationId: string | null): UseWebSocketReturn {
  const wsRef = useRef<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [status, setStatus] = useState("idle");
  const [events, setEvents] = useState<WsEvent[]>([]);

  useEffect(() => {
    if (!conversationId) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/${conversationId}`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => setConnected(true);
    ws.onclose = () => {
      setConnected(false);
      setStatus("idle");
    };
    ws.onerror = () => setConnected(false);

    ws.onmessage = (event) => {
      try {
        const data: WsEvent = JSON.parse(event.data);
        if (data.type === "status") {
          setStatus(data.state);
        }
        setEvents((prev) => [...prev, data]);
      } catch {
        // ignore
      }
    };

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [conversationId]);

  const send = useCallback((cmd: WsCommand) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(cmd));
    }
  }, []);

  const clearEvents = useCallback(() => {
    setEvents([]);
    setStatus("idle");
  }, []);

  return { connected, status, events, send, clearEvents };
}
