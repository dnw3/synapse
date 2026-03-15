// GatewayClient — standalone WebSocket RPC client for the v3 dashboard protocol.
// Not React-specific; wrap with useGatewayClient hook for React usage.

export interface HelloOk {
  protocol: number;
  server: { version: string; conn_id: string };
  features: { methods: string[]; events: string[] };
  snapshot: { presence: Record<string, unknown> | unknown[]; health: Record<string, unknown>; state_version: { presence: number; health: number } };
  auth_result?: { authenticated: boolean; role?: string; scopes: string[] };
}

interface PendingRequest {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export class GatewayClient {
  private ws: WebSocket | null = null;
  private pending = new Map<string, PendingRequest>();
  private eventHandlers = new Map<string, Set<(payload: unknown) => void>>();
  private _connected = false;
  private _helloOk: HelloOk | null = null;
  private reqId = 0;

  get isConnected() { return this._connected; }
  get methods() { return this._helloOk?.features.methods ?? []; }
  get events() { return this._helloOk?.features.events ?? []; }
  get helloOk() { return this._helloOk; }

  async connect(url: string, auth?: { token?: string; password?: string }): Promise<HelloOk> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(url);
      this.ws = ws;
      let challengeReceived = false;

      ws.onmessage = (event) => {
        const frame = JSON.parse(event.data);

        // Handle connect.challenge event
        if (frame.type === "event" && frame.event === "connect.challenge" && !challengeReceived) {
          challengeReceived = true;
          const connectReq = {
            type: "request",
            id: `rpc-${++this.reqId}`,
            method: "connect",
            params: {
              min_protocol: 3,
              max_protocol: 3,
              client: { id: "web", display_name: "Web Dashboard", platform: navigator.platform },
              ...(auth ? { auth } : {}),
            },
          };
          ws.send(JSON.stringify(connectReq));
          return;
        }

        // Handle connect response (hello-ok)
        if (frame.type === "response" && !this._connected) {
          if (frame.ok && frame.payload) {
            this._helloOk = frame.payload;
            this._connected = true;
            resolve(frame.payload);
          } else {
            reject(new Error(frame.error?.message ?? "Connection rejected"));
          }
          return;
        }

        // Handle RPC responses
        if (frame.type === "response") {
          const p = this.pending.get(frame.id);
          if (p) {
            clearTimeout(p.timer);
            this.pending.delete(frame.id);
            if (frame.ok) {
              p.resolve(frame.payload);
            } else {
              p.reject(new Error(frame.error?.message ?? "RPC error"));
            }
          }
          return;
        }

        // Handle events
        if (frame.type === "event") {
          const handlers = this.eventHandlers.get(frame.event);
          if (handlers) {
            for (const h of handlers) h(frame.payload);
          }
          // Fire wildcard handlers
          const wildcardHandlers = this.eventHandlers.get("*");
          if (wildcardHandlers) {
            for (const h of wildcardHandlers) h({ event: frame.event, payload: frame.payload });
          }
        }
      };

      ws.onerror = () => reject(new Error("WebSocket connection failed"));
      ws.onclose = () => {
        this._connected = false;
        for (const [, p] of this.pending) {
          clearTimeout(p.timer);
          p.reject(new Error("Connection closed"));
        }
        this.pending.clear();
      };
    });
  }

  async request<T = unknown>(method: string, params: Record<string, unknown> = {}, timeoutMs = 30000): Promise<T> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("Not connected");
    }
    const id = `rpc-${++this.reqId}`;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`RPC timeout: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject, timer });
      this.ws!.send(JSON.stringify({ type: "request", id, method, params }));
    });
  }

  onEvent(event: string, handler: (payload: unknown) => void): () => void {
    if (!this.eventHandlers.has(event)) {
      this.eventHandlers.set(event, new Set());
    }
    this.eventHandlers.get(event)!.add(handler);
    return () => { this.eventHandlers.get(event)?.delete(handler); };
  }

  close() {
    this._connected = false;
    this.ws?.close();
    this.ws = null;
  }
}
