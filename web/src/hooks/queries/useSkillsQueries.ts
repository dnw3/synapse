import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type {
  SkillEntry,
  StoreSearchResult,
  StoreSkillItem,
  StoreSkillDetail,
  StoreStatus,
} from "../../types/dashboard";

export const skillsKeys = {
  all: ["skills"] as const,
  list: () => [...skillsKeys.all, "list"] as const,
  files: (path: string) => [...skillsKeys.all, "files", path] as const,
  content: (path: string) => [...skillsKeys.all, "content", path] as const,
};

export const storeKeys = {
  all: ["store"] as const,
  search: (q: string, limit: number) => [...storeKeys.all, "search", q, limit] as const,
  list: (limit: number, sort?: string, offset?: number) =>
    [...storeKeys.all, "list", limit, sort, offset] as const,
  detail: (slug: string) => [...storeKeys.all, "detail", slug] as const,
  files: (slug: string) => [...storeKeys.all, "files", slug] as const,
  fileContent: (slug: string, filePath: string) =>
    [...storeKeys.all, "fileContent", slug, filePath] as const,
  status: () => [...storeKeys.all, "status"] as const,
};

export function useSkills() {
  return useQuery({
    queryKey: skillsKeys.list(),
    queryFn: () => fetchJSON<SkillEntry[]>("/skills"),
  });
}

export function useToggleSkill() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON<{ enabled: boolean }>(`/skills/${encodeURIComponent(name)}/toggle`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: skillsKeys.all });
      toast({ variant: "success", title: "Skill toggled" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to toggle skill", description: err.message });
    },
  });
}

export function useSkillFiles(path: string) {
  return useQuery({
    queryKey: skillsKeys.files(path),
    queryFn: () =>
      fetchJSON<{ files: { name: string; size: number }[] }>(
        `/skills/files?path=${encodeURIComponent(path)}`
      ),
    enabled: !!path,
  });
}

export function useSkillFileContent(path: string) {
  return useQuery({
    queryKey: skillsKeys.content(path),
    queryFn: () =>
      fetchJSON<{ content: string }>(
        `/skills/content?path=${encodeURIComponent(path)}`
      ),
    enabled: !!path,
  });
}

export function useStoreSearch(q: string, limit = 20) {
  return useQuery({
    queryKey: storeKeys.search(q, limit),
    queryFn: () =>
      fetchJSON<{ results: StoreSearchResult[]; source: string }>(
        `/store/search?q=${encodeURIComponent(q)}&limit=${limit}`
      ),
    enabled: !!q,
  });
}

export function useStoreList(limit = 20, sort?: string, offset?: number) {
  return useQuery({
    queryKey: storeKeys.list(limit, sort, offset),
    queryFn: () => {
      let path = `/store/skills?limit=${limit}`;
      if (sort) path += `&sort=${sort}`;
      if (offset) path += `&cursor=${offset}`;
      return fetchJSON<{ items: StoreSkillItem[]; source: string }>(path);
    },
  });
}

export function useStoreInstall() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ slug, version }: { slug: string; version?: string }) =>
      postJSON<{ ok: boolean }>("/store/install", { slug, version }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: skillsKeys.all });
      qc.invalidateQueries({ queryKey: storeKeys.status() });
      toast({ variant: "success", title: "Skill installed" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to install skill", description: err.message });
    },
  });
}

export function useStoreDetail(slug: string) {
  return useQuery({
    queryKey: storeKeys.detail(slug),
    queryFn: () =>
      fetchJSON<StoreSkillDetail>(`/store/skills/${encodeURIComponent(slug)}`),
    enabled: !!slug,
  });
}

export function useStoreFiles(slug: string) {
  return useQuery({
    queryKey: storeKeys.files(slug),
    queryFn: () =>
      fetchJSON<{ files: { name: string; size: number }[]; skillMd: string | null }>(
        `/store/skills/${encodeURIComponent(slug)}/files`
      ),
    enabled: !!slug,
  });
}

export function useStoreFileContent(slug: string, filePath: string) {
  return useQuery({
    queryKey: storeKeys.fileContent(slug, filePath),
    queryFn: () => {
      const encodedPath = filePath
        .split("/")
        .map(encodeURIComponent)
        .join("/");
      return fetchJSON<{ content: string | null }>(
        `/store/skills/${encodeURIComponent(slug)}/files/${encodedPath}`
      );
    },
    enabled: !!slug && !!filePath,
  });
}

export function useStoreStatus() {
  return useQuery({
    queryKey: storeKeys.status(),
    queryFn: () => fetchJSON<StoreStatus>("/store/status"),
  });
}
