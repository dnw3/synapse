import { useQuery, useMutation } from "@tanstack/react-query";
import { fetchJSON, fetchRaw } from "../../lib/api";
import { useToast } from "../../components/ui/toast";

export const logsKeys = {
  all: ["logs"] as const,
  list: (lines: number, level?: string) => [...logsKeys.all, "list", lines, level] as const,
};

export function useLogs(lines = 200, level?: string) {
  return useQuery({
    queryKey: logsKeys.list(lines, level),
    queryFn: () => {
      const qs = new URLSearchParams({ lines: String(lines) });
      if (level && level !== "all") qs.set("level", level);
      return fetchJSON<{ lines: string[]; file?: string }>(`/logs?${qs}`);
    },
  });
}

export function useExportLogs() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (): Promise<Blob> => {
      const res = await fetchRaw("/logs/export");
      return res.blob();
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to export logs", description: err.message });
    },
  });
}
