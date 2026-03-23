import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON, putJSON, patchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import { ConfigDataSchema, OkResponseSchema } from "../../schemas/dashboard";

export const configKeys = {
  all: ["config"] as const,
  detail: () => [...configKeys.all, "detail"] as const,
  schema: () => [...configKeys.all, "schema"] as const,
};

export function useConfig() {
  return useQuery({
    queryKey: configKeys.detail(),
    queryFn: () => fetchJSON("/config", ConfigDataSchema),
  });
}

export function useConfigSchema() {
  return useQuery({
    queryKey: configKeys.schema(),
    queryFn: () => fetchJSON<Record<string, unknown>>("/config/schema"),
  });
}

export function useSaveConfig() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (content: string) =>
      putJSON("/config", { content }, z.object({ success: z.boolean() })),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: configKeys.all });
      toast({ variant: "success", title: "Config saved" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to save config", description: err.message });
    },
  });
}

export function usePatchConfig() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (fields: Record<string, unknown>) =>
      patchJSON("/config", fields, OkResponseSchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: configKeys.all });
      toast({ variant: "success", title: "Config updated" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to update config", description: err.message });
    },
  });
}

export function useValidateConfig() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: (content: string) =>
      postJSON("/config/validate", { content }, z.object({ valid: z.boolean(), errors: z.array(z.string()).optional() })),
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to validate config", description: err.message });
    },
  });
}

export function useReloadConfig() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: () => postJSON("/config/reload", undefined, OkResponseSchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: configKeys.all });
      toast({ variant: "success", title: "Config reloaded" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to reload config", description: err.message });
    },
  });
}
