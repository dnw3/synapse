export type TraceStatus = "running" | "success" | "error";
export type TraceSubView = "overview" | "timeline" | "steps";

export interface TraceMetadata {
  total_tokens: number;
  duration_ms: number | null;
  model_calls: number;
  tool_calls: number;
  tools_used: string[];
  model: string | null;
  channel: string | null;
  user_message_preview: string;
  parent_request_id: string | null;
}

export interface Span {
  id: string;
  start_time: string;
  end_time: string | null;
  duration_ms: number | null;
  status: string;
  // SpanData fields are flattened into Span via serde(flatten)
  type: "model_call" | "tool_call";
  // ModelCall fields (present when type === "model_call")
  system_prompt?: string;
  user_message?: string;
  messages?: Array<{ role: string; content: string }>;
  message_count?: number;
  tool_count?: number;
  has_thinking?: boolean;
  input_tokens?: number;
  output_tokens?: number;
  total_tokens?: number;
  tool_calls_in_response?: number;
  tools?: string;
  response?: string;
  // ToolCall fields (present when type === "tool_call")
  tool?: string;
  args?: string;
  result?: string;
  error?: string | null;
}

export function isModelCallSpan(span: Span): boolean {
  return span.type === "model_call";
}

export function isToolCallSpan(span: Span): boolean {
  return span.type === "tool_call";
}

export interface TraceRecord {
  request_id: string;
  start_time: string;
  end_time: string | null;
  status: TraceStatus;
  metadata: TraceMetadata;
  spans: Span[];
}

export interface TraceListResponse {
  traces: TraceRecord[];
  total: number;
}

export interface TraceListParams {
  limit?: number;
  from?: string;
  to?: string;
  status?: string;
  model?: string;
  channel?: string;
  tool?: string;
  keyword?: string;
  min_tokens?: number;
  min_duration_ms?: number;
  parent?: string;
}
