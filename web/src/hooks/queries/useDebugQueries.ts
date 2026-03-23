import { useQuery, useMutation } from "@tanstack/react-query";
import { fetchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type {
  DebugInvokeRequest,
  DebugInvokeResponse,
  DebugHealthResponse,
} from "../../types/dashboard";

export const debugKeys = {
  all: ["debug"] as const,
  health: () => [...debugKeys.all, "health"] as const,
};

export function useDebugInvoke() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: (req: DebugInvokeRequest) =>
      postJSON<DebugInvokeResponse>("/debug/invoke", req),
    onError: (err: Error) => {
      toast({ variant: "error", title: "Debug invoke failed", description: err.message });
    },
  });
}

export function useDebugHealth() {
  return useQuery({
    queryKey: debugKeys.health(),
    queryFn: () => fetchJSON<DebugHealthResponse>("/debug/health"),
  });
}
