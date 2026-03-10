export interface Conversation {
  id: string;
  created_at: string;
  message_count: number;
}

export interface ToolCall {
  name: string;
  arguments: Record<string, unknown>;
}

export interface Message {
  role: "system" | "human" | "assistant" | "tool";
  content: string;
  tool_calls: ToolCall[];
  request_id?: string;
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
  | { type: "tool_call"; name: string; args: Record<string, unknown> }
  | { type: "tool_result"; name: string; content: string }
  | { type: "status"; state: "thinking" | "executing" | "idle" | "cancelled" | "pong"; request_id?: string }
  | { type: "canvas_update"; block_type: string; content: string; language?: string; attributes?: Record<string, unknown> }
  | { type: "approval_request"; tool_name: string; args_preview: string; risk_level: string }
  | { type: "subagent_complete"; task_id: string; summary: string }
  | { type: "done" }
  | { type: "error"; message: string; request_id?: string }
  | { type: "rpc_response"; id: string; result?: unknown; error?: string }
  | { type: "hello"; version: string; features: string[] };

export interface FileAttachment {
  id: string;
  filename: string;
  mime_type: string;
  url: string;
}

// WebSocket commands to server
export type WsCommand =
  | { type: "message"; content: string; attachments?: FileAttachment[]; idempotency_key?: string }
  | { type: "form_submit"; block_id: string; values: Record<string, unknown> }
  | { type: "approval_response"; approved: boolean; allow_all?: boolean }
  | { type: "cancel" }
  | { type: "rpc_request"; id: string; method: string; params?: Record<string, unknown> }
  | { type: "ping" };
