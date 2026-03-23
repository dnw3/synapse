import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON, postJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import {
  SkillEntrySchema,
  StoreSearchResultSchema,
  StoreSkillItemSchema,
  StoreSkillDetailSchema,
  StoreStatusSchema,
  OkResponseSchema,
} from "../../schemas/dashboard";

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
    queryFn: () => fetchJSON("/skills", z.array(SkillEntrySchema)),
  });
}

export function useToggleSkill() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON(`/skills/${encodeURIComponent(name)}/toggle`, undefined, z.object({ enabled: z.boolean() })),
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
      fetchJSON(
        `/skills/files?path=${encodeURIComponent(path)}`,
        z.object({ files: z.array(z.object({ name: z.string(), size: z.number() })) })
      ),
    enabled: !!path,
  });
}

export function useSkillFileContent(path: string) {
  return useQuery({
    queryKey: skillsKeys.content(path),
    queryFn: () =>
      fetchJSON(
        `/skills/content?path=${encodeURIComponent(path)}`,
        z.object({ content: z.string() })
      ),
    enabled: !!path,
  });
}

export function useStoreSearch(q: string, limit = 20) {
  return useQuery({
    queryKey: storeKeys.search(q, limit),
    queryFn: () =>
      fetchJSON(
        `/store/search?q=${encodeURIComponent(q)}&limit=${limit}`,
        z.object({ results: z.array(StoreSearchResultSchema), source: z.string() })
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
      return fetchJSON(path, z.object({ items: z.array(StoreSkillItemSchema), source: z.string() }));
    },
  });
}

export function useStoreInstall() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ slug, version }: { slug: string; version?: string }) =>
      postJSON("/store/install", { slug, version }, OkResponseSchema),
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
      fetchJSON(`/store/skills/${encodeURIComponent(slug)}`, StoreSkillDetailSchema),
    enabled: !!slug,
  });
}

export function useStoreFiles(slug: string) {
  return useQuery({
    queryKey: storeKeys.files(slug),
    queryFn: () =>
      fetchJSON(
        `/store/skills/${encodeURIComponent(slug)}/files`,
        z.object({ files: z.array(z.object({ name: z.string(), size: z.number() })), skillMd: z.string().nullable() })
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
      return fetchJSON(
        `/store/skills/${encodeURIComponent(slug)}/files/${encodedPath}`,
        z.object({ content: z.string().nullable() })
      );
    },
    enabled: !!slug && !!filePath,
  });
}

export function useStoreStatus() {
  return useQuery({
    queryKey: storeKeys.status(),
    queryFn: () => fetchJSON("/store/status", StoreStatusSchema),
  });
}
