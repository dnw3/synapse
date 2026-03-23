import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON, postJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import { PluginInfoSchema } from "../../schemas/dashboard";

export const pluginsKeys = {
  all: ["plugins"] as const,
  list: () => [...pluginsKeys.all, "list"] as const,
};

export function usePlugins() {
  return useQuery({
    queryKey: pluginsKeys.list(),
    queryFn: () => fetchJSON("/plugins", z.object({ plugins: z.array(PluginInfoSchema) })),
  });
}

export function useTogglePlugin() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ name, enabled }: { name: string; enabled: boolean }) =>
      postJSON(
        "/plugins/toggle",
        { name, enabled },
        z.object({ ok: z.boolean(), name: z.string(), enabled: z.boolean(), message: z.string().optional() })
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
      postJSON(
        "/plugins/service-control",
        { plugin, service, action },
        z.object({ ok: z.boolean(), service: z.string(), status: z.string() })
      ),
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
      postJSON(
        "/plugins/install",
        { name: path },
        z.object({ ok: z.boolean(), name: z.string().optional(), message: z.string().optional() })
      ),
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
