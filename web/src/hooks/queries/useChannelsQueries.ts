import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, postJSON, putJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type { ChannelEntry } from "../../types/dashboard";

export const channelsKeys = {
  all: ["channels"] as const,
  list: () => [...channelsKeys.all, "list"] as const,
};

export function useChannels() {
  return useQuery({
    queryKey: channelsKeys.list(),
    queryFn: () => fetchJSON<ChannelEntry[]>("/channels"),
  });
}

export function useToggleChannel() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON<{ enabled: boolean }>(`/channels/${encodeURIComponent(name)}/toggle`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: channelsKeys.all });
      toast({ variant: "success", title: "Channel toggled" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to toggle channel", description: err.message });
    },
  });
}

export function useUpdateChannelConfig() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ name, config }: { name: string; config: Record<string, string> }) =>
      putJSON<{ ok: boolean }>(`/channels/${encodeURIComponent(name)}/config`, config),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: channelsKeys.all });
      toast({ variant: "success", title: "Channel config updated" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to update channel config", description: err.message });
    },
  });
}
