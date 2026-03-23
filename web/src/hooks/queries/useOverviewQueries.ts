import { useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON } from "../../lib/api";
import {
  StatsDataSchema,
  UsageDataSchema,
  ProviderInfoSchema,
  HealthDataSchema,
  RequestMetricsResponseSchema,
  IdentityInfoSchema,
} from "../../schemas/dashboard";

export const statsKeys = {
  all: ["stats"] as const,
  detail: () => [...statsKeys.all, "detail"] as const,
};

export const usageKeys = {
  all: ["usage"] as const,
  detail: () => [...usageKeys.all, "detail"] as const,
};

export const providersKeys = {
  all: ["providers"] as const,
  list: () => [...providersKeys.all, "list"] as const,
};

export const healthKeys = {
  all: ["health"] as const,
  detail: () => [...healthKeys.all, "detail"] as const,
};

export const requestsKeys = {
  all: ["requests"] as const,
  list: () => [...requestsKeys.all, "list"] as const,
};

export const identityKeys = {
  all: ["identity"] as const,
  detail: (agent?: string) => [...identityKeys.all, "detail", agent] as const,
};

export function useStats() {
  return useQuery({
    queryKey: statsKeys.detail(),
    queryFn: () => fetchJSON("/stats", StatsDataSchema),
  });
}

export function useUsage() {
  return useQuery({
    queryKey: usageKeys.detail(),
    queryFn: () => fetchJSON("/usage", UsageDataSchema),
  });
}

export function useProviders() {
  return useQuery({
    queryKey: providersKeys.list(),
    queryFn: () => fetchJSON("/providers", z.array(ProviderInfoSchema)),
  });
}

export function useHealth() {
  return useQuery({
    queryKey: healthKeys.detail(),
    queryFn: () => fetchJSON("/health", HealthDataSchema),
  });
}

export function useRequests() {
  return useQuery({
    queryKey: requestsKeys.list(),
    queryFn: async () => {
      const resp = await fetchJSON("/requests", RequestMetricsResponseSchema);
      return resp.endpoints;
    },
  });
}

export function useIdentity(agent?: string) {
  return useQuery({
    queryKey: identityKeys.detail(agent),
    queryFn: () => {
      const qs = agent ? `?agent=${encodeURIComponent(agent)}` : "";
      return fetchJSON(`/identity${qs}`, IdentityInfoSchema);
    },
    staleTime: Infinity,
  });
}
