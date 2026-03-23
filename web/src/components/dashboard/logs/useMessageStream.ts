import { useState, useEffect, useRef, useCallback } from "react";

// ─── Message Stream Types ────────────────────────────────────────────────────

export type MessageDirection = "in" | "out";

export interface ChannelMessage {
  id: string;
  direction: MessageDirection;
  channel: string;
  sessionKey?: string;
  contentPreview: string;
  timestampMs: number;
  requestId?: string;
  to?: string;
}

/** Channel → badge color classes */
export const CHANNEL_COLORS: Record<string, { bg: string; text: string; border: string }> = {
  slack:     { bg: "bg-purple-500/15",   text: "text-purple-400",  border: "border-purple-500/25" },
  telegram:  { bg: "bg-blue-500/15",     text: "text-blue-400",    border: "border-blue-500/25" },
  lark:      { bg: "bg-green-500/15",    text: "text-green-400",   border: "border-green-500/25" },
  discord:   { bg: "bg-indigo-500/15",   text: "text-indigo-400",  border: "border-indigo-500/25" },
  webchat:   { bg: "bg-[var(--bg-content)]/60", text: "text-[var(--text-secondary)]", border: "border-[var(--border-subtle)]/40" },
  dingtalk:  { bg: "bg-orange-500/15",   text: "text-orange-400",  border: "border-orange-500/25" },
  whatsapp:  { bg: "bg-emerald-500/15",  text: "text-emerald-400", border: "border-emerald-500/25" },
  line:      { bg: "bg-lime-500/15",     text: "text-lime-400",    border: "border-lime-500/25" },
  mattermost:{ bg: "bg-cyan-500/15",     text: "text-cyan-400",    border: "border-cyan-500/25" },
  wechat:    { bg: "bg-green-600/15",    text: "text-green-500",   border: "border-green-600/25" },
};

export function getChannelColors(channel: string) {
  const key = channel.toLowerCase();
  return CHANNEL_COLORS[key] ?? { bg: "bg-[var(--accent)]/10", text: "text-[var(--accent-light)]", border: "border-[var(--accent)]/20" };
}

export const MAX_MESSAGES = 200;

/** Ring-buffer push: keep last N items */
function ringPush<T>(arr: T[], item: T, maxLen: number): T[] {
  const next = [...arr, item];
  return next.length > maxLen ? next.slice(next.length - maxLen) : next;
}

/** Hook: subscribe to message.received / message.sent via a dashboard WebSocket. */
export function useMessageStream() {
  const [messages, setMessages] = useState<ChannelMessage[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const unmountedRef = useRef(false);
  // Use a ref to allow the onclose handler to reference connect without hoisting issues
  const connectRef = useRef<(() => void) | null>(null);

  const connect = useCallback(() => {
    if (unmountedRef.current) return;
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws/_dashboard_events`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      if (!unmountedRef.current) setConnected(true);
    };

    ws.onclose = () => {
      if (unmountedRef.current) return;
      setConnected(false);
      // Use connectRef to schedule reconnect without forward-reference issue
      reconnectRef.current = setTimeout(() => connectRef.current?.(), 3000);
    };

    ws.onerror = () => { /* onclose fires after */ };

    ws.onmessage = (evt) => {
      if (unmountedRef.current) return;
      try {
        const frame = JSON.parse(evt.data as string);
        if (frame.type !== "event") return;
        const { event, payload } = frame as { event: string; payload: Record<string, unknown> };

        if (event === "message.received") {
          const msg: ChannelMessage = {
            id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            direction: "in",
            channel: String(payload.channel ?? "unknown"),
            sessionKey: payload.session_key ? String(payload.session_key) : undefined,
            contentPreview: String(payload.content_preview ?? ""),
            timestampMs: typeof payload.timestamp_ms === "number" ? payload.timestamp_ms : Date.now(),
            requestId: payload.request_id ? String(payload.request_id) : undefined,
            to: payload.to ? String(payload.to) : undefined,
          };
          setMessages(prev => ringPush(prev, msg, MAX_MESSAGES));
        } else if (event === "message.sent") {
          const msg: ChannelMessage = {
            id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            direction: "out",
            channel: String(payload.channel ?? "unknown"),
            sessionKey: undefined,
            contentPreview: payload.message_id ? `[msg:${String(payload.message_id)}]` : "(sent)",
            timestampMs: typeof payload.timestamp_ms === "number" ? payload.timestamp_ms : Date.now(),
            requestId: payload.request_id ? String(payload.request_id) : undefined,
            to: payload.to ? String(payload.to) : undefined,
          };
          setMessages(prev => ringPush(prev, msg, MAX_MESSAGES));
        }
      } catch { /* ignore */ }
    };
  }, []);

  // Keep connectRef in sync with latest connect
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  useEffect(() => {
    unmountedRef.current = false;
    connect();
    return () => {
      unmountedRef.current = true;
      if (reconnectRef.current !== null) {
        clearTimeout(reconnectRef.current);
        reconnectRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [connect]);

  const clearMessages = useCallback(() => setMessages([]), []);

  return { messages, connected, clearMessages };
}
