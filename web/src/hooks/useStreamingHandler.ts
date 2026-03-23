import { useCallback, useEffect, useReducer } from "react";
import type { Message } from "../types";
import type { UseGatewayReturn } from "./useGateway";

// ---------------------------------------------------------------------------
// Streaming reducer — replaces assistantContentRef / reasoningContentRef
// ---------------------------------------------------------------------------

export type StreamingReducerState = {
  messages: Message[];
  requestId: string | null;
  assistantContent: string;
  reasoningContent: string;
};

export type StreamingAction =
  | { type: "START"; requestId: string }
  | { type: "APPEND_ASSISTANT"; content: string }
  | { type: "APPEND_REASONING"; content: string }
  | { type: "ADD_TOOL_CALL"; message: Message }
  | { type: "ADD_TOOL_RESULT"; message: Message }
  | { type: "FLUSH_ASSISTANT" }
  | { type: "CLEAR" };

const initialStreamingState: StreamingReducerState = {
  messages: [],
  requestId: null,
  assistantContent: "",
  reasoningContent: "",
};

/** Build an assistant message from accumulated content. */
function buildAssistantMsg(s: StreamingReducerState): Message {
  return {
    role: "assistant",
    content: s.assistantContent,
    tool_calls: [],
    reasoning: s.reasoningContent || undefined,
  };
}

/** Replace last plain-assistant message, or append a new one. */
function upsertAssistant(msgs: Message[], msg: Message): Message[] {
  const lastIdx = msgs.length - 1;
  if (lastIdx >= 0 && msgs[lastIdx].role === "assistant" && msgs[lastIdx].tool_calls.length === 0) {
    const updated = [...msgs];
    updated[lastIdx] = msg;
    return updated;
  }
  return [...msgs, msg];
}

function streamingReducer(state: StreamingReducerState, action: StreamingAction): StreamingReducerState {
  switch (action.type) {
    case "START":
      return { ...state, requestId: action.requestId };

    case "APPEND_ASSISTANT": {
      const assistantContent = state.assistantContent + action.content;
      const next = { ...state, assistantContent };
      return { ...next, messages: upsertAssistant(state.messages, buildAssistantMsg(next)) };
    }

    case "APPEND_REASONING": {
      const reasoningContent = state.reasoningContent + action.content;
      const next = { ...state, reasoningContent };
      return { ...next, messages: upsertAssistant(state.messages, buildAssistantMsg(next)) };
    }

    case "ADD_TOOL_CALL": {
      // Flush accumulated assistant content first if any
      const hasContent = state.assistantContent || state.reasoningContent;
      if (hasContent) {
        const last = state.messages[state.messages.length - 1];
        const needsFlush = !last || last.role !== "assistant" || last.tool_calls.length > 0;
        const base = needsFlush
          ? [...state.messages, buildAssistantMsg(state)]
          : state.messages;
        return {
          ...state,
          assistantContent: "",
          reasoningContent: "",
          messages: [...base, action.message],
        };
      }
      return { ...state, messages: [...state.messages, action.message] };
    }

    case "ADD_TOOL_RESULT":
      return { ...state, messages: [...state.messages, action.message] };

    case "FLUSH_ASSISTANT": {
      if (!state.assistantContent && !state.reasoningContent) return state;
      return {
        ...state,
        assistantContent: "",
        reasoningContent: "",
        messages: upsertAssistant(state.messages, buildAssistantMsg(state)),
      };
    }

    case "CLEAR":
      return initialStreamingState;

    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// ApprovalRequest shape (matches original)
// ---------------------------------------------------------------------------

export interface ApprovalRequest {
  tool_name: string;
  args_preview: string;
  risk_level: string;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface UseStreamingHandlerReturn {
  streaming: { messages: Message[]; requestId: string | null };
  clearStreaming: () => void;
}

export function useStreamingHandler(
  gw: UseGatewayReturn,
  activeKeyRef: React.RefObject<string | null>,
  onTurnComplete: () => void,
  onApproval: (request: ApprovalRequest) => void,
  onError: (error: string) => void,
  onSessionsChanged: () => void,
): UseStreamingHandlerReturn {
  const [state, dispatch] = useReducer(streamingReducer, initialStreamingState);

  const clearStreaming = useCallback(() => {
    dispatch({ type: "CLEAR" });
  }, []);

  // Subscribe to streaming events
  useEffect(() => {
    const unsubscribe = gw.subscribe((event, payload) => {
      // Filter by sessionKey if present
      const evtKey = payload.sessionKey as string | undefined;
      if (evtKey && evtKey !== activeKeyRef.current) return;

      switch (event) {
        case "agent.message.start":
          dispatch({ type: "START", requestId: (payload.request_id as string) ?? "" });
          break;

        case "agent.message.delta":
          dispatch({ type: "APPEND_ASSISTANT", content: (payload.content as string) ?? "" });
          break;

        case "agent.thinking.delta":
          dispatch({ type: "APPEND_REASONING", content: (payload.content as string) ?? "" });
          break;

        case "agent.tool.start":
          dispatch({
            type: "ADD_TOOL_CALL",
            message: {
              role: "assistant" as const,
              content: "",
              tool_calls: [{
                name: (payload.name as string) ?? "",
                arguments: (payload.args as Record<string, unknown>) ?? {},
                display: payload.display as Message["tool_calls"][number]["display"],
              }],
            },
          });
          break;

        case "agent.tool.result":
          dispatch({
            type: "ADD_TOOL_RESULT",
            message: { role: "tool" as const, content: (payload.content as string) ?? "", tool_calls: [] },
          });
          break;

        case "approval.requested":
          onApproval({
            tool_name: (payload.tool_name as string) ?? "",
            args_preview: (payload.args_preview as string) ?? "",
            risk_level: (payload.risk_level as string) ?? "",
          });
          break;

        case "agent.turn.complete":
          onTurnComplete();
          break;

        case "agent.error": {
          const rid = (payload.request_id as string) ?? null;
          const msg = (payload.message as string) ?? "Unknown error";
          const errorMsg = rid ? `${msg}\n[LogID: ${rid}]` : msg;
          onError(errorMsg);
          break;
        }

        case "sessions.changed":
        case "session.compacted":
          onSessionsChanged();
          break;

        default:
          break;
      }
    });

    return unsubscribe;
  }, [clearStreaming, onTurnComplete, onApproval, onError, onSessionsChanged]); // eslint-disable-line react-hooks/exhaustive-deps

  return {
    streaming: { messages: state.messages, requestId: state.requestId },
    clearStreaming,
  };
}
