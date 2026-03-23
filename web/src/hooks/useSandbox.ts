import { useCallback } from "react";

export interface SandboxInstanceInfo {
  runtime_id: string;
  provider_id: string;
  runtime_label: string;
  scope_key: string;
  image: string | null;
  created_at: string;
  last_used_at: string;
}

export interface SandboxExplanation {
  agent_id: string;
  session_key: string;
  mode: string;
  scope: string;
  workspace_access: string;
  backend: string;
  is_sandboxed: boolean;
  scope_key: string;
  security: {
    cap_drop: string[];
    read_only_root: boolean;
    network_mode: string;
  };
}

export function useSandboxAPI() {
  const fetchJSON = useCallback(async <T,>(path: string): Promise<T | null> => {
    try {
      const res = await fetch(`/api/sandbox${path}`);
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  const postJSON = useCallback(async <T,>(path: string, body?: unknown): Promise<T | null> => {
    try {
      const res = await fetch(`/api/sandbox${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
      });
      if (!res.ok) return null;
      return await res.json();
    } catch {
      return null;
    }
  }, []);

  const deleteJSON = useCallback(async (path: string): Promise<boolean> => {
    try {
      const res = await fetch(`/api/sandbox${path}`, { method: "DELETE" });
      return res.ok;
    } catch {
      return false;
    }
  }, []);

  return {
    listInstances: () => fetchJSON<SandboxInstanceInfo[]>(""),
    explain: (session?: string, agent?: string) => {
      const params = new URLSearchParams();
      if (session) params.set("session", session);
      if (agent) params.set("agent", agent);
      return fetchJSON<SandboxExplanation>(`/explain?${params}`);
    },
    recreate: (filter: { all?: boolean; session?: string; agent?: string }) =>
      postJSON<{ count: number }>("/recreate", filter),
    destroy: (runtimeId: string) => deleteJSON(`/${runtimeId}`),
    listProviders: () => fetchJSON<string[]>("/providers"),
  };
}
