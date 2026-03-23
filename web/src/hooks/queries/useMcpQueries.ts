import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON, postJSON, putJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import { McpServerInfoSchema, McpTestResultSchema } from "../../schemas/dashboard";
import type { McpServerInfo } from "../../types/dashboard";

export const mcpKeys = {
  all: ["mcp"] as const,
  list: () => [...mcpKeys.all, "list"] as const,
};

export function useMcpServers() {
  return useQuery({
    queryKey: mcpKeys.list(),
    queryFn: () => fetchJSON("/mcp", z.array(McpServerInfoSchema)),
  });
}

export function useCreateMcpServer() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (
      server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">
    ) => postJSON("/mcp", server, McpServerInfoSchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: mcpKeys.all });
      toast({ variant: "success", title: "MCP server created" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to create MCP server", description: err.message });
    },
  });
}

export function useUpdateMcpServer() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ name, server }: { name: string; server: Partial<McpServerInfo> }) =>
      putJSON(`/mcp/${encodeURIComponent(name)}`, server, McpServerInfoSchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: mcpKeys.all });
      toast({ variant: "success", title: "MCP server updated" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to update MCP server", description: err.message });
    },
  });
}

export function useDeleteMcpServer() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) => deleteJSON(`/mcp/${encodeURIComponent(name)}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: mcpKeys.all });
      toast({ variant: "success", title: "MCP server deleted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to delete MCP server", description: err.message });
    },
  });
}

export function useTestMcpServer() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON(`/mcp/${encodeURIComponent(name)}/test`, undefined, McpTestResultSchema),
    onSuccess: (data) => {
      if (data?.success) {
        toast({ variant: "success", title: "MCP server test passed" });
      } else {
        toast({ variant: "error", title: "MCP server test failed", description: data?.error ?? undefined });
      }
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to test MCP server", description: err.message });
    },
  });
}

export function usePersistMcpServer() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON(`/mcp/${encodeURIComponent(name)}/persist`, undefined, McpServerInfoSchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: mcpKeys.all });
      toast({ variant: "success", title: "MCP server persisted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to persist MCP server", description: err.message });
    },
  });
}
