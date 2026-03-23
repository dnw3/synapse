import type { SessionEntry } from "../../../types/dashboard";

export type SortField = "id" | "created_at" | "message_count" | "token_count";
export type SortOrder = "asc" | "desc";

export const PAGE_LIMIT = 50;

export const THINKING_OPTIONS = ["off", "low", "medium", "high", "adaptive"] as const;

export const CHANNEL_COLORS: Record<string, string> = {
  web: "bg-blue-500/10 text-blue-400 border-blue-500/20",
  lark: "bg-teal-500/10 text-teal-400 border-teal-500/20",
  telegram: "bg-sky-500/10 text-sky-400 border-sky-500/20",
  discord: "bg-indigo-500/10 text-indigo-400 border-indigo-500/20",
  slack: "bg-purple-500/10 text-purple-400 border-purple-500/20",
};

export const KIND_COLORS: Record<string, string> = {
  direct: "bg-green-500/10 text-green-400 border-green-500/20",
  group: "bg-orange-500/10 text-orange-400 border-orange-500/20",
  main: "bg-[var(--accent)]/10 text-[var(--accent)] border-[var(--accent)]/20",
};

export function extractAgentId(sessionKey: string): string {
  const match = sessionKey.match(/^agent:([^:]+):/);
  return match?.[1] ?? "default";
}

/** Normalized session with `id` always present (populated from `key`). */
export type NormalizedSession = SessionEntry & { id: string };

/** Normalize a raw API session object: ensure `id` mirrors `key`. */
export function normalizeSession(raw: Record<string, unknown>): NormalizedSession {
  const key = (raw.key as string) ?? (raw.id as string) ?? "";
  return {
    ...(raw as unknown as SessionEntry),
    key,
    id: key,
  };
}
