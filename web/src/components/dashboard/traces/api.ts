import type { TraceListParams, TraceListResponse, TraceRecord } from "./types";

export async function fetchTraces(params: TraceListParams = {}): Promise<TraceListResponse> {
  const searchParams = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      searchParams.set(key, String(value));
    }
  }
  const url = `/api/traces${searchParams.toString() ? `?${searchParams}` : ""}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Failed to fetch traces: ${res.status}`);
  return res.json();
}

export async function fetchTraceDetail(requestId: string): Promise<TraceRecord | null> {
  const res = await fetch(`/api/traces/${encodeURIComponent(requestId)}`);
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Failed to fetch trace: ${res.status}`);
  return res.json();
}
