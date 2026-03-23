import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, postJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type { PluginInfo } from "../../types/dashboard";

export const pluginsKeys = {
  all: ["plugins"] as const,
  list: () => [...pluginsKeys.all, "list"] as const,
};

export function usePlugins() {
  return useQuery({
    queryKey: pluginsKeys.list(),
    queryFn: () => fetchJSON<{ plugins: PluginInfo[] }>("/plugins"),
  });
}

export function useTogglePlugin() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ name, enabled }: { name: string; enabled: boolean }) =>
      postJSON<{ ok: boolean; name: string; enabled: boolean; message?: string }>(
        "/plugins/toggle",
        { name, enabled }
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: pluginsKeys.all });
      toast({ variant: "success", title: "Plugin toggled" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to toggle plugin", description: err.message });
    },
  });
}

export function useControlService() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({
      plugin,
      service,
      action,
    }: {
      plugin: string;
      service: string;
      action: "start" | "stop";
    }) =>
      postJSON<{ ok: boolean; service: string; status: string }>("/plugins/service-control", {
        plugin,
        service,
        action,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: pluginsKeys.all });
      toast({ variant: "success", title: "Service control applied" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to control service", description: err.message });
    },
  });
}

export function useInstallPlugin() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (path: string) =>
      postJSON<{ ok: boolean; name?: string; message?: string }>("/plugins/install", {
        name: path,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: pluginsKeys.all });
      toast({ variant: "success", title: "Plugin installed" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to install plugin", description: err.message });
    },
  });
}

export function useRemovePlugin() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) => deleteJSON(`/plugins/${encodeURIComponent(name)}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: pluginsKeys.all });
      toast({ variant: "success", title: "Plugin removed" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to remove plugin", description: err.message });
    },
  });
}
