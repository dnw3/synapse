import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, putJSON, patchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type { ConfigData } from "../../types/dashboard";

export const configKeys = {
  all: ["config"] as const,
  detail: () => [...configKeys.all, "detail"] as const,
  schema: () => [...configKeys.all, "schema"] as const,
};

export function useConfig() {
  return useQuery({
    queryKey: configKeys.detail(),
    queryFn: () => fetchJSON<ConfigData>("/config"),
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
      putJSON<{ success: boolean }>("/config", { content }),
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
      patchJSON<{ ok: boolean }>("/config", fields),
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
      postJSON<{ valid: boolean; errors?: string[] }>("/config/validate", { content }),
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to validate config", description: err.message });
    },
  });
}

export function useReloadConfig() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: () => postJSON<{ ok: boolean }>("/config/reload"),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: configKeys.all });
      toast({ variant: "success", title: "Config reloaded" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to reload config", description: err.message });
    },
  });
}
