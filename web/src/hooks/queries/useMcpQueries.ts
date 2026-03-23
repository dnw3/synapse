import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, postJSON, putJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type { McpServerInfo, McpTestResult } from "../../types/dashboard";

export const mcpKeys = {
  all: ["mcp"] as const,
  list: () => [...mcpKeys.all, "list"] as const,
};

export function useMcpServers() {
  return useQuery({
    queryKey: mcpKeys.list(),
    queryFn: () => fetchJSON<McpServerInfo[]>("/mcp"),
  });
}

export function useCreateMcpServer() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (
      server: Omit<McpServerInfo, "status" | "tools" | "lastChecked" | "error">
    ) => postJSON<McpServerInfo>("/mcp", server),
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
      putJSON<McpServerInfo>(`/mcp/${encodeURIComponent(name)}`, server),
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
      postJSON<McpTestResult>(`/mcp/${encodeURIComponent(name)}/test`),
    onSuccess: (data) => {
      if (data?.success) {
        toast({ variant: "success", title: "MCP server test passed" });
      } else {
        toast({ variant: "error", title: "MCP server test failed", description: data?.error });
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
      postJSON<McpServerInfo>(`/mcp/${encodeURIComponent(name)}/persist`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: mcpKeys.all });
      toast({ variant: "success", title: "MCP server persisted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to persist MCP server", description: err.message });
    },
  });
}
