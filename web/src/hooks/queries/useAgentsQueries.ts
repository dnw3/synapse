import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON, postJSON, putJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import {
  AgentEntrySchema,
  ToolCatalogGroupSchema,
  DebugInvokeResponseSchema,
} from "../../schemas/dashboard";
import type {
  AgentEntry,
  BindingEntry,
  BroadcastGroupEntry,
} from "../../types/dashboard";

export const agentsKeys = {
  all: ["agents"] as const,
  list: () => [...agentsKeys.all, "list"] as const,
};

export const toolsCatalogKeys = {
  all: ["toolsCatalog"] as const,
  list: () => [...toolsCatalogKeys.all, "list"] as const,
};

export const bindingsKeys = {
  all: ["bindings"] as const,
  list: () => [...bindingsKeys.all, "list"] as const,
};

export const broadcastsKeys = {
  all: ["broadcasts"] as const,
  list: () => [...broadcastsKeys.all, "list"] as const,
};

export function useAgents() {
  return useQuery({
    queryKey: agentsKeys.list(),
    queryFn: () => fetchJSON("/agents", z.array(AgentEntrySchema)),
  });
}

export function useCreateAgent() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (agent: Partial<AgentEntry>) =>
      postJSON("/agents", agent, AgentEntrySchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: agentsKeys.all });
      toast({ variant: "success", title: "Agent created" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to create agent", description: err.message });
    },
  });
}

export function useUpdateAgent() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ name, agent }: { name: string; agent: Partial<AgentEntry> }) =>
      putJSON(`/agents/${encodeURIComponent(name)}`, agent, AgentEntrySchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: agentsKeys.all });
      toast({ variant: "success", title: "Agent updated" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to update agent", description: err.message });
    },
  });
}

export function useDeleteAgent() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) => deleteJSON(`/agents/${encodeURIComponent(name)}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: agentsKeys.all });
      toast({ variant: "success", title: "Agent deleted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to delete agent", description: err.message });
    },
  });
}

export function useToolsCatalog() {
  return useQuery({
    queryKey: toolsCatalogKeys.list(),
    queryFn: () => fetchJSON("/tools", z.array(ToolCatalogGroupSchema)),
  });
}

export function useBindings() {
  return useQuery({
    queryKey: bindingsKeys.list(),
    queryFn: async (): Promise<BindingEntry[]> => {
      const resp = await postJSON("/debug/invoke", {
        method: "bindings.list",
        params: {},
      }, DebugInvokeResponseSchema);
      return (resp?.result as { bindings: BindingEntry[] } | undefined)?.bindings ?? [];
    },
  });
}

export function useBroadcasts() {
  return useQuery({
    queryKey: broadcastsKeys.list(),
    queryFn: async (): Promise<BroadcastGroupEntry[]> => {
      const resp = await postJSON("/debug/invoke", {
        method: "broadcasts.list",
        params: {},
      }, DebugInvokeResponseSchema);
      return (
        (resp?.result as { broadcasts: BroadcastGroupEntry[] } | undefined)?.broadcasts ?? []
      );
    },
  });
}
