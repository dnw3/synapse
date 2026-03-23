import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, deleteJSON, patchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type { SessionEntry } from "../../types/dashboard";

export const sessionsKeys = {
  all: ["sessions"] as const,
  list: (params?: { limit?: number; offset?: number; sort?: string; order?: string }) =>
    [...sessionsKeys.all, "list", params] as const,
};

export function useSessions(params?: {
  limit?: number;
  offset?: number;
  sort?: string;
  order?: string;
}) {
  return useQuery({
    queryKey: sessionsKeys.list(params),
    queryFn: () => {
      const qs = new URLSearchParams();
      if (params?.limit) qs.set("limit", String(params.limit));
      if (params?.offset) qs.set("offset", String(params.offset));
      if (params?.sort) qs.set("sort", params.sort);
      if (params?.order) qs.set("order", params.order);
      const q = qs.toString();
      return fetchJSON<{ sessions: SessionEntry[]; total: number }>(
        `/sessions${q ? `?${q}` : ""}`
      );
    },
  });
}

export function useDeleteSession() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (id: string) => deleteJSON(`/sessions/${encodeURIComponent(id)}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: sessionsKeys.all });
      toast({ variant: "success", title: "Session deleted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to delete session", description: err.message });
    },
  });
}

export function useRenameSession() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ id, displayName }: { id: string; displayName: string }) =>
      patchJSON<{ ok: boolean }>(`/sessions/${encodeURIComponent(id)}`, {
        display_name: displayName,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: sessionsKeys.all });
      toast({ variant: "success", title: "Session renamed" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to rename session", description: err.message });
    },
  });
}

export function usePatchSessionOverrides() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({
      id,
      overrides,
    }: {
      id: string;
      overrides: { label?: string; thinking?: string; verbose?: string };
    }) =>
      patchJSON<{ ok: boolean }>(
        `/sessions/${encodeURIComponent(id)}`,
        overrides
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: sessionsKeys.all });
      toast({ variant: "success", title: "Session updated" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to update session", description: err.message });
    },
  });
}

export function useCompactSession() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (id: string) =>
      postJSON<{ ok: boolean }>(`/sessions/${encodeURIComponent(id)}/compact`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: sessionsKeys.all });
      toast({ variant: "success", title: "Session compacted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to compact session", description: err.message });
    },
  });
}
