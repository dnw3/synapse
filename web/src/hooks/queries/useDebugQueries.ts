import { useQuery, useMutation } from "@tanstack/react-query";
import { fetchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import {
  DebugInvokeResponseSchema,
  DebugHealthResponseSchema,
} from "../../schemas/dashboard";
import type { DebugInvokeRequest } from "../../types/dashboard";

export const debugKeys = {
  all: ["debug"] as const,
  health: () => [...debugKeys.all, "health"] as const,
};

export function useDebugInvoke() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: (req: DebugInvokeRequest) =>
      postJSON("/debug/invoke", req, DebugInvokeResponseSchema),
    onError: (err: Error) => {
      toast({ variant: "error", title: "Debug invoke failed", description: err.message });
    },
  });
}

export function useDebugHealth() {
  return useQuery({
    queryKey: debugKeys.health(),
    queryFn: () => fetchJSON("/debug/health", DebugHealthResponseSchema),
  });
}
