export interface Session {
  sessionKey: string;
  displayName?: string;
  channel?: string;
  kind?: string;
  messageCount?: number;
  createdAt?: string;
  updatedAt?: string;
}

/** @deprecated Use Session instead */
export type Conversation = Session;

export interface ToolDisplay {
  emoji: string;
  label: string;
  verb: string;
  detail: string;
}

export interface ToolCall {
  name: string;
  arguments: Record<string, unknown>;
  display?: ToolDisplay;
}

export interface MessageUsage {
  input_tokens?: number;
  output_tokens?: number;
  cache_read_tokens?: number;
  cache_write_tokens?: number;
  cost_usd?: number;
  duration_ms?: number;
}

export interface Message {
  role: "system" | "human" | "assistant" | "tool";
  content: string;
  tool_calls: ToolCall[];
  request_id?: string;
  timestamp?: number;
  reasoning?: string;
  usage?: MessageUsage;
  model?: string;
  stop_reason?: string;
}

export interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
}

export interface FileContent {
  path: string;
  content: string;
}

// WebSocket events from server
export type WsEvent =
  | { type: "token"; content: string }
  | { type: "reasoning"; content: string }
  | { type: "tool_call"; name: string; args: Record<string, unknown> }
  | { type: "tool_result"; name: string; content: string }
  | { type: "status"; state: "thinking" | "executing" | "idle" | "cancelled" | "pong"; request_id?: string }
  | { type: "canvas_update"; block_type: string; content: string; language?: string; attributes?: Record<string, unknown> }
  | { type: "approval_request"; tool_name: string; args_preview: string; risk_level: string }
  | { type: "subagent_complete"; task_id: string; summary: string }
  | { type: "done"; usage?: { input_tokens?: number; output_tokens?: number; cost_usd?: number }; model?: string; stop_reason?: string }
  | { type: "error"; message: string; request_id?: string }
  | { type: "rpc_response"; id: string; result?: unknown; error?: string }
  | { type: "hello"; version: string; features: string[] }
  | { type: "event"; event: string; payload: unknown };

export interface FileAttachment {
  id: string;
  filename: string;
  mime_type: string;
  url: string;
}

/** Delivery routing target for chat.send v3 protocol.
 * When omitted the server defaults to webchat (current behaviour). */
export interface DeliveryTarget {
  channel: string;
  to?: string;
  account_id?: string;
  thread_id?: string;
}

// WebSocket commands to server
export type WsCommand =
  | { type: "message"; content: string; attachments?: FileAttachment[]; idempotency_key?: string; delivery?: DeliveryTarget }
  | { type: "form_submit"; block_id: string; values: Record<string, unknown> }
  | { type: "approval_response"; approved: boolean; allow_all?: boolean }
  | { type: "cancel" }
  | { type: "rpc_request"; id: string; method: string; params?: Record<string, unknown> }
  | { type: "ping" };
